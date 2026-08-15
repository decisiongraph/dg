use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::document::Document;
use crate::schema::Schema;
use crate::users::OrgConfig;

use super::document::{
    error_diagnostic, infer_type_from_path, singleton_matches, validate_document,
    validate_singleton,
};
use super::{Diagnostic, FileResult, Severity, ValidationResult};

/// Validate all markdown files in a directory against a schema.
pub fn validate_directory(
    dir: impl AsRef<Path>,
    schema: &Schema,
    pattern: Option<&str>,
    user_config: Option<&OrgConfig>,
) -> crate::error::Result<ValidationResult> {
    let dir = dir.as_ref();
    let files = crate::discovery::discover_files(dir, pattern, &[], false)?;

    // Build known file set and known ID set for cross-ref validation
    let known_files: HashSet<PathBuf> = files
        .iter()
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.clone()))
        .collect();

    let mut known_ids: HashSet<String> = HashSet::new();
    for path in &files {
        known_ids.insert(crate::graph::path_to_id(path));
    }

    // Validate files in parallel — all shared state is read-only
    #[allow(clippy::type_complexity)]
    let per_file: Vec<(Option<FileResult>, Option<(String, String)>)> = files
        .par_iter()
        .map(|path| {
            let doc = match Document::from_file(path) {
                Ok(d) => d,
                Err(e) => return (Some(error_diagnostic(path, &e)), None),
            };

            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let rel_path = path.strip_prefix(dir).unwrap_or(path);

            // Skip deeply nested READMEs inside services/apps/infra
            if filename == "README.md" {
                if let Some(first) = rel_path
                    .components()
                    .next()
                    .and_then(|c| c.as_os_str().to_str())
                {
                    if matches!(first, "services" | "apps" | "infra")
                        && rel_path.components().count() > 3
                    {
                        return (None, None);
                    }
                }
            }

            if let Some(type_def) = schema
                .types
                .iter()
                .find(|t| t.singleton && singleton_matches(t, filename, rel_path))
            {
                return (Some(validate_singleton(&doc, type_def, user_config)), None);
            }

            let doc_id = crate::graph::path_to_id(path);
            let type_name = schema
                .type_name_for_doc_id(&doc_id)
                .or_else(|| infer_type_from_path(path, dir, schema))
                .or_else(|| {
                    doc.frontmatter
                        .as_ref()
                        .and_then(|fm| fm.get_display("type"))
                });

            let type_name = match type_name {
                Some(t) => t,
                None => return (None, None),
            };

            let type_entry = (path.display().to_string(), type_name);
            let result = validate_document(&doc, schema, &known_files, &known_ids, user_config);
            (Some(result), Some(type_entry))
        })
        .collect();

    let mut file_results = Vec::new();
    let mut type_entries = Vec::new();
    for (fr, te) in per_file {
        if let Some(r) = fr {
            file_results.push(r);
        }
        if let Some(e) = te {
            type_entries.push(e);
        }
    }

    // Skip directory-level checks when filtering by pattern
    if pattern.is_none() {
        validate_type_counts(&type_entries, schema, &mut file_results);
        validate_singleton_presence(&files, dir, schema, &mut file_results);
        validate_license_file(&files, dir, schema, &mut file_results);
        validate_team_docs(dir, user_config, &mut file_results);
        validate_service_readmes(dir, &mut file_results);
        validate_hardcoded_test_ports(dir, &mut file_results);
        validate_dependabot(dir, &mut file_results);
    }

    // Strip root prefix so paths are relative to project root
    let prefix = dir.display().to_string();
    for fr in &mut file_results {
        if let Some(rest) = fr.path.strip_prefix(&prefix) {
            fr.path = rest.trim_start_matches('/').to_string();
        }
    }

    Ok(ValidationResult { file_results })
}

fn validate_type_counts(
    type_entries: &[(String, String)],
    schema: &Schema,
    file_results: &mut Vec<FileResult>,
) {
    let mut type_counts: HashMap<&str, Vec<&str>> = HashMap::new();
    for (path, type_name) in type_entries {
        type_counts
            .entry(type_name.as_str())
            .or_default()
            .push(path.as_str());
    }

    for type_def in &schema.types {
        if let Some(max) = type_def.max_count {
            if let Some(paths) = type_counts.get(type_def.name.as_str()) {
                if paths.len() > max {
                    let diag = Diagnostic {
                        severity: Severity::Error,
                        code: "T010".into(),
                        message: format!(
                            "type \"{}\" has {} document(s) but max_count is {}",
                            type_def.name,
                            paths.len(),
                            max
                        ),
                        location: format!("type \"{}\"", type_def.name),
                        hint: Some(format!("files: {}", paths.join(", "))),
                    };
                    if let Some(excess_path) = paths.get(max) {
                        if let Some(fr) = file_results.iter_mut().find(|fr| fr.path == *excess_path)
                        {
                            fr.diagnostics.push(diag);
                        } else {
                            file_results.push(FileResult {
                                path: excess_path.to_string(),
                                diagnostics: vec![diag],
                            });
                        }
                    }
                }
            }
        }
    }
}

pub(crate) fn validate_singleton_presence(
    files: &[PathBuf],
    dir: &Path,
    schema: &Schema,
    file_results: &mut Vec<FileResult>,
) {
    for type_def in &schema.types {
        if !type_def.singleton {
            continue;
        }
        let pattern = match &type_def.match_pattern {
            Some(p) => p,
            None => continue,
        };

        if type_def.max_count != Some(1) {
            continue;
        }

        let found = files.iter().any(|p| {
            let filename = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let rel_path = p.strip_prefix(dir).unwrap_or(p);
            singleton_matches(type_def, filename, rel_path)
        });

        if !found {
            let has_required = type_def.sections.iter().any(|s| s.required);
            if has_required {
                let expected_path = match type_def.folder.as_deref() {
                    Some(".") | None => pattern.clone(),
                    Some(folder) => format!("{folder}/{pattern}"),
                };
                file_results.push(FileResult {
                    path: expected_path.clone(),
                    diagnostics: vec![Diagnostic {
                        severity: Severity::Error,
                        code: "T020".into(),
                        message: format!(
                            "singleton type \"{}\" expects file \"{}\" but it was not found",
                            type_def.name, expected_path
                        ),
                        location: format!("type \"{}\"", type_def.name),
                        hint: Some(format!("create {} in the project", expected_path)),
                    }],
                });
            }
        }
    }
}

pub(crate) fn validate_license_file(
    files: &[PathBuf],
    dir: &Path,
    schema: &Schema,
    file_results: &mut Vec<FileResult>,
) {
    let readme_type = match schema.types.iter().find(|t| {
        t.singleton
            && t.folder.as_deref() == Some(".")
            && t.match_pattern.as_deref() == Some("README.md")
    }) {
        Some(t) => t,
        None => return,
    };

    if !readme_type
        .sections
        .iter()
        .any(|s| s.name.eq_ignore_ascii_case("license") && s.required)
    {
        return;
    }

    let readme_path = match files.iter().find(|p| {
        let rel = p.strip_prefix(dir).unwrap_or(p);
        rel == Path::new("README.md")
    }) {
        Some(p) => p,
        None => return,
    };

    let doc = match Document::from_file(readme_path) {
        Ok(d) => d,
        Err(_) => return,
    };
    let parsed = doc.parse_body();
    let license_section = match parsed.find_section("License") {
        Some(s) => s,
        None => return,
    };

    if license_section
        .content
        .to_ascii_lowercase()
        .contains("proprietary")
    {
        return;
    }

    const LICENSE_NAMES: &[&str] = &[
        "LICENSE",
        "LICENSE.md",
        "LICENSE.txt",
        "LICENCE",
        "LICENCE.md",
        "LICENCE.txt",
        "COPYING",
        "COPYING.txt",
    ];

    if LICENSE_NAMES.iter().any(|name| dir.join(name).exists()) {
        return;
    }

    file_results.push(FileResult {
        path: readme_path.display().to_string(),
        diagnostics: vec![Diagnostic {
            severity: Severity::Error,
            code: "L001".into(),
            message: "non-proprietary license requires a LICENSE file at project root".into(),
            location: "License section".into(),
            hint: Some(
                "add a LICENSE file (e.g. LICENSE, LICENSE.md, or LICENSE.txt), \
                 or write 'Proprietary' in the License section"
                    .into(),
            ),
        }],
    });
}

fn validate_team_docs(
    dir: &Path,
    user_config: Option<&OrgConfig>,
    file_results: &mut Vec<FileResult>,
) {
    let teams_dir = dir.join("docs/teams");
    let entries = match std::fs::read_dir(&teams_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let known_teams: HashSet<&str> = match user_config {
        Some(oc) => oc.teams.keys().map(String::as_str).collect(),
        None => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };
        if !known_teams.contains(stem) {
            file_results.push(FileResult {
                path: path.display().to_string(),
                diagnostics: vec![Diagnostic {
                    severity: Severity::Warning,
                    code: "T030".into(),
                    message: format!("team doc '{stem}.md' has no matching team in org.kdl"),
                    location: "docs/teams/".into(),
                    hint: Some(format!(
                        "add team \"{stem}\" to .dg/org.kdl or rename/remove this file"
                    )),
                }],
            });
        }
    }
}

/// SV011/SV012/SV013: warn when a GitHub-hosted project lacks Dependabot coverage.
/// Skipped entirely for projects not hosted on github.com or using Renovate.
fn validate_dependabot(dir: &Path, file_results: &mut Vec<FileResult>) {
    match crate::code_refs::detect_repo_web_url(dir) {
        Some((url, _)) if url.starts_with("https://github.com/") => {}
        _ => return,
    }
    dependabot_diagnostics(dir, file_results);
}

/// Dependabot coverage checks without the GitHub-hosting gate (separate for testability).
pub(crate) fn dependabot_diagnostics(dir: &Path, file_results: &mut Vec<FileResult>) {
    use crate::dependabot as db;

    if db::has_renovate(dir) {
        return;
    }

    let hits = db::detect_ecosystems(dir);
    if !hits.is_empty() {
        let config_path = db::find_config(dir);
        let labels = hits.iter().map(db::EcosystemHit::label).collect::<Vec<_>>();
        match config_path {
            None => {
                file_results.push(FileResult {
                    path: dir.join(".github/dependabot.yml").display().to_string(),
                    diagnostics: vec![Diagnostic {
                        severity: Severity::Warning,
                        code: "SV011".into(),
                        message: format!(
                            "GitHub-hosted project has no .github/dependabot.yml but {} package ecosystem(s) were detected",
                            hits.len()
                        ),
                        location: ".github/dependabot.yml".into(),
                        hint: Some(format!(
                            "run `dg init --dependabot` to generate one covering: {}",
                            labels.join(", ")
                        )),
                    }],
                });
            }
            Some(path) => {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    let missing = db::uncovered_hits(&text, &hits);
                    if !missing.is_empty() {
                        let missing_labels = missing
                            .iter()
                            .map(db::EcosystemHit::label)
                            .collect::<Vec<_>>();
                        file_results.push(FileResult {
                            path: path.display().to_string(),
                            diagnostics: vec![Diagnostic {
                                severity: Severity::Warning,
                                code: "SV012".into(),
                                message: format!(
                                    "dependabot.yml is missing updates entries for {} detected ecosystem(s)",
                                    missing.len()
                                ),
                                location: ".github/dependabot.yml".into(),
                                hint: Some(format!(
                                    "add package-ecosystem entries for: {}",
                                    missing_labels.join(", ")
                                )),
                            }],
                        });
                    }
                }
            }
        }
    }

    // Dependabot's nix ecosystem only covers flake.lock — devenv.lock needs a workflow
    if db::detect_devenv(dir) && !db::has_devenv_update_workflow(dir) {
        file_results.push(FileResult {
            path: dir
                .join(".github/workflows/update-devenv-lock.yml")
                .display()
                .to_string(),
            diagnostics: vec![Diagnostic {
                severity: Severity::Warning,
                code: "SV013".into(),
                message: "devenv.lock drifts silently: Dependabot does not cover devenv and no \
                          `devenv update` workflow was found"
                    .into(),
                location: ".github/workflows/".into(),
                hint: Some(
                    "run `dg init --dependabot` to generate a scheduled workflow that runs \
                     `devenv update` and opens a PR"
                        .into(),
                ),
            }],
        });
    }
}

/// Options for running service linter/test checks.
#[derive(Debug, Default, Clone)]
pub struct ServiceCheckOptions {
    /// Skip auto-installing JS dependencies when node_modules is missing.
    pub no_install: bool,
}

/// Run detected linters and test suites for services/apps/infra.
/// Produces SV006/SV007 (linters), SV009/SV010 (tests) and SV014
/// (JS dependencies not installed) diagnostics.
pub fn validate_service_checks(dir: &Path, opts: &ServiceCheckOptions) -> Vec<FileResult> {
    let mut results = Vec::new();
    let mut ctx = crate::toolchain::ToolchainContext::new(dir, opts.no_install);
    let dirs = ["services", "apps", "infra"];

    for kind_dir in &dirs {
        let target = dir.join(kind_dir);
        let entries = match std::fs::read_dir(&target) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let service_dir = entry.path();
            if !service_dir.is_dir() {
                continue;
            }

            let readme = service_dir.join("README.md");
            let folder_name = service_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let rel = format!("{kind_dir}/{folder_name}/README.md");
            let location = format!("{kind_dir}/{folder_name}/");

            let tech = crate::service::extract_tech_stack(&service_dir);
            let fm = readme
                .exists()
                .then(|| crate::document::Document::from_file(&readme).ok())
                .flatten();
            let fm_ref = fm.as_ref().and_then(|d| d.frontmatter.as_ref());

            let practices = crate::service::detect_engineering_practices(
                &service_dir,
                &tech.primary_language,
                fm_ref,
            );

            let linter_tool = practices
                .has_linter
                .then_some(practices.linter_tool.as_deref())
                .flatten();
            let test_framework = practices
                .has_tests
                .then_some(practices.test_framework.as_deref())
                .flatten();

            if linter_tool.is_none() && test_framework.is_none() {
                continue;
            }

            // JS services need node_modules before any tool can run.
            let is_js = matches!(
                tech.primary_language.as_str(),
                "JavaScript" | "TypeScript" | "Node.js"
            );
            let js = is_js.then(|| crate::toolchain::detect_js_toolchain(&service_dir, dir));
            if let Some(js) = &js {
                if let Some(diag) = ensure_js_deps_diagnostic(&mut ctx, js, &location) {
                    results.push(FileResult {
                        path: rel,
                        diagnostics: vec![diag],
                    });
                    // Running tools without deps would produce misleading
                    // SV007/SV010 failures.
                    continue;
                }
            }

            if let Some(tool) = linter_tool {
                if let Some(fr) =
                    check_service_linter(&ctx, &service_dir, tool, js.as_ref(), &rel, &location)
                {
                    results.push(fr);
                }
            }

            if let Some(framework) = test_framework {
                if let Some(fr) =
                    check_service_tests(&ctx, &service_dir, framework, js.as_ref(), &rel, &location)
                {
                    results.push(fr);
                }
            }
        }
    }

    results
}

/// Ensure JS deps are installed; return an SV014 diagnostic when they are
/// missing and could not be installed.
fn ensure_js_deps_diagnostic(
    ctx: &mut crate::toolchain::ToolchainContext,
    js: &crate::toolchain::JsToolchain,
    location: &str,
) -> Option<Diagnostic> {
    use crate::toolchain::InstallOutcome;

    let pm = js.pm.name();
    let (install_prog, install_args) = js.pm.install_command(js.has_lockfile);
    let install_cmd = std::iter::once(install_prog)
        .chain(install_args.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    let root = js.workspace_root.display();

    let hint = match ctx.ensure_js_deps(js) {
        InstallOutcome::AlreadyInstalled | InstallOutcome::Installed => return None,
        InstallOutcome::SkippedNoInstall => {
            format!("node_modules missing in {root}; rerun without --no-install or run `{install_cmd}` there")
        }
        InstallOutcome::SkippedNoPm { pm_binary } => {
            let mut hint = format!(
                "`{pm_binary}` is not on PATH; install it (e.g. via corepack) and run `{install_cmd}` in {root}"
            );
            if let Some(env_hint) = ctx.env_hint() {
                hint.push_str("\n        ");
                hint.push_str(&env_hint);
            }
            hint
        }
        InstallOutcome::Failed {
            exit_code,
            output_tail,
        } => {
            let code = exit_code
                .map(|c| format!(" (exit code {c})"))
                .unwrap_or_default();
            format!("`{install_cmd}` failed{code} in {root}:\n        {output_tail}")
        }
    };

    Some(Diagnostic {
        severity: Severity::Warning,
        code: "SV014".into(),
        message: format!("dependencies not installed for {location} ({pm})"),
        location: location.to_string(),
        hint: Some(hint),
    })
}

/// Run the detected linter and produce SV006/SV007 diagnostics.
fn check_service_linter(
    ctx: &crate::toolchain::ToolchainContext,
    service_dir: &Path,
    tool: &str,
    js: Option<&crate::toolchain::JsToolchain>,
    rel: &str,
    location: &str,
) -> Option<FileResult> {
    let mut cmd = crate::service::resolve_lint_command(tool, js)?;
    let (program, args) = ctx.finalize(&cmd.program, cmd.args.clone());
    cmd.program = program;
    cmd.args = args;

    let lint_result = crate::service::run_linter(service_dir, &cmd);

    if lint_result.command_not_found {
        let mut hint = format!("install {tool} or set has_linter: false in frontmatter");
        if let Some(env_hint) = ctx.env_hint() {
            hint.push_str("\n        ");
            hint.push_str(&env_hint);
        }
        return Some(FileResult {
            path: rel.to_string(),
            diagnostics: vec![Diagnostic {
                severity: Severity::Warning,
                code: "SV006".into(),
                message: format!("linter \"{tool}\" detected but not installed"),
                location: location.to_string(),
                hint: Some(hint),
            }],
        });
    }

    if lint_result.success {
        return None;
    }

    let issue_count = lint_result.issues.len();
    let label = if issue_count > 0 {
        format!("{tool} found {issue_count} issue(s) in {location}")
    } else {
        // Exit code non-zero but couldn't parse issues
        let code = lint_result
            .exit_code
            .map(|c| format!(" (exit code {c})"))
            .unwrap_or_default();
        format!("{tool} failed{code} in {location}")
    };

    let hint = if lint_result.issues.is_empty() {
        crate::toolchain::output_preview(&lint_result.stdout, &lint_result.stderr, 10)
    } else {
        let max_show = 10;
        let mut hint_lines: Vec<String> = lint_result
            .issues
            .iter()
            .take(max_show)
            .map(|i| i.to_hint_line())
            .collect();
        if issue_count > max_show {
            hint_lines.push(format!("... and {} more", issue_count - max_show));
        }
        Some(hint_lines.join("\n        "))
    };

    Some(FileResult {
        path: rel.to_string(),
        diagnostics: vec![Diagnostic {
            severity: Severity::Error,
            code: "SV007".into(),
            message: label,
            location: location.to_string(),
            hint,
        }],
    })
}

/// Run the detected test suite and produce SV009/SV010 diagnostics.
fn check_service_tests(
    ctx: &crate::toolchain::ToolchainContext,
    service_dir: &Path,
    framework: &str,
    js: Option<&crate::toolchain::JsToolchain>,
    rel: &str,
    location: &str,
) -> Option<FileResult> {
    let mut cmd = crate::service::resolve_test_command(framework, js)?;
    let (program, args) = ctx.finalize(&cmd.program, cmd.args.clone());
    cmd.program = program;
    cmd.args = args;

    let test_result = crate::service::run_tests(service_dir, &cmd);

    if test_result.command_not_found {
        let mut hint = format!("install {framework} or set has_tests: false in frontmatter");
        if let Some(env_hint) = ctx.env_hint() {
            hint.push_str("\n        ");
            hint.push_str(&env_hint);
        }
        return Some(FileResult {
            path: rel.to_string(),
            diagnostics: vec![Diagnostic {
                severity: Severity::Warning,
                code: "SV009".into(),
                message: format!("test runner \"{framework}\" detected but not installed"),
                location: location.to_string(),
                hint: Some(hint),
            }],
        });
    }

    if test_result.success {
        return None;
    }

    let code = test_result
        .exit_code
        .map(|c| format!(" (exit code {c})"))
        .unwrap_or_default();
    // Test runners like vitest print failures at the end of stdout, so show
    // the output tail (stderr appended when present).
    let hint = crate::toolchain::output_preview(&test_result.stdout, &test_result.stderr, 12);

    Some(FileResult {
        path: rel.to_string(),
        diagnostics: vec![Diagnostic {
            severity: Severity::Error,
            code: "SV010".into(),
            message: format!("{framework} failed{code} in {location}"),
            location: location.to_string(),
            hint,
        }],
    })
}

/// SV015: a Phoenix test config that binds a hardcoded port with
/// `server: true` (the Wallaby setup) collides with parallel test runs or a
/// stale process (`:eaddrinuse`). Suggest an OS-assigned port instead.
pub(crate) fn validate_hardcoded_test_ports(dir: &Path, file_results: &mut Vec<FileResult>) {
    let dirs = ["services", "apps", "infra"];
    for kind_dir in &dirs {
        let target = dir.join(kind_dir);
        let entries = match std::fs::read_dir(&target) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let service_dir = entry.path();
            if !service_dir.is_dir() {
                continue;
            }
            let config_path = service_dir.join("config").join("test.exs");
            let content = match std::fs::read_to_string(&config_path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            // Strip full-line comments so commented-out examples don't trigger.
            let code: String = content
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n");

            // Only a config with `server: true` actually binds the port
            // during `mix test`.
            if !code.contains("server: true") {
                continue;
            }
            let port = match hardcoded_listener_port(&code) {
                Some(p) => p,
                None => continue,
            };

            let folder_name = service_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let rel = format!("{kind_dir}/{folder_name}/config/test.exs");

            let uses_wallaby = code.contains(":wallaby") || code.contains("Wallaby");
            let mut hint = "use `port: 0` so the OS assigns a free port and parallel test runs \
                 never collide with :eaddrinuse; resolve the bound port at runtime \
                 via `Endpoint.server_info(:http)` in test_helper.exs"
                .to_string();
            if uses_wallaby {
                hint.push_str(
                    "\n        then point Wallaby at it: \
                     `Application.put_env(:wallaby, :base_url, \"http://localhost:#{port}\")` \
                     instead of a hardcoded base_url",
                );
            }

            file_results.push(FileResult {
                path: rel.clone(),
                diagnostics: vec![Diagnostic {
                    severity: Severity::Warning,
                    code: "SV015".into(),
                    message: format!(
                        "test endpoint binds hardcoded port {port} (server: true) in {rel}"
                    ),
                    location: rel.clone(),
                    hint: Some(hint),
                }],
            });
        }
    }
}

/// Find a literal non-zero port inside an `http:`/`https:` listener keyword
/// list (e.g. `http: [ip: {127, 0, 0, 1}, port: 4002]`). Ports outside a
/// listener list (like `url: [port: 443]`) don't bind and are ignored.
fn hardcoded_listener_port(code: &str) -> Option<u32> {
    for key in ["http: [", "https: ["] {
        let mut rest = code;
        while let Some(start) = rest.find(key) {
            let after = &rest[start + key.len()..];
            let list = &after[..after.find(']').unwrap_or(after.len())];
            if let Some(pos) = list.find("port:") {
                let value = list[pos + "port:".len()..].trim_start();
                let digits: String = value.chars().take_while(|c| c.is_ascii_digit()).collect();
                match digits.parse::<u32>() {
                    Ok(0) | Err(_) => {}
                    Ok(port) => return Some(port),
                }
            }
            rest = after;
        }
    }
    None
}

pub(crate) fn validate_service_readmes(dir: &Path, file_results: &mut Vec<FileResult>) {
    let dirs = ["services", "apps", "infra"];
    for kind_dir in &dirs {
        let target = dir.join(kind_dir);
        let entries = match std::fs::read_dir(&target) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let readme = path.join("README.md");
            let folder_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let rel = format!("{kind_dir}/{folder_name}/README.md");

            if !readme.exists() {
                file_results.push(FileResult {
                    path: readme.display().to_string(),
                    diagnostics: vec![Diagnostic {
                        severity: Severity::Error,
                        code: "SV001".into(),
                        message: format!("{kind_dir}/{folder_name}/ is missing README.md"),
                        location: rel,
                        hint: Some(format!(
                            "create {kind_dir}/{folder_name}/README.md with YAML frontmatter containing at least 'owner'"
                        )),
                    }],
                });
                continue;
            }

            let doc = match crate::document::Document::from_file(&readme) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let fm = doc.frontmatter.as_ref();

            if fm.is_none() {
                file_results.push(FileResult {
                    path: readme.display().to_string(),
                    diagnostics: vec![Diagnostic {
                        severity: Severity::Error,
                        code: "SV002".into(),
                        message: format!("{rel} has no YAML frontmatter"),
                        location: rel,
                        hint: Some(
                            "add YAML frontmatter with at least 'owner' and 'status' fields, e.g.:\n---\nowner: handle\nstatus: live\n---"
                                .into(),
                        ),
                    }],
                });
                continue;
            }

            let has_owner = fm
                .and_then(|f| f.get_display("owner"))
                .map(|s| !s.is_empty() && s != "Unknown")
                .unwrap_or(false);

            if !has_owner {
                file_results.push(FileResult {
                    path: readme.display().to_string(),
                    diagnostics: vec![Diagnostic {
                        severity: Severity::Error,
                        code: "SV003".into(),
                        message: format!("{rel} is missing required 'owner' field in frontmatter"),
                        location: rel.clone(),
                        hint: Some(
                            "add 'owner: handle' or 'owner: team-name' to the YAML frontmatter"
                                .into(),
                        ),
                    }],
                });
            }

            // SV004/SV005: engineering practices checks
            let tech = crate::service::extract_tech_stack(&path);
            let practices =
                crate::service::detect_engineering_practices(&path, &tech.primary_language, fm);

            let mut practice_diags = Vec::new();

            if !practices.has_linter {
                practice_diags.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "SV004".into(),
                    message: format!("{rel} has no linter configuration detected"),
                    location: rel.clone(),
                    hint: Some(
                        "add linter config (e.g. .eslintrc, ruff.toml) or set has_linter: true in frontmatter"
                            .into(),
                    ),
                });
            }

            if !practices.has_tests {
                practice_diags.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "SV005".into(),
                    message: format!("{rel} has no test files or directories detected"),
                    location: rel,
                    hint: Some(
                        "add test files/dirs (e.g. tests/, __tests__/) or set has_tests: true in frontmatter"
                            .into(),
                    ),
                });
            }

            if !practice_diags.is_empty() {
                file_results.push(FileResult {
                    path: readme.display().to_string(),
                    diagnostics: practice_diags,
                });
            }

            // SV006: Architecture section must contain a mermaid or d2 diagram (≥5 lines)
            validate_architecture_diagram(&doc.body, &readme, kind_dir, folder_name, file_results);

            // SV008: EOL version warnings (only when avatars/ureq feature is enabled)
            #[cfg(feature = "avatars")]
            {
                let cache_dir = dir.join(".dg").join("cache");
                if cache_dir.join("eol").exists() {
                    let today = eol_today_str();
                    let eol_warnings = crate::eol::check_service_eol(&tech, &cache_dir, &today);
                    let eol_diags: Vec<Diagnostic> = eol_warnings
                        .into_iter()
                        .map(|w| {
                            let eol_since = w
                                .eol_date
                                .as_deref()
                                .map(|d| format!(" (since {d})"))
                                .unwrap_or_default();
                            let slug = w.product.to_lowercase().replace(' ', "").replace(".", "");
                            Diagnostic {
                                severity: Severity::Warning,
                                code: "SV008".into(),
                                message: format!(
                                    "{} {} is end-of-life{}",
                                    w.product, w.version, eol_since
                                ),
                                location: format!("{kind_dir}/{folder_name}/"),
                                hint: Some(format!(
                                    "upgrade to a supported version (see https://endoflife.date/{slug})"
                                )),
                            }
                        })
                        .collect();
                    if !eol_diags.is_empty() {
                        file_results.push(FileResult {
                            path: readme.display().to_string(),
                            diagnostics: eol_diags,
                        });
                    }
                }
            }
        }
    }
}

/// Validate that an ## Architecture section exists and contains a mermaid/d2 diagram with ≥5 lines.
fn validate_architecture_diagram(
    body: &str,
    readme_path: &Path,
    kind_dir: &str,
    folder_name: &str,
    file_results: &mut Vec<FileResult>,
) {
    let rel = format!("{kind_dir}/{folder_name}/README.md");
    let lines: Vec<&str> = body.lines().collect();

    // Find ## Architecture heading
    let arch_start = lines.iter().position(|l| {
        let trimmed = l.trim().to_lowercase();
        trimmed == "## architecture" || trimmed.starts_with("## architecture ")
    });

    let Some(arch_idx) = arch_start else {
        file_results.push(FileResult {
            path: readme_path.display().to_string(),
            diagnostics: vec![Diagnostic {
                severity: Severity::Warning,
                code: "SV006".into(),
                message: format!("{rel} is missing an ## Architecture section"),
                location: rel,
                hint: Some(
                    "add an ## Architecture section with a mermaid or d2 diagram showing inputs, outputs, and integrations".into(),
                ),
            }],
        });
        return;
    };

    // Find the end of the Architecture section (next ## heading or end of file)
    let arch_end = lines[arch_idx + 1..]
        .iter()
        .position(|l| l.starts_with("## "))
        .map(|p| arch_idx + 1 + p)
        .unwrap_or(lines.len());

    let section_lines = &lines[arch_idx + 1..arch_end];

    // Look for a fenced code block with mermaid or d2 language
    let mut in_diagram = false;
    let mut diagram_lines = 0usize;
    let mut has_diagram = false;

    for line in section_lines {
        let trimmed = line.trim();
        if !in_diagram && (trimmed.starts_with("```mermaid") || trimmed.starts_with("```d2")) {
            in_diagram = true;
            diagram_lines = 0;
            continue;
        }
        if in_diagram {
            if trimmed == "```" {
                if diagram_lines >= 5 {
                    has_diagram = true;
                }
                in_diagram = false;
            } else {
                // Count non-empty lines
                if !trimmed.is_empty() {
                    diagram_lines += 1;
                }
            }
        }
    }

    if !has_diagram {
        file_results.push(FileResult {
            path: readme_path.display().to_string(),
            diagnostics: vec![Diagnostic {
                severity: Severity::Warning,
                code: "SV006".into(),
                message: format!(
                    "{rel} ## Architecture section must contain a mermaid or d2 diagram with at least 5 lines"
                ),
                location: rel,
                hint: Some(
                    "add a ```mermaid or ```d2 fenced code block showing service inputs, outputs, and integrations".into(),
                ),
            }],
        });
    }
}

#[cfg(feature = "avatars")]
fn eol_today_str() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (secs / 86400) as i64;
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

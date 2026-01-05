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

/// Run detected linters for services/apps/infra and produce SV006/SV007 diagnostics.
pub fn validate_service_linters(dir: &Path) -> Vec<FileResult> {
    let mut results = Vec::new();
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

            if !practices.has_linter {
                continue;
            }

            let tool = match &practices.linter_tool {
                Some(t) => t,
                None => continue,
            };

            let cmd = match crate::service::resolve_lint_command(tool) {
                Some(c) => c,
                None => continue,
            };

            let lint_result = crate::service::run_linter(&service_dir, &cmd);

            if lint_result.command_not_found {
                results.push(FileResult {
                    path: rel.clone(),
                    diagnostics: vec![Diagnostic {
                        severity: Severity::Warning,
                        code: "SV006".into(),
                        message: format!("linter \"{tool}\" detected but not installed"),
                        location: format!("{kind_dir}/{folder_name}/"),
                        hint: Some(format!(
                            "install {tool} or set has_linter: false in frontmatter"
                        )),
                    }],
                });
                continue;
            }

            if !lint_result.success {
                let issue_count = lint_result.issues.len();
                let label = if issue_count > 0 {
                    format!("{tool} found {issue_count} issue(s) in {kind_dir}/{folder_name}/")
                } else {
                    // Exit code non-zero but couldn't parse issues
                    let code = lint_result
                        .exit_code
                        .map(|c| format!(" (exit code {c})"))
                        .unwrap_or_default();
                    format!("{tool} failed{code} in {kind_dir}/{folder_name}/")
                };

                let hint = if lint_result.issues.is_empty() {
                    // Show first few lines of stderr as hint
                    let preview: String = lint_result
                        .stderr
                        .lines()
                        .take(5)
                        .collect::<Vec<_>>()
                        .join("\n        ");
                    if preview.is_empty() {
                        None
                    } else {
                        Some(preview)
                    }
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

                results.push(FileResult {
                    path: rel,
                    diagnostics: vec![Diagnostic {
                        severity: Severity::Error,
                        code: "SV007".into(),
                        message: label,
                        location: format!("{kind_dir}/{folder_name}/"),
                        hint,
                    }],
                });
            }
        }
    }

    results
}

/// Run detected test suites for services/apps/infra and produce SV009/SV010 diagnostics.
pub fn validate_service_tests(dir: &Path) -> Vec<FileResult> {
    let mut results = Vec::new();
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

            if !practices.has_tests {
                continue;
            }

            let framework = match &practices.test_framework {
                Some(t) => t,
                None => continue,
            };

            let cmd = match crate::service::resolve_test_command(framework) {
                Some(c) => c,
                None => continue,
            };

            let test_result = crate::service::run_tests(&service_dir, &cmd);

            if test_result.command_not_found {
                results.push(FileResult {
                    path: rel.clone(),
                    diagnostics: vec![Diagnostic {
                        severity: Severity::Warning,
                        code: "SV009".into(),
                        message: format!("test runner \"{framework}\" detected but not installed"),
                        location: format!("{kind_dir}/{folder_name}/"),
                        hint: Some(format!(
                            "install {framework} or set has_tests: false in frontmatter"
                        )),
                    }],
                });
                continue;
            }

            if !test_result.success {
                let code = test_result
                    .exit_code
                    .map(|c| format!(" (exit code {c})"))
                    .unwrap_or_default();

                // Show first few lines of combined output as hint
                let combined = if test_result.stderr.is_empty() {
                    test_result.stdout.clone()
                } else {
                    test_result.stderr.clone()
                };
                let hint = {
                    let preview: String = combined
                        .lines()
                        .take(10)
                        .collect::<Vec<_>>()
                        .join("\n        ");
                    if preview.is_empty() {
                        None
                    } else {
                        Some(preview)
                    }
                };

                results.push(FileResult {
                    path: rel,
                    diagnostics: vec![Diagnostic {
                        severity: Severity::Error,
                        code: "SV010".into(),
                        message: format!("{framework} failed{code} in {kind_dir}/{folder_name}/"),
                        location: format!("{kind_dir}/{folder_name}/"),
                        hint,
                    }],
                });
            }
        }
    }

    results
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

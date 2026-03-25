//! Hook handlers for AI agents and git hooks.
//!
//! These commands are called by wrapper scripts (Claude Code hooks, git hooks).
//! Putting logic here allows users to upgrade dg and get new functionality
//! without regenerating hook scripts.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct HooksArgs {
    #[command(subcommand)]
    pub command: HooksCommand,
}

#[derive(Subcommand)]
pub enum HooksCommand {
    /// Check for FIXME/TBD markers after file write
    CheckFixme {
        /// Path to the file that was written
        file_path: Option<String>,
    },
    /// Check if changed files match decision doc code_paths
    CheckCode {
        /// Changed file paths (from git diff or CLI args)
        files: Vec<String>,
    },
    /// Git prepare-commit-msg hook: auto-add Refs trailer for staged docs
    PrepareCommitMsg {
        /// Path to the commit message file
        message_file: String,
        /// Source of the commit message (message, template, merge, squash, commit)
        source: Option<String>,
    },
    /// Git commit-msg hook: warn if staged doc changes aren't referenced
    CommitMsg {
        /// Path to the commit message file
        message_file: String,
    },
    /// PreToolUse hook: deny forbidden commands (e.g. dg init --eject)
    DenyCommand,
    /// Run linters for services/apps (AI agent integration)
    CheckLint,
    /// Stop hook: check for remaining work (validation errors, suggestions, unimplemented specs)
    Stop,
}

pub fn run(args: &HooksArgs, root: &Path) -> Result<()> {
    match &args.command {
        HooksCommand::CheckFixme { file_path } => check_fixme(file_path.as_deref(), root),
        HooksCommand::CheckCode { files } => check_code(root, files),
        HooksCommand::PrepareCommitMsg {
            message_file,
            source,
        } => prepare_commit_msg(message_file, source.as_deref()),
        HooksCommand::CommitMsg { message_file } => commit_msg(message_file),
        HooksCommand::DenyCommand => deny_command(),
        HooksCommand::CheckLint => check_lint(root),
        HooksCommand::Stop => stop(root),
    }
}

fn check_fixme(file_path: Option<&str>, root: &Path) -> Result<()> {
    let Some(path_str) = file_path else {
        return Ok(());
    };

    // Only check files in docs/ directory
    if !path_str.starts_with("docs/") {
        return Ok(());
    }

    let abs = root.join(path_str);
    if !abs.is_file() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&abs)?;
    let content_upper = content.to_uppercase();

    let markers = ["FIXME", "TBD", "TODO", "XXX", "[TBD]", "[FIXME]"];
    let count: usize = markers
        .iter()
        .map(|m| content_upper.matches(m).count())
        .sum();

    if count > 0 {
        eprintln!();
        eprintln!("⚠️  Document has {count} incomplete marker(s) (TBD/FIXME).");
        eprintln!("   Use AskUserQuestion to gather missing info, then update the document.");
        eprintln!("   Only leave markers if user says they don't know or asks you to proceed.");
        eprintln!();
    }

    Ok(())
}

fn check_code(root: &Path, files: &[String]) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }

    let dg_dir = root.join(".dg");
    if !dg_dir.is_dir() {
        return Ok(());
    }

    let schema = load_schema(&dg_dir)?;

    let matches =
        md_db::code_paths::check_code_paths(root, &schema, files).context("check-code failed")?;

    if matches.is_empty() {
        return Ok(());
    }

    // Group by changed file
    let mut by_file: std::collections::BTreeMap<&str, Vec<&md_db::code_paths::CodePathMatch>> =
        std::collections::BTreeMap::new();
    for m in &matches {
        by_file.entry(&m.changed_file).or_default().push(m);
    }

    eprintln!();
    eprintln!(
        "⚠️  {} file(s) match decision doc code_paths:",
        by_file.len()
    );
    for (file, doc_matches) in &by_file {
        for m in doc_matches {
            let title = m.title.as_deref().unwrap_or("untitled");
            let status = m.status.as_deref().unwrap_or("unknown");
            eprintln!("   {file}  →  {} \"{title}\" ({status})", m.doc_id);
        }
    }
    eprintln!("   Review these docs and update if your changes affect the decisions.");
    eprintln!();

    Ok(())
}

/// Get staged file paths from git.
fn staged_files() -> Vec<String> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only", "--diff-filter=ACMR"])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// Extract document IDs from staged file paths (e.g. docs/architecture/adr-001-foo.md → ADR-001).
fn extract_doc_ids(files: &[String]) -> Vec<String> {
    let mut ids: Vec<String> = files
        .iter()
        .filter(|f| f.starts_with("docs/") && f.ends_with(".md"))
        .filter_map(|f| {
            let path = std::path::Path::new(f);
            let id = md_db::graph::path_to_id(path);
            // Only keep IDs that look like PREFIX-NNN (not arbitrary filenames)
            if id.contains('-')
                && id
                    .split('-')
                    .next()
                    .is_some_and(|p| p.chars().all(|c| c.is_ascii_alphabetic()))
                && id
                    .split('-')
                    .nth(1)
                    .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
            {
                Some(id)
            } else {
                None
            }
        })
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

fn prepare_commit_msg(message_file: &str, source: Option<&str>) -> Result<()> {
    // Skip for merge/squash commits — git generates those messages
    if matches!(source, Some("merge") | Some("squash")) {
        return Ok(());
    }

    let ids = extract_doc_ids(&staged_files());
    if ids.is_empty() {
        return Ok(());
    }

    let content =
        std::fs::read_to_string(message_file).context("failed to read commit message file")?;

    // Don't add if Refs: trailer already exists
    if content.lines().any(|l| l.starts_with("Refs:")) {
        return Ok(());
    }

    let refs_line = format!("Refs: {}", ids.join(", "));

    // Insert before comment lines (lines starting with #) at the end
    let mut lines: Vec<&str> = content.lines().collect();
    let insert_pos = lines
        .iter()
        .position(|l| l.starts_with('#'))
        .unwrap_or(lines.len());

    // Add blank line before Refs if needed
    if insert_pos > 0 && !lines[insert_pos - 1].is_empty() {
        lines.insert(insert_pos, "");
        lines.insert(insert_pos + 1, &refs_line);
    } else {
        lines.insert(insert_pos, &refs_line);
    }

    // Ensure trailing newline
    let mut result = lines.join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }

    std::fs::write(message_file, result).context("failed to write commit message file")?;

    Ok(())
}

fn commit_msg(message_file: &str) -> Result<()> {
    let content =
        std::fs::read_to_string(message_file).context("failed to read commit message file")?;

    let ids = extract_doc_ids(&staged_files());
    if ids.is_empty() {
        return Ok(());
    }

    // Check which IDs are mentioned anywhere in the commit message
    let content_upper = content.to_uppercase();
    let missing: Vec<&str> = ids
        .iter()
        .filter(|id| !content_upper.contains(id.as_str()))
        .map(|s| s.as_str())
        .collect();

    if !missing.is_empty() {
        eprintln!();
        eprintln!(
            "⚠️  Staged doc(s) not referenced in commit message: {}",
            missing.join(", ")
        );
        eprintln!("   Consider adding: Refs: {}", missing.join(", "));
        eprintln!();
    }

    // Always succeed — this is advisory, not blocking
    Ok(())
}

fn stop(root: &Path) -> Result<()> {
    let dg_dir = root.join(".dg");
    if !dg_dir.is_dir() {
        return Ok(());
    }

    let schema = load_schema(&dg_dir)?;
    let org = load_org_config(&dg_dir);
    let mut issues = Vec::new();

    // 1. Validate — block on errors
    let val_result = md_db::validation::validate_directory(root, &schema, None, org.as_ref())?;
    let errors = val_result.total_errors();
    if errors > 0 {
        issues.push(format!(
            "dg validate found {errors} error(s). Run `dg validate` and fix them."
        ));
    }

    // 2. Suggest — advisory (print but don't block)
    let today = today_str();
    let suggestions = md_db::suggest::suggest_directory(root, &schema, None, &today)?;
    let total = suggestions.total();
    if total > 0 {
        let doc_count = suggestions
            .file_results
            .iter()
            .filter(|f| !f.suggestions.is_empty())
            .count();
        eprintln!(
            "ℹ️  dg suggest found {total} suggestion(s) across {doc_count} document(s). Run `dg suggest` for details."
        );
    }

    // 3. Check for SPEC docs without implementation code
    let has_code = ["services", "apps", "src", "lib"].iter().any(|d| {
        let dir = root.join(d);
        dir.is_dir()
            && std::fs::read_dir(&dir)
                .map(|mut r| r.next().is_some())
                .unwrap_or(false)
    });

    if !has_code {
        let files = md_db::discovery::discover_files(root, None, &[], false)?;
        for f in &files {
            let id = md_db::graph::path_to_id(f);
            if !id.starts_with("SPEC-") {
                continue;
            }
            let doc = match md_db::document::Document::from_file(f) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let fm = match &doc.frontmatter {
                Some(fm) => fm,
                None => continue,
            };
            let title = fm.get_display("title").filter(|t| !t.is_empty());
            let status = fm.get_display("status").unwrap_or_default();
            let label = match &title {
                Some(t) => format!("{id} \"{t}\""),
                None => id.clone(),
            };
            issues.push(format!(
                "{label} ({status}) has no implementation code yet.\n  \
                 Start building based on the spec: create the project in services/ or apps/."
            ));
        }
    }

    if issues.is_empty() {
        return Ok(());
    }

    eprintln!("Stop hook found remaining work:\n");
    for issue in &issues {
        eprintln!("- {issue}");
    }
    eprintln!("\nPlease continue working on these items.");
    std::process::exit(2);
}

fn load_schema(dg_dir: &Path) -> Result<md_db::schema::Schema> {
    let schema_path = dg_dir.join("schema.kdl");
    if schema_path.is_file() {
        md_db::schema::Schema::from_file(&schema_path).context("failed to load schema")
    } else {
        md_db::schema::Schema::from_str(dg_schemas::SCHEMA)
            .context("failed to parse built-in schema")
    }
}

fn load_org_config(dg_dir: &Path) -> Option<md_db::users::OrgConfig> {
    let org_path = dg_dir.join(md_db::users::ORG_CONFIG_FILENAME);
    if org_path.is_file() {
        md_db::users::OrgConfig::from_file(&org_path).ok()
    } else {
        None
    }
}

fn today_str() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (secs / 86400) as i64;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn check_lint(root: &Path) -> Result<()> {
    let dirs = ["services", "apps", "infra"];

    for kind_dir in &dirs {
        let target = root.join(kind_dir);
        let entries = match std::fs::read_dir(&target) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let service_dir = entry.path();
            if !service_dir.is_dir() {
                continue;
            }

            let folder_name = service_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let tech = md_db::service::extract_tech_stack(&service_dir);
            let practices = md_db::service::detect_engineering_practices(
                &service_dir,
                &tech.primary_language,
                None,
            );

            if !practices.has_linter {
                continue;
            }
            let tool = match &practices.linter_tool {
                Some(t) => t,
                None => continue,
            };
            let cmd = match md_db::service::resolve_lint_command(tool) {
                Some(c) => c,
                None => continue,
            };

            let result = md_db::service::run_linter(&service_dir, &cmd);

            if result.command_not_found {
                continue;
            }

            if !result.success {
                let count = result.issues.len();
                if count > 0 {
                    eprintln!();
                    eprintln!(
                        "\u{26a0}\u{fe0f}  {tool} found {count} issue(s) in {kind_dir}/{folder_name}:"
                    );
                    for issue in result.issues.iter().take(10) {
                        eprintln!("   {}", issue.to_hint_line());
                    }
                    if count > 10 {
                        eprintln!("   ... and {} more", count - 10);
                    }
                    eprintln!("   Fix these linter issues before committing.");
                    eprintln!();
                } else {
                    eprintln!();
                    eprintln!("\u{26a0}\u{fe0f}  {tool} failed in {kind_dir}/{folder_name}.");
                    for line in result.stderr.lines().take(5) {
                        eprintln!("   {line}");
                    }
                    eprintln!();
                }
            }
        }
    }

    // Always exit 0 — advisory only
    Ok(())
}

/// Deny forbidden commands in PreToolUse Bash hook.
/// Reads CLAUDE_TOOL_INPUT env (JSON with "command" field).
/// Prints block JSON to stdout if command is forbidden.
fn deny_command() -> Result<()> {
    let input = std::env::var("CLAUDE_TOOL_INPUT").unwrap_or_default();
    if let Some((decision, reason)) = deny_command_with_input(&input) {
        let response = serde_json::json!({ "decision": decision, "reason": reason });
        println!("{response}");
    }
    Ok(())
}

/// Inner logic for deny_command — returns Some((decision, reason)) if the command should be blocked.
fn deny_command_with_input(input: &str) -> Option<(String, String)> {
    let command = serde_json::from_str::<serde_json::Value>(input)
        .ok()
        .and_then(|v| v.get("command").and_then(|c| c.as_str()).map(String::from))
        .unwrap_or_default();

    const DENIED: &[(&str, &str)] = &[(
        "dg init --eject",
        "dg init --eject is reserved for humans only. Ask the user to run it manually.",
    )];

    for (pattern, reason) in DENIED {
        if command.contains(pattern) {
            return Some(("block".to_string(), reason.to_string()));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    // ── Template validation helpers ───────────────────────────────────────────

    #[derive(clap::Parser)]
    struct TestHooksCli {
        #[command(subcommand)]
        cmd: HooksCommand,
    }

    fn valid_hook_subcommand_names() -> Vec<String> {
        use clap::CommandFactory;
        TestHooksCli::command()
            .get_subcommands()
            .map(|sc| sc.get_name().to_string())
            .collect()
    }

    /// Extract `dg hooks <subcommand>` names from any string (JSON or shell script).
    fn extract_hook_subcommands(content: &str) -> Vec<String> {
        let mut result = Vec::new();
        for line in content.lines() {
            if let Some(pos) = line.find("dg hooks ") {
                let rest = &line[pos + "dg hooks ".len()..];
                let cmd = rest
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim_matches('"');
                if !cmd.is_empty() && !cmd.starts_with('$') {
                    result.push(cmd.to_string());
                }
            }
        }
        result.dedup();
        result
    }

    // ── Template validation tests ─────────────────────────────────────────────

    #[test]
    fn hooks_settings_is_valid_json() {
        serde_json::from_str::<serde_json::Value>(dg_schemas::HOOKS_SETTINGS).unwrap();
    }

    #[test]
    fn hooks_settings_subcommands_match_implementation() {
        let valid = valid_hook_subcommand_names();
        for cmd in extract_hook_subcommands(dg_schemas::HOOKS_SETTINGS) {
            assert!(
                valid.contains(&cmd),
                "HOOKS_SETTINGS references unknown subcommand 'dg hooks {cmd}'. Valid: {valid:?}"
            );
        }
    }

    #[test]
    fn gemini_hook_scripts_subcommands_match_implementation() {
        let valid = valid_hook_subcommand_names();
        for (name, content) in [
            ("check-fixme.sh", dg_schemas::GEMINI_HOOK_CHECK_FIXME),
            ("check-code.sh", dg_schemas::GEMINI_HOOK_CHECK_CODE),
        ] {
            for cmd in extract_hook_subcommands(content) {
                assert!(
                    valid.contains(&cmd),
                    "Gemini {name} references unknown subcommand 'dg hooks {cmd}'. Valid: {valid:?}"
                );
            }
        }
    }

    #[test]
    fn opencode_hook_scripts_subcommands_match_implementation() {
        let valid = valid_hook_subcommand_names();
        for (name, content) in [
            ("check-fixme.sh", dg_schemas::OPENCODE_HOOK_CHECK_FIXME),
            ("check-code.sh", dg_schemas::OPENCODE_HOOK_CHECK_CODE),
        ] {
            for cmd in extract_hook_subcommands(content) {
                assert!(
                    valid.contains(&cmd),
                    "OpenCode {name} references unknown subcommand 'dg hooks {cmd}'. Valid: {valid:?}"
                );
            }
        }
    }

    // ── deny_command unit tests ───────────────────────────────────────────────

    #[test]
    fn deny_command_blocks_eject() {
        let result = deny_command_with_input(r#"{"command":"dg init --eject"}"#);
        assert!(result.is_some());
        let (decision, _reason) = result.unwrap();
        assert_eq!(decision, "block");
    }

    #[test]
    fn deny_command_allows_normal_commands() {
        assert!(deny_command_with_input(r#"{"command":"dg list"}"#).is_none());
    }

    #[test]
    fn deny_command_empty_input_is_allowed() {
        assert!(deny_command_with_input("").is_none());
    }

    // ── check_fixme unit tests ────────────────────────────────────────────────

    #[test]
    fn check_fixme_detects_markers_in_docs() {
        let tmp = tempfile::tempdir().unwrap();
        let docs_dir = tmp.path().join("docs");
        fs::create_dir_all(&docs_dir).unwrap();
        fs::write(docs_dir.join("adr-001.md"), "# ADR\n\nFIXME: incomplete").unwrap();
        // Should not error — warnings go to stderr, not errors
        check_fixme(Some("docs/adr-001.md"), tmp.path()).unwrap();
    }

    #[test]
    fn check_fixme_ignores_files_outside_docs() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("main.rs"), "FIXME: fix me").unwrap();
        check_fixme(Some("main.rs"), tmp.path()).unwrap();
    }

    #[test]
    fn check_fixme_clean_doc_passes_silently() {
        let tmp = tempfile::tempdir().unwrap();
        let docs_dir = tmp.path().join("docs");
        fs::create_dir_all(&docs_dir).unwrap();
        fs::write(docs_dir.join("adr-001.md"), "# ADR\n\nAll good here.").unwrap();
        check_fixme(Some("docs/adr-001.md"), tmp.path()).unwrap();
    }

    #[test]
    fn check_fixme_nonexistent_file_is_ok() {
        let tmp = tempfile::tempdir().unwrap();
        check_fixme(Some("docs/nonexistent.md"), tmp.path()).unwrap();
    }

    // ── extract_doc_ids unit tests ────────────────────────────────────────────

    #[test]
    fn extract_doc_ids_parses_doc_paths() {
        let files = vec!["docs/architecture/adr-001-foo.md".to_string()];
        assert_eq!(extract_doc_ids(&files), vec!["ADR-001"]);
    }

    #[test]
    fn extract_doc_ids_ignores_non_doc_paths() {
        let files = vec!["src/main.rs".to_string(), "Cargo.toml".to_string()];
        assert!(extract_doc_ids(&files).is_empty());
    }
}

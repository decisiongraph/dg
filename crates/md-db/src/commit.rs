//! Git commit message parsing and validation for DecisionGraph.
//!
//! Parses conventional commits with git trailers (Refs: DOC-001) and validates
//! document references against schema rules.

use crate::schema::Schema;
use anyhow::{Context, Result};
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::BTreeMap;
use std::path::Path;

lazy_static! {
    /// Conventional commit subject line: type(scope)!: subject
    static ref SUBJECT_RE: Regex = Regex::new(
        r"^([a-z]+)(?:\(([^)]+)\))?(!)?:\s*(.+)$"
    ).unwrap();

    /// Git trailer format: Key: value
    static ref TRAILER_RE: Regex = Regex::new(
        r"^([A-Z][A-Za-z-\s]+):\s*(.+)$"
    ).unwrap();

    /// Document ID pattern (matches schema, case-insensitive)
    static ref DOC_ID_RE: Regex = Regex::new(
        r"(?i)\b(ADR|OPP|POL|INC|SPEC)-\d{3}\b"
    ).unwrap();

    /// Merge commit detection
    static ref MERGE_RE: Regex = Regex::new(
        r"^Merge\s+(branch|pull\s+request|remote-tracking\s+branch)"
    ).unwrap();

    /// Revert commit detection
    static ref REVERT_RE: Regex = Regex::new(
        r#"^Revert\s+"#
    ).unwrap();

    /// WIP commit detection (case-insensitive, at start of subject only)
    static ref WIP_RE: Regex = Regex::new(
        r"(?i)^wip[:\s]"
    ).unwrap();
}

/// Parsed conventional commit message with git trailers.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitMessage {
    /// Commit type (feat, fix, chore, etc.)
    pub type_: String,
    /// Optional scope (component/module)
    pub scope: Option<String>,
    /// Subject line (first line after type)
    pub subject: String,
    /// Optional body text (paragraphs before trailers)
    pub body: Option<String>,
    /// Document IDs from Refs trailer
    pub doc_ids: Vec<String>,
    /// Has breaking change indicator (! or BREAKING CHANGE trailer)
    pub breaking: bool,
    /// All git trailers (Refs, Co-authored-by, etc.)
    pub trailers: BTreeMap<String, Vec<String>>,
}

/// Parse a git commit message into structured components.
///
/// Supports conventional commits format with git trailers:
/// ```text
/// type(scope): subject
///
/// Body paragraphs.
///
/// Refs: DOC-001, DOC-002
/// Co-authored-by: Name <email>
/// ```
pub fn parse_commit_message(msg: &str) -> Result<CommitMessage> {
    let lines: Vec<&str> = msg.lines().collect();
    if lines.is_empty() {
        anyhow::bail!("Empty commit message");
    }

    // Parse subject line
    let subject_line = lines[0].trim();
    let caps = SUBJECT_RE
        .captures(subject_line)
        .context("Invalid conventional commit format. Expected: type(scope): subject")?;

    let type_ = caps[1].to_string();
    let scope = caps.get(2).map(|m| m.as_str().to_string());
    let breaking_marker = caps.get(3).is_some();
    let subject = caps[4].to_string();

    // Split message into body and trailers
    let (body_lines, trailer_lines) = split_body_trailers(&lines[1..]);

    // Parse body
    let body = if body_lines.is_empty() {
        None
    } else {
        Some(body_lines.join("\n").trim().to_string())
    };

    // Parse trailers
    let mut trailers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in trailer_lines {
        if let Some(caps) = TRAILER_RE.captures(line) {
            let key = caps[1].to_string();
            let value = caps[2].trim().to_string();
            trailers.entry(key).or_default().push(value);
        }
    }

    // Extract document IDs from Refs trailer
    let doc_ids = extract_doc_ids_from_trailers(&trailers);

    // Check for breaking change
    let has_breaking_trailer =
        trailers.contains_key("BREAKING CHANGE") || trailers.contains_key("BREAKING-CHANGE");
    let breaking = breaking_marker || has_breaking_trailer;

    Ok(CommitMessage {
        type_,
        scope,
        subject,
        body,
        doc_ids,
        breaking,
        trailers,
    })
}

/// Split message lines into body and trailers.
///
/// Trailers are at the end of the message, starting from the last blank line.
fn split_body_trailers<'a>(lines: &'a [&'a str]) -> (Vec<&'a str>, Vec<&'a str>) {
    if lines.is_empty() {
        return (vec![], vec![]);
    }

    // Find last blank line separator
    let mut last_blank_idx = None;
    for (i, line) in lines.iter().enumerate().rev() {
        if line.trim().is_empty() {
            last_blank_idx = Some(i);
            break;
        }
    }

    if let Some(blank_idx) = last_blank_idx {
        // Check if everything after blank line looks like trailers
        let potential_trailers: Vec<&str> = lines[blank_idx + 1..]
            .iter()
            .filter(|l| !l.trim().is_empty())
            .copied()
            .collect();

        let all_trailers = potential_trailers
            .iter()
            .all(|line| TRAILER_RE.is_match(line));

        if all_trailers && !potential_trailers.is_empty() {
            // Split at blank line
            return (lines[..blank_idx].to_vec(), potential_trailers);
        }
    }

    // No trailers found, entire thing is body
    (lines.to_vec(), vec![])
}

/// Extract document IDs from Refs trailer values.
fn extract_doc_ids_from_trailers(trailers: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut doc_ids = Vec::new();

    if let Some(refs_values) = trailers.get("Refs") {
        for value in refs_values {
            // Split on comma and extract IDs
            for id in DOC_ID_RE.find_iter(value) {
                doc_ids.push(id.as_str().to_string());
            }
        }
    }

    doc_ids
}

/// Extract document IDs from a file path.
///
/// Example: `docs/architecture/adr-001.md` → `["ADR-001"]`
pub fn extract_doc_ids_from_path(path: &Path) -> Vec<String> {
    let path_str = path.to_string_lossy();
    DOC_ID_RE
        .find_iter(&path_str)
        .map(|m| m.as_str().to_uppercase())
        .collect()
}

/// Check if commit is a special type that should skip validation.
///
/// Returns true for: merge commits, revert commits, WIP commits.
pub fn is_special_commit(msg: &str) -> bool {
    let first_line = msg.lines().next().unwrap_or("");

    if MERGE_RE.is_match(first_line) || REVERT_RE.is_match(first_line) {
        return true;
    }

    // Check for WIP at the start of the whole message
    if WIP_RE.is_match(first_line) {
        return true;
    }

    // Check for WIP in the subject part (after type:)
    // For "feat(scope): wip something", the subject is "wip something"
    if let Some(caps) = SUBJECT_RE.captures(first_line) {
        let subject = &caps[4];
        if subject.to_lowercase().starts_with("wip ") || subject.to_lowercase() == "wip" {
            return true;
        }
    }

    false
}

/// Validation warning for commit messages.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitWarning {
    /// Warning message
    pub message: String,
    /// Severity level
    pub level: WarningLevel,
}

/// Warning severity levels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WarningLevel {
    /// Informational (best practices)
    Info,
    /// Should be fixed (missing doc reference)
    Warning,
    /// Invalid format or broken reference
    Error,
}

/// Validate a commit message against schema and config rules.
///
/// Returns a list of warnings. Always returns Ok (warnings don't block commits).
pub fn validate_commit_message(
    msg: &CommitMessage,
    schema: &Schema,
    docs_root: &Path,
    recommend_refs_for: &[String],
) -> Result<Vec<CommitWarning>> {
    let mut warnings = Vec::new();

    // Validate document ID format
    for doc_id in &msg.doc_ids {
        let matches_format = schema.ref_formats.iter().any(|rf| {
            Regex::new(&rf.pattern)
                .map(|re| re.is_match(doc_id))
                .unwrap_or(false)
        });

        if !matches_format {
            warnings.push(CommitWarning {
                message: format!("Invalid document ID format: {}", doc_id),
                level: WarningLevel::Error,
            });
        }
    }

    // Check document existence
    for doc_id in &msg.doc_ids {
        let exists = check_document_exists(docs_root, doc_id);
        if !exists {
            warnings.push(CommitWarning {
                message: format!("Document not found: {}", doc_id),
                level: WarningLevel::Warning,
            });
        }
    }

    // Check if commit type should have document references
    if recommend_refs_for.contains(&msg.type_) && msg.doc_ids.is_empty() {
        warnings.push(CommitWarning {
            message: format!(
                "'{}' commits should reference a document (use Refs: DOC-001)",
                msg.type_
            ),
            level: WarningLevel::Warning,
        });
    }

    Ok(warnings)
}

/// Check if a document exists in the docs directory.
fn check_document_exists(docs_root: &Path, doc_id: &str) -> bool {
    // Convert ID to lowercase for filename matching
    let id_lower = doc_id.to_lowercase();

    // Determine type folder
    let folder = match &id_lower[..3] {
        "adr" => "architecture",
        "opp" => "opportunities",
        "pol" => "policies",
        "inc" => "incidents",
        "spe" => "specifications",
        _ => return false,
    };

    let type_dir = docs_root.join(folder);
    if !type_dir.exists() {
        return false;
    }

    // Look for any file matching the pattern: {id}*.md
    if let Ok(entries) = std::fs::read_dir(&type_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.to_lowercase().starts_with(&id_lower) && name.ends_with(".md") {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_commit_with_trailer() {
        let msg = "feat(auth): add OAuth2\n\nImplements flow.\n\nRefs: ADR-001";
        let parsed = parse_commit_message(msg).unwrap();
        assert_eq!(parsed.type_, "feat");
        assert_eq!(parsed.scope, Some("auth".to_string()));
        assert_eq!(parsed.subject, "add OAuth2");
        assert_eq!(parsed.body, Some("Implements flow.".to_string()));
        assert_eq!(parsed.doc_ids, vec!["ADR-001"]);
        assert!(!parsed.breaking);
    }

    #[test]
    fn test_parse_multiple_docs_in_trailer() {
        let msg = "fix(api): handle nulls\n\nRefs: ADR-001, INC-002";
        let parsed = parse_commit_message(msg).unwrap();
        assert_eq!(parsed.doc_ids, vec!["ADR-001", "INC-002"]);
    }

    #[test]
    fn test_parse_commit_without_trailer() {
        let msg = "chore: update dependencies";
        let parsed = parse_commit_message(msg).unwrap();
        assert_eq!(parsed.type_, "chore");
        assert!(parsed.doc_ids.is_empty());
        assert_eq!(parsed.scope, None);
    }

    #[test]
    fn test_parse_breaking_change_marker() {
        let msg = "feat(api)!: remove legacy endpoint";
        let parsed = parse_commit_message(msg).unwrap();
        assert!(parsed.breaking);
    }

    #[test]
    fn test_parse_breaking_change_trailer() {
        let msg = "feat(api): update response\n\nBREAKING CHANGE: removes field X";
        let parsed = parse_commit_message(msg).unwrap();
        assert!(parsed.breaking);
    }

    #[test]
    fn test_is_merge_commit() {
        assert!(is_special_commit("Merge branch 'feature' into main"));
        assert!(is_special_commit(
            "Merge pull request #123 from user/branch"
        ));
        assert!(!is_special_commit("feat: add feature"));
    }

    #[test]
    fn test_is_wip_commit() {
        assert!(is_special_commit("wip: testing auth"));
        assert!(is_special_commit("WIP: auth flow"));
        assert!(is_special_commit("feat(auth): wip oauth flow"));
        assert!(!is_special_commit("feat: add wip endpoint"));
    }

    #[test]
    fn test_is_revert_commit() {
        assert!(is_special_commit("Revert \"feat: add feature\""));
        assert!(!is_special_commit("feat: revert changes"));
    }

    #[test]
    fn test_extract_doc_id_from_path() {
        let path = Path::new("docs/architecture/adr-001-use-postgres.md");
        let ids = extract_doc_ids_from_path(path);
        assert_eq!(ids, vec!["ADR-001"]);
    }

    #[test]
    fn test_extract_multiple_doc_ids_from_path() {
        let path = Path::new("docs/opportunities/opp-002.md");
        let ids = extract_doc_ids_from_path(path);
        assert_eq!(ids, vec!["OPP-002"]);
    }

    #[test]
    fn test_parse_multiple_trailers() {
        let msg = "feat: add auth\n\nRefs: ADR-001\nCo-authored-by: Bob <bob@example.com>";
        let parsed = parse_commit_message(msg).unwrap();
        assert_eq!(parsed.doc_ids, vec!["ADR-001"]);
        assert_eq!(
            parsed.trailers.get("Co-authored-by").unwrap(),
            &vec!["Bob <bob@example.com>"]
        );
    }

    #[test]
    fn test_parse_invalid_format() {
        let msg = "not a conventional commit";
        assert!(parse_commit_message(msg).is_err());
    }

    #[test]
    fn test_split_body_trailers() {
        let lines = vec![
            "",
            "This is the body.",
            "More body text.",
            "",
            "Refs: ADR-001",
            "Co-authored-by: Alice",
        ];
        let (body, trailers) = split_body_trailers(&lines);
        assert_eq!(body.len(), 3); // blank + 2 body lines
        assert_eq!(trailers.len(), 2);
    }

    #[test]
    fn test_split_no_trailers() {
        let lines = vec!["", "Just body text.", "More text."];
        let (body, trailers) = split_body_trailers(&lines);
        assert_eq!(body.len(), 3);
        assert_eq!(trailers.len(), 0);
    }
}

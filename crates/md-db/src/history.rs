use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use git2::Repository;
use serde::Serialize;

/// A status transition detected from git history.
#[derive(Debug, Clone, Serialize)]
pub struct StatusTransition {
    pub from_status: Option<String>,
    pub to_status: String,
    pub date: String,
    pub commit_sha: String,
}

/// Walk HEAD history for the given document paths and extract status transitions.
///
/// Only walks OPP files (typically <50). Limits: 500 commits or 2 years of history.
/// Returns a map from document ID to list of transitions (oldest first).
pub fn collect_status_history(
    repo_path: &Path,
    doc_paths: &[(String, PathBuf)],
) -> Result<BTreeMap<String, Vec<StatusTransition>>, git2::Error> {
    let repo = Repository::open(repo_path)?;
    let mut result: BTreeMap<String, Vec<StatusTransition>> = BTreeMap::new();

    if doc_paths.is_empty() {
        return Ok(result);
    }

    // Walk HEAD revwalk
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    // 2 years in seconds
    let two_years_secs: i64 = 2 * 365 * 24 * 60 * 60;
    let head_commit = repo.head()?.peel_to_commit()?;
    let cutoff_time = head_commit.time().seconds() - two_years_secs;

    let mut commit_count = 0u32;
    let max_commits = 500u32;

    // Track last-seen status per doc to detect transitions
    // We walk newest-first, so we build transitions in reverse
    let mut last_status: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut raw_transitions: BTreeMap<String, Vec<StatusTransition>> = BTreeMap::new();

    // Compute repo-relative paths for each doc
    let repo_root = repo
        .workdir()
        .unwrap_or(repo_path)
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());

    let relative_paths: Vec<(String, String)> = doc_paths
        .iter()
        .filter_map(|(id, path)| {
            let abs = path.canonicalize().ok()?;
            let rel = abs.strip_prefix(&repo_root).ok()?;
            Some((id.clone(), rel.to_string_lossy().to_string()))
        })
        .collect();

    if relative_paths.is_empty() {
        return Ok(result);
    }

    for oid in revwalk {
        let oid = match oid {
            Ok(o) => o,
            Err(_) => continue,
        };

        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Check limits
        commit_count += 1;
        if commit_count > max_commits {
            break;
        }
        if commit.time().seconds() < cutoff_time {
            break;
        }

        let tree = match commit.tree() {
            Ok(t) => t,
            Err(_) => continue,
        };

        let date = format_commit_date(&commit);
        let sha = oid.to_string();

        for (doc_id, rel_path) in &relative_paths {
            let blob_content = match tree.get_path(Path::new(rel_path)) {
                Ok(entry) => {
                    let obj = match entry.to_object(&repo) {
                        Ok(o) => o,
                        Err(_) => continue,
                    };
                    match obj.as_blob() {
                        Some(blob) => String::from_utf8_lossy(blob.content()).to_string(),
                        None => continue,
                    }
                }
                Err(_) => continue, // file didn't exist in this commit
            };

            let status = extract_status_from_content(&blob_content);

            let prev = last_status.get(doc_id).cloned().flatten();
            if prev.as_deref() != status.as_deref() {
                if let Some(ref s) = status {
                    raw_transitions
                        .entry(doc_id.clone())
                        .or_default()
                        .push(StatusTransition {
                            from_status: prev,
                            to_status: s.clone(),
                            date: date.clone(),
                            commit_sha: sha.clone(),
                        });
                }
            }
            last_status.insert(doc_id.clone(), status);
        }
    }

    // Reverse transitions so oldest is first
    for (id, mut transitions) in raw_transitions {
        transitions.reverse();
        result.insert(id, transitions);
    }

    Ok(result)
}

/// Extract YAML frontmatter status field from raw file content.
fn extract_status_from_content(content: &str) -> Option<String> {
    // Quick parse: find ---\n...\n--- and extract status field
    if !content.starts_with("---") {
        return None;
    }
    let after_first = &content[3..];
    let end = after_first.find("\n---")?;
    let yaml_block = &after_first[..end];

    for line in yaml_block.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("status:") {
            let val = rest.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

/// Format a git commit's author date as YYYY-MM-DD.
fn format_commit_date(commit: &git2::Commit) -> String {
    let time = commit.time();
    let secs = time.seconds();
    // Convert epoch seconds to YYYY-MM-DD using simple arithmetic
    // (days since epoch → civil date)
    let days = secs / 86400;
    epoch_days_to_date(days)
}

/// Convert days since 1970-01-01 to YYYY-MM-DD string.
/// Uses the same algorithm as suggest.rs.
fn epoch_days_to_date(days: i64) -> String {
    // Civil date from epoch days (Howard Hinnant algorithm)
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_status() {
        let content = "---\ntitle: Test\nstatus: pursuing\ntype: opp\n---\n\nBody\n";
        assert_eq!(
            extract_status_from_content(content),
            Some("pursuing".to_string())
        );
    }

    #[test]
    fn test_extract_status_quoted() {
        let content = "---\ntitle: Test\nstatus: \"completed\"\n---\n\nBody\n";
        assert_eq!(
            extract_status_from_content(content),
            Some("completed".to_string())
        );
    }

    #[test]
    fn test_extract_status_missing() {
        let content = "---\ntitle: Test\ntype: opp\n---\n\nBody\n";
        assert_eq!(extract_status_from_content(content), None);
    }

    #[test]
    fn test_extract_status_no_frontmatter() {
        let content = "# No frontmatter\n\nBody\n";
        assert_eq!(extract_status_from_content(content), None);
    }

    #[test]
    fn test_epoch_days_to_date() {
        assert_eq!(epoch_days_to_date(0), "1970-01-01");
        assert_eq!(epoch_days_to_date(18628), "2021-01-01");
        // 2026-02-11 = day 20495
        assert_eq!(epoch_days_to_date(20495), "2026-02-11");
    }
}

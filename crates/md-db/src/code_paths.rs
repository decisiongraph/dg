use std::path::Path;

use crate::document::Document;
use crate::schema::Schema;

/// A match between a changed file and a decision document's code_paths.
#[derive(Debug, Clone)]
pub struct CodePathMatch {
    /// The changed file that matched (as provided)
    pub changed_file: String,
    /// Document ID (e.g. "ADR-001")
    pub doc_id: String,
    /// Document title from frontmatter
    pub title: Option<String>,
    /// Document type (e.g. "adr", "pol")
    pub doc_type: Option<String>,
    /// Document status (e.g. "accepted", "active")
    pub status: Option<String>,
    /// The glob pattern that matched
    pub matched_pattern: String,
    /// Relative path to the decision document
    pub doc_path: String,
}

/// Check changed files against code_paths in decision documents.
///
/// Discovers all documents in `dir`, extracts `code_paths` from frontmatter,
/// and glob-matches each changed file against the patterns.
pub fn check_code_paths(
    dir: impl AsRef<Path>,
    schema: &Schema,
    changed_files: &[String],
) -> crate::error::Result<Vec<CodePathMatch>> {
    let dir = dir.as_ref();
    let files = crate::discovery::discover_files(dir, None, &[], false)?;

    let mut matches = Vec::new();

    for path in &files {
        let doc = match Document::from_file(path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let fm = match &doc.frontmatter {
            Some(fm) => fm,
            None => continue,
        };

        // Only check doc types that have code_paths
        let doc_type = fm
            .get_display("type")
            .or_else(|| crate::validation::infer_type_from_path(path, dir, schema));

        let code_paths = match fm.get("code_paths") {
            Some(val) => extract_string_array(val),
            None => continue,
        };

        if code_paths.is_empty() {
            continue;
        }

        let doc_id = crate::graph::path_to_id(path);
        let title = fm
            .get_display("title")
            .or_else(|| crate::ast_util::first_heading_text(&doc.body));
        let status = fm.get_display("status");
        let doc_path = path.strip_prefix(dir).unwrap_or(path).display().to_string();

        for changed in changed_files {
            for pattern in &code_paths {
                if matches_code_path(changed, pattern) {
                    matches.push(CodePathMatch {
                        changed_file: changed.clone(),
                        doc_id: doc_id.clone(),
                        title: title.clone(),
                        doc_type: doc_type.clone(),
                        status: status.clone(),
                        matched_pattern: pattern.clone(),
                        doc_path: doc_path.clone(),
                    });
                    break; // One match per (changed_file, doc) pair is enough
                }
            }
        }
    }

    Ok(matches)
}

/// Match a file path against a code_paths glob pattern.
fn matches_code_path(file: &str, pattern: &str) -> bool {
    // Use glob::Pattern with options that allow ** matching
    let opts = glob::MatchOptions {
        case_sensitive: true,
        require_literal_separator: false,
        require_literal_leading_dot: false,
    };
    match glob::Pattern::new(pattern) {
        Ok(pat) => pat.matches_with(file, opts),
        Err(_) => false,
    }
}

/// Extract a Vec<String> from a serde_yaml::Value (string array or single string).
fn extract_string_array(val: &serde_yaml::Value) -> Vec<String> {
    match val {
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        serde_yaml::Value::String(s) => vec![s.clone()],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_code_path_glob() {
        assert!(matches_code_path("src/db/connection.rs", "src/db/**"));
        assert!(matches_code_path("src/db/pool/mod.rs", "src/db/**"));
        assert!(!matches_code_path("src/api/routes.rs", "src/db/**"));
    }

    #[test]
    fn test_matches_code_path_exact() {
        assert!(matches_code_path("Makefile", "Makefile"));
        assert!(!matches_code_path("Makefile.bak", "Makefile"));
    }

    #[test]
    fn test_matches_code_path_wildcard() {
        assert!(matches_code_path(
            "migrations/001_init.sql",
            "migrations/**"
        ));
        assert!(matches_code_path("config/database.toml", "config/*.toml"));
        assert!(!matches_code_path("config/database.yaml", "config/*.toml"));
    }

    #[test]
    fn test_extract_string_array() {
        let seq = serde_yaml::Value::Sequence(vec![
            serde_yaml::Value::String("src/db/**".into()),
            serde_yaml::Value::String("migrations/**".into()),
        ]);
        assert_eq!(
            extract_string_array(&seq),
            vec!["src/db/**", "migrations/**"]
        );

        let single = serde_yaml::Value::String("src/db/**".into());
        assert_eq!(extract_string_array(&single), vec!["src/db/**"]);

        let null = serde_yaml::Value::Null;
        assert!(extract_string_array(&null).is_empty());
    }
}

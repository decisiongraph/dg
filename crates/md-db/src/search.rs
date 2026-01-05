use std::path::{Path, PathBuf};

use comrak::Arena;
use ignore::WalkBuilder;
use rayon::prelude::*;
use serde::Serialize;

use crate::ast_util;
use crate::discovery;
use crate::error::Result;
use crate::frontmatter::Frontmatter;

/// A single match within a document.
#[derive(Debug, Clone, Serialize)]
pub struct Match {
    /// The section heading where the match was found, or "frontmatter".
    pub section: String,
    /// 1-based line number in the original file.
    pub line: usize,
    /// Context snippet around the match.
    pub context: String,
}

/// Search result for one document (may contain multiple matches).
#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub path: String,
    /// Document ID from frontmatter (title), if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub matches: Vec<Match>,
}

/// Options controlling search behavior.
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    /// Only match within this section (case-insensitive heading match).
    pub section_filter: Option<String>,
    /// Only match within this frontmatter field.
    pub field_filter: Option<String>,
    /// Maximum total results (documents) to return.
    pub max_results: Option<usize>,
}

/// Search all markdown documents under `dir` for `query`.
/// Uses ignore::WalkBuilder (respects .gitignore) and rayon for parallel file processing.
pub fn search_documents(
    dir: impl AsRef<Path>,
    query: &str,
    options: &SearchOptions,
) -> Result<Vec<SearchResult>> {
    let dir = dir.as_ref();

    // Phase 1: Collect paths (respects .gitignore)
    let paths: Vec<PathBuf> = WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(true)
        .follow_links(true)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter(|e| !discovery::is_ignored_dir(e.path()))
        .filter(|e| e.path().extension().and_then(|ext| ext.to_str()) == Some("md"))
        .map(|e| e.into_path())
        .collect();

    // Phase 2: Parallel search
    let mut results: Vec<SearchResult> = paths
        .par_iter()
        .filter_map(|path| {
            let raw = std::fs::read_to_string(path).ok()?;
            search_single_document(path, &raw, query, options)
        })
        .collect();

    results.sort_by(|a, b| a.path.cmp(&b.path));

    // Apply max_results after sort (deterministic)
    if let Some(max) = options.max_results {
        results.truncate(max);
    }

    Ok(results)
}

/// Search a single document's raw content. Returns None if no matches.
fn search_single_document(
    path: &Path,
    raw: &str,
    query: &str,
    options: &SearchOptions,
) -> Option<SearchResult> {
    let (fm, body) = match Frontmatter::try_parse(raw) {
        Ok(r) => r,
        Err(_) => (None, raw.to_string()),
    };

    // Count lines in frontmatter block to compute body line offset.
    let body_line_offset = compute_body_line_offset(raw, &body);

    let doc_id = fm
        .as_ref()
        .and_then(extract_doc_id)
        .or_else(|| crate::ast_util::first_heading_text(&body));

    let mut matches = Vec::new();

    // Search frontmatter fields (unless section filter is set).
    if options.section_filter.is_none() {
        if let Some(ref fm) = fm {
            search_frontmatter(fm, raw, query, options, &mut matches);
        }
    }

    // Search body sections (unless field filter is set).
    if options.field_filter.is_none() {
        search_body(&body, body_line_offset, query, options, &mut matches);
    }

    if matches.is_empty() {
        return None;
    }

    Some(SearchResult {
        path: path.display().to_string(),
        id: doc_id,
        matches,
    })
}

/// Compute the 1-based line offset where the body starts in the raw file.
fn compute_body_line_offset(raw: &str, body: &str) -> usize {
    if let Some(pos) = raw.find(body) {
        raw[..pos].lines().count()
    } else {
        0
    }
}

/// Search frontmatter string fields for query.
fn search_frontmatter(
    fm: &Frontmatter,
    raw: &str,
    query: &str,
    options: &SearchOptions,
    matches: &mut Vec<Match>,
) {
    let raw_lines: Vec<&str> = raw.lines().collect();

    for key in fm.keys() {
        // If field_filter is set, only search that field.
        if let Some(ref field) = options.field_filter {
            if !key.eq_ignore_ascii_case(field) {
                continue;
            }
        }

        let display = match fm.get_display(key) {
            Some(v) => v,
            None => continue,
        };

        if !contains_match(&display, query, options.case_sensitive) {
            continue;
        }

        // Find the line in raw that contains this field.
        let needle = format!("{key}:");
        let line_num = raw_lines
            .iter()
            .position(|l| l.starts_with(&needle))
            .map(|i| i + 1) // 1-based
            .unwrap_or(1);

        let context = format!("{key}: {display}");
        matches.push(Match {
            section: "frontmatter".to_string(),
            line: line_num,
            context: highlight_match(&context, query, options.case_sensitive),
        });
    }
}

/// Search body content using comrak AST to identify sections.
fn search_body(
    body: &str,
    line_offset: usize,
    query: &str,
    options: &SearchOptions,
    matches: &mut Vec<Match>,
) {
    let arena = Arena::new();
    let root = ast_util::parse_md(&arena, body);

    // Build section map: for each line in body, what section heading is it in?
    let headings = ast_util::find_headings(root, None);
    let body_lines: Vec<&str> = body.lines().collect();

    // Build section ranges: (heading_text, start_line_0based, end_line_0based_exclusive)
    let mut section_ranges: Vec<(String, usize, usize)> = Vec::new();
    for heading_node in &headings {
        let heading_text = ast_util::collect_text(heading_node).trim().to_string();
        let sourcepos = heading_node.data.borrow().sourcepos;
        let start_line = sourcepos.start.line.saturating_sub(1); // 0-based
        section_ranges.push((heading_text, start_line, body_lines.len()));
    }
    // Set end of each section to start of next
    for i in 0..section_ranges.len().saturating_sub(1) {
        section_ranges[i].2 = section_ranges[i + 1].1;
    }

    // Search line by line
    for (line_idx, line) in body_lines.iter().enumerate() {
        if !contains_match(line, query, options.case_sensitive) {
            continue;
        }

        let section_name = section_for_line(line_idx, &section_ranges);

        // Apply section filter.
        if let Some(ref filter) = options.section_filter {
            if !section_name.eq_ignore_ascii_case(filter) {
                continue;
            }
        }

        let context = build_context(&body_lines, line_idx, 1);
        let file_line = line_offset + line_idx + 1; // 1-based

        matches.push(Match {
            section: section_name,
            line: file_line,
            context: highlight_match(&context, query, options.case_sensitive),
        });
    }
}

/// Determine which section a line belongs to.
fn section_for_line(line_idx: usize, sections: &[(String, usize, usize)]) -> String {
    for (name, start, end) in sections.iter().rev() {
        if line_idx >= *start && line_idx < *end {
            return name.clone();
        }
    }
    "(root)".to_string()
}

/// Build context snippet: the matching line plus surrounding lines.
fn build_context(lines: &[&str], match_idx: usize, radius: usize) -> String {
    let start = match_idx.saturating_sub(radius);
    let end = (match_idx + radius + 1).min(lines.len());
    let mut parts = Vec::new();
    for line in &lines[start..end] {
        let line = line.trim();
        if !line.is_empty() {
            parts.push(line);
        }
    }
    parts.join(" ")
}

/// Check if haystack contains query (case-sensitive or not).
fn contains_match(haystack: &str, query: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        haystack.contains(query)
    } else {
        haystack.to_lowercase().contains(&query.to_lowercase())
    }
}

/// Highlight the query match in context by wrapping in *asterisks*.
fn highlight_match(context: &str, query: &str, case_sensitive: bool) -> String {
    if case_sensitive {
        context.replace(query, &format!("*{query}*"))
    } else {
        // Find and replace preserving original case
        let lower_ctx = context.to_lowercase();
        let lower_query = query.to_lowercase();
        let mut result = String::new();
        let mut start = 0;
        while let Some(pos) = lower_ctx[start..].find(&lower_query) {
            let abs_pos = start + pos;
            result.push_str(&context[start..abs_pos]);
            result.push('*');
            result.push_str(&context[abs_pos..abs_pos + query.len()]);
            result.push('*');
            start = abs_pos + query.len();
        }
        result.push_str(&context[start..]);
        result
    }
}

/// Extract a document ID from frontmatter (use title).
fn extract_doc_id(fm: &Frontmatter) -> Option<String> {
    fm.get_display("title")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn create_test_dir() -> PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("md_db_search_test_{id}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_test_doc(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    const DOC1: &str = "\
---
title: Use PostgreSQL
type: adr
status: accepted
---

# Decision

We will use PostgreSQL as our primary database.

## Rationale

PostgreSQL offers great reliability and connection pooling support.

# Consequences

Some consequences here.
";

    const DOC2: &str = "\
---
title: Database Incident
type: inc
status: resolved
---

# Summary

Database connection pool exhausted due to leaked connections.

# Root Cause

Missing connection.close() in error paths causing connection pooling exhaustion.

# Resolution

Added explicit connection.close() in finally blocks.
";

    #[test]
    fn test_basic_search() {
        let dir = create_test_dir();
        write_test_doc(&dir, "adr-001.md", DOC1);
        write_test_doc(&dir, "inc-001.md", DOC2);

        let opts = SearchOptions::default();
        let results = search_documents(&dir, "connection pooling", &opts).unwrap();

        assert_eq!(results.len(), 2);
        for r in &results {
            assert!(!r.matches.is_empty());
        }
    }

    #[test]
    fn test_case_sensitive_search() {
        let dir = create_test_dir();
        write_test_doc(&dir, "adr-001.md", DOC1);

        let opts = SearchOptions {
            case_sensitive: true,
            ..Default::default()
        };
        let results = search_documents(&dir, "postgresql", &opts).unwrap();
        assert!(results.is_empty());

        let results = search_documents(&dir, "PostgreSQL", &opts).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_section_filter() {
        let dir = create_test_dir();
        write_test_doc(&dir, "inc-001.md", DOC2);

        let opts = SearchOptions {
            section_filter: Some("Root Cause".to_string()),
            ..Default::default()
        };
        let results = search_documents(&dir, "connection", &opts).unwrap();
        assert_eq!(results.len(), 1);
        for m in &results[0].matches {
            assert_eq!(m.section, "Root Cause");
        }
    }

    #[test]
    fn test_field_filter() {
        let dir = create_test_dir();
        write_test_doc(&dir, "adr-001.md", DOC1);

        let opts = SearchOptions {
            field_filter: Some("title".to_string()),
            ..Default::default()
        };
        let results = search_documents(&dir, "PostgreSQL", &opts).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].matches.len(), 1);
        assert_eq!(results[0].matches[0].section, "frontmatter");
    }

    #[test]
    fn test_max_results() {
        let dir = create_test_dir();
        write_test_doc(&dir, "adr-001.md", DOC1);
        write_test_doc(&dir, "inc-001.md", DOC2);

        let opts = SearchOptions {
            max_results: Some(1),
            ..Default::default()
        };
        let results = search_documents(&dir, "connection", &opts).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_no_matches() {
        let dir = create_test_dir();
        write_test_doc(&dir, "adr-001.md", DOC1);

        let opts = SearchOptions::default();
        let results = search_documents(&dir, "nonexistent_xyz_term", &opts).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_highlight_match() {
        let ctx = "We use connection pooling for performance";
        let result = highlight_match(ctx, "connection pooling", false);
        assert!(result.contains("*connection pooling*"));
    }

    #[test]
    fn test_frontmatter_search() {
        let dir = create_test_dir();
        write_test_doc(&dir, "adr-001.md", DOC1);

        let opts = SearchOptions::default();
        let results = search_documents(&dir, "accepted", &opts).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0]
            .matches
            .iter()
            .any(|m| m.section == "frontmatter"));
    }

    #[test]
    fn test_search_respects_gitignore() {
        let dir = create_test_dir();
        write_test_doc(&dir, "visible.md", DOC1);

        // Create a gitignored subdirectory
        let ignored_dir = dir.join("ignored_dir");
        fs::create_dir_all(&ignored_dir).unwrap();
        write_test_doc(&ignored_dir, "hidden.md", DOC2);

        // Initialize git repo + .gitignore
        fs::write(dir.join(".gitignore"), "ignored_dir/\n").unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&dir)
            .output()
            .unwrap();

        let opts = SearchOptions::default();
        let results = search_documents(&dir, "connection", &opts).unwrap();

        // Only the non-gitignored doc should appear
        assert_eq!(results.len(), 1, "gitignored files should be excluded");
        assert!(results[0].path.contains("visible.md"));
    }

    #[test]
    fn test_context_includes_section_name() {
        let dir = create_test_dir();
        write_test_doc(&dir, "inc-001.md", DOC2);

        let opts = SearchOptions::default();
        let results = search_documents(&dir, "finally blocks", &opts).unwrap();
        assert_eq!(results.len(), 1);
        let m = &results[0].matches[0];
        assert_eq!(m.section, "Resolution");
    }
}

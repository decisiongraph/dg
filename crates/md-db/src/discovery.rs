use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use walkdir::WalkDir;

use crate::error::Result;
use crate::frontmatter::Frontmatter;

/// Directories that are always skipped during discovery, regardless of .gitignore.
const IGNORED_DIRS: &[&str] = &[
    // DecisionGraph internals
    ".dg",
    // AI/LLM tool configs (contain .md that aren't decision docs)
    ".claude",
    ".gemini",
    ".agents",
    ".cursor",
    ".copilot",
    ".aider",
    // VCS internals
    ".git",
    ".hg",
    ".svn",
    // IDE/editor
    ".idea",
    ".vscode",
    ".zed",
    // Build/deps/cache (safety net if not in .gitignore)
    "node_modules",
    "__pycache__",
    ".next",
    ".nuxt",
    ".turbo",
    ".cache",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
];

/// Returns true if any path component matches an ignored directory name.
pub fn is_ignored_dir(path: &Path) -> bool {
    path.components()
        .any(|c| IGNORED_DIRS.contains(&c.as_os_str().to_str().unwrap_or("")))
}

/// A filter for frontmatter fields.
#[derive(Debug, Clone)]
pub enum Filter {
    /// Field must equal value.
    FieldEquals { key: String, value: String },
    /// Field must NOT equal value.
    FieldNotEquals { key: String, value: String },
    /// Field value must contain substring.
    FieldContains { key: String, value: String },
    /// Field value must be one of these values (comma-separated in CLI).
    FieldIn { key: String, values: Vec<String> },
    /// Field must exist.
    HasField(String),
    /// Field must NOT exist.
    NotHasField(String),
}

/// Discover markdown files in a directory with optional filtering.
pub fn discover_files(
    dir: impl AsRef<Path>,
    pattern: Option<&str>,
    filters: &[Filter],
    no_ignore: bool,
) -> Result<Vec<PathBuf>> {
    let dir = dir.as_ref();
    let glob_pattern = pattern.unwrap_or("*.md");

    let mut results = Vec::new();

    let walker = WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(!no_ignore)
        .git_global(!no_ignore)
        .git_exclude(!no_ignore)
        .follow_links(true)
        .build();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        if is_ignored_dir(path) {
            continue;
        }

        // Check glob pattern against filename
        if !matches_glob(path, glob_pattern) {
            continue;
        }

        // If there are filters, parse frontmatter and check
        if !filters.is_empty() {
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let fm = match Frontmatter::try_parse(&content) {
                Ok((Some(fm), _)) => fm,
                _ => continue,
            };

            if !check_filters(&fm, filters) {
                continue;
            }
        }

        results.push(path.to_path_buf());
    }

    results.sort();
    Ok(results)
}

fn matches_glob(path: &Path, pattern: &str) -> bool {
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };

    // Use glob::Pattern for matching
    match glob::Pattern::new(pattern) {
        Ok(pat) => pat.matches(file_name),
        Err(_) => false,
    }
}

fn check_filters(fm: &Frontmatter, filters: &[Filter]) -> bool {
    for filter in filters {
        match filter {
            Filter::FieldEquals { key, value } => match fm.get_display(key) {
                Some(v) if v == *value => {}
                _ => return false,
            },
            Filter::FieldNotEquals { key, value } => {
                match fm.get_display(key) {
                    Some(v) if v != *value => {}
                    None => {} // field absent counts as "not equal"
                    _ => return false,
                }
            }
            Filter::FieldContains { key, value } => match fm.get_display(key) {
                Some(v) if v.contains(value.as_str()) => {}
                _ => return false,
            },
            Filter::FieldIn { key, values } => match fm.get_display(key) {
                Some(v) if values.contains(&v) => {}
                _ => return false,
            },
            Filter::HasField(key) => {
                if !fm.has_field(key) {
                    return false;
                }
            }
            Filter::NotHasField(key) => {
                if fm.has_field(key) {
                    return false;
                }
            }
        }
    }
    true
}

/// Discover singleton files matching schema type patterns in a directory.
/// Returns files that match any singleton type's match pattern.
pub fn discover_singleton_files(
    dir: impl AsRef<Path>,
    singleton_patterns: &[&str],
) -> Result<Vec<PathBuf>> {
    if singleton_patterns.is_empty() {
        return Ok(Vec::new());
    }

    let dir = dir.as_ref();
    let mut results = Vec::new();

    for entry in WalkDir::new(dir).follow_links(true).into_iter().flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if is_ignored_dir(path) {
            continue;
        }

        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        if singleton_patterns.contains(&file_name) {
            results.push(path.to_path_buf());
        }
    }

    results.sort();
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_glob() {
        let path = Path::new("docs/adr-001.md");
        assert!(matches_glob(path, "*.md"));
        assert!(matches_glob(path, "adr-*.md"));
        assert!(!matches_glob(path, "*.txt"));
    }
}

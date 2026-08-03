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

/// Optional project-root file listing gitignore-syntax patterns to exclude from discovery.
const DGIGNORE_FILENAME: &str = ".dgignore";

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

    let mut builder = WalkBuilder::new(dir);
    builder
        .hidden(false)
        .git_ignore(!no_ignore)
        .git_global(!no_ignore)
        .git_exclude(!no_ignore)
        .follow_links(true);
    if !no_ignore {
        builder.add_custom_ignore_filename(DGIGNORE_FILENAME);
    }

    for entry in builder.build().filter_map(|e| e.ok()) {
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

    fn project(dgignore: Option<&str>, gitignore: Option<&str>) -> tempfile::TempDir {
        // The .git dir is required: without it WalkBuilder's git matchers stay inert
        // and .gitignore-precedence assertions would pass for the wrong reason.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("book")).unwrap();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("top.md"), "x").unwrap();
        std::fs::write(root.join("book/doc.md"), "x").unwrap();
        std::fs::write(root.join("docs/adr-001.md"), "x").unwrap();
        if let Some(c) = dgignore {
            std::fs::write(root.join(".dgignore"), c).unwrap();
        }
        if let Some(c) = gitignore {
            std::fs::write(root.join(".gitignore"), c).unwrap();
        }
        dir
    }

    fn discovered(dir: &Path, pattern: Option<&str>, no_ignore: bool) -> Vec<String> {
        discover_files(dir, pattern, &[], no_ignore)
            .unwrap()
            .iter()
            .map(|p| {
                p.strip_prefix(dir)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[test]
    fn test_dgignore_excludes_dir() {
        let dir = project(Some("book/\n"), None);
        let found = discovered(dir.path(), None, false);
        assert!(
            !found.iter().any(|p| p == "book/doc.md"),
            ".dgignore 'book/' must exclude book/doc.md, got {found:?}"
        );
        assert!(found.iter().any(|p| p == "top.md"), "got {found:?}");
    }

    #[test]
    fn test_no_dgignore_discovers_all() {
        let dir = project(None, None);
        let found = discovered(dir.path(), None, false);
        assert!(found.iter().any(|p| p == "book/doc.md"), "got {found:?}");
        assert!(found.iter().any(|p| p == "top.md"), "got {found:?}");
    }

    #[test]
    fn test_dgignore_bypassed_with_no_ignore() {
        let dir = project(Some("book/\n"), None);
        let found = discovered(dir.path(), None, true);
        assert!(
            found.iter().any(|p| p == "book/doc.md"),
            "no_ignore=true must bypass .dgignore, got {found:?}"
        );
    }

    #[test]
    fn test_dgignore_with_pattern_still_excludes() {
        let dir = project(Some("book/\n"), None);
        let found = discovered(dir.path(), Some("*.md"), false);
        assert!(
            !found.iter().any(|p| p == "book/doc.md"),
            "a --pattern glob must not bypass .dgignore, got {found:?}"
        );
        assert!(found.iter().any(|p| p == "top.md"), "got {found:?}");
    }

    #[test]
    fn test_dgignore_comments_and_blanks() {
        let dir = project(Some("# a comment\n\n   \nbook/\n"), None);
        let found = discovered(dir.path(), None, false);
        assert!(
            !found.iter().any(|p| p == "book/doc.md"),
            "comments/blank lines must not break exclusion, got {found:?}"
        );
        assert!(found.iter().any(|p| p == "top.md"), "got {found:?}");
    }

    #[test]
    fn test_dgignore_negation_outranks_gitignore() {
        let dir = project(Some("!docs/\n"), Some("docs/\n"));
        let found = discovered(dir.path(), None, false);
        assert!(
            found.iter().any(|p| p == "docs/adr-001.md"),
            ".dgignore '!docs/' must re-include a directory excluded by .gitignore, got {found:?}"
        );
    }

    #[test]
    fn test_dgignore_globstar_cannot_unprune_directory() {
        // Locks the documented limitation: a pruned directory is re-included only by
        // negating the directory itself ('!docs/'). '!docs/**' compiles to 'docs/**/*',
        // which never matches the `docs` dir, so the walker never descends into it.
        let dir = project(Some("!docs/**\n"), Some("docs/\n"));
        let found = discovered(dir.path(), None, false);
        assert!(
            !found.iter().any(|p| p == "docs/adr-001.md"),
            "'!docs/**' must not be mistaken for a working re-include, got {found:?}"
        );
    }

    #[test]
    fn test_dgignore_hides_real_decision_docs() {
        // Pins the precondition behind README's "list only paths that hold no decision
        // documents": an ignored decision doc is absent from the set DocGraph::next_id
        // maxes over, so ignoring such a folder yields colliding IDs.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("docs/architecture")).unwrap();
        std::fs::write(root.join("docs/architecture/adr-001-first.md"), "x").unwrap();
        std::fs::write(root.join(".dgignore"), "docs/architecture/\n").unwrap();

        assert!(
            discovered(root, None, false).is_empty(),
            "the ignored decision doc must be invisible to discovery"
        );
        assert!(
            discovered(root, None, true)
                .iter()
                .any(|p| p == "docs/architecture/adr-001-first.md"),
            "and visible again when ignore files are disabled"
        );
    }

    #[test]
    fn test_nested_dgignore_applies_to_its_own_subtree() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("top.md"), "x").unwrap();
        std::fs::write(root.join("sub/hidden.md"), "x").unwrap();
        std::fs::write(root.join("sub/.dgignore"), "hidden.md\n").unwrap();

        let found = discovered(root, None, false);
        assert!(
            !found.iter().any(|p| p == "sub/hidden.md"),
            "a nested .dgignore must apply to its own directory, got {found:?}"
        );
        assert!(found.iter().any(|p| p == "top.md"), "got {found:?}");
    }

    #[test]
    fn test_dgignore_leading_slash_anchors_to_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("sub/book")).unwrap();
        std::fs::create_dir_all(root.join("book")).unwrap();
        std::fs::write(root.join("book/a.md"), "x").unwrap();
        std::fs::write(root.join("sub/book/b.md"), "x").unwrap();
        std::fs::write(root.join(".dgignore"), "/book/\n").unwrap();

        let found = discovered(root, None, false);
        assert!(!found.iter().any(|p| p == "book/a.md"), "got {found:?}");
        assert!(
            found.iter().any(|p| p == "sub/book/b.md"),
            "'/book/' anchors to the project root and must not match sub/book/, got {found:?}"
        );
    }
}

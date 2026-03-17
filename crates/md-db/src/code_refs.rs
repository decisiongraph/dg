//! Code & commit reference scanner.
//!
//! Scans source code files and git commit messages for document ID mentions
//! (e.g. ADR-001, OPP-002). Builds an inverted index: doc_id → [{file, line, text}].
//! Cached to `.dg/cache/code-refs.json` with incremental updates.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::time::SystemTime;

use ignore::WalkBuilder;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::discovery::is_ignored_dir;
use crate::schema::Schema;

/// A reference to a document ID found in a source code file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeRef {
    /// Repo-relative file path.
    pub file: String,
    /// 1-based line number.
    pub line: usize,
    /// Trimmed line content (truncated at 200 chars).
    pub text: String,
    /// Up to 2 lines before the match (trimmed, oldest first).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_before: Vec<String>,
    /// Up to 2 lines after the match (trimmed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context_after: Vec<String>,
}

/// Internal cache entry for a single code reference (includes context).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedFileRef {
    doc_id: String,
    line: usize,
    text: String,
    #[serde(default)]
    context_before: Vec<String>,
    #[serde(default)]
    context_after: Vec<String>,
}

/// A reference to a document ID found in a git commit message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRef {
    /// Short hash (8 chars).
    pub sha: String,
    /// First line of commit message.
    pub subject: String,
    /// YYYY-MM-DD.
    pub date: String,
    /// Commit author name.
    pub author: String,
    /// Line from commit body containing the doc ID reference (if not in subject).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_context: Option<String>,
}

/// Per-document code references.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocCodeRefs {
    /// Source code references.
    pub code: Vec<CodeRef>,
    /// Git commit references.
    pub commits: Vec<CommitRef>,
}

/// Cached state for a single scanned file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileState {
    mtime_secs: u64,
    size: u64,
    content_hash: u64,
}

/// Persistent cache for code reference scanning.
#[derive(Debug, Serialize, Deserialize)]
pub struct CodeRefCache {
    /// Per-file scan state.
    #[serde(default)]
    file_states: HashMap<String, FileState>,
    /// Per-file discovered refs: path → [CachedFileRef].
    #[serde(default)]
    file_refs: HashMap<String, Vec<CachedFileRef>>,
    /// HEAD SHA at last git scan.
    #[serde(default)]
    last_commit_sha: Option<String>,
    /// Inverted index: doc_id → DocCodeRefs.
    #[serde(default)]
    pub index: BTreeMap<String, DocCodeRefs>,
    /// Whether cache has been modified since load.
    #[serde(skip)]
    dirty: bool,
}

impl CodeRefCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            file_states: HashMap::new(),
            file_refs: HashMap::new(),
            last_commit_sha: None,
            index: BTreeMap::new(),
            dirty: false,
        }
    }

    /// Load from JSON file. Returns empty cache if file missing/invalid.
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            return Self::new();
        }
        match std::fs::read_to_string(path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_else(|_| Self::new()),
            Err(_) => Self::new(),
        }
    }

    /// Save to JSON file.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string(self)?;
        std::fs::write(path, data)
    }

    /// Whether cache was modified since load.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Get code refs for a specific document ID.
    pub fn get(&self, doc_id: &str) -> Option<&DocCodeRefs> {
        self.index.get(doc_id)
    }
}

impl Default for CodeRefCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Directories to skip when scanning for code references (in addition to IGNORED_DIRS).
const CODE_SKIP_DIRS: &[&str] = &["docs", "doc", "site", "example-site"];

/// Build a regex matching document ID patterns from schema type prefixes + aliases.
fn build_doc_id_regex(schema: &Schema) -> Option<Regex> {
    let mut prefixes: Vec<String> = Vec::new();
    for t in &schema.types {
        if t.singleton {
            continue;
        }
        prefixes.push(t.name.to_uppercase());
        for a in &t.aliases {
            prefixes.push(a.to_uppercase());
        }
    }
    if prefixes.is_empty() {
        return None;
    }
    prefixes.sort();
    prefixes.dedup();
    let alts = prefixes.join("|");
    // Case-insensitive word-boundary match
    Regex::new(&format!(r"(?i)\b({alts})-(\d+)\b")).ok()
}

/// Check if a file is likely binary by reading first 512 bytes.
fn is_binary_file(path: &Path) -> bool {
    let Ok(f) = std::fs::File::open(path) else {
        return true;
    };
    let mut buf = [0u8; 512];
    use std::io::Read;
    let Ok(n) = (&f).read(&mut buf) else {
        return true;
    };
    buf[..n].contains(&0)
}

/// FNV-1a hash (same as cache.rs).
fn simple_hash(data: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in data.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn mtime_secs(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Scan source code files for document ID references.
///
/// Updates the cache incrementally: only re-scans files whose mtime+size changed,
/// then re-hashes to confirm content change.
pub fn scan_code_refs(root: &Path, schema: &Schema, cache: &mut CodeRefCache) {
    let re = match build_doc_id_regex(schema) {
        Some(r) => r,
        None => return,
    };

    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .follow_links(true)
        .build();

    let mut seen_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if is_ignored_dir(path) {
            continue;
        }

        // Skip docs and other non-code directories
        let rel = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let rel_str = rel.to_string_lossy();

        // Skip known non-code dirs
        let first_component = rel.components().next().and_then(|c| c.as_os_str().to_str());
        if let Some(dir) = first_component {
            if CODE_SKIP_DIRS.contains(&dir) {
                continue;
            }
        }

        // Skip markdown files (those are the docs themselves)
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            continue;
        }

        // Skip binary files
        if is_binary_file(path) {
            continue;
        }

        let key = rel_str.to_string();
        seen_paths.insert(key.clone());

        // Check if file changed since last scan
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let new_mtime = mtime_secs(&meta);
        let new_size = meta.len();

        if let Some(state) = cache.file_states.get(&key) {
            if state.mtime_secs == new_mtime && state.size == new_size {
                continue; // No change
            }
        }

        // File changed — read and re-hash
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let new_hash = simple_hash(&content);

        if let Some(state) = cache.file_states.get(&key) {
            if state.content_hash == new_hash {
                // Content unchanged despite mtime change — update mtime only
                cache.file_states.insert(
                    key.clone(),
                    FileState {
                        mtime_secs: new_mtime,
                        size: new_size,
                        content_hash: new_hash,
                    },
                );
                cache.dirty = true;
                continue;
            }
        }

        // Content changed — re-scan
        let lines: Vec<&str> = content.lines().collect();
        let mut file_matches: Vec<CachedFileRef> = Vec::new();
        for (line_num, line) in lines.iter().enumerate() {
            for m in re.find_iter(line) {
                let doc_id = m.as_str().to_uppercase();
                let text: String = line.trim().chars().take(200).collect();
                let context_before = (line_num.saturating_sub(2)..line_num)
                    .map(|i| lines[i].trim().chars().take(200).collect())
                    .collect();
                let context_after = (line_num + 1..usize::min(lines.len(), line_num + 3))
                    .map(|i| lines[i].trim().chars().take(200).collect())
                    .collect();
                file_matches.push(CachedFileRef {
                    doc_id,
                    line: line_num + 1,
                    text,
                    context_before,
                    context_after,
                });
            }
        }

        cache.file_states.insert(
            key.clone(),
            FileState {
                mtime_secs: new_mtime,
                size: new_size,
                content_hash: new_hash,
            },
        );
        cache.file_refs.insert(key, file_matches);
        cache.dirty = true;
    }

    // Prune deleted files
    let stale_keys: Vec<String> = cache
        .file_states
        .keys()
        .filter(|k| !seen_paths.contains(*k))
        .cloned()
        .collect();
    for key in stale_keys {
        cache.file_states.remove(&key);
        cache.file_refs.remove(&key);
        cache.dirty = true;
    }

    // Rebuild inverted index from file_refs (code side only — preserve commits)
    rebuild_code_index(cache);
}

/// Scan git commit messages for document ID references.
///
/// Incrementally scans from HEAD back to the last-scanned SHA.
#[cfg(feature = "git")]
pub fn scan_commit_refs(root: &Path, schema: &Schema, cache: &mut CodeRefCache) {
    let re = match build_doc_id_regex(schema) {
        Some(r) => r,
        None => return,
    };

    let repo = match git2::Repository::discover(root) {
        Ok(r) => r,
        Err(_) => return,
    };

    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return,
    };
    let head_oid = match head.target() {
        Some(o) => o,
        None => return,
    };
    let head_sha = format!("{}", head_oid);

    // If HEAD hasn't changed, nothing to do
    if cache.last_commit_sha.as_deref() == Some(&head_sha) {
        return;
    }

    // Validate cached stop SHA — if it can't be resolved (force-push/rebase),
    // clear all cached commits and do a full rescan.
    let stop_sha = if let Some(ref sha) = cache.last_commit_sha {
        if git2::Oid::from_str(sha)
            .ok()
            .and_then(|oid| repo.find_commit(oid).ok())
            .is_some()
        {
            Some(sha.clone())
        } else {
            // Stale stop SHA — purge all cached commits
            for entry in cache.index.values_mut() {
                entry.commits.clear();
            }
            cache
                .index
                .retain(|_, v| !v.code.is_empty() || !v.commits.is_empty());
            cache.last_commit_sha = None;
            cache.dirty = true;
            None
        }
    } else {
        None
    };

    let mut revwalk = match repo.revwalk() {
        Ok(r) => r,
        Err(_) => return,
    };
    revwalk.set_sorting(git2::Sort::TIME).ok();
    if revwalk.push(head_oid).is_err() {
        return;
    }

    // Collect all new commit refs
    let mut new_commit_refs: Vec<(String, CommitRef)> = Vec::new();
    let mut scanned = 0usize;
    const MAX_COMMITS: usize = 5000;

    for oid_result in revwalk {
        let oid = match oid_result {
            Ok(o) => o,
            Err(_) => break,
        };
        let sha_str = format!("{}", oid);

        // Stop if we've reached the last-scanned commit
        if let Some(ref stop) = stop_sha {
            if sha_str == *stop {
                break;
            }
        }

        scanned += 1;
        if scanned > MAX_COMMITS {
            break;
        }

        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let msg = match commit.message() {
            Some(m) => m,
            None => continue,
        };

        // Skip special commits
        if crate::commit::is_special_commit(msg) {
            continue;
        }

        let subject = msg.lines().next().unwrap_or("").to_string();
        let short_sha = &sha_str[..8.min(sha_str.len())];

        // Extract date
        let time = commit.time();
        let secs = time.seconds();
        let date = epoch_secs_to_date(secs);

        let author = commit.author().name().unwrap_or("unknown").to_string();

        // Find doc IDs in subject + body, capturing body context
        let body = msg.lines().skip(1).collect::<Vec<_>>().join("\n");
        let mut seen_ids = std::collections::HashSet::new();
        for m in re.find_iter(msg) {
            let doc_id = m.as_str().to_uppercase();
            if !seen_ids.insert(doc_id.clone()) {
                continue; // deduplicate within same commit
            }

            // If the match is in the body (not the subject), capture that line
            let body_context = if m.start() >= subject.len() {
                body.lines()
                    .find(|line| line.to_uppercase().contains(&doc_id))
                    .map(|line| {
                        let trimmed = line.trim().to_string();
                        if trimmed.len() > 200 {
                            format!("{}…", &trimmed[..200])
                        } else {
                            trimmed
                        }
                    })
            } else {
                None
            };

            new_commit_refs.push((
                doc_id,
                CommitRef {
                    sha: short_sha.to_string(),
                    subject: subject.clone(),
                    date: date.clone(),
                    author: author.clone(),
                    body_context,
                },
            ));
        }
    }

    if !new_commit_refs.is_empty() || cache.last_commit_sha.as_deref() != Some(&head_sha) {
        // Add new commit refs to index (deduplicate by sha)
        for (doc_id, commit_ref) in new_commit_refs {
            let entry = cache.index.entry(doc_id).or_default();
            if !entry.commits.iter().any(|c| c.sha == commit_ref.sha) {
                entry.commits.push(commit_ref);
            }
        }

        // Prune stale individual commit SHAs that no longer resolve
        let mut pruned = false;
        for entry in cache.index.values_mut() {
            let before = entry.commits.len();
            entry
                .commits
                .retain(|c| repo.revparse_single(&c.sha).is_ok());
            if entry.commits.len() < before {
                pruned = true;
            }
        }
        if pruned {
            cache
                .index
                .retain(|_, v| !v.code.is_empty() || !v.commits.is_empty());
        }

        // Sort commits by date descending
        for entry in cache.index.values_mut() {
            entry.commits.sort_by(|a, b| b.date.cmp(&a.date));
        }

        cache.last_commit_sha = Some(head_sha);
        cache.dirty = true;
    }
}

/// No-op stub when git feature is disabled.
#[cfg(not(feature = "git"))]
pub fn scan_commit_refs(_root: &Path, _schema: &Schema, _cache: &mut CodeRefCache) {}

/// Detect git remote "origin" URL and convert to a web-browsable base URL.
/// Returns `(web_base_url, default_branch)`.
#[cfg(feature = "git")]
pub fn detect_repo_web_url(root: &Path) -> Option<(String, String)> {
    let repo = git2::Repository::discover(root).ok()?;
    let remote = repo.find_remote("origin").ok()?;
    let url_str = remote.url()?;
    let web_url = git_remote_to_web_url(url_str)?;
    let branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "main".to_string());
    Some((web_url, branch))
}

/// No-op stub when git feature is disabled.
#[cfg(not(feature = "git"))]
pub fn detect_repo_web_url(_root: &Path) -> Option<(String, String)> {
    None
}

/// Convert a git remote URL (SSH or HTTPS) to a web-browsable URL.
#[cfg(feature = "git")]
fn git_remote_to_web_url(url: &str) -> Option<String> {
    // SSH: git@github.com:user/repo.git
    if let Some(rest) = url.strip_prefix("git@") {
        let rest = rest.replace(':', "/");
        let rest = rest.strip_suffix(".git").unwrap_or(&rest);
        return Some(format!("https://{rest}"));
    }
    // HTTPS: https://github.com/user/repo.git
    if url.starts_with("https://") || url.starts_with("http://") {
        let clean = url.strip_suffix(".git").unwrap_or(url);
        return Some(clean.to_string());
    }
    None
}

/// Rebuild the code portion of the inverted index from file_refs.
fn rebuild_code_index(cache: &mut CodeRefCache) {
    // Clear all code refs (keep commits)
    for entry in cache.index.values_mut() {
        entry.code.clear();
    }

    // Re-populate from file_refs
    for (file_path, refs) in &cache.file_refs {
        for r in refs {
            let entry = cache.index.entry(r.doc_id.clone()).or_default();
            entry.code.push(CodeRef {
                file: file_path.clone(),
                line: r.line,
                text: r.text.clone(),
                context_before: r.context_before.clone(),
                context_after: r.context_after.clone(),
            });
        }
    }

    // Sort code refs by file path + line
    for entry in cache.index.values_mut() {
        entry
            .code
            .sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    }

    // Remove entries with no refs at all
    cache
        .index
        .retain(|_, v| !v.code.is_empty() || !v.commits.is_empty());
}

/// Convert epoch seconds to YYYY-MM-DD using Hinnant's algorithm.
#[cfg(feature = "git")]
fn epoch_secs_to_date(secs: i64) -> String {
    let days = secs / 86400;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SCHEMA: &str = r#"
type "adr" {
    field "title" type="string" required=#true
    field "status" type="string" required=#true
    section "Context"
}
type "opp" {
    field "title" type="string" required=#true
    field "status" type="string" required=#true
}
type "inc" {
    field "title" type="string" required=#true
    field "status" type="string" required=#true
}
type "pol" {
    field "title" type="string" required=#true
    field "status" type="string" required=#true
}
    "#;

    fn test_schema() -> crate::schema::Schema {
        crate::schema::Schema::from_str(TEST_SCHEMA).unwrap()
    }

    #[test]
    fn test_build_doc_id_regex() {
        let schema = test_schema();
        let re = build_doc_id_regex(&schema).unwrap();
        assert!(re.is_match("ADR-001"));
        assert!(re.is_match("opp-042"));
        assert!(re.is_match("see INC-003 for details"));
        assert!(!re.is_match("random text"));
    }

    #[test]
    fn test_cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cache_path = dir.path().join("code-refs.json");

        let mut cache = CodeRefCache::new();
        cache.index.insert(
            "ADR-001".to_string(),
            DocCodeRefs {
                code: vec![CodeRef {
                    file: "src/main.rs".to_string(),
                    line: 42,
                    text: "// Implements ADR-001".to_string(),
                    context_before: vec![],
                    context_after: vec![],
                }],
                commits: vec![],
            },
        );
        cache.save(&cache_path).unwrap();

        let loaded = CodeRefCache::load(&cache_path);
        assert!(loaded.index.contains_key("ADR-001"));
        assert_eq!(loaded.index["ADR-001"].code.len(), 1);
        assert_eq!(loaded.index["ADR-001"].code[0].line, 42);
    }

    #[test]
    fn test_is_binary_file() {
        let dir = tempfile::tempdir().unwrap();

        // Text file
        let text_path = dir.path().join("test.rs");
        std::fs::write(&text_path, "fn main() { // ADR-001 }").unwrap();
        assert!(!is_binary_file(&text_path));

        // Binary file
        let bin_path = dir.path().join("test.bin");
        std::fs::write(&bin_path, b"\x00\x01\x02\x03").unwrap();
        assert!(is_binary_file(&bin_path));
    }

    #[test]
    fn test_simple_hash_consistency() {
        assert_eq!(simple_hash("hello"), simple_hash("hello"));
        assert_ne!(simple_hash("hello"), simple_hash("world"));
    }

    #[test]
    fn test_scan_code_refs_basic() {
        let dir = tempfile::tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(
            src_dir.join("main.rs"),
            "fn main() {\n    // Implements ADR-001\n    // See OPP-002\n}\n",
        )
        .unwrap();
        // Also create a .md file that should be skipped
        std::fs::write(dir.path().join("README.md"), "# ADR-001 reference").unwrap();

        let schema = test_schema();
        let mut cache = CodeRefCache::new();
        scan_code_refs(dir.path(), &schema, &mut cache);

        assert!(cache.index.contains_key("ADR-001"));
        assert!(cache.index.contains_key("OPP-002"));
        // Code refs should come from .rs file, not .md
        let adr_refs = &cache.index["ADR-001"];
        assert_eq!(adr_refs.code.len(), 1);
        assert!(adr_refs.code[0].file.ends_with("main.rs"));
        assert_eq!(adr_refs.code[0].line, 2);
    }

    #[test]
    fn test_incremental_scan() {
        let dir = tempfile::tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("lib.rs"), "// ADR-001\n").unwrap();

        let schema = test_schema();
        let mut cache = CodeRefCache::new();

        // First scan
        scan_code_refs(dir.path(), &schema, &mut cache);
        assert!(cache.is_dirty());
        assert!(cache.index.contains_key("ADR-001"));

        // Reset dirty flag
        cache.dirty = false;

        // Second scan (no changes) → not dirty
        scan_code_refs(dir.path(), &schema, &mut cache);
        assert!(!cache.is_dirty());

        // Modify file
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(src_dir.join("lib.rs"), "// ADR-001\n// OPP-002\n").unwrap();

        // Third scan → dirty, new ref found
        scan_code_refs(dir.path(), &schema, &mut cache);
        assert!(cache.is_dirty());
        assert!(cache.index.contains_key("OPP-002"));
    }
}

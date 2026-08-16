use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::frontmatter::Frontmatter;

/// Cached metadata for a single document file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Hash of file content (to detect changes even if mtime is unreliable).
    pub content_hash: u64,
    /// Parsed frontmatter fields (serialized as JSON-compatible map).
    pub frontmatter: Option<HashMap<String, serde_json::Value>>,
    /// File modification time (seconds since UNIX epoch for portability).
    pub mtime_secs: u64,
    /// File size in bytes.
    pub size: u64,
    /// Canonical document ID (e.g. "ADR-001"), populated by graph refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_id: Option<String>,
    /// Document type from frontmatter (e.g. "adr").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_type: Option<String>,
    /// Document title from frontmatter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Document status from frontmatter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Outgoing relation edges: (target_id, relation_name).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<(String, String)>,
}

/// In-memory document cache with file-based JSON persistence.
#[derive(Debug, Clone)]
pub struct DocCache {
    entries: HashMap<PathBuf, CacheEntry>,
    dirty: bool,
}

impl DocCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            dirty: false,
        }
    }

    /// Load cache from a JSON file. Returns empty cache if file doesn't exist or is invalid.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let data = std::fs::read_to_string(path)?;
        let entries: HashMap<PathBuf, CacheEntry> =
            serde_json::from_str(&data).map_err(Error::Json)?;
        Ok(Self {
            entries,
            dirty: false,
        })
    }

    /// Save cache to a JSON file.
    pub fn save(&self, path: &Path) -> Result<()> {
        let data = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Get a cached entry for a path.
    pub fn get(&self, path: &Path) -> Option<&CacheEntry> {
        self.entries.get(path)
    }

    /// Check if a cached entry is stale by comparing mtime and size against the filesystem.
    /// Returns `true` if the file has changed or the entry doesn't exist.
    pub fn is_stale(&self, path: &Path) -> bool {
        let entry = match self.entries.get(path) {
            Some(e) => e,
            None => return true,
        };

        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return true,
        };

        entry.mtime_secs != mtime_secs(&meta) || entry.size != meta.len()
    }

    /// Remove the cache entry for a path.
    pub fn invalidate(&mut self, path: &Path) {
        if self.entries.remove(path).is_some() {
            self.dirty = true;
        }
    }

    /// Re-read and re-parse a file if its cache entry is stale.
    /// Updates the cache entry in-place and marks cache as dirty.
    pub fn refresh(&mut self, path: &Path) -> Result<()> {
        if !self.is_stale(path) {
            return Ok(());
        }

        let content = std::fs::read_to_string(path)?;
        let meta = std::fs::metadata(path)?;
        let content_hash = simple_hash(&content);

        let frontmatter = match Frontmatter::try_parse(&content) {
            Ok((Some(fm), _)) => {
                let json = fm.to_json();
                match json {
                    serde_json::Value::Object(map) => Some(map.into_iter().collect()),
                    _ => None,
                }
            }
            _ => None,
        };

        let entry = CacheEntry {
            content_hash,
            frontmatter,
            mtime_secs: mtime_secs(&meta),
            size: meta.len(),
            doc_id: None,
            doc_type: None,
            title: None,
            status: None,
            edges: Vec::new(),
        };

        self.entries.insert(path.to_path_buf(), entry);
        self.dirty = true;
        Ok(())
    }

    /// Re-read, re-parse a file and populate graph-related fields (doc_id, edges, etc).
    /// Used by `DocGraph::build_cached()` for stale entries.
    pub fn refresh_for_graph(&mut self, path: &Path, schema: &crate::schema::Schema) -> Result<()> {
        let content = std::fs::read_to_string(path)?;
        let meta = std::fs::metadata(path)?;
        let content_hash = simple_hash(&content);

        let parsed = Frontmatter::try_parse(&content);
        let (fm_opt, _body) = match parsed {
            Ok((fm, body)) => (fm, body),
            Err(_) => (None, content.clone()),
        };

        let frontmatter = fm_opt.as_ref().and_then(|fm| {
            let json = fm.to_json();
            match json {
                serde_json::Value::Object(map) => Some(map.into_iter().collect()),
                _ => None,
            }
        });

        let doc_id = crate::graph::path_to_id_with_schema(path, schema);

        // Extract title from H1 heading, falling back to frontmatter
        let body_title = crate::ast_util::first_heading_text(&_body);

        // Extract graph metadata: type from ID prefix, title from H1, status from frontmatter
        let (doc_type, title, status) = if let Some(ref fm) = fm_opt {
            (
                schema
                    .type_name_for_doc_id(&doc_id)
                    .or_else(|| fm.get_display("type")),
                body_title.or_else(|| fm.get_display("title")),
                fm.get_display("status"),
            )
        } else {
            // Check singleton types
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let singleton_type = schema
                .types
                .iter()
                .find(|t| t.singleton && t.match_pattern.as_deref() == Some(filename));
            (singleton_type.map(|t| t.name.clone()), body_title, None)
        };

        // Extract edges from relation fields
        let mut edges = Vec::new();
        if let Some(ref fm) = fm_opt {
            let relation_names = schema.all_relation_field_names();
            for rel_name in &relation_names {
                if let Some(val) = fm.get(rel_name) {
                    let refs = crate::graph::extract_refs(val);
                    for target in refs {
                        edges.push((
                            crate::graph::ref_value_to_id(&target, path, schema),
                            rel_name.to_string(),
                        ));
                    }
                }
            }
        }

        let entry = CacheEntry {
            content_hash,
            frontmatter,
            mtime_secs: mtime_secs(&meta),
            size: meta.len(),
            doc_id: Some(doc_id),
            doc_type,
            title,
            status,
            edges,
        };

        self.entries.insert(path.to_path_buf(), entry);
        self.dirty = true;
        Ok(())
    }

    /// Returns true if the cache has been modified since load.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all cached paths and entries.
    pub fn iter(&self) -> impl Iterator<Item = (&PathBuf, &CacheEntry)> {
        self.entries.iter()
    }

    /// Remove entries for files that no longer exist on disk.
    /// Returns the number of entries removed.
    pub fn prune_missing(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|path, _| path.exists());
        let removed = before - self.entries.len();
        if removed > 0 {
            self.dirty = true;
        }
        removed
    }
}

impl Default for DocCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract mtime as seconds since UNIX epoch from file metadata.
fn mtime_secs(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Simple non-cryptographic hash for content change detection.
/// Uses FNV-1a for speed.
fn simple_hash(data: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in data.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_temp_md(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_new_cache_is_empty() {
        let cache = DocCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert!(!cache.is_dirty());
    }

    #[test]
    fn test_refresh_populates_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = create_temp_md(
            dir.path(),
            "test.md",
            "---\ntitle: Hello\nstatus: draft\n---\n\n# Body\n",
        );

        let mut cache = DocCache::new();
        assert!(cache.is_stale(&path));

        cache.refresh(&path).unwrap();
        assert!(!cache.is_stale(&path));
        assert!(cache.is_dirty());

        let entry = cache.get(&path).unwrap();
        assert!(entry.content_hash != 0);
        assert!(entry.size > 0);

        let fm = entry.frontmatter.as_ref().unwrap();
        assert_eq!(fm.get("title").unwrap(), "Hello");
        assert_eq!(fm.get("status").unwrap(), "draft");
    }

    #[test]
    fn test_invalidate_removes_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = create_temp_md(dir.path(), "test.md", "---\ntitle: X\n---\nbody\n");

        let mut cache = DocCache::new();
        cache.refresh(&path).unwrap();
        assert!(cache.get(&path).is_some());

        cache.invalidate(&path);
        assert!(cache.get(&path).is_none());
        assert!(cache.is_stale(&path));
    }

    #[test]
    fn test_stale_after_file_change() {
        let dir = tempfile::tempdir().unwrap();
        let path = create_temp_md(dir.path(), "test.md", "---\ntitle: V1\n---\nbody\n");

        let mut cache = DocCache::new();
        cache.refresh(&path).unwrap();
        assert!(!cache.is_stale(&path));

        // Modify the file (change content and size)
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&path, "---\ntitle: V2 changed\n---\nnew body content\n").unwrap();

        assert!(cache.is_stale(&path));
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let md_path = create_temp_md(dir.path(), "doc.md", "---\ntitle: Cached\n---\nbody\n");
        let cache_path = dir.path().join(".md-db-cache.json");

        let mut cache = DocCache::new();
        cache.refresh(&md_path).unwrap();
        cache.save(&cache_path).unwrap();

        let loaded = DocCache::load(&cache_path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(!loaded.is_dirty());

        let entry = loaded.get(&md_path).unwrap();
        let fm = entry.frontmatter.as_ref().unwrap();
        assert_eq!(fm.get("title").unwrap(), "Cached");
    }

    #[test]
    fn test_load_nonexistent_returns_empty() {
        let cache = DocCache::load(Path::new("/tmp/nonexistent-cache-file.json")).unwrap();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_prune_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = create_temp_md(dir.path(), "ephemeral.md", "---\ntitle: X\n---\nbody\n");

        let mut cache = DocCache::new();
        cache.refresh(&path).unwrap();
        assert_eq!(cache.len(), 1);

        std::fs::remove_file(&path).unwrap();
        let removed = cache.prune_missing();
        assert_eq!(removed, 1);
        assert!(cache.is_empty());
        assert!(cache.is_dirty());
    }

    #[test]
    fn test_no_frontmatter_cached_as_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = create_temp_md(
            dir.path(),
            "plain.md",
            "# Just a heading\n\nNo frontmatter.\n",
        );

        let mut cache = DocCache::new();
        cache.refresh(&path).unwrap();

        let entry = cache.get(&path).unwrap();
        assert!(entry.frontmatter.is_none());
    }

    #[test]
    fn test_content_hash_changes() {
        let hash1 = simple_hash("hello world");
        let hash2 = simple_hash("hello world!");
        let hash3 = simple_hash("hello world");
        assert_ne!(hash1, hash2);
        assert_eq!(hash1, hash3);
    }

    #[test]
    fn test_default_trait() {
        let cache = DocCache::default();
        assert!(cache.is_empty());
    }
}

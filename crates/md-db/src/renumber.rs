//! Chronological ID reordering for decision documents.
//!
//! Reassigns document IDs so that the earliest document (by `date` field)
//! gets 001, next gets 002, etc. Follows the compute plan → apply plan pattern
//! from `sync.rs`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::discovery;
use crate::document::Document;
use crate::error::Result;
use crate::graph::{extract_id_from_stem, path_to_id_with_schema};
use crate::schema::Schema;

/// A file rename action.
#[derive(Debug, Clone)]
pub struct RenameAction {
    /// Original file path.
    pub old_path: PathBuf,
    /// Destination file path after renumber.
    pub new_path: PathBuf,
    /// Original document ID (e.g. "ADR-005").
    pub old_id: String,
    /// New document ID (e.g. "ADR-002").
    pub new_id: String,
}

/// A single ID match found in a file.
#[derive(Debug, Clone)]
pub struct IdMatch {
    /// 1-based line number.
    pub line_number: usize,
    /// Full line content.
    pub line_content: String,
    /// The old ID that was matched.
    pub old_id: String,
    /// The new ID to replace with.
    pub new_id: String,
}

/// A file that contains ID references needing update.
#[derive(Debug, Clone)]
pub struct FileUpdate {
    /// Path to the file.
    pub path: PathBuf,
    /// Individual matches within the file.
    pub matches: Vec<IdMatch>,
}

/// Complete renumber plan.
#[derive(Debug, Clone)]
pub struct RenumberPlan {
    /// File renames (markdown docs whose ID changes).
    pub renames: Vec<RenameAction>,
    /// Files containing ID references that need updating.
    pub file_updates: Vec<FileUpdate>,
    /// Mapping from old ID to new ID (uppercase).
    pub id_mapping: BTreeMap<String, String>,
}

impl RenumberPlan {
    /// True when no changes are needed.
    pub fn is_empty(&self) -> bool {
        self.renames.is_empty()
    }

    /// Human-readable summary for dry-run output.
    pub fn to_report(&self, root: &Path) -> String {
        let mut out = String::new();

        if self.renames.is_empty() {
            out.push_str("Already in chronological order. Nothing to renumber.\n");
            return out;
        }

        // ID mapping
        out.push_str("Renumber mapping:\n");
        for (old, new) in &self.id_mapping {
            out.push_str(&format!("  {old} -> {new}\n"));
        }

        // File renames
        out.push_str("\nFile renames:\n");
        for r in &self.renames {
            let old = r.old_path.strip_prefix(root).unwrap_or(&r.old_path);
            let new = r.new_path.strip_prefix(root).unwrap_or(&r.new_path);
            out.push_str(&format!("  {} -> {}\n", old.display(), new.display()));
        }

        // Reference updates
        if !self.file_updates.is_empty() {
            let total_matches: usize = self.file_updates.iter().map(|f| f.matches.len()).sum();
            out.push_str(&format!(
                "\nReference updates ({total_matches} match(es) across {} file(s)):\n",
                self.file_updates.len()
            ));
            for fu in &self.file_updates {
                let rel = fu.path.strip_prefix(root).unwrap_or(&fu.path);
                out.push_str(&format!("  {}:\n", rel.display()));
                for m in &fu.matches {
                    let preview = m.line_content.trim();
                    out.push_str(&format!(
                        "    line {}: {} -> {}: {preview}\n",
                        m.line_number, m.old_id, m.new_id,
                    ));
                }
            }
        }

        let total_refs: usize = self.file_updates.iter().map(|f| f.matches.len()).sum();
        out.push_str(&format!(
            "\n{} file(s) to rename, {} reference(s) to update.\n",
            self.renames.len(),
            total_refs,
        ));

        out
    }
}

struct DocInfo {
    path: PathBuf,
    id: String,
    prefix: String,
    number: u32,
    date: Option<String>,
    slug: String,
}

/// Compute a renumber plan without modifying any files.
pub fn compute_renumber_plan(
    dir: &Path,
    schema: &Schema,
    filter_type: Option<&str>,
) -> Result<RenumberPlan> {
    let files = discovery::discover_files(dir, None, &[], false)?;

    // Parse each file, extract ID, date, type prefix, slug
    let mut docs: Vec<DocInfo> = Vec::new();
    for path in &files {
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let upper_stem = stem.to_uppercase().replace('_', "-");

        let id_part = match extract_id_from_stem(&upper_stem) {
            Some(id) => id,
            None => continue, // skip non-ID files (README.md etc)
        };

        let parts: Vec<&str> = id_part.splitn(2, '-').collect();
        if parts.len() != 2 {
            continue;
        }
        let prefix = parts[0].to_string();
        let number: u32 = match parts[1].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Extract slug (everything after the ID in the lowercase normalized stem)
        let normalized = stem.to_lowercase().replace('_', "-");
        let id_lower = id_part.to_lowercase();
        let slug = if normalized.starts_with(&id_lower) {
            normalized[id_lower.len()..].to_string()
        } else {
            String::new()
        };

        // Load frontmatter to get date
        let date = Document::from_file(path)
            .ok()
            .and_then(|doc| doc.frontmatter.as_ref()?.get("date").cloned())
            .and_then(|v| match v {
                serde_yaml::Value::String(s) => Some(s),
                _ => None,
            });

        let id = path_to_id_with_schema(path, schema);
        docs.push(DocInfo {
            path: path.clone(),
            id,
            prefix,
            number,
            date,
            slug,
        });
    }

    // Group by type prefix
    let mut groups: BTreeMap<String, Vec<DocInfo>> = BTreeMap::new();
    for doc in docs {
        groups.entry(doc.prefix.clone()).or_default().push(doc);
    }

    // Filter by --type if specified
    if let Some(filter) = filter_type {
        let filter_upper = filter.to_uppercase();
        groups.retain(|k, _| k == &filter_upper);
    }

    // Sort each group and assign new sequential IDs
    let mut id_mapping: BTreeMap<String, String> = BTreeMap::new();
    let mut renames: Vec<RenameAction> = Vec::new();

    for (_prefix, mut group) in groups {
        // Sort by date ascending; no-date docs last; ties by original number
        group.sort_by(|a, b| match (&a.date, &b.date) {
            (Some(da), Some(db)) => da.cmp(db).then(a.number.cmp(&b.number)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.number.cmp(&b.number),
        });

        for (i, doc) in group.iter().enumerate() {
            let new_num = i as u32 + 1;
            let new_id = format!("{}-{:03}", doc.prefix, new_num);

            if doc.id != new_id {
                id_mapping.insert(doc.id.clone(), new_id.clone());

                let new_filename = format!("{}{}.md", new_id.to_lowercase(), doc.slug);
                let new_path = doc
                    .path
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join(new_filename);

                renames.push(RenameAction {
                    old_path: doc.path.clone(),
                    new_path,
                    old_id: doc.id.clone(),
                    new_id,
                });
            }
        }
    }

    if renames.is_empty() {
        return Ok(RenumberPlan {
            renames: Vec::new(),
            file_updates: Vec::new(),
            id_mapping: BTreeMap::new(),
        });
    }

    // Scan ALL text files for ID references
    let file_updates = scan_file_updates(dir, &id_mapping)?;

    Ok(RenumberPlan {
        renames,
        file_updates,
        id_mapping,
    })
}

/// Apply a renumber plan: rename files and update all references.
pub fn apply_renumber_plan(plan: &RenumberPlan) -> Result<()> {
    // Phase 1: Two-phase rename to avoid collisions
    // old -> temp
    let mut temp_map: Vec<(PathBuf, PathBuf)> = Vec::new();
    for action in &plan.renames {
        let temp = action.old_path.with_extension("md.dg-renumber-tmp");
        std::fs::rename(&action.old_path, &temp)?;
        temp_map.push((temp, action.new_path.clone()));
    }
    // temp -> final
    for (temp, final_path) in &temp_map {
        std::fs::rename(temp, final_path)?;
    }

    // Build path mapping: old_path -> new_path for renamed files
    let path_map: BTreeMap<PathBuf, PathBuf> = plan
        .renames
        .iter()
        .map(|r| (r.old_path.clone(), r.new_path.clone()))
        .collect();

    // Phase 2: Update references in all affected files
    let re = build_id_regex(&plan.id_mapping)?;

    for fu in &plan.file_updates {
        // Resolve actual path (file might have been renamed)
        let actual_path = path_map.get(&fu.path).unwrap_or(&fu.path);

        let content = std::fs::read_to_string(actual_path)?;
        let updated = replace_ids(&re, &content, &plan.id_mapping);

        if updated != content {
            std::fs::write(actual_path, &updated)?;
        }
    }

    Ok(())
}

/// Scan all text files under `dir` for ID references matching the mapping.
fn scan_file_updates(dir: &Path, id_mapping: &BTreeMap<String, String>) -> Result<Vec<FileUpdate>> {
    if id_mapping.is_empty() {
        return Ok(Vec::new());
    }

    let re = build_id_regex(id_mapping)?;
    let mut updates: Vec<FileUpdate> = Vec::new();

    for entry in ignore::WalkBuilder::new(dir).build().flatten() {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        let path = entry.path();

        if crate::discovery::is_ignored_dir(path) {
            continue;
        }

        // Read file, skip binary/unreadable
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut matches = Vec::new();
        for (line_idx, line) in content.lines().enumerate() {
            for m in re.find_iter(line) {
                let matched_upper = m.as_str().to_uppercase();
                if let Some(new_id) = id_mapping.get(&matched_upper) {
                    matches.push(IdMatch {
                        line_number: line_idx + 1,
                        line_content: line.to_string(),
                        old_id: matched_upper,
                        new_id: new_id.clone(),
                    });
                }
            }
        }

        if !matches.is_empty() {
            updates.push(FileUpdate {
                path: path.to_path_buf(),
                matches,
            });
        }
    }

    // Sort by path for deterministic output
    updates.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(updates)
}

/// Build a case-insensitive word-boundary regex matching any of the old IDs.
fn build_id_regex(id_mapping: &BTreeMap<String, String>) -> Result<Regex> {
    let pattern_parts: Vec<String> = id_mapping.keys().map(|id| regex::escape(id)).collect();
    let pattern = format!(r"(?i)\b({})\b", pattern_parts.join("|"));
    Regex::new(&pattern).map_err(|e| crate::error::Error::SchemaParse(format!("regex: {e}")))
}

/// Replace all old IDs in text with new IDs, preserving case.
fn replace_ids(re: &Regex, text: &str, id_mapping: &BTreeMap<String, String>) -> String {
    re.replace_all(text, |caps: &regex::Captures| {
        let matched = &caps[0];
        let upper = matched.to_uppercase();
        let new_id = match id_mapping.get(&upper) {
            Some(id) => id,
            None => return matched.to_string(),
        };

        // Preserve case: if matched is all lowercase, emit lowercase new ID
        if matched
            .chars()
            .all(|c| c.is_ascii_lowercase() || !c.is_alphabetic())
        {
            new_id.to_lowercase()
        } else {
            new_id.clone()
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn minimal_schema() -> Schema {
        Schema::from_str(
            r#"
relation "supersedes" inverse="superseded_by" cardinality="one"
relation "enables" inverse="enabled_by" cardinality="many"
type "adr" {
    field "title" type="string" required=#true
    field "status" type="enum" required=#true {
        values "proposed" "accepted"
    }
    field "date" type="string"
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_already_in_order() {
        let dir = std::env::temp_dir().join("dg_renumber_in_order");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(
            dir.join("adr-001.md"),
            "---\ntitle: First\nstatus: accepted\ndate: \"2024-01-01\"\n---\n# Decision\nA.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();
        fs::write(
            dir.join("adr-002.md"),
            "---\ntitle: Second\nstatus: accepted\ndate: \"2024-02-01\"\n---\n# Decision\nB.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();

        let schema = minimal_schema();
        let plan = compute_renumber_plan(&dir, &schema, None).unwrap();
        assert!(plan.is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_reverse_order() {
        let dir = std::env::temp_dir().join("dg_renumber_reverse");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // ADR-001 has later date, ADR-002 has earlier date
        fs::write(
            dir.join("adr-001.md"),
            "---\ntitle: Late\nstatus: accepted\ndate: \"2024-06-01\"\n---\n# Decision\nA.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();
        fs::write(
            dir.join("adr-002.md"),
            "---\ntitle: Early\nstatus: accepted\ndate: \"2024-01-01\"\n---\n# Decision\nB.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();

        let schema = minimal_schema();
        let plan = compute_renumber_plan(&dir, &schema, None).unwrap();

        assert_eq!(plan.renames.len(), 2);
        assert_eq!(plan.id_mapping.get("ADR-001").unwrap(), "ADR-002");
        assert_eq!(plan.id_mapping.get("ADR-002").unwrap(), "ADR-001");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_apply_renames_and_updates_refs() {
        let dir = std::env::temp_dir().join("dg_renumber_apply");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // ADR-001 (late) references ADR-002
        fs::write(
            dir.join("adr-001.md"),
            "---\ntitle: Late\nstatus: accepted\ndate: \"2024-06-01\"\nenables: ADR-002\n---\n# Decision\nSee [ADR-002](./adr-002.md).\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();
        // ADR-002 (early) has no refs
        fs::write(
            dir.join("adr-002.md"),
            "---\ntitle: Early\nstatus: accepted\ndate: \"2024-01-01\"\n---\n# Decision\nB.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();

        let schema = minimal_schema();
        let plan = compute_renumber_plan(&dir, &schema, None).unwrap();
        assert!(!plan.is_empty());

        apply_renumber_plan(&plan).unwrap();

        // adr-001.md should now be the early doc (was adr-002)
        assert!(dir.join("adr-001.md").exists());
        assert!(dir.join("adr-002.md").exists());

        let doc1 = Document::from_file(dir.join("adr-001.md")).unwrap();
        assert_eq!(
            doc1.frontmatter.as_ref().unwrap().get("title").unwrap(),
            &serde_yaml::Value::String("Early".into())
        );

        // The late doc (now adr-002) should have updated refs
        let doc2 = Document::from_file(dir.join("adr-002.md")).unwrap();
        let body = &doc2.body;
        assert!(
            body.contains("ADR-001") || body.contains("adr-001"),
            "body should reference updated ID: {body}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_slugged_filenames_preserved() {
        let dir = std::env::temp_dir().join("dg_renumber_slug");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(
            dir.join("adr-001-use-postgresql.md"),
            "---\ntitle: PostgreSQL\nstatus: accepted\ndate: \"2024-06-01\"\n---\n# Decision\nA.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();
        fs::write(
            dir.join("adr-002-use-redis.md"),
            "---\ntitle: Redis\nstatus: accepted\ndate: \"2024-01-01\"\n---\n# Decision\nB.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();

        let schema = minimal_schema();
        let plan = compute_renumber_plan(&dir, &schema, None).unwrap();

        // adr-002-use-redis (early) -> adr-001-use-redis
        // adr-001-use-postgresql (late) -> adr-002-use-postgresql
        let redis_rename = plan.renames.iter().find(|r| r.old_id == "ADR-002").unwrap();
        assert!(
            redis_rename
                .new_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains("use-redis"),
            "slug should be preserved: {:?}",
            redis_rename.new_path
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_no_date_docs_go_last() {
        let dir = std::env::temp_dir().join("dg_renumber_nodate");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // ADR-001 has no date, ADR-002 has a date
        fs::write(
            dir.join("adr-001.md"),
            "---\ntitle: No Date\nstatus: proposed\n---\n# Decision\nA.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();
        fs::write(
            dir.join("adr-002.md"),
            "---\ntitle: Has Date\nstatus: accepted\ndate: \"2024-01-01\"\n---\n# Decision\nB.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();

        let schema = minimal_schema();
        let plan = compute_renumber_plan(&dir, &schema, None).unwrap();

        // ADR-002 (has date) should become ADR-001, ADR-001 (no date) should become ADR-002
        assert_eq!(plan.id_mapping.get("ADR-002").unwrap(), "ADR-001");
        assert_eq!(plan.id_mapping.get("ADR-001").unwrap(), "ADR-002");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_type_filter() {
        let dir = std::env::temp_dir().join("dg_renumber_filter");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(
            dir.join("adr-001.md"),
            "---\ntitle: A\nstatus: accepted\ndate: \"2024-06-01\"\n---\n# Decision\nA.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();
        fs::write(
            dir.join("adr-002.md"),
            "---\ntitle: B\nstatus: accepted\ndate: \"2024-01-01\"\n---\n# Decision\nB.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();

        let schema = minimal_schema();

        // Filter for OPP (no OPP files) -> no renames
        let plan = compute_renumber_plan(&dir, &schema, Some("opp")).unwrap();
        assert!(plan.is_empty());

        // Filter for ADR -> should find renames
        let plan = compute_renumber_plan(&dir, &schema, Some("adr")).unwrap();
        assert!(!plan.is_empty());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_code_ref_detection() {
        let dir = std::env::temp_dir().join("dg_renumber_coderef");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Markdown docs
        fs::write(
            dir.join("adr-001.md"),
            "---\ntitle: Late\nstatus: accepted\ndate: \"2024-06-01\"\n---\n# Decision\nA.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();
        fs::write(
            dir.join("adr-002.md"),
            "---\ntitle: Early\nstatus: accepted\ndate: \"2024-01-01\"\n---\n# Decision\nB.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();

        // Source file with code comment referencing ADR-001
        fs::write(
            dir.join("main.rs"),
            "fn main() {\n    // ADR-001: Use PostgreSQL\n    println!(\"hello\");\n}\n",
        )
        .unwrap();

        let schema = minimal_schema();
        let plan = compute_renumber_plan(&dir, &schema, None).unwrap();

        // Should find the code ref in main.rs
        let code_update = plan
            .file_updates
            .iter()
            .find(|f| f.path.file_name().unwrap().to_str().unwrap() == "main.rs");
        assert!(
            code_update.is_some(),
            "should detect code ref in main.rs, updates: {:?}",
            plan.file_updates
                .iter()
                .map(|f| f.path.display().to_string())
                .collect::<Vec<_>>()
        );

        // Apply and verify
        apply_renumber_plan(&plan).unwrap();

        let code = fs::read_to_string(dir.join("main.rs")).unwrap();
        assert!(
            code.contains("ADR-002"),
            "code comment should be updated: {code}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_cross_type_ref_updates() {
        let dir = std::env::temp_dir().join("dg_renumber_crosstype");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(
            dir.join("adr-001.md"),
            "---\ntitle: Late ADR\nstatus: accepted\ndate: \"2024-06-01\"\n---\n# Decision\nA.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();
        fs::write(
            dir.join("adr-002.md"),
            "---\ntitle: Early ADR\nstatus: accepted\ndate: \"2024-01-01\"\n---\n# Decision\nB.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();
        // OPP references ADR-001 in frontmatter
        fs::write(
            dir.join("opp-001.md"),
            "---\ntitle: My Opp\nstatus: proposed\nenabled_by: ADR-001\n---\n# Decision\nC.\n# Consequences\n## Positive\nOk.\n",
        ).unwrap();

        let schema = minimal_schema();
        let plan = compute_renumber_plan(&dir, &schema, Some("adr")).unwrap();

        // Only ADR files should be renamed
        assert!(plan.renames.iter().all(|r| r.old_id.starts_with("ADR-")));

        // OPP file should be in file_updates (has ref to ADR-001)
        let opp_update = plan
            .file_updates
            .iter()
            .find(|f| f.path.file_name().unwrap().to_str().unwrap() == "opp-001.md");
        assert!(opp_update.is_some(), "OPP file should have ref updates");

        // Apply and verify OPP refs updated
        apply_renumber_plan(&plan).unwrap();

        let opp = fs::read_to_string(dir.join("opp-001.md")).unwrap();
        assert!(
            opp.contains("ADR-002"),
            "OPP should reference new ADR ID: {opp}"
        );

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_replace_ids_preserves_case() {
        let mut mapping = BTreeMap::new();
        mapping.insert("ADR-001".to_string(), "ADR-002".to_string());

        let re = build_id_regex(&mapping).unwrap();

        assert_eq!(replace_ids(&re, "ADR-001", &mapping), "ADR-002");
        assert_eq!(replace_ids(&re, "adr-001", &mapping), "adr-002");
        assert_eq!(
            replace_ids(&re, "see ADR-001 and adr-001", &mapping),
            "see ADR-002 and adr-002"
        );
    }
}

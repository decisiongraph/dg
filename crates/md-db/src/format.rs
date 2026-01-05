use std::path::{Path, PathBuf};

use comrak::Arena;

use crate::ast_util;
use crate::discovery;
use crate::document::Document;
use crate::error::Result;
use crate::schema::{Schema, SectionDef, TypeDef};
use crate::table::Table;

/// A single formatting change applied to a file.
#[derive(Debug, Clone)]
pub struct FormatChange {
    pub path: PathBuf,
    pub description: String,
}

/// Result of formatting a directory.
#[derive(Debug)]
pub struct FormatResult {
    pub changes: Vec<FormatChange>,
    pub errors: Vec<(PathBuf, String)>,
}

/// Format all documents in a directory to match schema definitions.
pub fn format_directory(
    dir: &Path,
    schema: &Schema,
    pattern: Option<&str>,
    dry_run: bool,
) -> Result<FormatResult> {
    let files = discovery::discover_files(dir, pattern, &[], false)?;

    // Also discover singleton files
    let singleton_patterns: Vec<&str> = schema
        .types
        .iter()
        .filter(|t| t.singleton)
        .filter_map(|t| t.match_pattern.as_deref())
        .collect();
    let singleton_files = discovery::discover_singleton_files(dir, &singleton_patterns)?;

    let mut all_files: Vec<PathBuf> = files;
    for f in singleton_files {
        if !all_files.contains(&f) {
            all_files.push(f);
        }
    }
    all_files.sort();

    let mut result = FormatResult {
        changes: Vec::new(),
        errors: Vec::new(),
    };

    for path in &all_files {
        match format_file(path, schema, dry_run) {
            Ok(changes) => result.changes.extend(changes),
            Err(e) => result.errors.push((path.clone(), e.to_string())),
        }
    }

    Ok(result)
}

/// Format a single file to match its schema type definition.
/// Returns the list of changes made (or that would be made in dry-run mode).
pub fn format_file(path: &Path, schema: &Schema, dry_run: bool) -> Result<Vec<FormatChange>> {
    let mut doc = Document::from_file(path)?;
    let mut changes = Vec::new();

    // Determine document type: explicit field, then singleton match, then folder inference
    let type_name = doc
        .frontmatter
        .as_ref()
        .and_then(|fm| fm.get_display("type"))
        .or_else(|| {
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            schema
                .types
                .iter()
                .find(|t| t.singleton && t.match_pattern.as_deref() == Some(file_name))
                .map(|t| t.name.clone())
        })
        .or_else(|| infer_type_from_folder(path, schema));

    let type_name = match type_name {
        Some(name) => name,
        None => return Ok(changes),
    };

    let type_def = match schema.get_type(&type_name) {
        Some(td) => td,
        None => return Ok(changes),
    };

    // Safety check: bail if frontmatter has raw content but parsed as empty
    if let Some(ref fm) = doc.frontmatter {
        if fm.data().is_empty() {
            let has_raw_content = doc
                .raw
                .strip_prefix("---\n")
                .and_then(|s| s.split_once("\n---"))
                .map(|(matter, _)| !matter.trim().is_empty())
                .unwrap_or(false);
            if has_raw_content {
                return Err(crate::error::Error::FrontmatterParse(
                    "frontmatter has content but parsed as empty — possible YAML syntax error"
                        .into(),
                ));
            }
        }
    }

    // Strip @ prefix from user/org fields (@ is a reserved YAML character)
    if let Some(ref mut fm) = doc.frontmatter {
        if strip_at_prefix(fm, type_def) {
            changes.push(FormatChange {
                path: path.to_path_buf(),
                description: "stripped '@' prefix from user references".into(),
            });
            doc.rebuild_raw();
        }
    }

    // Remove empty array fields — absent field and [] are semantically identical
    if let Some(ref mut fm) = doc.frontmatter {
        if strip_empty_arrays(fm) {
            changes.push(FormatChange {
                path: path.to_path_buf(),
                description: "removed empty array fields".into(),
            });
            doc.rebuild_raw();
        }
    }

    // Remove frontmatter keys not defined in schema (type fields + relations)
    if let Some(ref mut fm) = doc.frontmatter {
        if strip_undefined_keys(fm, type_def, schema) {
            changes.push(FormatChange {
                path: path.to_path_buf(),
                description: "removed undefined frontmatter keys".into(),
            });
            doc.rebuild_raw();
        }
    }

    // Format frontmatter field order and grouping
    if let Some(ref fm) = doc.frontmatter {
        let grouped = fm.to_grouped_yaml(type_def, schema);
        // Extract current frontmatter YAML from raw (between --- delimiters)
        let current_fm_yaml = doc
            .raw
            .strip_prefix("---\n")
            .and_then(|s| s.split_once("\n---"))
            .map(|(fm_str, _)| format!("{}\n", fm_str))
            .unwrap_or_default();
        if grouped != current_fm_yaml {
            changes.push(FormatChange {
                path: path.to_path_buf(),
                description: "reordered frontmatter fields".into(),
            });
            // Rebuild raw with grouped frontmatter
            let mut raw = String::new();
            raw.push_str("---\n");
            raw.push_str(&grouped);
            raw.push_str("---\n");
            raw.push_str(&doc.body);
            doc.raw = raw;
        }
    }

    format_sections(&mut doc, &type_def.sections, path, &mut changes)?;
    format_rule_sections(&mut doc, type_def, path, &mut changes)?;

    if !changes.is_empty() && !dry_run {
        doc.save()?;
    }

    Ok(changes)
}

/// Infer document type from file path by matching against schema folder definitions.
/// Strip `@` prefix from user/org type fields in frontmatter.
/// Returns true if any changes were made.
fn strip_at_prefix(fm: &mut crate::frontmatter::Frontmatter, type_def: &TypeDef) -> bool {
    use crate::schema::FieldType;
    use serde_yaml::Value;

    let user_fields: Vec<String> = type_def
        .fields
        .iter()
        .filter(|f| {
            matches!(
                f.field_type,
                FieldType::User | FieldType::UserArray | FieldType::Org | FieldType::OrgArray
            )
        })
        .map(|f| f.name.clone())
        .collect();

    let mut changed = false;
    for field_name in &user_fields {
        let Some(val) = fm.data_mut().get_mut(field_name) else {
            continue;
        };
        match val {
            Value::String(s) if s.starts_with('@') => {
                *s = s.trim_start_matches('@').to_string();
                changed = true;
            }
            Value::Sequence(seq) => {
                for item in seq.iter_mut() {
                    if let Value::String(s) = item {
                        if s.starts_with('@') {
                            *s = s.trim_start_matches('@').to_string();
                            changed = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    changed
}

/// Remove empty array fields from frontmatter. Returns true if any were removed.
fn strip_empty_arrays(fm: &mut crate::frontmatter::Frontmatter) -> bool {
    let empty_keys: Vec<String> = fm
        .data()
        .iter()
        .filter(|(_, v)| v.as_sequence().is_some_and(|s| s.is_empty()))
        .map(|(k, _)| k.clone())
        .collect();
    let changed = !empty_keys.is_empty();
    for key in empty_keys {
        fm.remove(&key);
    }
    changed
}

/// Remove frontmatter keys not defined in schema type fields or relations.
fn strip_undefined_keys(
    fm: &mut crate::frontmatter::Frontmatter,
    type_def: &TypeDef,
    schema: &Schema,
) -> bool {
    let undefined: Vec<String> = fm
        .keys()
        .filter(|key| {
            // Check type-specific fields
            if type_def.fields.iter().any(|f| f.name == **key) {
                return false;
            }
            // Check relation fields
            if schema.find_relation(key).is_some() {
                return false;
            }
            true
        })
        .cloned()
        .collect();
    let changed = !undefined.is_empty();
    for key in undefined {
        fm.remove(&key);
    }
    changed
}

fn infer_type_from_folder(path: &Path, schema: &Schema) -> Option<String> {
    let path_str = path.to_string_lossy();
    for type_def in &schema.types {
        if let Some(ref folder) = type_def.folder {
            // Check if the file path contains the type's folder
            if path_str.contains(folder.as_str()) {
                return Some(type_def.name.clone());
            }
        }
    }
    None
}

fn format_sections(
    doc: &mut Document,
    section_defs: &[SectionDef],
    path: &Path,
    changes: &mut Vec<FormatChange>,
) -> Result<()> {
    for sec_def in section_defs {
        // Table column reordering
        if let Some(ref table_def) = sec_def.table {
            let col_order: Vec<String> = table_def.columns.iter().map(|c| c.name.clone()).collect();
            if !col_order.is_empty() {
                match doc.reorder_table(&sec_def.name, 0, &col_order) {
                    Ok(Some(_old_headers)) => {
                        changes.push(FormatChange {
                            path: path.to_path_buf(),
                            description: format!("reordered table columns in '{}'", sec_def.name),
                        });
                    }
                    Ok(None) => {}
                    Err(_) => {} // Section or table not found — skip
                }
            }
        }

        // List ordering fix
        if let Some(ref list_def) = sec_def.list {
            if let Some(ordered) = list_def.ordered {
                match doc.fix_list_ordering(&sec_def.name, ordered) {
                    Ok(true) => {
                        let kind = if ordered { "numbered" } else { "bullet" };
                        changes.push(FormatChange {
                            path: path.to_path_buf(),
                            description: format!("converted list to {kind} in '{}'", sec_def.name),
                        });
                    }
                    Ok(false) => {}
                    Err(_) => {} // Section not found — skip
                }
            }
        }

        // Five Whys formatting: split "N. Question? Answer" into multi-line
        if sec_def.name.to_lowercase().contains("whys") {
            match doc.format_whys(&sec_def.name) {
                Ok(true) => {
                    changes.push(FormatChange {
                        path: path.to_path_buf(),
                        description: format!("formatted whys in '{}'", sec_def.name),
                    });
                }
                Ok(false) => {}
                Err(_) => {}
            }
        }

        // Recurse into children
        format_sections(doc, &sec_def.children, path, changes)?;
    }

    Ok(())
}

/// Auto-convert list→table in sections when conditional rules match.
/// For each `then-section-table` rule whose condition is met, if the section
/// has a list but no table, convert the list items into table rows.
fn format_rule_sections(
    doc: &mut Document,
    type_def: &TypeDef,
    path: &Path,
    changes: &mut Vec<FormatChange>,
) -> Result<()> {
    // Collect replacements first to avoid borrow conflicts with doc
    let replacements = collect_rule_section_replacements(doc, type_def);

    for (section_name, table_md, description) in replacements {
        if doc
            .replace_section_content(&section_name, &table_md)
            .is_ok()
        {
            changes.push(FormatChange {
                path: path.to_path_buf(),
                description,
            });
        }
    }

    Ok(())
}

/// Collect (section_name, table_markdown, description) tuples for rule-based conversions.
fn collect_rule_section_replacements(
    doc: &Document,
    type_def: &TypeDef,
) -> Vec<(String, String, String)> {
    let mut replacements = Vec::new();

    let fm = match &doc.frontmatter {
        Some(fm) => fm,
        None => return replacements,
    };

    for rule in &type_def.rules {
        if rule.then_section_table.is_empty() {
            continue;
        }
        let val = match fm
            .get(&rule.when_field)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
        {
            Some(s) => s,
            None => continue,
        };
        if !rule.matches(&val) {
            continue;
        }

        for override_def in &rule.then_section_table {
            let section_content = match doc.get_section(&override_def.section) {
                Ok(s) => s.content,
                Err(_) => continue,
            };

            let arena = Arena::new();
            let root = ast_util::parse_md(&arena, &section_content);
            let tables = ast_util::find_tables(root);
            if !tables.is_empty() {
                continue; // Already has a table
            }

            let items = ast_util::extract_list_items(root);
            if items.is_empty() {
                continue; // No list to convert
            }

            let headers: Vec<String> = override_def
                .table
                .columns
                .iter()
                .map(|c| c.name.clone())
                .collect();

            let rows: Vec<Vec<String>> = items
                .iter()
                .map(|item| {
                    headers
                        .iter()
                        .enumerate()
                        .map(|(i, _)| {
                            if i == 0 {
                                "pending".to_string()
                            } else if i == 1 {
                                item.clone()
                            } else {
                                String::new()
                            }
                        })
                        .collect()
                })
                .collect();

            let table = Table::new(headers, rows);
            replacements.push((
                override_def.section.clone(),
                table.to_markdown(),
                format!(
                    "converted list to table in '{}' (rule: {})",
                    override_def.section,
                    rule.condition_display()
                ),
            ));
        }
    }

    replacements
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Schema;

    fn test_schema() -> Schema {
        Schema::from_str(
            r#"
type "test" {
    field "title" type="string" required=#true
    field "type" type="string" required=#true
    section "Data" {
        table {
            column "A" type="string"
            column "B" type="string"
            column "C" type="string"
        }
    }
    section "Steps" {
        list ordered=#true
    }
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_format_file_reorders_table() {
        let dir = std::env::temp_dir().join("dg_fmt_test_table");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("test.md");
        std::fs::write(
            &path,
            "\
---
title: Test
type: test
---

# Data

| C | A | B |
|---|---|---|
| c1 | a1 | b1 |

# Steps

1. first
2. second
",
        )
        .unwrap();

        let schema = test_schema();
        let changes = format_file(&path, &schema, false).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].description.contains("reordered"));

        // Verify file was updated
        let doc = Document::from_file(&path).unwrap();
        let section = doc.get_section("Data").unwrap();
        let tables = section.tables();
        assert_eq!(tables[0].headers(), &["A", "B", "C"]);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_format_file_fixes_list_ordering() {
        let dir = std::env::temp_dir().join("dg_fmt_test_list");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("test.md");
        std::fs::write(
            &path,
            "\
---
title: Test
type: test
---

# Data

| A | B | C |
|---|---|---|
| a1 | b1 | c1 |

# Steps

- first
- second
- third
",
        )
        .unwrap();

        let schema = test_schema();
        let changes = format_file(&path, &schema, false).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(changes[0].description.contains("numbered"));

        let doc = Document::from_file(&path).unwrap();
        let section = doc.get_section("Steps").unwrap();
        assert!(section.content.contains("1. first"));
        assert!(section.content.contains("2. second"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_format_file_dry_run() {
        let dir = std::env::temp_dir().join("dg_fmt_test_dry");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("test.md");
        let original = "\
---
title: Test
type: test
---

# Data

| C | A | B |
|---|---|---|
| c1 | a1 | b1 |

# Steps

1. ok
";
        std::fs::write(&path, original).unwrap();

        let schema = test_schema();
        let changes = format_file(&path, &schema, true).unwrap();
        assert!(!changes.is_empty());

        // File should NOT be modified
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, original);

        std::fs::remove_dir_all(&dir).ok();
    }

    fn rule_section_schema() -> Schema {
        Schema::from_str(
            r#"
type "opp" {
    field "title" type="string"
    field "type" type="string"
    field "status" type="string"
    section "Action Items"

    rule "pursuing needs table" {
        when "status" equals="pursuing"
        then-section-table "Action Items" {
            table {
                column "Status" type="string" required=#true
                column "Item" type="string" required=#true
                column "Due" type="string"
            }
        }
    }
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_format_rule_sections_list_to_table() {
        let dir = std::env::temp_dir().join("dg_fmt_test_rule_l2t");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("opp-001.md");
        std::fs::write(
            &path,
            "\
---
title: Test
type: opp
status: pursuing
---

# Action Items

- Do thing A
- Do thing B
",
        )
        .unwrap();

        let schema = rule_section_schema();
        let changes = format_file(&path, &schema, false).unwrap();
        assert!(!changes.is_empty(), "should have changes");
        assert!(
            changes
                .iter()
                .any(|c| c.description.contains("converted list to table")),
            "should contain list-to-table change: {:?}",
            changes.iter().map(|c| &c.description).collect::<Vec<_>>()
        );

        let doc = Document::from_file(&path).unwrap();
        let section = doc.get_section("Action Items").unwrap();
        let tables = section.tables();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].headers(), &["Status", "Item", "Due"]);
        assert_eq!(tables[0].get_cell("Status", 0), Some("pending"));
        assert_eq!(tables[0].get_cell("Item", 0), Some("Do thing A"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_format_rule_sections_skips_existing_table() {
        let dir = std::env::temp_dir().join("dg_fmt_test_rule_skip");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("opp-002.md");
        std::fs::write(
            &path,
            "\
---
title: Test
type: opp
status: pursuing
---

# Action Items

| Status | Item | Due |
|---|---|---|
| pending | Existing | 2025-01-01 |
",
        )
        .unwrap();

        let schema = rule_section_schema();
        let changes = format_file(&path, &schema, false).unwrap();
        assert!(
            !changes
                .iter()
                .any(|c| c.description.contains("converted list to table")),
            "should not convert when table already exists"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_format_rule_sections_condition_not_met() {
        let dir = std::env::temp_dir().join("dg_fmt_test_rule_nomet");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("opp-003.md");
        std::fs::write(
            &path,
            "\
---
title: Test
type: opp
status: exploring
---

# Action Items

- Item one
- Item two
",
        )
        .unwrap();

        let schema = rule_section_schema();
        let changes = format_file(&path, &schema, false).unwrap();
        assert!(
            !changes
                .iter()
                .any(|c| c.description.contains("converted list to table")),
            "should not convert when condition not met"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_format_file_rejects_unparseable_frontmatter() {
        let dir = std::env::temp_dir().join("dg_fmt_test_bad_yaml");
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("test.md");
        let original = "---\nresponders: [@mikko]\ntype: test\n---\n\n# Body\n";
        std::fs::write(&path, original).unwrap();

        let schema = test_schema();
        let result = format_file(&path, &schema, false);
        assert!(result.is_err(), "should error on unparseable YAML");

        // File must be unchanged
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, original);

        std::fs::remove_dir_all(&dir).ok();
    }
}

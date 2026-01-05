//! Schema migration — detect schema changes and help migrate documents.
//!
//! Compares two schemas, produces a diff, scans documents, and builds a migration plan.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::discovery;
use crate::document::Document;
use crate::schema::{FieldDef, FieldType, Schema, SectionDef, TypeDef};

// ─── Schema Diff ─────────────────────────────────────────────────────────────

/// Difference between two schema versions.
#[derive(Debug, Clone)]
pub struct SchemaDiff {
    pub added_types: Vec<String>,
    pub removed_types: Vec<String>,
    pub type_changes: Vec<TypeChange>,
}

/// Changes within a single type definition.
#[derive(Debug, Clone)]
pub struct TypeChange {
    pub type_name: String,
    pub added_fields: Vec<FieldDef>,
    pub removed_fields: Vec<FieldDef>,
    pub changed_fields: Vec<FieldChange>,
    pub added_sections: Vec<String>,
    pub removed_sections: Vec<String>,
}

/// A field that changed between schema versions.
#[derive(Debug, Clone)]
pub struct FieldChange {
    pub name: String,
    pub old: FieldDef,
    pub new: FieldDef,
    /// Enum values removed (only for enum fields).
    pub removed_enum_values: Vec<String>,
    /// Enum values added (only for enum fields).
    pub added_enum_values: Vec<String>,
}

impl fmt::Display for SchemaDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Schema diff:")?;
        for t in &self.added_types {
            writeln!(f, "  + type \"{t}\"")?;
        }
        for t in &self.removed_types {
            writeln!(f, "  - type \"{t}\"")?;
        }
        for tc in &self.type_changes {
            for field in &tc.added_fields {
                let default_info = field
                    .default
                    .as_deref()
                    .map(|d| format!(", default={d}"))
                    .unwrap_or_default();
                let req = if field.required { ", required" } else { "" };
                writeln!(
                    f,
                    "  + field \"{}\" (type={}{}{}) on type \"{}\"",
                    field.name, field.field_type, req, default_info, tc.type_name
                )?;
            }
            for field in &tc.removed_fields {
                writeln!(
                    f,
                    "  - field \"{}\" on type \"{}\"",
                    field.name, tc.type_name
                )?;
            }
            for fc in &tc.changed_fields {
                if fc.old.field_type != fc.new.field_type {
                    writeln!(
                        f,
                        "  ~ field \"{}\" type changed {} -> {} on type \"{}\"",
                        fc.name, fc.old.field_type, fc.new.field_type, tc.type_name
                    )?;
                }
                for v in &fc.removed_enum_values {
                    writeln!(
                        f,
                        "  - enum value \"{v}\" removed from \"{}\" on type \"{}\"",
                        fc.name, tc.type_name
                    )?;
                }
                for v in &fc.added_enum_values {
                    writeln!(
                        f,
                        "  + enum value \"{v}\" added to \"{}\" on type \"{}\"",
                        fc.name, tc.type_name
                    )?;
                }
                if fc.old.required != fc.new.required {
                    let change = if fc.new.required {
                        "now required"
                    } else {
                        "now optional"
                    };
                    writeln!(
                        f,
                        "  ~ field \"{}\" {change} on type \"{}\"",
                        fc.name, tc.type_name
                    )?;
                }
            }
            for s in &tc.added_sections {
                writeln!(f, "  + section \"{s}\" on type \"{}\"", tc.type_name)?;
            }
            for s in &tc.removed_sections {
                writeln!(f, "  - section \"{s}\" on type \"{}\"", tc.type_name)?;
            }
        }
        Ok(())
    }
}

/// Compare two schemas and return the diff.
pub fn diff_schemas(old: &Schema, new: &Schema) -> SchemaDiff {
    let old_names: HashSet<&str> = old.types.iter().map(|t| t.name.as_str()).collect();
    let new_names: HashSet<&str> = new.types.iter().map(|t| t.name.as_str()).collect();

    let added_types: Vec<String> = new_names
        .difference(&old_names)
        .map(|s| s.to_string())
        .collect();
    let removed_types: Vec<String> = old_names
        .difference(&new_names)
        .map(|s| s.to_string())
        .collect();

    let mut type_changes = Vec::new();
    for new_type in &new.types {
        if let Some(old_type) = old.get_type(&new_type.name) {
            let tc = diff_type(old_type, new_type);
            if !tc.is_empty() {
                type_changes.push(tc);
            }
        }
    }

    SchemaDiff {
        added_types,
        removed_types,
        type_changes,
    }
}

fn diff_type(old: &TypeDef, new: &TypeDef) -> TypeChange {
    let old_fields: HashMap<&str, &FieldDef> =
        old.fields.iter().map(|f| (f.name.as_str(), f)).collect();
    let new_fields: HashMap<&str, &FieldDef> =
        new.fields.iter().map(|f| (f.name.as_str(), f)).collect();

    let old_field_names: HashSet<&str> = old_fields.keys().copied().collect();
    let new_field_names: HashSet<&str> = new_fields.keys().copied().collect();

    let added_fields: Vec<FieldDef> = new_field_names
        .difference(&old_field_names)
        .map(|name| new_fields[name].clone())
        .collect();

    let removed_fields: Vec<FieldDef> = old_field_names
        .difference(&new_field_names)
        .map(|name| old_fields[name].clone())
        .collect();

    let mut changed_fields = Vec::new();
    for name in old_field_names.intersection(&new_field_names) {
        let of = old_fields[name];
        let nf = new_fields[name];
        if fields_differ(of, nf) {
            let (removed_enum_values, added_enum_values) = diff_enum_values(of, nf);
            changed_fields.push(FieldChange {
                name: name.to_string(),
                old: of.clone(),
                new: nf.clone(),
                removed_enum_values,
                added_enum_values,
            });
        }
    }

    let old_sections = collect_section_names(&old.sections);
    let new_sections = collect_section_names(&new.sections);

    let added_sections: Vec<String> = new_sections.difference(&old_sections).cloned().collect();
    let removed_sections: Vec<String> = old_sections.difference(&new_sections).cloned().collect();

    TypeChange {
        type_name: new.name.clone(),
        added_fields,
        removed_fields,
        changed_fields,
        added_sections,
        removed_sections,
    }
}

impl TypeChange {
    fn is_empty(&self) -> bool {
        self.added_fields.is_empty()
            && self.removed_fields.is_empty()
            && self.changed_fields.is_empty()
            && self.added_sections.is_empty()
            && self.removed_sections.is_empty()
    }
}

impl SchemaDiff {
    /// True when nothing changed.
    pub fn is_empty(&self) -> bool {
        self.added_types.is_empty() && self.removed_types.is_empty() && self.type_changes.is_empty()
    }
}

fn fields_differ(a: &FieldDef, b: &FieldDef) -> bool {
    a.field_type != b.field_type || a.required != b.required || a.default != b.default
}

fn diff_enum_values(old: &FieldDef, new: &FieldDef) -> (Vec<String>, Vec<String>) {
    if let (FieldType::Enum(old_vals), FieldType::Enum(new_vals)) =
        (&old.field_type, &new.field_type)
    {
        let old_set: HashSet<&str> = old_vals.iter().map(|s| s.as_str()).collect();
        let new_set: HashSet<&str> = new_vals.iter().map(|s| s.as_str()).collect();
        let removed: Vec<String> = old_set
            .difference(&new_set)
            .map(|s| s.to_string())
            .collect();
        let added: Vec<String> = new_set
            .difference(&old_set)
            .map(|s| s.to_string())
            .collect();
        (removed, added)
    } else {
        (Vec::new(), Vec::new())
    }
}

fn collect_section_names(sections: &[SectionDef]) -> HashSet<String> {
    let mut names = HashSet::new();
    for s in sections {
        names.insert(s.name.clone());
        names.extend(collect_section_names(&s.children));
    }
    names
}

// ─── Migration Plan ──────────────────────────────────────────────────────────

/// A concrete plan of actions to apply to documents.
#[derive(Debug, Clone)]
pub struct MigrationPlan {
    pub actions: Vec<MigrationAction>,
}

/// A single migration action, with the affected document paths.
#[derive(Debug, Clone)]
pub struct MigrationAction {
    pub kind: ActionKind,
    pub affected_docs: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum ActionKind {
    /// Add a field with a default value.
    AddField {
        type_name: String,
        field_name: String,
        default_value: String,
    },
    /// Remove a field from documents.
    RemoveField {
        type_name: String,
        field_name: String,
    },
    /// Warn that docs use a removed enum value (manual fix needed).
    RemovedEnumValue {
        type_name: String,
        field_name: String,
        value: String,
    },
    /// Add an empty section scaffold.
    AddSection {
        type_name: String,
        section_name: String,
    },
}

impl fmt::Display for MigrationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.actions.is_empty() {
            writeln!(f, "No migrations needed.")?;
            return Ok(());
        }
        writeln!(f, "Migrations:")?;
        for action in &self.actions {
            let count = action.affected_docs.len();
            match &action.kind {
                ActionKind::AddField {
                    field_name,
                    default_value,
                    ..
                } => {
                    writeln!(
                        f,
                        "  {count} doc(s): add field {field_name}={default_value}"
                    )?;
                }
                ActionKind::RemoveField { field_name, .. } => {
                    writeln!(f, "  {count} doc(s): remove field {field_name}")?;
                }
                ActionKind::RemovedEnumValue {
                    field_name, value, ..
                } => {
                    writeln!(
                        f,
                        "  {count} doc(s): WARNING — using removed enum value \"{value}\" in field \"{field_name}\" (manual fix needed)"
                    )?;
                }
                ActionKind::AddSection { section_name, .. } => {
                    writeln!(f, "  {count} doc(s): add section \"{section_name}\"")?;
                }
            }
        }
        Ok(())
    }
}

/// Scan documents on disk and compute a migration plan from the diff.
pub fn compute_migration(diff: &SchemaDiff, dir: &Path) -> MigrationPlan {
    let mut actions = Vec::new();

    // Discover all markdown files once
    let all_files = discovery::discover_files(dir, Some("*.md"), &[], false).unwrap_or_default();

    // Build a map: type_name -> Vec<(PathBuf, Document)>
    let mut docs_by_type: HashMap<String, Vec<(PathBuf, Document)>> = HashMap::new();
    for path in &all_files {
        if let Ok(doc) = Document::from_file(path) {
            // Infer type from filename ID prefix, falling back to frontmatter
            let doc_id = crate::graph::path_to_id(path);
            let type_val = doc_id
                .split('-')
                .next()
                .map(|p| p.to_lowercase())
                .or_else(|| {
                    doc.frontmatter
                        .as_ref()
                        .and_then(|fm| fm.get_display("type"))
                });
            if let Some(type_val) = type_val {
                docs_by_type
                    .entry(type_val)
                    .or_default()
                    .push((path.clone(), doc));
            }
        }
    }

    for tc in &diff.type_changes {
        let docs = docs_by_type.get(&tc.type_name).cloned().unwrap_or_default();

        // Added fields with defaults
        for field in &tc.added_fields {
            if let Some(ref default) = field.default {
                let affected: Vec<PathBuf> = docs
                    .iter()
                    .filter(|(_, doc)| {
                        doc.frontmatter
                            .as_ref()
                            .map(|fm| !fm.has_field(&field.name))
                            .unwrap_or(true)
                    })
                    .map(|(p, _)| p.clone())
                    .collect();

                if !affected.is_empty() {
                    actions.push(MigrationAction {
                        kind: ActionKind::AddField {
                            type_name: tc.type_name.clone(),
                            field_name: field.name.clone(),
                            default_value: default.clone(),
                        },
                        affected_docs: affected,
                    });
                }
            }
        }

        // Removed fields
        for field in &tc.removed_fields {
            let affected: Vec<PathBuf> = docs
                .iter()
                .filter(|(_, doc)| {
                    doc.frontmatter
                        .as_ref()
                        .map(|fm| fm.has_field(&field.name))
                        .unwrap_or(false)
                })
                .map(|(p, _)| p.clone())
                .collect();

            if !affected.is_empty() {
                actions.push(MigrationAction {
                    kind: ActionKind::RemoveField {
                        type_name: tc.type_name.clone(),
                        field_name: field.name.clone(),
                    },
                    affected_docs: affected,
                });
            }
        }

        // Removed enum values
        for fc in &tc.changed_fields {
            for removed_val in &fc.removed_enum_values {
                let affected: Vec<PathBuf> = docs
                    .iter()
                    .filter(|(_, doc)| {
                        doc.frontmatter
                            .as_ref()
                            .and_then(|fm| fm.get_display(&fc.name))
                            .map(|v| v == *removed_val)
                            .unwrap_or(false)
                    })
                    .map(|(p, _)| p.clone())
                    .collect();

                if !affected.is_empty() {
                    actions.push(MigrationAction {
                        kind: ActionKind::RemovedEnumValue {
                            type_name: tc.type_name.clone(),
                            field_name: fc.name.clone(),
                            value: removed_val.clone(),
                        },
                        affected_docs: affected,
                    });
                }
            }
        }

        // Added sections
        for section_name in &tc.added_sections {
            let affected: Vec<PathBuf> = docs
                .iter()
                .filter(|(_, doc)| doc.get_section(section_name).is_err())
                .map(|(p, _)| p.clone())
                .collect();

            if !affected.is_empty() {
                actions.push(MigrationAction {
                    kind: ActionKind::AddSection {
                        type_name: tc.type_name.clone(),
                        section_name: section_name.clone(),
                    },
                    affected_docs: affected,
                });
            }
        }
    }

    MigrationPlan { actions }
}

/// Apply a migration plan: mutate documents on disk.
pub fn apply_migration(plan: &MigrationPlan) -> Result<ApplyResult, crate::error::Error> {
    let mut modified = 0u32;
    let mut warnings = 0u32;

    for action in &plan.actions {
        match &action.kind {
            ActionKind::AddField {
                field_name,
                default_value,
                ..
            } => {
                for path in &action.affected_docs {
                    let mut doc = Document::from_file(path)?;
                    doc.set_field_from_str(field_name, default_value);
                    doc.save()?;
                    modified += 1;
                }
            }
            ActionKind::RemoveField { field_name, .. } => {
                for path in &action.affected_docs {
                    let mut doc = Document::from_file(path)?;
                    doc.remove_field(field_name);
                    doc.save()?;
                    modified += 1;
                }
            }
            ActionKind::RemovedEnumValue { .. } => {
                // Cannot auto-fix — just count as warning
                warnings += action.affected_docs.len() as u32;
            }
            ActionKind::AddSection { section_name, .. } => {
                for path in &action.affected_docs {
                    let mut doc = Document::from_file(path)?;
                    // Append an empty section scaffold at the end
                    let section_md = format!("\n# {section_name}\n\n<!-- TODO: fill in -->\n");
                    doc.body.push_str(&section_md);
                    // Rebuild raw from frontmatter + body, then write directly
                    let mut raw = String::new();
                    if let Some(ref fm) = doc.frontmatter {
                        raw.push_str("---\n");
                        raw.push_str(&fm.to_yaml_string());
                        raw.push_str("---\n");
                    }
                    raw.push_str(&doc.body);
                    let path = doc.path.as_ref().ok_or(crate::error::Error::NoPath)?;
                    std::fs::write(path, &raw)
                        .map_err(|_| crate::error::Error::WriteFailed(path.clone()))?;
                    modified += 1;
                }
            }
        }
    }

    Ok(ApplyResult { modified, warnings })
}

/// Summary after applying migrations.
#[derive(Debug, Clone)]
pub struct ApplyResult {
    pub modified: u32,
    pub warnings: u32,
}

impl fmt::Display for ApplyResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Applied: {} doc(s) modified, {} warning(s) (manual fix needed)",
            self.modified, self.warnings
        )
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn schema_v1() -> Schema {
        Schema::from_str(
            r#"
type "adr" {
    field "title" type="string" required=#true
    field "status" type="enum" required=#true {
        values "proposed" "accepted" "rejected" "deprecated"
    }
    field "date" type="string"
    section "Decision" required=#true
    section "Consequences"
}
"#,
        )
        .unwrap()
    }

    fn schema_v2() -> Schema {
        Schema::from_str(
            r#"
type "adr" {
    field "title" type="string" required=#true
    field "status" type="enum" required=#true {
        values "proposed" "accepted" "superseded"
    }
    field "priority" type="enum" required=#true default="medium" {
        values "low" "medium" "high"
    }
    section "Decision" required=#true
    section "Consequences"
    section "References"
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_diff_detects_added_field() {
        let diff = diff_schemas(&schema_v1(), &schema_v2());
        let tc = &diff.type_changes[0];
        assert!(tc.added_fields.iter().any(|f| f.name == "priority"));
    }

    #[test]
    fn test_diff_detects_removed_field() {
        let diff = diff_schemas(&schema_v1(), &schema_v2());
        let tc = &diff.type_changes[0];
        assert!(tc.removed_fields.iter().any(|f| f.name == "date"));
    }

    #[test]
    fn test_diff_detects_removed_enum_values() {
        let diff = diff_schemas(&schema_v1(), &schema_v2());
        let tc = &diff.type_changes[0];
        let status_change = tc
            .changed_fields
            .iter()
            .find(|c| c.name == "status")
            .unwrap();
        assert!(status_change
            .removed_enum_values
            .contains(&"rejected".to_string()));
        assert!(status_change
            .removed_enum_values
            .contains(&"deprecated".to_string()));
    }

    #[test]
    fn test_diff_detects_added_enum_values() {
        let diff = diff_schemas(&schema_v1(), &schema_v2());
        let tc = &diff.type_changes[0];
        let status_change = tc
            .changed_fields
            .iter()
            .find(|c| c.name == "status")
            .unwrap();
        assert!(status_change
            .added_enum_values
            .contains(&"superseded".to_string()));
    }

    #[test]
    fn test_diff_detects_added_section() {
        let diff = diff_schemas(&schema_v1(), &schema_v2());
        let tc = &diff.type_changes[0];
        assert!(tc.added_sections.contains(&"References".to_string()));
    }

    #[test]
    fn test_diff_detects_added_type() {
        let old =
            Schema::from_str(r#"type "adr" { field "x" type="string"; section "S" }"#).unwrap();
        let new = Schema::from_str(
            r#"
type "adr" { field "x" type="string"; section "S" }
type "rfc" { field "x" type="string"; section "S" }
"#,
        )
        .unwrap();
        let diff = diff_schemas(&old, &new);
        assert!(diff.added_types.contains(&"rfc".to_string()));
    }

    #[test]
    fn test_diff_detects_removed_type() {
        let old = Schema::from_str(
            r#"
type "adr" { field "x" type="string"; section "S" }
type "rfc" { field "x" type="string"; section "S" }
"#,
        )
        .unwrap();
        let new =
            Schema::from_str(r#"type "adr" { field "x" type="string"; section "S" }"#).unwrap();
        let diff = diff_schemas(&old, &new);
        assert!(diff.removed_types.contains(&"rfc".to_string()));
    }

    #[test]
    fn test_empty_diff_for_identical_schemas() {
        let s = schema_v1();
        let diff = diff_schemas(&s, &s);
        assert!(diff.is_empty());
    }

    #[test]
    fn test_display_schema_diff() {
        let diff = diff_schemas(&schema_v1(), &schema_v2());
        let output = diff.to_string();
        assert!(output.contains("Schema diff:"));
        assert!(output.contains("priority"));
    }

    #[test]
    fn test_compute_migration_on_fixtures() {
        let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");

        // Use v1 = fixture schema, v2 = modified schema with new field
        let old_schema = Schema::from_file(fixtures.join("schema.kdl")).unwrap();
        let new_schema_str = std::fs::read_to_string(fixtures.join("schema.kdl")).unwrap()
            + r#"
"#;
        // Parse original, then manually add a field to create a diff
        let mut new_schema = Schema::from_str(&new_schema_str).unwrap();
        // Add a priority field to "adr" type
        if let Some(adr) = new_schema.types.iter_mut().find(|t| t.name == "adr") {
            adr.fields.push(FieldDef {
                name: "urgency".to_string(),
                field_type: FieldType::Enum(vec![
                    "low".to_string(),
                    "medium".to_string(),
                    "high".to_string(),
                ]),
                required: true,
                pattern: None,
                description: None,
                default: Some("medium".to_string()),
                transitions: Vec::new(),
            });
        }

        let diff = diff_schemas(&old_schema, &new_schema);
        assert!(!diff.is_empty());

        let plan = compute_migration(&diff, &fixtures);
        // Should find ADR docs that need the new urgency field
        let add_actions: Vec<_> = plan
            .actions
            .iter()
            .filter(|a| matches!(&a.kind, ActionKind::AddField { field_name, .. } if field_name == "urgency"))
            .collect();
        // At least the fixture ADR docs should be affected
        assert!(
            !add_actions.is_empty(),
            "should find docs needing new field"
        );
    }

    #[test]
    fn test_migration_plan_display_empty() {
        let plan = MigrationPlan {
            actions: Vec::new(),
        };
        assert!(plan.to_string().contains("No migrations needed"));
    }

    #[test]
    fn test_migration_plan_display_with_actions() {
        let plan = MigrationPlan {
            actions: vec![
                MigrationAction {
                    kind: ActionKind::AddField {
                        type_name: "adr".into(),
                        field_name: "priority".into(),
                        default_value: "medium".into(),
                    },
                    affected_docs: vec![PathBuf::from("a.md"), PathBuf::from("b.md")],
                },
                MigrationAction {
                    kind: ActionKind::RemovedEnumValue {
                        type_name: "adr".into(),
                        field_name: "status".into(),
                        value: "wontfix".into(),
                    },
                    affected_docs: vec![PathBuf::from("c.md")],
                },
            ],
        };
        let output = plan.to_string();
        assert!(output.contains("2 doc(s): add field priority=medium"));
        assert!(output.contains("WARNING"));
        assert!(output.contains("wontfix"));
    }

    #[test]
    fn test_apply_migration_add_field() {
        let dir = std::env::temp_dir().join("md_db_migrate_test_apply");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create a test document
        let doc_path = dir.join("test-001.md");
        std::fs::write(
            &doc_path,
            "---\ntype: adr\ntitle: Test\nstatus: proposed\n---\n\n# Decision\n\nSome text.\n",
        )
        .unwrap();

        let plan = MigrationPlan {
            actions: vec![MigrationAction {
                kind: ActionKind::AddField {
                    type_name: "adr".into(),
                    field_name: "priority".into(),
                    default_value: "medium".into(),
                },
                affected_docs: vec![doc_path.clone()],
            }],
        };

        let result = apply_migration(&plan).unwrap();
        assert_eq!(result.modified, 1);

        // Verify the field was added
        let doc = Document::from_file(&doc_path).unwrap();
        assert_eq!(
            doc.frontmatter().unwrap().get_display("priority").unwrap(),
            "medium"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_apply_migration_remove_field() {
        let dir = std::env::temp_dir().join("md_db_migrate_test_remove");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let doc_path = dir.join("test-001.md");
        std::fs::write(
            &doc_path,
            "---\ntype: adr\ntitle: Test\nold_field: value\n---\n\n# Body\n",
        )
        .unwrap();

        let plan = MigrationPlan {
            actions: vec![MigrationAction {
                kind: ActionKind::RemoveField {
                    type_name: "adr".into(),
                    field_name: "old_field".into(),
                },
                affected_docs: vec![doc_path.clone()],
            }],
        };

        let result = apply_migration(&plan).unwrap();
        assert_eq!(result.modified, 1);

        let doc = Document::from_file(&doc_path).unwrap();
        assert!(!doc.frontmatter().unwrap().has_field("old_field"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}

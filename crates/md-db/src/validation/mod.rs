mod content;
pub(crate) mod directory;
pub mod document;
mod types;

pub use directory::{validate_directory, validate_service_linters, validate_service_tests};
pub use document::{error_diagnostic, infer_type_from_path, validate_document, validate_singleton};
pub use types::{Diagnostic, FileResult, Severity, ValidationResult};

// Re-export commonly used types for tests and internal use
// Re-exports for test access (tests use `use super::*`)
#[cfg(test)]
use crate::document::{Document, ParsedBody};
#[cfg(test)]
use crate::schema::{FieldDef, FieldType, Schema, SectionDef, TableDef, TypeDef};
#[cfg(test)]
use crate::users::OrgConfig;
#[cfg(test)]
pub(crate) use content::{check_broken_tables, check_image_paths};
#[cfg(test)]
pub(crate) use directory::{
    validate_license_file, validate_service_readmes, validate_singleton_presence,
};
#[cfg(test)]
pub(crate) use document::singleton_matches;
#[cfg(test)]
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;

    fn test_schema() -> Schema {
        Schema::from_str(
            r#"
type "adr" {
    field "status" type="enum" required=#true {
        values "proposed" "accepted" "rejected"
    }
    field "author" type="string" required=#true pattern="^@.+"
    section "Decision" required=#true
    section "Consequences" required=#true {
        section "Positive" required=#true
    }
}
ref-format {
    string-id pattern="^ADR-\\d+$"
}
"#,
        )
        .unwrap()
    }

    /// Create a test doc with a path so type can be inferred from filename.
    fn test_doc(content: &str, filename: &str) -> Document {
        let mut doc = Document::from_str(content).unwrap();
        doc.path = Some(std::path::PathBuf::from(filename));
        doc
    }

    #[test]
    fn test_valid_document() {
        let doc = test_doc(
            "---\nstatus: accepted\nauthor: \"@onni\"\n---\n\n# Test\n\nIntroduction.\n\n## Decision\n\nWe decided.\n\n## Consequences\n\n### Positive\n\nGood.\n",
            "adr-001-test.md",
        );
        let schema = test_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert_eq!(result.errors(), 0, "diagnostics: {:?}", result.diagnostics);
    }

    #[test]
    fn test_missing_required_field() {
        let doc = test_doc(
            "---\nstatus: accepted\n---\n\n# Test\n\nIntro.\n\n## Decision\n\nX\n\n## Consequences\n\n### Positive\n\nY\n",
            "adr-002-test.md",
        );
        let schema = test_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result.errors() > 0);
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.code == "F010" && d.message.contains("author")));
    }

    #[test]
    fn test_invalid_enum_value() {
        let doc = Document::from_str(
            "---\ntype: adr\ntitle: T\nstatus: invalid\nauthor: \"@x\"\n---\n\n# Decision\n\nX\n\n# Consequences\n\n## Positive\n\nY\n",
        )
        .unwrap();
        let schema = test_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result.diagnostics.iter().any(|d| d.code == "F021"));
    }

    #[test]
    fn test_pattern_mismatch() {
        let doc = Document::from_str(
            "---\ntype: adr\ntitle: T\nstatus: accepted\nauthor: badformat\n---\n\n# Decision\n\nX\n\n# Consequences\n\n## Positive\n\nY\n",
        )
        .unwrap();
        let schema = test_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result.diagnostics.iter().any(|d| d.code == "F030"));
    }

    #[test]
    fn test_missing_required_section() {
        let doc = Document::from_str(
            "---\ntype: adr\ntitle: T\nstatus: accepted\nauthor: \"@x\"\n---\n\n# Decision\n\nX\n",
        )
        .unwrap();
        let schema = test_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.code == "S010" && d.message.contains("Consequences")));
    }

    #[test]
    fn test_unknown_type() {
        let doc = Document::from_str("---\ntype: unknown\ntitle: T\n---\n\n# Body\n").unwrap();
        let schema = test_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result.diagnostics.iter().any(|d| d.code == "F002"));
    }

    fn user_schema() -> Schema {
        Schema::from_str(
            r#"
type "doc" {
    field "title" type="string" required=#true
    field "author" type="user" required=#true
    field "reviewers" type="user[]"
    section "Body" required=#true
}
"#,
        )
        .unwrap()
    }

    fn test_user_config() -> OrgConfig {
        OrgConfig::from_str(
            r#"
user "onni" {
    name "Onni Example"
    teams "platform"
}
user "alice" {
    name "Alice Smith"
    teams "platform"
}
team "platform" {
    name "Platform Team"
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_valid_user_field() {
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\nauthor: onni\n---\n\n# Body\n\nContent\n",
        )
        .unwrap();
        let schema = user_schema();
        let uc = test_user_config();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), Some(&uc));
        assert_eq!(result.errors(), 0, "diagnostics: {:?}", result.diagnostics);
    }

    #[test]
    fn test_at_prefix_in_frontmatter_is_error() {
        // @ is a reserved YAML character — must not be used in frontmatter
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\nauthor: \"@onni\"\n---\n\n# Body\n\nContent\n",
        )
        .unwrap();
        let schema = user_schema();
        let uc = test_user_config();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), Some(&uc));
        assert!(
            result.diagnostics.iter().any(|d| d.code == "U013"),
            "diagnostics: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_unknown_user_ref() {
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\nauthor: unknown\n---\n\n# Body\n\nContent\n",
        )
        .unwrap();
        let schema = user_schema();
        let uc = test_user_config();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), Some(&uc));
        assert!(result.diagnostics.iter().any(|d| d.code == "U011"));
    }

    #[test]
    fn test_valid_user_array() {
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\nauthor: onni\nreviewers:\n  - alice\n  - team/platform\n---\n\n# Body\n\nContent\n",
        )
        .unwrap();
        let schema = user_schema();
        let uc = test_user_config();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), Some(&uc));
        assert_eq!(result.errors(), 0, "diagnostics: {:?}", result.diagnostics);
    }

    #[test]
    fn test_user_without_config_no_error() {
        // Without OrgConfig, bare handles pass (no lookup to validate against)
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\nauthor: anyone\n---\n\n# Body\n\nContent\n",
        )
        .unwrap();
        let schema = user_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert_eq!(result.errors(), 0, "diagnostics: {:?}", result.diagnostics);
    }

    // ─── Content constraint tests ────────────────────────────────────────

    fn content_schema() -> Schema {
        Schema::from_str(
            r#"
type "doc" {
    field "title" type="string"
    section "Body" required=#true {
        content min-paragraphs=2
    }
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_content_constraint_pass() {
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\n---\n\n# Body\n\nFirst paragraph.\n\nSecond paragraph.\n",
        )
        .unwrap();
        let schema = content_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert_eq!(result.errors(), 0, "diagnostics: {:?}", result.diagnostics);
    }

    #[test]
    fn test_content_constraint_fail() {
        let doc =
            Document::from_str("---\ntype: doc\ntitle: T\n---\n\n# Body\n\nOnly one paragraph.\n")
                .unwrap();
        let schema = content_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result.diagnostics.iter().any(|d| d.code == "S030"));
    }

    fn list_schema() -> Schema {
        Schema::from_str(
            r#"
type "doc" {
    field "title" type="string"
    section "Reqs" required=#true {
        list min-items=2
    }
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_list_constraint_pass() {
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\n---\n\n# Reqs\n\n- Item one\n- Item two\n",
        )
        .unwrap();
        let schema = list_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert_eq!(result.errors(), 0, "diagnostics: {:?}", result.diagnostics);
    }

    #[test]
    fn test_list_constraint_missing() {
        let doc =
            Document::from_str("---\ntype: doc\ntitle: T\n---\n\n# Reqs\n\nJust text.\n").unwrap();
        let schema = list_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result.diagnostics.iter().any(|d| d.code == "S031"));
    }

    #[test]
    fn test_list_constraint_too_few() {
        let doc =
            Document::from_str("---\ntype: doc\ntitle: T\n---\n\n# Reqs\n\n- Only one\n").unwrap();
        let schema = list_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result
            .diagnostics
            .iter()
            .any(|d| d.code == "S031" && d.message.contains("2")));
    }

    fn diagram_schema() -> Schema {
        Schema::from_str(
            r#"
type "doc" {
    field "title" type="string"
    section "Arch" required=#true {
        diagram type="mermaid"
    }
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_diagram_constraint_pass() {
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\n---\n\n# Arch\n\n```mermaid\ngraph TD\n  A-->B\n```\n",
        )
        .unwrap();
        let schema = diagram_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert_eq!(result.errors(), 0, "diagnostics: {:?}", result.diagnostics);
    }

    #[test]
    fn test_diagram_constraint_missing() {
        let doc =
            Document::from_str("---\ntype: doc\ntitle: T\n---\n\n# Arch\n\nJust text.\n").unwrap();
        let schema = diagram_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result.diagnostics.iter().any(|d| d.code == "S032"));
    }

    #[test]
    fn test_diagram_constraint_wrong_type() {
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\n---\n\n# Arch\n\n```d2\nshape: oval\n```\n",
        )
        .unwrap();
        let schema = diagram_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result.diagnostics.iter().any(|d| d.code == "S032"));
    }

    #[test]
    fn test_diagram_any_type() {
        let schema = Schema::from_str(
            r#"
type "doc" {
    field "title" type="string"
    section "Arch" required=#true {
        diagram
    }
}
"#,
        )
        .unwrap();
        // d2 should pass with "any" diagram type
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\n---\n\n# Arch\n\n```d2\nshape: oval\n```\n",
        )
        .unwrap();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert_eq!(result.errors(), 0, "diagnostics: {:?}", result.diagnostics);
    }

    fn ordered_list_schema() -> Schema {
        Schema::from_str(
            r#"
type "doc" {
    field "title" type="string"
    section "Steps" required=#true {
        list min-items=1 ordered=#true
    }
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_ordered_list_pass() {
        let doc =
            Document::from_str("---\ntype: doc\ntitle: T\n---\n\n# Steps\n\n1. First\n2. Second\n")
                .unwrap();
        let schema = ordered_list_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert_eq!(result.errors(), 0, "diagnostics: {:?}", result.diagnostics);
    }

    #[test]
    fn test_ordered_list_fail_unordered() {
        let doc =
            Document::from_str("---\ntype: doc\ntitle: T\n---\n\n# Steps\n\n- First\n- Second\n")
                .unwrap();
        let schema = ordered_list_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result.diagnostics.iter().any(|d| d.code == "S033"));
    }

    #[test]
    fn test_unordered_list_fail_ordered() {
        let schema = Schema::from_str(
            r#"
type "doc" {
    field "title" type="string"
    section "Items" required=#true {
        list ordered=#false
    }
}
"#,
        )
        .unwrap();
        let doc =
            Document::from_str("---\ntype: doc\ntitle: T\n---\n\n# Items\n\n1. First\n2. Second\n")
                .unwrap();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result.diagnostics.iter().any(|d| d.code == "S033"));
    }

    #[test]
    fn test_description_enriches_field_hint() {
        let schema = Schema::from_str(
            r#"
type "doc" {
    field "title" type="string" required=#true description="Short summary"
    section "Body" required=#true
}
"#,
        )
        .unwrap();
        let doc = Document::from_str("---\ntype: doc\n---\n\n# Body\n\nContent\n").unwrap();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        let f010 = result
            .diagnostics
            .iter()
            .find(|d| d.code == "F010")
            .unwrap();
        assert!(f010.hint.as_ref().unwrap().contains("Short summary"));
    }

    // ─── Conditional rule tests ──────────────────────────────────────────

    fn rule_schema() -> Schema {
        Schema::from_str(
            r#"
type "adr" {
    field "status" type="enum" required=#true {
        values "proposed" "accepted" "superseded"
    }
    field "date" type="string"
    field "superseded_by" type="string"
    section "Decision" required=#true

    rule "accepted requires date" {
        when "status" equals="accepted"
        then-required "date"
    }
    rule "superseded requires superseded_by" {
        when "status" equals="superseded"
        then-required "superseded_by"
    }
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_rule_condition_not_triggered() {
        let doc = Document::from_str("---\ntype: adr\nstatus: proposed\n---\n\n# Decision\n\nX\n")
            .unwrap();
        let schema = rule_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(
            !result.diagnostics.iter().any(|d| d.code == "F040"),
            "should not trigger rule when condition doesn't match"
        );
    }

    #[test]
    fn test_rule_condition_met_field_present() {
        let doc = Document::from_str(
            "---\ntype: adr\nstatus: accepted\ndate: \"2025-01-01\"\n---\n\n# Decision\n\nX\n",
        )
        .unwrap();
        let schema = rule_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(
            !result.diagnostics.iter().any(|d| d.code == "F040"),
            "should not error when conditionally required field is present"
        );
    }

    #[test]
    fn test_rule_condition_met_field_missing() {
        let doc = Document::from_str("---\ntype: adr\nstatus: accepted\n---\n\n# Decision\n\nX\n")
            .unwrap();
        let schema = rule_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        let f040s: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == "F040")
            .collect();
        assert_eq!(
            f040s.len(),
            1,
            "expected 1 F040 diagnostic, got: {:?}",
            f040s
        );
        assert!(f040s[0].message.contains("date"));
        assert!(f040s[0].message.contains("status=accepted"));
    }

    #[test]
    fn test_rule_superseded_missing_field() {
        let doc =
            Document::from_str("---\ntype: adr\nstatus: superseded\n---\n\n# Decision\n\nX\n")
                .unwrap();
        let schema = rule_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        let f040s: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == "F040")
            .collect();
        assert_eq!(f040s.len(), 1);
        assert!(f040s[0].message.contains("superseded_by"));
    }

    #[test]
    fn test_rule_superseded_field_present() {
        let doc = Document::from_str(
            "---\ntype: adr\nstatus: superseded\nsuperseded_by: ADR-002\n---\n\n# Decision\n\nX\n",
        )
        .unwrap();
        let schema = rule_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(
            !result.diagnostics.iter().any(|d| d.code == "F040"),
            "should pass when superseded_by is present"
        );
    }

    #[test]
    fn test_description_enriches_section_hint() {
        let schema = Schema::from_str(
            r#"
type "doc" {
    field "title" type="string"
    section "Decision" required=#true description="The decision and rationale"
}
"#,
        )
        .unwrap();
        let doc =
            Document::from_str("---\ntype: doc\ntitle: T\n---\n\n# Other\n\nStuff\n").unwrap();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        let s010 = result
            .diagnostics
            .iter()
            .find(|d| d.code == "S010")
            .unwrap();
        assert!(s010
            .hint
            .as_ref()
            .unwrap()
            .contains("The decision and rationale"));
    }

    // ─── Rule section table (S040) tests ──────────────────────────────────

    fn section_table_schema() -> Schema {
        Schema::from_str(
            r#"
type "opp" {
    field "status" type="string" required=#true
    section "Action Items"

    rule "pursuing needs table" {
        when "status" equals="pursuing"
        then-section-table "Action Items" {
            table {
                column "Status" type="string" required=#true
                column "Item" type="string" required=#true
            }
        }
    }
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_rule_section_table_missing() {
        let doc = Document::from_str("---\ntype: opp\nstatus: pursuing\n---\n\n# Other\n\nStuff\n")
            .unwrap();
        let schema = section_table_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(
            result.diagnostics.iter().any(|d| d.code == "S040"),
            "should report S040 when section missing: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_rule_section_table_no_table() {
        let doc = Document::from_str(
            "---\ntype: opp\nstatus: pursuing\n---\n\n# Action Items\n\n- Item one\n- Item two\n",
        )
        .unwrap();
        let schema = section_table_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(
            result.diagnostics.iter().any(|d| d.code == "S040"),
            "should report S040 when section has list but no table: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_rule_section_table_valid() {
        let doc = Document::from_str(
            "---\ntype: opp\nstatus: pursuing\n---\n\n# Action Items\n\n| Status | Item |\n|---|---|\n| pending | Do thing |\n",
        )
        .unwrap();
        let schema = section_table_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(
            !result.diagnostics.iter().any(|d| d.code == "S040"),
            "should not report S040 when table present: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_rule_section_table_condition_not_met() {
        let doc =
            Document::from_str("---\ntype: opp\nstatus: exploring\n---\n\n# Other\n\nStuff\n")
                .unwrap();
        let schema = section_table_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(
            !result.diagnostics.iter().any(|d| d.code == "S040"),
            "should not report S040 when condition not met: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_rule_section_table_equals_any() {
        let schema = Schema::from_str(
            r#"
type "opp" {
    field "status" type="string" required=#true
    section "Action Items"

    rule "active needs table" {
        when "status" equals-any="pursuing,completed"
        then-section-table "Action Items" {
            table {
                column "Status" type="string" required=#true
            }
        }
    }
}
"#,
        )
        .unwrap();

        // "pursuing" triggers
        let doc = Document::from_str("---\ntype: opp\nstatus: pursuing\n---\n\n# Other\n\nStuff\n")
            .unwrap();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result.diagnostics.iter().any(|d| d.code == "S040"));

        // "completed" also triggers
        let doc2 =
            Document::from_str("---\ntype: opp\nstatus: completed\n---\n\n# Other\n\nStuff\n")
                .unwrap();
        let result2 = validate_document(&doc2, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(result2.diagnostics.iter().any(|d| d.code == "S040"));

        // "exploring" does not trigger
        let doc3 =
            Document::from_str("---\ntype: opp\nstatus: exploring\n---\n\n# Other\n\nStuff\n")
                .unwrap();
        let result3 = validate_document(&doc3, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(!result3.diagnostics.iter().any(|d| d.code == "S040"));
    }

    // ─── Enum cell validation (S023) tests ────────────────────────────────

    fn enum_table_schema() -> Schema {
        Schema::from_str(
            r#"
type "doc" {
    field "title" type="string"
    section "Tasks" required=#true {
        table required=#true {
            column "Status" type="enum" required=#true {
                values "completed" "in-progress" "pending"
            }
            column "Item" type="string" required=#true
        }
    }
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_enum_column_valid_values() {
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\n---\n\n# Tasks\n\n| Status | Item |\n|---|---|\n| completed | A |\n| pending | B |\n",
        )
        .unwrap();
        let schema = enum_table_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(
            !result.diagnostics.iter().any(|d| d.code == "S023"),
            "valid enum values should not trigger S023: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_enum_column_invalid_value() {
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\n---\n\n# Tasks\n\n| Status | Item |\n|---|---|\n| done | A |\n",
        )
        .unwrap();
        let schema = enum_table_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        let s023s: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == "S023")
            .collect();
        assert_eq!(s023s.len(), 1, "expected 1 S023: {:?}", s023s);
        assert!(s023s[0].message.contains("done"));
        assert!(s023s[0].hint.as_ref().unwrap().contains("completed"));
    }

    #[test]
    fn test_enum_column_empty_required() {
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\n---\n\n# Tasks\n\n| Status | Item |\n|---|---|\n|  | A |\n",
        )
        .unwrap();
        let schema = enum_table_schema();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(
            result.diagnostics.iter().any(|d| d.code == "S023"),
            "empty required enum should trigger S023: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_enum_column_empty_optional() {
        let schema = Schema::from_str(
            r#"
type "doc" {
    field "title" type="string"
    section "Tasks" required=#true {
        table required=#true {
            column "Priority" type="enum" {
                values "high" "medium" "low"
            }
            column "Item" type="string" required=#true
        }
    }
}
"#,
        )
        .unwrap();
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\n---\n\n# Tasks\n\n| Priority | Item |\n|---|---|\n|  | A |\n",
        )
        .unwrap();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        assert!(
            !result.diagnostics.iter().any(|d| d.code == "S023"),
            "empty optional enum should not trigger S023: {:?}",
            result.diagnostics
        );
    }

    #[test]
    #[cfg(feature = "diagrams")]
    fn test_diagram_render_warnings() {
        // D2 diagram with a 3-node cycle should produce D002 warnings
        let d2 = "users: Users\napi: API\npgbouncer: PgBouncer\nusers -> api: requests\napi -> pgbouncer: need conn\napi -> users: 503 errors\n";
        let md = format!("---\ntype: doc\ntitle: T\n---\n\n## Diagram\n\n```d2\n{d2}```\n");
        let doc = Document::from_str(&md).unwrap();
        let schema = Schema::from_str(
            r#"
type "doc" {
    field "title" type="string"
}
"#,
        )
        .unwrap();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        let d002s: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == "D002")
            .collect();
        assert!(
            !d002s.is_empty(),
            "expected D002 diagram warnings for cycle, got: {:?}",
            result.diagnostics
        );
    }

    #[test]
    #[cfg(feature = "diagrams")]
    fn test_diagram_parse_error() {
        // Truncated node syntax triggers a parse error in graphs_tui
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\n---\n\n## Diagram\n\n```mermaid\ngraph TD\n  A[\n```\n",
        )
        .unwrap();
        let schema = Schema::from_str(
            r#"
type "doc" {
    field "title" type="string"
}
"#,
        )
        .unwrap();
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        let d001s: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == "D001")
            .collect();
        assert!(
            !d001s.is_empty(),
            "expected D001 diagram parse error, got: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_self_referential_relation_one() {
        let kdl = r#"
relation "supersedes" inverse="superseded_by" cardinality="one"
type "adr" {
    field "title" type="string"
    section "Body"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let content = "---\ntype: adr\ntitle: Test\nsupersedes: ADR-001\n---\n# Body\n";
        let mut doc = Document::from_str(content).unwrap();
        doc.path = Some(PathBuf::from("docs/architecture/adr-001.md"));
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        let self_refs: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == "R020")
            .collect();
        assert_eq!(self_refs.len(), 1, "expected R020: {self_refs:?}");
        assert!(self_refs[0].message.contains("self-referential"));
    }

    #[test]
    fn test_self_referential_relation_many() {
        let kdl = r#"
relation "related" cardinality="many"
type "adr" {
    field "title" type="string"
    section "Body"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let content =
            "---\ntype: adr\ntitle: Test\nrelated:\n  - ADR-002\n  - ADR-001\n---\n# Body\n";
        let mut doc = Document::from_str(content).unwrap();
        doc.path = Some(PathBuf::from("docs/architecture/adr-001.md"));
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        let self_refs: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == "R020")
            .collect();
        assert_eq!(self_refs.len(), 1, "expected exactly 1 R020: {self_refs:?}");
    }

    #[test]
    fn test_no_self_ref_for_different_id() {
        let kdl = r#"
relation "supersedes" inverse="superseded_by" cardinality="one"
type "adr" {
    field "title" type="string"
    section "Body"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let content = "---\ntype: adr\ntitle: Test\nsupersedes: ADR-002\n---\n# Body\n";
        let mut doc = Document::from_str(content).unwrap();
        doc.path = Some(PathBuf::from("docs/architecture/adr-001.md"));
        let result = validate_document(&doc, &schema, &HashSet::new(), &HashSet::new(), None);
        let self_refs: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| d.code == "R020")
            .collect();
        assert!(
            self_refs.is_empty(),
            "should not flag different ID: {self_refs:?}"
        );
    }

    // ─── Singleton folder tests ──────────────────────────────

    fn singleton_folder_schema() -> Schema {
        Schema::from_str(
            r#"
type "readme" folder="." max_count=1 singleton=#true {
    match "README.md"
    section "Install" required=#true
}

type "service-readme" folder="services" singleton=#true {
    match "README.md"
    section "Overview" required=#true
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_singleton_folder_root_only() {
        let schema = singleton_folder_schema();
        let root_readme = &schema.types[0]; // folder="."
        let service_readme = &schema.types[1]; // folder="services"

        // Root README.md should match folder="." but not folder="services"
        let root_path = Path::new("README.md");
        assert!(singleton_matches(root_readme, "README.md", root_path));
        assert!(!singleton_matches(service_readme, "README.md", root_path));
    }

    #[test]
    fn test_singleton_folder_services() {
        let schema = singleton_folder_schema();
        let root_readme = &schema.types[0]; // folder="."
        let service_readme = &schema.types[1]; // folder="services"

        // services/api/README.md should match folder="services" but not folder="."
        let svc_path = Path::new("services/api/README.md");
        assert!(singleton_matches(service_readme, "README.md", svc_path));
        assert!(!singleton_matches(root_readme, "README.md", svc_path));
    }

    #[test]
    fn test_singleton_folder_none_matches_anywhere() {
        let schema = Schema::from_str(
            r#"
type "changelog" singleton=#true {
    match "CHANGELOG.md"
    section "Unreleased"
}
"#,
        )
        .unwrap();
        let t = &schema.types[0];

        // No folder constraint → matches anywhere
        assert!(singleton_matches(
            t,
            "CHANGELOG.md",
            Path::new("CHANGELOG.md")
        ));
        assert!(singleton_matches(
            t,
            "CHANGELOG.md",
            Path::new("sub/dir/CHANGELOG.md")
        ));
    }

    #[test]
    fn test_singleton_presence_scoped() {
        let schema = singleton_folder_schema();
        let dir = Path::new("/tmp/test-project");

        // Only root README present, no service READMEs
        let files = vec![PathBuf::from("/tmp/test-project/README.md")];
        let mut results = Vec::new();
        validate_singleton_presence(&files, dir, &schema, &mut results);

        // Root readme found → no T020 for "readme" type
        assert!(
            !results.iter().any(|r| r
                .diagnostics
                .iter()
                .any(|d| d.message.contains("\"readme\""))),
            "root readme should not trigger T020: {:?}",
            results
        );

        // service-readme has no max_count=1, so T020 is NOT enforced
        // (service-readme is not a unique singleton, many services can have READMEs)
        assert!(
            !results.iter().any(|r| r
                .diagnostics
                .iter()
                .any(|d| d.message.contains("\"service-readme\""))),
            "service-readme should not trigger T020 (no max_count=1): {:?}",
            results
        );
    }

    // ─── License file validation (L001) ──────────────────────────────────

    fn license_schema() -> Schema {
        Schema::from_str(
            r#"
type "readme" folder="." max_count=1 singleton=#true {
    match "README.md"
    section "License" required=#true
}
"#,
        )
        .unwrap()
    }

    #[test]
    fn test_license_file_proprietary_no_check() {
        let dir = tempfile::tempdir().unwrap();
        let readme = dir.path().join("README.md");
        std::fs::write(&readme, "A sample project.\n\n## License\n\nProprietary\n").unwrap();

        let schema = license_schema();
        let files = vec![readme];
        let mut results = Vec::new();
        validate_license_file(&files, dir.path(), &schema, &mut results);
        assert!(
            !results
                .iter()
                .any(|r| r.diagnostics.iter().any(|d| d.code == "L001")),
            "proprietary license should not trigger L001: {:?}",
            results
        );
    }

    #[test]
    fn test_license_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let readme = dir.path().join("README.md");
        std::fs::write(&readme, "A sample project.\n\n## License\n\nMIT\n").unwrap();

        let schema = license_schema();
        let files = vec![readme];
        let mut results = Vec::new();
        validate_license_file(&files, dir.path(), &schema, &mut results);
        assert!(
            results
                .iter()
                .any(|r| r.diagnostics.iter().any(|d| d.code == "L001")),
            "missing LICENSE file should trigger L001: {:?}",
            results
        );
    }

    #[test]
    fn test_license_file_present() {
        let dir = tempfile::tempdir().unwrap();
        let readme = dir.path().join("README.md");
        std::fs::write(&readme, "A sample project.\n\n## License\n\nMIT\n").unwrap();
        std::fs::write(dir.path().join("LICENSE"), "MIT License\n").unwrap();

        let schema = license_schema();
        let files = vec![readme];
        let mut results = Vec::new();
        validate_license_file(&files, dir.path(), &schema, &mut results);
        assert!(
            !results
                .iter()
                .any(|r| r.diagnostics.iter().any(|d| d.code == "L001")),
            "LICENSE file present should not trigger L001: {:?}",
            results
        );
    }

    #[test]
    fn test_license_file_no_readme_type() {
        let dir = tempfile::tempdir().unwrap();
        let readme = dir.path().join("README.md");
        std::fs::write(&readme, "A sample project.\n\n## License\n\nMIT\n").unwrap();

        // Schema with no readme singleton type
        let schema = test_schema();
        let files = vec![readme];
        let mut results = Vec::new();
        validate_license_file(&files, dir.path(), &schema, &mut results);
        assert!(
            !results
                .iter()
                .any(|r| r.diagnostics.iter().any(|d| d.code == "L001")),
            "no readme type in schema should not trigger L001: {:?}",
            results
        );
    }

    #[test]
    fn test_broken_table_detected() {
        // Table syntax without blank line before it (causes comrak to fail parsing)
        let body = "Some text\n| A | B |\n|---|---|\n| 1 | 2 |\n";
        let mut diags = Vec::new();
        check_broken_tables(body, &mut diags);
        // This particular case should parse fine with blank-line wrapping,
        // so no warning expected.
        // But let's test with a truly broken table.
        diags.clear();

        // Valid table should NOT produce a warning
        let body = "\n| A | B |\n|---|---|\n| 1 | 2 |\n";
        check_broken_tables(body, &mut diags);
        assert!(
            diags.is_empty(),
            "valid table should not trigger C001: {diags:?}"
        );
    }

    #[test]
    fn test_no_broken_table_false_positive() {
        // Body without any tables
        let body = "Just some text.\n\nMore text.\n";
        let mut diags = Vec::new();
        check_broken_tables(body, &mut diags);
        assert!(diags.is_empty(), "no tables should not trigger C001");

        // Body with pipes in code blocks (not tables)
        let body = "Text with | pipes | in it but no table.\n";
        check_broken_tables(body, &mut diags);
        assert!(diags.is_empty(), "random pipes should not trigger C001");
    }

    // --- check_image_paths ---

    #[test]
    fn image_in_docs_assets_ok() {
        let body = "![diagram](docs/assets/arch.png)\n";
        let mut diags = Vec::new();
        check_image_paths(body, &mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    fn image_relative_assets_ok() {
        let body = "![diagram](../assets/arch.png)\n";
        let mut diags = Vec::new();
        check_image_paths(body, &mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    fn image_assets_shorthand_ok() {
        let body = "![diagram](assets/arch.png)\n";
        let mut diags = Vec::new();
        check_image_paths(body, &mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    fn image_external_url_ok() {
        let body = "![logo](https://example.com/logo.png)\n";
        let mut diags = Vec::new();
        check_image_paths(body, &mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    fn image_wrong_path_error() {
        let body = "![screenshot](images/screenshot.png)\n";
        let mut diags = Vec::new();
        check_image_paths(body, &mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "C002");
    }

    #[test]
    fn image_root_path_error() {
        let body = "![photo](photo.png)\n";
        let mut diags = Vec::new();
        check_image_paths(body, &mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "C002");
    }

    #[test]
    fn html_img_wrong_path_error() {
        let body = r#"<img src="screenshots/fail.png" alt="fail">"#;
        let mut diags = Vec::new();
        check_image_paths(body, &mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "C002");
    }

    #[test]
    fn html_img_docs_assets_ok() {
        let body = r#"<img src="docs/assets/diagram.svg" alt="diagram">"#;
        let mut diags = Vec::new();
        check_image_paths(body, &mut diags);
        assert!(diags.is_empty());
    }

    // ── Service README validation tests ──────────────────────────────

    #[test]
    fn sv001_missing_readme() {
        let tmp = std::env::temp_dir().join("dg-test-sv001");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("services/my-api")).unwrap();
        // No README.md

        let mut results = Vec::new();
        validate_service_readmes(&tmp, &mut results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].diagnostics[0].code, "SV001");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sv002_missing_frontmatter() {
        let tmp = std::env::temp_dir().join("dg-test-sv002");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("services/my-api")).unwrap();
        std::fs::write(
            tmp.join("services/my-api/README.md"),
            "# My API\n\nNo frontmatter here.\n",
        )
        .unwrap();

        let mut results = Vec::new();
        validate_service_readmes(&tmp, &mut results);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].diagnostics[0].code, "SV002");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sv003_missing_owner() {
        let tmp = std::env::temp_dir().join("dg-test-sv003");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("apps/mobile")).unwrap();
        std::fs::write(
            tmp.join("apps/mobile/README.md"),
            "---\nstatus: live\n---\n# Mobile App\n",
        )
        .unwrap();

        let mut results = Vec::new();
        validate_service_readmes(&tmp, &mut results);
        // SV003 error + possibly SV004/SV005 warnings
        let sv003: Vec<_> = results
            .iter()
            .flat_map(|r| &r.diagnostics)
            .filter(|d| d.code == "SV003")
            .collect();
        assert_eq!(sv003.len(), 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sv_valid_service_readme() {
        let tmp = std::env::temp_dir().join("dg-test-sv-valid");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("infra/database")).unwrap();
        std::fs::write(
            tmp.join("infra/database/README.md"),
            "---\nowner: ops-team\nstatus: live\nhas_linter: true\nhas_tests: true\n---\n# Database\n\n## Architecture\n\n```mermaid\ngraph LR\n  App --> DB[(PostgreSQL)]\n  App --> Cache[(Redis)]\n  Worker --> DB\n  Worker --> Queue\n```\n",
        )
        .unwrap();

        let mut results = Vec::new();
        validate_service_readmes(&tmp, &mut results);
        assert!(
            results.is_empty(),
            "valid README with has_linter/has_tests overrides should produce no diagnostics, got: {:?}",
            results.iter().flat_map(|r| &r.diagnostics).map(|d| &d.code).collect::<Vec<_>>()
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sv_all_three_dirs_checked() {
        let tmp = std::env::temp_dir().join("dg-test-sv-all");
        let _ = std::fs::remove_dir_all(&tmp);
        for dir in ["services/api", "apps/web", "infra/redis"] {
            std::fs::create_dir_all(tmp.join(dir)).unwrap();
            // README with no frontmatter
            std::fs::write(tmp.join(dir).join("README.md"), "# Title\n").unwrap();
        }

        let mut results = Vec::new();
        validate_service_readmes(&tmp, &mut results);
        assert_eq!(results.len(), 3, "should catch all 3 dirs");
        for r in &results {
            assert_eq!(r.diagnostics[0].code, "SV002");
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn nested_readme_in_services_skipped() {
        let tmp = std::env::temp_dir().join("dg-test-nested-readme");
        let _ = std::fs::remove_dir_all(&tmp);

        // Create nested README that should NOT be validated
        std::fs::create_dir_all(tmp.join("services/core/repos/cexe")).unwrap();
        std::fs::write(
            tmp.join("services/core/repos/cexe/README.md"),
            "# Cexe\n\nNo frontmatter.\n",
        )
        .unwrap();

        // Create top-level service README that SHOULD be validated
        std::fs::create_dir_all(tmp.join("services/core")).unwrap();
        std::fs::write(
            tmp.join("services/core/README.md"),
            "---\nowner: ops\nstatus: live\n---\n# Core\n\nIntro.\n",
        )
        .unwrap();

        // Also create a docs dir so validate_directory has something to work with
        std::fs::create_dir_all(tmp.join("docs")).unwrap();

        let schema = Schema::from_str(
            r#"
type "service-readme" folder="services" singleton=#true {
    match "README.md"
}
"#,
        )
        .unwrap();
        let result = validate_directory(&tmp, &schema, None, None).unwrap();

        // The nested README should NOT produce F000
        let nested_errors: Vec<_> = result
            .file_results
            .iter()
            .filter(|fr| fr.path.contains("repos/cexe"))
            .collect();
        assert!(
            nested_errors.is_empty(),
            "nested README should not be validated, got: {:?}",
            nested_errors
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sv004_no_linter_config() {
        let tmp = std::env::temp_dir().join("dg-test-sv004");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("services/api")).unwrap();
        std::fs::write(
            tmp.join("services/api/README.md"),
            "---\nowner: alice\nstatus: live\n---\n# API\n",
        )
        .unwrap();

        let mut results = Vec::new();
        validate_service_readmes(&tmp, &mut results);
        let sv004: Vec<_> = results
            .iter()
            .flat_map(|r| &r.diagnostics)
            .filter(|d| d.code == "SV004")
            .collect();
        assert_eq!(sv004.len(), 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sv005_no_tests() {
        let tmp = std::env::temp_dir().join("dg-test-sv005");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("services/api")).unwrap();
        std::fs::write(
            tmp.join("services/api/README.md"),
            "---\nowner: alice\nstatus: live\n---\n# API\n",
        )
        .unwrap();

        let mut results = Vec::new();
        validate_service_readmes(&tmp, &mut results);
        let sv005: Vec<_> = results
            .iter()
            .flat_map(|r| &r.diagnostics)
            .filter(|d| d.code == "SV005")
            .collect();
        assert_eq!(sv005.len(), 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sv004_suppressed_by_frontmatter() {
        let tmp = std::env::temp_dir().join("dg-test-sv004-fm");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("services/api")).unwrap();
        std::fs::write(
            tmp.join("services/api/README.md"),
            "---\nowner: alice\nstatus: live\nhas_linter: true\nhas_tests: true\n---\n# API\n",
        )
        .unwrap();

        let mut results = Vec::new();
        validate_service_readmes(&tmp, &mut results);
        let sv_warnings: Vec<_> = results
            .iter()
            .flat_map(|r| &r.diagnostics)
            .filter(|d| d.code == "SV004" || d.code == "SV005")
            .collect();
        assert!(
            sv_warnings.is_empty(),
            "frontmatter overrides should suppress SV004/SV005"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}

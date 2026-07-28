use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_yaml::Value;

use crate::frontmatter::Frontmatter;
use crate::schema::{Cardinality, FieldDef, FieldType, Schema, SectionDef, TypeDef};

/// Generate a markdown document from a schema type definition.
///
/// `fields` are user-supplied overrides as (key, raw_value_string) pairs.
/// If `fill` is true, date-pattern placeholders are replaced with real dates.
pub fn generate_document(
    type_def: &TypeDef,
    _schema: &Schema,
    fields: &[(String, String)],
) -> String {
    generate_document_opts(type_def, _schema, fields, &[], false)
}

/// Like `generate_document` but with `fill` option and relation values.
///
/// `rels` are (field_name, refs) pairs. Cardinality determines YAML shape:
/// `One` → single string, `Many` → sequence.
pub fn generate_document_opts(
    type_def: &TypeDef,
    schema: &Schema,
    fields: &[(String, String)],
    rels: &[(String, Vec<String>)],
    fill: bool,
) -> String {
    let overrides: BTreeMap<&str, &str> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    // Build frontmatter (no type or title — type is inferred from filename, title from H1)
    let mut data = BTreeMap::new();

    for field in &type_def.fields {
        let value = if let Some(&raw) = overrides.get(field.name.as_str()) {
            crate::frontmatter::parse_yaml_value(raw)
        } else {
            default_value(field, fill)
        };
        data.insert(field.name.clone(), value);
    }

    // Append relation fields
    for (name, refs) in rels {
        let cardinality = schema
            .relation_cardinality(name)
            .unwrap_or(Cardinality::Many);
        let value = match cardinality {
            Cardinality::One => Value::String(refs.first().cloned().unwrap_or_default()),
            Cardinality::Many => {
                Value::Sequence(refs.iter().map(|r| Value::String(r.clone())).collect())
            }
        };
        data.insert(name.clone(), value);
    }

    let fm = Frontmatter::from_data(data);
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&fm.to_yaml_string());
    out.push_str("---\n");

    // H1 title from override or placeholder
    let title = overrides.get("title").copied().unwrap_or("Untitled");
    out.push_str(&format!("\n# {title}\n\n"));
    out.push_str("<!-- Brief introduction: what this document is about -->\n");

    // Build body from sections
    for section in &type_def.sections {
        render_section(&mut out, section, 2);
    }

    out
}

/// Title -> filename slug, shared by `dg new` (CLI) and the `dg-new` MCP tool so
/// both derive identical filenames (e.g. `"Use PostgreSQL!"` -> `use-postgresql`).
pub fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn default_value(field_def: &FieldDef, fill: bool) -> Value {
    // Schema-defined default takes priority
    if let Some(ref default_str) = field_def.default {
        return expand_default(default_str);
    }

    // Check for date-like patterns
    if let Some(ref pat) = field_def.pattern {
        if pat.contains(r"\d{4}") && pat.contains(r"\d{2}") {
            if fill {
                // --fill: use real dates
                if pat.contains('T') {
                    return Value::String(format_now());
                }
                return Value::String(format_today());
            }
            if pat.contains('T') {
                return Value::String("YYYY-MM-DDT00:00:00Z".to_string());
            }
            return Value::String("YYYY-MM-DD".to_string());
        }
    }

    match &field_def.field_type {
        FieldType::String => Value::String(String::new()),
        FieldType::Number => Value::Number(0.into()),
        FieldType::Bool => Value::Bool(false),
        FieldType::Date => Value::String(format_today()),
        FieldType::Enum(values) => {
            if let Some(first) = values.first() {
                Value::String(first.clone())
            } else {
                Value::String(String::new())
            }
        }
        FieldType::User => Value::String(String::new()),
        FieldType::UserArray => Value::Sequence(vec![]),
        FieldType::Org => Value::String(String::new()),
        FieldType::OrgArray => Value::Sequence(vec![]),
        FieldType::Ref => Value::String(String::new()),
        FieldType::RefArray => Value::Sequence(vec![]),
        FieldType::StringArray => Value::Sequence(vec![]),
    }
}

fn expand_default(s: &str) -> Value {
    match s {
        "$TODAY" => Value::String(format_today()),
        "$NOW" => Value::String(format_now()),
        other => crate::frontmatter::parse_yaml_value(other),
    }
}

/// Format current date as YYYY-MM-DD without external crate.
fn format_today() -> String {
    let (year, month, day) = civil_date_from_epoch();
    format!("{year:04}-{month:02}-{day:02}")
}

/// Format current datetime as YYYY-MM-DDTHH:MM:SSZ without external crate.
fn format_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (year, month, day) = civil_date_from_epoch();
    let day_secs = (secs % 86400) as u32;
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Convert current unix timestamp to (year, month, day) in UTC.
fn civil_date_from_epoch() -> (i32, u32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (secs / 86400) as i64;
    // Algorithm from Howard Hinnant's chrono-compatible date library
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
    (y as i32, m, d)
}

fn render_section(out: &mut String, section: &SectionDef, depth: u8) {
    // Heading
    out.push('\n');
    for _ in 0..depth {
        out.push('#');
    }
    out.push(' ');
    out.push_str(&section.name);
    out.push('\n');
    out.push('\n');

    // Table scaffold if defined
    if let Some(ref table_def) = section.table {
        let headers: Vec<&str> = table_def.columns.iter().map(|c| c.name.as_str()).collect();
        out.push_str("| ");
        out.push_str(&headers.join(" | "));
        out.push_str(" |\n");

        out.push('|');
        for _ in &table_def.columns {
            out.push_str("---|");
        }
        out.push('\n');
    } else if section.children.is_empty() {
        // Emit description hint for leaf sections without tables
        if let Some(ref desc) = section.description {
            out.push_str(&format!("<!-- {desc} -->\n\n"));
        }
    }

    // Child sections
    for child in &section.children {
        render_section(out, child, depth + 1);
    }
}

/// Return the default value for a field as a plain string.
///
/// Returns `None` if the field has no meaningful default (e.g. user types, arrays).
/// Used by the autofix command to insert missing required fields.
pub fn field_default_string(field_def: &FieldDef) -> Option<String> {
    // Schema-defined default takes priority
    if let Some(ref default_str) = field_def.default {
        return Some(expand_default_string(default_str));
    }

    // Date-like patterns
    if let Some(ref pat) = field_def.pattern {
        if pat.contains(r"\d{4}") && pat.contains(r"\d{2}") {
            return Some(format_today());
        }
    }

    match &field_def.field_type {
        FieldType::String => None, // empty string is not useful
        FieldType::Number => Some("0".to_string()),
        FieldType::Bool => Some("false".to_string()),
        FieldType::Enum(values) => values.first().cloned(),
        _ => None, // user, ref, arrays — no sensible default
    }
}

/// Expand a schema default string to its final value.
fn expand_default_string(s: &str) -> String {
    match s {
        "$TODAY" => format_today(),
        "$NOW" => format_now(),
        other => other.to_string(),
    }
}

/// Compute Levenshtein edit distance between two strings.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a_len = a.len();
    let b_len = b.len();
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b_len]
}

/// Find closest match for `value` among `candidates` by Levenshtein distance.
/// Returns `None` if candidates is empty or best distance exceeds threshold.
pub fn closest_match<'a>(
    value: &str,
    candidates: &[&'a str],
    max_distance: usize,
) -> Option<&'a str> {
    candidates
        .iter()
        .map(|c| (*c, levenshtein(value, c)))
        .filter(|(_, d)| *d <= max_distance)
        .min_by_key(|(_, d)| *d)
        .map(|(c, _)| c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Schema;

    #[test]
    fn test_generate_minimal() {
        let kdl = r#"
type "test" {
    field "status" type="enum" required=#true {
        values "draft" "active"
    }
    section "Body" required=#true
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let type_def = schema.get_type("test").unwrap();
        let doc = generate_document(type_def, &schema, &[]);

        assert!(
            !doc.contains("type: test"),
            "type should not be in frontmatter"
        );
        assert!(doc.contains("# Untitled"), "should have H1 title");
        assert!(doc.contains("status: draft")); // first enum value
        assert!(doc.contains("## Body"), "sections should be H2");
    }

    #[test]
    fn test_generate_with_overrides() {
        let kdl = r#"
type "test" {
    field "status" type="enum" required=#true {
        values "draft" "active"
    }
    section "Body" required=#true
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let type_def = schema.get_type("test").unwrap();
        let fields = vec![
            ("title".to_string(), "My Title".to_string()),
            ("status".to_string(), "active".to_string()),
        ];
        let doc = generate_document(type_def, &schema, &fields);

        assert!(doc.contains("# My Title"), "title should be in H1");
        assert!(!doc.contains("title: My Title"), "title not in frontmatter");
        assert!(doc.contains("status: active"));
    }

    #[test]
    fn test_generate_with_table_scaffold() {
        let kdl = r#"
type "test" {
    section "Data" {
        table {
            column "Name" type="string" required=#true
            column "Score" type="number"
        }
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let type_def = schema.get_type("test").unwrap();
        let doc = generate_document(type_def, &schema, &[]);

        assert!(doc.contains("| Name | Score |"));
        assert!(doc.contains("|---|---|"));
    }

    #[test]
    fn test_generate_nested_sections() {
        let kdl = r#"
type "test" {
    section "Parent" {
        section "Child"
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let type_def = schema.get_type("test").unwrap();
        let doc = generate_document(type_def, &schema, &[]);

        assert!(doc.contains("## Parent"), "sections should be H2");
        assert!(doc.contains("### Child"), "child sections should be H3");
    }

    #[test]
    fn test_generate_date_pattern_default() {
        let kdl = r#"
type "test" {
    field "date" type="string" pattern="^\\d{4}-\\d{2}-\\d{2}$"
    field "timestamp" type="string" pattern="^\\d{4}-\\d{2}-\\d{2}T"
    section "Body"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let type_def = schema.get_type("test").unwrap();
        let doc = generate_document(type_def, &schema, &[]);

        assert!(doc.contains("YYYY-MM-DD"));
        assert!(doc.contains("YYYY-MM-DDT00:00:00Z"));
    }

    #[test]
    fn test_generate_full_schema() {
        let content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&content).unwrap();
        let type_def = schema.get_type("adr").unwrap();
        let fields = vec![
            ("title".to_string(), "Test ADR".to_string()),
            ("status".to_string(), "proposed".to_string()),
            ("author".to_string(), "onni".to_string()),
            ("date".to_string(), "2025-01-01".to_string()),
        ];
        let doc = generate_document(type_def, &schema, &fields);

        assert!(!doc.contains("type: adr"), "type not in frontmatter");
        assert!(doc.contains("# Test ADR"), "title in H1");
        assert!(doc.contains("status: proposed"));
        // serde_yaml may or may not quote strings; just check the values are present
        assert!(doc.contains("onni"));
        assert!(doc.contains("2025-01-01"));
        assert!(doc.contains("## Decision"), "sections are H2");
        assert!(doc.contains("## Consequences"));
        assert!(doc.contains("### Positive"), "subsections are H3");
    }

    #[test]
    fn test_schema_default_static() {
        let kdl = r#"
type "test" {
    field "status" type="enum" default="proposed" {
        values "proposed" "accepted"
    }
    section "Body"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let type_def = schema.get_type("test").unwrap();
        let doc = generate_document(type_def, &schema, &[]);
        assert!(doc.contains("status: proposed"));
    }

    #[test]
    fn test_schema_default_today() {
        let kdl = r#"
type "test" {
    field "date" type="string" default="$TODAY"
    section "Body"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let type_def = schema.get_type("test").unwrap();
        let doc = generate_document(type_def, &schema, &[]);
        // Should contain a real date like 2026-02-06, not placeholder
        let re = regex::Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap();
        assert!(re.is_match(&doc), "expected date pattern in: {doc}");
        assert!(
            !doc.contains("YYYY"),
            "should not contain placeholder: {doc}"
        );
    }

    #[test]
    fn test_schema_default_now() {
        let kdl = r#"
type "test" {
    field "ts" type="string" default="$NOW"
    section "Body"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let type_def = schema.get_type("test").unwrap();
        let doc = generate_document(type_def, &schema, &[]);
        let re = regex::Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z").unwrap();
        assert!(re.is_match(&doc), "expected ISO datetime in: {doc}");
    }

    #[test]
    fn test_section_description_hint() {
        let kdl = r#"
type "test" {
    section "Summary" required=#true description="Brief overview of what happened"
    section "Details" {
        section "Inner" description="Should not emit hint (parent has children)"
    }
    section "Data" description="Should not emit hint (has table)" {
        table {
            column "Key" type="string"
        }
    }
    section "Plain"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let type_def = schema.get_type("test").unwrap();
        let doc = generate_document(type_def, &schema, &[]);

        // Leaf section with description -> hint
        assert!(
            doc.contains("<!-- Brief overview of what happened -->"),
            "leaf section with description should have hint: {doc}"
        );
        // Section with table -> no hint (table is already a hint)
        assert!(
            !doc.contains("<!-- Should not emit hint (has table) -->"),
            "table section should not have hint: {doc}"
        );
        // Plain section without description -> no hint
        assert!(
            !doc.contains("<!-- -->"),
            "section without description should have no hint"
        );
    }

    #[test]
    fn test_override_beats_schema_default() {
        let kdl = r#"
type "test" {
    field "status" type="enum" default="proposed" {
        values "proposed" "accepted"
    }
    section "Body"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let type_def = schema.get_type("test").unwrap();
        let fields = vec![("status".to_string(), "accepted".to_string())];
        let doc = generate_document(type_def, &schema, &fields);
        assert!(doc.contains("status: accepted"));
    }

    #[test]
    fn test_civil_date_sanity() {
        // Just ensure it returns a plausible date
        let (y, m, d) = civil_date_from_epoch();
        assert!((2024..=2100).contains(&y));
        assert!((1..=12).contains(&m));
        assert!((1..=31).contains(&d));
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Use PostgreSQL!"), "use-postgresql");
        assert_eq!(
            slugify("SDK Architecture For Partner Integrations"),
            "sdk-architecture-for-partner-integrations"
        );
        // leading / trailing / consecutive separators collapse
        assert_eq!(slugify("  --Hello,, World--  "), "hello-world");
        assert_eq!(slugify("a.b/c"), "a-b-c");
        // path-traversal characters are neutralised to separators
        assert_eq!(slugify("../../etc/passwd"), "etc-passwd");
        // digits are kept
        assert_eq!(slugify("ADR 42 v2"), "adr-42-v2");
        // empty / punctuation-only / non-ASCII-only -> empty slug
        assert_eq!(slugify(""), "");
        assert_eq!(slugify("!!!"), "");
        assert_eq!(slugify("データベース移行"), "");
        assert_eq!(slugify("Привет"), "");
    }
}

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use clap::Args;
use md_db::graph::DocGraph;
use md_db::schema::{FieldType, Schema, TypeDef};
use md_db::template::{self, closest_match};
use md_db::users::OrgConfig;

#[derive(Args)]
pub struct NewArgs {
    /// Document type (e.g. adr, pol, opp, inc, spec, proc)
    #[arg(name = "TYPE")]
    pub doc_type: String,

    /// [TITLE] [relation ref...] [--field=value...] [--edit] [--fill]
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub rest: Vec<String>,
}

#[derive(Debug)]
struct ParsedArgs {
    title: Option<String>,
    fields: Vec<(String, String)>,
    rels: Vec<(String, Vec<String>)>,
    edit: bool,
    fill: bool,
}

/// Parse the trailing args using schema knowledge of field and relation names.
fn parse_rest_args(rest: &[String], schema: &Schema, type_name: &str) -> Result<ParsedArgs> {
    let type_def = schema.get_type(type_name);
    let field_names: Vec<&str> = type_def
        .map(|t| t.fields.iter().map(|f| f.name.as_str()).collect())
        .unwrap_or_default();
    let relation_names = schema.all_relation_field_names();

    let mut parsed = ParsedArgs {
        title: None,
        fields: Vec::new(),
        rels: Vec::new(),
        edit: false,
        fill: false,
    };

    let mut i = 0;
    while i < rest.len() {
        let arg = &rest[i];

        // --edit / --fill booleans
        if arg == "--edit" {
            parsed.edit = true;
            i += 1;
            continue;
        }
        if arg == "--fill" {
            parsed.fill = true;
            i += 1;
            continue;
        }

        // --key=value field
        if let Some(kv) = arg.strip_prefix("--") {
            if let Some((key, value)) = kv.split_once('=') {
                if !field_names.contains(&key) {
                    bail!(
                        "unknown field '{key}' for type '{type_name}'. Known fields: {}",
                        field_names.join(", ")
                    );
                }
                parsed.fields.push((key.to_string(), value.to_string()));
                i += 1;
                continue;
            }
            // --key value (two-arg field)
            let key = kv;
            if i + 1 >= rest.len() {
                bail!("--{key} requires a value");
            }
            if !field_names.contains(&key) {
                eprintln!("warning: unknown field '{key}'");
            }
            let value = &rest[i + 1];
            parsed.fields.push((key.to_string(), value.clone()));
            i += 2;
            continue;
        }

        // Bare word matching a relation name → consume next arg as refs
        if relation_names.contains(&arg.as_str()) {
            if i + 1 >= rest.len() {
                bail!("relation '{arg}' requires at least one ref (e.g. {arg} ADR-001)");
            }
            let refs_str = &rest[i + 1];
            let refs: Vec<String> = refs_str.split(',').map(|s| s.trim().to_string()).collect();
            parsed.rels.push((arg.clone(), refs));
            i += 2;
            continue;
        }

        // First unrecognized bare word → title
        if parsed.title.is_none() {
            parsed.title = Some(arg.clone());
            i += 1;
            continue;
        }

        // Second unrecognized bare word → error (likely a typo'd relation name)
        let hint = if let Some(suggestion) = closest_match(arg, &relation_names, 3) {
            format!("did you mean '{suggestion}'?")
        } else {
            format!("known relations: {}", relation_names.join(", "))
        };
        bail!("unknown relation '{arg}'\n{hint}");
    }

    Ok(parsed)
}

/// Warn if any field values don't match their enum's allowed values.
fn warn_invalid_enums(fields: &[(String, String)], type_def: &TypeDef) {
    for (key, value) in fields {
        if let Some(field) = type_def.fields.iter().find(|f| f.name == *key) {
            if let FieldType::Enum(ref allowed) = field.field_type {
                if !allowed.contains(value) {
                    eprintln!(
                        "warning: invalid value '{value}' for '{key}', allowed: {}",
                        allowed.join(", ")
                    );
                }
            }
        }
    }
}

/// Warn if any user-typed field values don't exist in org.kdl.
fn warn_unknown_users(fields: &[(String, String)], type_def: &TypeDef, org: Option<&OrgConfig>) {
    let org = match org {
        Some(o) => o,
        None => return,
    };
    for (key, value) in fields {
        if let Some(field) = type_def.fields.iter().find(|f| f.name == *key) {
            let is_user_field = matches!(field.field_type, FieldType::User | FieldType::UserArray);
            if is_user_field {
                let handle = value.strip_prefix('@').unwrap_or(value);
                if !org.users.contains_key(handle) {
                    eprintln!(
                        "warning: unknown user '{value}' for '{key}' — register with `dg team add-user {handle}`"
                    );
                }
            }
        }
    }
}

/// Warn if any user-supplied date fields are in the future.
fn warn_future_dates(fields: &[(String, String)], type_def: &TypeDef) {
    let today = today_epoch_days();
    for (key, value) in fields {
        // Check if this field has a date pattern in the schema
        let is_date_field = type_def.fields.iter().any(|f| {
            f.name == *key
                && f.pattern
                    .as_ref()
                    .is_some_and(|p| p.contains(r"\d{4}") && p.contains(r"\d{2}"))
        });
        if !is_date_field {
            continue;
        }
        // Parse YYYY-MM-DD prefix
        if let Some(days) = parse_date_to_epoch_days(value) {
            if days > today {
                eprintln!("warning: '{key}' is set to a future date ({value})");
            }
        }
    }
}

/// Parse "YYYY-MM-DD..." to epoch days, returns None if unparsable.
fn parse_date_to_epoch_days(s: &str) -> Option<i64> {
    let date_part = if s.len() >= 10 { &s[..10] } else { s };
    let mut parts = date_part.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(civil_to_epoch_days(y, m, d))
}

/// Convert (year, month, day) to epoch days using Howard Hinnant's algorithm.
fn civil_to_epoch_days(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe as i64 - 719468
}

/// Today as epoch days.
fn today_epoch_days() -> i64 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    (secs / 86400) as i64
}

pub fn run(
    root: &Path,
    schema: &Schema,
    args: &NewArgs,
    cache: &mut md_db::cache::DocCache,
    org: Option<&OrgConfig>,
) -> Result<()> {
    let type_def = schema.get_type(&args.doc_type).ok_or_else(|| {
        let available: Vec<_> = schema
            .types
            .iter()
            .map(|t| {
                if t.aliases.is_empty() {
                    t.name.clone()
                } else {
                    format!("{} ({})", t.name, t.aliases.join(", "))
                }
            })
            .collect();
        anyhow::anyhow!(
            "unknown type '{}', available: {}",
            args.doc_type,
            available.join(", ")
        )
    })?;

    // Resolve alias to canonical type name for ID generation
    let canonical_type = &type_def.name;

    let parsed = parse_rest_args(&args.rest, schema, canonical_type)?;

    let mut fields = parsed.fields;

    // Title goes into H1 heading (not frontmatter), but pass via fields for template
    if let Some(title) = &parsed.title {
        if !fields.iter().any(|(k, _)| k == "title") {
            fields.push(("title".to_string(), title.clone()));
        }
    }

    // Build graph to get next ID and validate refs
    let graph = DocGraph::build_cached(root, schema, cache)
        .context("failed to build doc graph for auto-id")?;

    // Validate all relation refs point to existing documents
    for (rel_name, refs) in &parsed.rels {
        for r in refs {
            let id = r.to_uppercase();
            if !graph.nodes.contains_key(&id) {
                bail!("document {r} not found (referenced by '{rel_name}')");
            }
        }
    }

    // Warn if any date fields are set to future dates
    warn_future_dates(&fields, type_def);

    // Warn if any enum values are invalid
    warn_invalid_enums(&fields, type_def);

    // Warn if any user values are unknown
    warn_unknown_users(&fields, type_def, org);

    let next_id = graph.next_id(canonical_type);

    let folder = type_def.folder.as_deref().unwrap_or("docs");
    let slug = parsed
        .title
        .as_deref()
        .map(slugify_title)
        .unwrap_or_default();
    let filename = if slug.is_empty() {
        bail!("title is required (used for filename slug)");
    } else {
        format!("{}-{slug}.md", next_id.to_lowercase())
    };
    let path = root.join(folder).join(&filename);

    // Ensure parent dir exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
    }

    let content =
        template::generate_document_opts(type_def, schema, &fields, &parsed.rels, parsed.fill);
    std::fs::write(&path, &content)
        .with_context(|| format!("failed to write {}", path.display()))?;

    eprintln!("{next_id} -> {}", path.display());

    if parsed.edit {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        let status = std::process::Command::new(&editor)
            .arg(&path)
            .status()
            .with_context(|| format!("failed to open {editor}"))?;
        if !status.success() {
            bail!("editor exited with {status}");
        }
    }

    Ok(())
}

/// Convert a title to a URL-friendly slug for filenames.
fn slugify_title(title: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_schema() -> Schema {
        let content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        Schema::from_str(&content).unwrap()
    }

    #[test]
    fn test_parse_title_only() {
        let schema = fixture_schema();
        let rest = vec!["Use Postgresql".to_string()];
        let parsed = parse_rest_args(&rest, &schema, "adr").unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Use Postgresql"));
        assert!(parsed.fields.is_empty());
        assert!(parsed.rels.is_empty());
        assert!(!parsed.edit);
        assert!(!parsed.fill);
    }

    #[test]
    fn test_parse_title_with_field() {
        let schema = fixture_schema();
        let rest = vec![
            "Use Postgresql".to_string(),
            "--status=accepted".to_string(),
        ];
        let parsed = parse_rest_args(&rest, &schema, "adr").unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Use Postgresql"));
        assert_eq!(parsed.fields, vec![("status".into(), "accepted".into())]);
    }

    #[test]
    fn test_parse_unknown_relation_errors() {
        let schema = fixture_schema();
        let rest = vec![
            "Use Postgresql".to_string(),
            "implements".to_string(),
            "OPP-001".to_string(),
        ];
        // "implements" is not in the fixture schema
        let result = parse_rest_args(&rest, &schema, "adr");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unknown relation 'implements'"));
    }

    #[test]
    fn test_parse_known_relation() {
        let schema = fixture_schema();
        let rest = vec![
            "Use Postgresql".to_string(),
            "supersedes".to_string(),
            "ADR-001".to_string(),
        ];
        let parsed = parse_rest_args(&rest, &schema, "adr").unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Use Postgresql"));
        assert_eq!(parsed.rels.len(), 1);
        assert_eq!(parsed.rels[0].0, "supersedes");
        assert_eq!(parsed.rels[0].1, vec!["ADR-001"]);
    }

    #[test]
    fn test_parse_relation_multi_ref() {
        let schema = fixture_schema();
        let rest = vec![
            "Test".to_string(),
            "related".to_string(),
            "OPP-001,OPP-002".to_string(),
        ];
        let parsed = parse_rest_args(&rest, &schema, "adr").unwrap();
        assert_eq!(parsed.rels[0].1, vec!["OPP-001", "OPP-002"]);
    }

    #[test]
    fn test_parse_edit_and_fill() {
        let schema = fixture_schema();
        let rest = vec![
            "--fill".to_string(),
            "Test".to_string(),
            "--edit".to_string(),
        ];
        let parsed = parse_rest_args(&rest, &schema, "adr").unwrap();
        assert!(parsed.edit);
        assert!(parsed.fill);
        assert_eq!(parsed.title.as_deref(), Some("Test"));
    }

    #[test]
    fn test_parse_field_two_arg() {
        let schema = fixture_schema();
        let rest = vec![
            "Test".to_string(),
            "--status".to_string(),
            "accepted".to_string(),
        ];
        let parsed = parse_rest_args(&rest, &schema, "adr").unwrap();
        assert_eq!(parsed.fields, vec![("status".into(), "accepted".into())]);
    }

    #[test]
    fn test_parse_no_title() {
        let schema = fixture_schema();
        let rest = vec!["--fill".to_string()];
        let parsed = parse_rest_args(&rest, &schema, "opp").unwrap();
        assert!(parsed.title.is_none());
        assert!(parsed.fill);
    }

    #[test]
    fn test_parse_unknown_bare_word_errors() {
        let schema = fixture_schema();
        let rest = vec!["Test".to_string(), "bogus".to_string(), "FOO".to_string()];
        let result = parse_rest_args(&rest, &schema, "adr");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unknown relation 'bogus'"));
    }

    #[test]
    fn test_parse_relation_before_title() {
        let schema = fixture_schema();
        let rest = vec!["supersedes".to_string(), "ADR-001".to_string()];
        let parsed = parse_rest_args(&rest, &schema, "adr").unwrap();
        assert!(parsed.title.is_none());
        assert_eq!(parsed.rels[0].0, "supersedes");
        assert_eq!(parsed.rels[0].1, vec!["ADR-001"]);
    }

    #[test]
    fn test_parse_mixed_order() {
        let schema = fixture_schema();
        let rest = vec![
            "--status=accepted".to_string(),
            "Use Postgresql".to_string(),
            "enables".to_string(),
            "OPP-001,OPP-002".to_string(),
            "--edit".to_string(),
        ];
        let parsed = parse_rest_args(&rest, &schema, "adr").unwrap();
        assert_eq!(parsed.title.as_deref(), Some("Use Postgresql"));
        assert_eq!(parsed.fields, vec![("status".into(), "accepted".into())]);
        assert_eq!(parsed.rels[0].0, "enables");
        assert_eq!(parsed.rels[0].1, vec!["OPP-001", "OPP-002"]);
        assert!(parsed.edit);
    }

    #[test]
    fn test_parse_inverse_relation() {
        let schema = fixture_schema();
        let rest = vec![
            "Test".to_string(),
            "superseded_by".to_string(),
            "ADR-002".to_string(),
        ];
        let parsed = parse_rest_args(&rest, &schema, "adr").unwrap();
        assert_eq!(parsed.rels[0].0, "superseded_by");
    }

    #[test]
    fn test_parse_date_to_epoch_days() {
        // 1970-01-01 = day 0
        assert_eq!(parse_date_to_epoch_days("1970-01-01"), Some(0));
        // 2025-01-01
        assert!(parse_date_to_epoch_days("2025-01-01").unwrap() > 0);
        // Datetime strings (YYYY-MM-DDT...) should parse the date prefix
        assert!(parse_date_to_epoch_days("2025-06-15T12:00:00Z").is_some());
        // Invalid
        assert!(parse_date_to_epoch_days("not-a-date").is_none());
        assert!(parse_date_to_epoch_days("2025-13-01").is_none());
        assert!(parse_date_to_epoch_days("2025-00-01").is_none());
    }

    #[test]
    fn test_civil_to_epoch_days_roundtrip() {
        // 1970-01-01 should be 0
        assert_eq!(civil_to_epoch_days(1970, 1, 1), 0);
        // 2000-01-01 should be 10957
        assert_eq!(civil_to_epoch_days(2000, 1, 1), 10957);
    }

    #[test]
    fn test_today_epoch_days_is_reasonable() {
        let today = today_epoch_days();
        // Should be after 2024-01-01 (epoch day ~19723)
        assert!(today > 19723);
    }

    #[test]
    fn test_parse_unknown_field_errors() {
        let schema = fixture_schema();
        let rest = vec!["Test".to_string(), "--bogus=field".to_string()];
        let result = parse_rest_args(&rest, &schema, "adr");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unknown field 'bogus'"));
        assert!(msg.contains("Known fields:"));
    }

    #[test]
    fn test_warn_invalid_enums_no_panic() {
        let schema = fixture_schema();
        let type_def = schema.get_type("adr").unwrap();
        // Valid value — should not panic or print
        warn_invalid_enums(&[("status".into(), "accepted".into())], type_def);
        // Invalid value — should warn (writes to stderr, no crash)
        warn_invalid_enums(&[("status".into(), "open".into())], type_def);
    }

    #[test]
    fn test_warn_unknown_users_no_panic() {
        let schema = fixture_schema();
        let type_def = schema.get_type("adr").unwrap();
        // No org config — should not panic
        warn_unknown_users(&[("author".into(), "anyone".into())], type_def, None);
    }
}

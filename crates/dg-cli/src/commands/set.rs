use std::path::Path;

use anyhow::{bail, Context, Result};
use clap::Args;
use md_db::document::Document;
use md_db::graph;
use md_db::schema::Schema;

#[derive(Args)]
pub struct SetArgs {
    /// Document ID (e.g. ADR-001) or file path
    #[arg(name = "ID")]
    pub id: String,

    /// Field assignments: key=value (set) or key+=value (append)
    pub assignments: Vec<String>,

    /// Remove frontmatter field(s)
    #[arg(long)]
    pub remove: Vec<String>,

    /// Target section for --content/--append/--add-row
    #[arg(long)]
    pub section: Option<String>,

    /// Replace section content (requires --section)
    #[arg(long)]
    pub content: Option<String>,

    /// Read section content from file (requires --section)
    #[arg(long)]
    pub content_file: Option<std::path::PathBuf>,

    /// Append text to section (requires --section)
    #[arg(long)]
    pub append: Option<String>,

    /// Add comma-separated row to first table in section (requires --section)
    #[arg(long)]
    pub add_row: Option<String>,

    /// Preview without writing
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(root: &Path, schema: &Schema, args: &SetArgs) -> Result<()> {
    let path = super::show::resolve_id_to_path(root, schema, &args.id)?;
    let original = std::fs::read_to_string(&path)?;
    let mut doc = Document::from_file(&path)?;
    let doc_id = graph::path_to_id(&path);

    // Field assignments
    for a in &args.assignments {
        if let Some((k, v)) = a.split_once("+=") {
            // Warn if schema says this is a scalar (non-array) field or relation
            if let Some(rel) = schema.find_relation(k) {
                if rel.0.cardinality == md_db::schema::Cardinality::One {
                    eprintln!(
                        "warning: '{k}' is a single-ref relation — use '{k}={v}' instead of '{k}+={v}'"
                    );
                }
            } else if let Some(type_def) = schema.get_type_for_doc_id(&doc_id) {
                if let Some(field) = type_def.fields.iter().find(|f| f.name == k) {
                    if !field.field_type.is_array() {
                        eprintln!(
                            "warning: '{k}' is a scalar field — use '{k}={v}' instead of '{k}+={v}'"
                        );
                    }
                }
            }
            doc.append_field_from_str(k, v);
            eprintln!("{doc_id}: {k}+={v}");
        } else if let Some((k, v)) = a.split_once('=') {
            doc.set_field_from_str(k, v);
            eprintln!("{doc_id}: {k}={v}");
        } else {
            bail!("invalid assignment: {a} (expected key=value or key+=value)");
        }
    }

    // --remove
    for key in &args.remove {
        doc.remove_field(key);
        eprintln!("{doc_id}: removed {key}");
    }

    // Resolve content from --content or --content-file
    let content_text = match (&args.content, &args.content_file) {
        (Some(_), Some(_)) => bail!("cannot use both --content and --content-file"),
        (Some(text), None) => Some(text.clone()),
        (None, Some(path)) => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            Some(text)
        }
        (None, None) => None,
    };

    // Section operations (require --section)
    if let Some(ref heading) = args.section {
        if let Some(ref text) = content_text {
            doc.replace_section_content(heading, &format!("{text}\n"))?;
            eprintln!("{doc_id}: replaced section \"{heading}\"");
        }
        if let Some(ref text) = args.append {
            doc.append_to_section(heading, &format!("{text}\n"))?;
            eprintln!("{doc_id}: appended to \"{heading}\"");
        }
        if let Some(ref row) = args.add_row {
            let vals: Vec<String> = row.split(',').map(|s| s.trim().to_string()).collect();
            doc.add_table_row(heading, 0, vals)?;
            eprintln!("{doc_id}: added row to \"{heading}\"");
        }
    }

    if doc.raw == original {
        eprintln!("{doc_id}: no changes");
        return Ok(());
    }

    if args.dry_run {
        eprintln!("{doc_id}: dry-run, not saved");
        print!("{}", doc.raw);
    } else {
        doc.save()?;
    }

    Ok(())
}

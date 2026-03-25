use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use md_db::schema::{FieldType, Schema};

#[derive(Args)]
pub struct SchemaArgs {
    /// Document type to show (e.g. adr, inc, opp, pol, spec, proc)
    #[arg(name = "TYPE")]
    pub doc_type: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn run(_root: &Path, schema: &Schema, args: &SchemaArgs) -> Result<()> {
    match &args.doc_type {
        Some(type_name) => show_type(schema, type_name, args.json),
        None => list_types(schema, args.json),
    }
}

fn list_types(schema: &Schema, json: bool) -> Result<()> {
    if json {
        let types: Vec<serde_json::Value> = schema
            .types
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "aliases": t.aliases,
                    "description": t.description,
                    "folder": t.folder,
                    "fields": t.fields.len(),
                    "sections": t.sections.len(),
                    "singleton": t.singleton,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&types)?);
        return Ok(());
    }

    println!("Document Types");
    println!("{}", "=".repeat(40));
    for t in &schema.types {
        let aliases = if t.aliases.is_empty() {
            String::new()
        } else {
            format!(" ({})", t.aliases.join(", "))
        };
        let desc = t
            .description
            .as_deref()
            .map(|d| format!(" — {d}"))
            .unwrap_or_default();
        println!("  {}{aliases}{desc}", t.name);
    }

    println!("\nRelations");
    println!("{}", "-".repeat(40));
    for r in &schema.relations {
        let card = match r.cardinality {
            md_db::schema::Cardinality::One => "one",
            md_db::schema::Cardinality::Many => "many",
        };
        let inv = r
            .inverse
            .as_deref()
            .map(|i| format!(" (inverse: {i})"))
            .unwrap_or_default();
        let desc = r
            .description
            .as_deref()
            .map(|d| format!(" — {d}"))
            .unwrap_or_default();
        println!("  {} [{card}]{inv}{desc}", r.name);
    }

    println!("\nUse `dg schema TYPE` for details on a specific type.");
    Ok(())
}

fn show_type(schema: &Schema, type_name: &str, json: bool) -> Result<()> {
    let type_def = schema
        .get_type(type_name)
        .with_context(|| format!("unknown type '{type_name}'"))?;

    if json {
        let fields: Vec<serde_json::Value> = type_def
            .fields
            .iter()
            .map(|f| {
                let mut obj = serde_json::json!({
                    "name": f.name,
                    "type": f.field_type.to_string(),
                    "required": f.required,
                });
                if let Some(ref desc) = f.description {
                    obj["description"] = serde_json::json!(desc);
                }
                if let Some(ref def) = f.default {
                    obj["default"] = serde_json::json!(def);
                }
                if let Some(ref pat) = f.pattern {
                    obj["pattern"] = serde_json::json!(pat);
                }
                if let FieldType::Enum(ref vals) = f.field_type {
                    obj["values"] = serde_json::json!(vals);
                }
                if !f.transitions.is_empty() {
                    let trans: Vec<serde_json::Value> = f
                        .transitions
                        .iter()
                        .map(|(from, tos)| serde_json::json!({"from": from, "to": tos}))
                        .collect();
                    obj["transitions"] = serde_json::json!(trans);
                }
                obj
            })
            .collect();

        let sections: Vec<serde_json::Value> =
            type_def.sections.iter().map(section_to_json).collect();

        let result = serde_json::json!({
            "name": type_def.name,
            "description": type_def.description,
            "aliases": type_def.aliases,
            "folder": type_def.folder,
            "singleton": type_def.singleton,
            "fields": fields,
            "sections": sections,
        });
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    // Text output
    let desc = type_def
        .description
        .as_deref()
        .unwrap_or("(no description)");
    println!("{} — {desc}", type_def.name.to_uppercase());
    if !type_def.aliases.is_empty() {
        println!("  Aliases: {}", type_def.aliases.join(", "));
    }
    if let Some(ref folder) = type_def.folder {
        println!("  Folder: {folder}");
    }
    println!();

    // Fields
    if !type_def.fields.is_empty() {
        println!("Fields:");
        for f in &type_def.fields {
            let req = if f.required { " *" } else { "" };
            let desc = f
                .description
                .as_deref()
                .map(|d| format!("  {d}"))
                .unwrap_or_default();
            let default = f
                .default
                .as_deref()
                .map(|d| format!(" [default: {d}]"))
                .unwrap_or_default();
            println!("  {}{req}: {}{default}{desc}", f.name, f.field_type);

            if !f.transitions.is_empty() {
                for (from, tos) in &f.transitions {
                    println!("    {} \u{2192} {}", from, tos.join(" | "));
                }
            }
        }
        println!();
    }

    // Sections
    if !type_def.sections.is_empty() {
        println!("Sections:");
        print_sections(&type_def.sections, 1);
        println!();
    }

    // Rules
    if !type_def.rules.is_empty() {
        println!("Rules:");
        for r in &type_def.rules {
            let cond = r.condition_display();
            if !r.then_required.is_empty() {
                println!("  when {cond}: require {}", r.then_required.join(", "));
            }
            for st in &r.then_section_table {
                println!("  when {cond}: \"{}\" must have table", st.section);
            }
        }
    }

    Ok(())
}

fn print_sections(sections: &[md_db::schema::SectionDef], depth: usize) {
    let indent = "  ".repeat(depth);
    for s in sections {
        let req = if s.required { " *" } else { "" };
        let desc = s
            .description
            .as_deref()
            .map(|d| format!("  ({d})"))
            .unwrap_or_default();

        let mut constraints = Vec::new();
        if s.table.is_some() {
            constraints.push("table");
        }
        if s.diagram.is_some() {
            constraints.push("diagram");
        }
        if s.list.is_some() {
            constraints.push("list");
        }
        if let Some(ref c) = s.content {
            if let Some(min) = c.min_paragraphs {
                constraints.push(if min == 1 { "text" } else { "text (multi)" });
            }
        }
        let constraint_str = if constraints.is_empty() {
            String::new()
        } else {
            format!(" [{}]", constraints.join(", "))
        };

        println!("{indent}{}{req}{constraint_str}{desc}", s.name);

        if !s.children.is_empty() {
            print_sections(&s.children, depth + 1);
        }
    }
}

fn section_to_json(s: &md_db::schema::SectionDef) -> serde_json::Value {
    let children: Vec<serde_json::Value> = s.children.iter().map(section_to_json).collect();
    let mut obj = serde_json::json!({
        "name": s.name,
        "required": s.required,
    });
    if let Some(ref desc) = s.description {
        obj["description"] = serde_json::json!(desc);
    }
    if s.table.is_some() {
        obj["has_table"] = serde_json::json!(true);
    }
    if s.diagram.is_some() {
        obj["has_diagram"] = serde_json::json!(true);
    }
    if s.list.is_some() {
        obj["has_list"] = serde_json::json!(true);
    }
    if !children.is_empty() {
        obj["children"] = serde_json::json!(children);
    }
    obj
}

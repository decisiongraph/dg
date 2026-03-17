use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use clap::Args;
use markdown_tui::RenderOptions;
use md_db::ast_util;
use md_db::discovery::{self, Filter};
use md_db::frontmatter::Frontmatter;
use md_db::graph;
use md_db::output::{self, ListEntry, OutputFormat};
use md_db::schema::Schema;
use md_db::users::{OrgConfig, ORG_CONFIG_FILENAME};

#[derive(Args)]
pub struct ListArgs {
    /// Filter by document type (adr, pol, opp, inc, spec)
    #[arg(long = "type", short = 't')]
    pub doc_type: Option<String>,

    /// Filter by status
    #[arg(long)]
    pub status: Option<String>,

    /// Filter by org (documents owned by users in this org)
    #[arg(long)]
    pub org: Option<String>,

    /// Sort by field (default: path)
    #[arg(long, default_value = "path")]
    pub sort: String,

    /// Output format (text, json, compact)
    #[arg(long, default_value = "text")]
    pub format: String,

    /// Fields to display (repeatable)
    #[arg(long = "field")]
    pub fields: Vec<String>,

    /// Hide untyped documents (markdown files without a type field)
    #[arg(long)]
    pub no_untyped: bool,

    /// Output as JSON (shorthand for --format json)
    #[arg(long)]
    pub json: bool,

    /// Group output by field (supported: "type")
    #[arg(long)]
    pub group_by: Option<String>,
}

/// Check whether a ListEntry has a known document type in its frontmatter.
fn is_typed(entry: &ListEntry) -> bool {
    !entry_type(entry).is_empty()
}

/// Extract the type field from a ListEntry's frontmatter, or "" if missing.
fn entry_type(entry: &ListEntry) -> &str {
    entry
        .frontmatter_json
        .as_ref()
        .and_then(|f| f.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

/// Build table rows (Document, Status, Date) for typed entries.
fn build_table_rows(root: &Path, entries: &[&ListEntry]) -> Vec<Vec<String>> {
    entries
        .iter()
        .map(|e| {
            let full_path = root.join(&e.path);
            let id = graph::path_to_id(&full_path);
            let fm = &e.frontmatter_json;
            let title = fm
                .as_ref()
                .and_then(|f| f.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("-");
            let document = format!("{id}: {title}");
            let status = fm
                .as_ref()
                .and_then(|f| f.get("status"))
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .to_string();
            let date = fm
                .as_ref()
                .and_then(|f| f.get("date"))
                .and_then(|v| v.as_str())
                .unwrap_or("-")
                .to_string();
            vec![document, status, date]
        })
        .collect()
}

pub fn run(root: &Path, schema: &Schema, args: &ListArgs, users: Option<&OrgConfig>) -> Result<()> {
    let mut filters: Vec<Filter> = Vec::new();

    // Resolve alias to canonical type name (e.g. "opportunity" → "opp")
    let canonical_type = args.doc_type.as_ref().map(|t| {
        schema
            .get_type(t)
            .map(|td| td.name.clone())
            .unwrap_or_else(|| t.clone())
    });

    if let Some(doc_type) = &canonical_type {
        filters.push(Filter::FieldEquals {
            key: "type".to_string(),
            value: doc_type.clone(),
        });
    }

    if let Some(status) = &args.status {
        filters.push(Filter::FieldEquals {
            key: "status".to_string(),
            value: status.clone(),
        });
    }

    // If filtering by type, scope to that type's folder
    let discover_dir = if let Some(doc_type) = &canonical_type {
        if let Some(type_def) = schema.get_type(doc_type) {
            if let Some(folder) = &type_def.folder {
                root.join(folder)
            } else {
                root.to_path_buf()
            }
        } else {
            root.to_path_buf()
        }
    } else {
        root.to_path_buf()
    };

    let files = discovery::discover_files(&discover_dir, None, &filters, false)
        .context("failed to discover files")?;

    let mut entries: Vec<ListEntry> = Vec::new();
    for path in &files {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let (fm_json, body) = match Frontmatter::try_parse(&content) {
            Ok((Some(fm), body)) => (Some(fm.to_json()), body),
            Ok((None, body)) => (None, body),
            Err(_) => (None, content.clone()),
        };
        let has_type = fm_json
            .as_ref()
            .and_then(|f| f.get("type"))
            .and_then(|v| v.as_str())
            .is_some_and(|t| !t.is_empty());
        let heading = if has_type {
            None
        } else {
            ast_util::first_heading_text(&body)
        };
        entries.push(ListEntry {
            path: path
                .strip_prefix(root)
                .unwrap_or(path)
                .display()
                .to_string(),
            frontmatter_json: fm_json,
            heading,
        });
    }

    // --org filter
    if let Some(org_id) = &args.org {
        let config =
            users.ok_or_else(|| anyhow::anyhow!("--org requires {ORG_CONFIG_FILENAME}"))?;
        if config.orgs.is_empty() {
            bail!("no orgs defined in {ORG_CONFIG_FILENAME}");
        }
        let org_set = config.expand_org(org_id);
        let org_users = config.users_in_org(org_id);
        entries.retain(|e| {
            let fm = match &e.frontmatter_json {
                Some(fm) => fm,
                None => return false,
            };
            // Match 1: doc has explicit org field matching
            let org_match = fm
                .get("org")
                .and_then(|v| v.as_str())
                .map(|v| v.strip_prefix("@org/").unwrap_or(v))
                .is_some_and(|o| org_set.contains(o));
            // Match 2: doc author/owner belongs to org
            let user_match = ["author", "owner"].iter().any(|field| {
                fm.get(*field)
                    .and_then(|v| v.as_str())
                    .map(|v| v.strip_prefix('@').unwrap_or(v))
                    .is_some_and(|h| org_users.contains(h))
            });
            org_match || user_match
        });
    }

    // Validate --group-by
    if let Some(ref group) = args.group_by {
        if group != "type" {
            bail!("unsupported --group-by value: {group:?} (supported: \"type\")");
        }
    }

    // Partition into typed and untyped
    let (mut typed, mut untyped): (Vec<_>, Vec<_>) = entries.into_iter().partition(is_typed);

    // Exclude untyped files that are not in docs/ and not named README.md —
    // these are project-level markdown files (CLAUDE.md, AGENTS.md, etc.) that
    // are not decision documents and would only clutter the output.
    untyped.retain(|e| {
        let p = std::path::Path::new(&e.path);
        p.starts_with("docs") || p.file_name().is_some_and(|n| n == "README.md")
    });

    // When grouping by type, default sort to date descending (unless user explicitly set --sort)
    let effective_sort = if args.group_by.is_some() && args.sort == "path" {
        "date"
    } else {
        args.sort.as_str()
    };

    // Sort typed entries
    match effective_sort {
        "date" => sort_by_field(&mut typed, "date", true),
        "title" => sort_by_field(&mut typed, "title", false),
        "status" => sort_by_field(&mut typed, "status", false),
        _ => {} // "path" — already sorted by discover_files
    }

    // Sort untyped by path
    untyped.sort_by(|a, b| a.path.cmp(&b.path));

    // Optionally drop untyped
    if args.no_untyped {
        untyped.clear();
    }

    let effective_format = if args.json { "json" } else { &args.format };
    let format = effective_format
        .parse::<OutputFormat>()
        .unwrap_or(OutputFormat::Text);

    if args.group_by.is_some() {
        render_grouped(root, &typed, &untyped, format, args)?;
    } else {
        render_flat(root, &typed, &untyped, format, args)?;
    }

    Ok(())
}

/// Render flat (non-grouped) output.
fn render_flat(
    root: &Path,
    typed: &[ListEntry],
    untyped: &[ListEntry],
    format: OutputFormat,
    args: &ListArgs,
) -> Result<()> {
    if format == OutputFormat::Text {
        let width = crossterm::terminal::size()
            .map(|(cols, _)| cols as usize)
            .unwrap_or(80);
        let options = RenderOptions {
            width,
            ..Default::default()
        };

        if typed.is_empty() && untyped.is_empty() {
            println!("No documents found.");
        } else {
            if !typed.is_empty() {
                let refs: Vec<&ListEntry> = typed.iter().collect();
                let headers = &["Document", "Status", "Date"];
                let rows = build_table_rows(root, &refs);
                let rendered = markdown_tui::render_table(headers, &rows, &options);
                print!("{rendered}");
            }

            if !untyped.is_empty() {
                if !typed.is_empty() {
                    println!();
                }
                println!("Untyped documents:");
                let headers = &["Path", "Title"];
                let rows: Vec<Vec<String>> = untyped
                    .iter()
                    .map(|e| {
                        let title = e.heading.as_deref().unwrap_or("-").to_string();
                        vec![e.path.clone(), title]
                    })
                    .collect();
                let rendered = markdown_tui::render_table(headers, &rows, &options);
                print!("{rendered}");
            }
        }
    } else {
        // For non-text formats, recombine typed + untyped (typed first)
        let all: Vec<&ListEntry> = typed.iter().chain(untyped.iter()).collect();
        let display_fields = if args.fields.is_empty() {
            None
        } else {
            Some(args.fields.clone())
        };
        // format_list expects &[ListEntry], collect owned refs
        let owned: Vec<ListEntry> = all
            .into_iter()
            .map(|e| ListEntry {
                path: e.path.clone(),
                frontmatter_json: e.frontmatter_json.clone(),
                heading: e.heading.clone(),
            })
            .collect();
        let out = output::format_list(&owned, format, &display_fields);
        print!("{out}");
    }
    Ok(())
}

/// Render grouped-by-type output.
fn render_grouped(
    root: &Path,
    typed: &[ListEntry],
    untyped: &[ListEntry],
    format: OutputFormat,
    args: &ListArgs,
) -> Result<()> {
    // Group typed entries by type, preserving sort order within groups
    let mut groups: BTreeMap<String, Vec<&ListEntry>> = BTreeMap::new();
    for entry in typed {
        let t = entry_type(entry).to_uppercase();
        groups.entry(t).or_default().push(entry);
    }

    if format == OutputFormat::Text {
        let width = crossterm::terminal::size()
            .map(|(cols, _)| cols as usize)
            .unwrap_or(80);
        let options = RenderOptions {
            width,
            ..Default::default()
        };

        if groups.is_empty() && untyped.is_empty() {
            println!("No documents found.");
            return Ok(());
        }

        let headers = &["Document", "Status", "Date"];
        let mut first = true;
        for (type_name, entries) in &groups {
            if !first {
                println!();
            }
            first = false;
            println!("## {type_name} ({})", entries.len());
            let rows = build_table_rows(root, entries);
            let rendered = markdown_tui::render_table(headers, &rows, &options);
            print!("{rendered}");
        }

        if !untyped.is_empty() {
            if !first {
                println!();
            }
            println!("## Untyped ({})", untyped.len());
            let untyped_headers = &["Path", "Title"];
            let rows: Vec<Vec<String>> = untyped
                .iter()
                .map(|e| {
                    let title = e.heading.as_deref().unwrap_or("-").to_string();
                    vec![e.path.clone(), title]
                })
                .collect();
            let rendered = markdown_tui::render_table(untyped_headers, &rows, &options);
            print!("{rendered}");
        }
    } else {
        // JSON: grouped object { "ADR": [...], "OPP": [...] }
        let display_fields = if args.fields.is_empty() {
            None
        } else {
            Some(&args.fields)
        };

        let mut map = serde_json::Map::new();
        for (type_name, entries) in &groups {
            let arr: Vec<serde_json::Value> = entries
                .iter()
                .map(|e| entry_to_json(e, &display_fields))
                .collect();
            map.insert(type_name.clone(), serde_json::Value::Array(arr));
        }

        if !untyped.is_empty() {
            let arr: Vec<serde_json::Value> = untyped
                .iter()
                .map(|e| entry_to_json(e, &display_fields))
                .collect();
            map.insert("Untyped".to_string(), serde_json::Value::Array(arr));
        }

        let out = serde_json::to_string_pretty(&serde_json::Value::Object(map)).unwrap_or_default();
        print!("{out}");
    }
    Ok(())
}

/// Convert a ListEntry to a JSON Value, optionally filtering fields.
fn entry_to_json(entry: &ListEntry, fields: &Option<&Vec<String>>) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "path".to_string(),
        serde_json::Value::String(entry.path.clone()),
    );
    if let Some(ref h) = entry.heading {
        obj.insert("heading".to_string(), serde_json::Value::String(h.clone()));
    }
    if let Some(ref fm) = entry.frontmatter_json {
        match fields {
            Some(field_list) => {
                for f in *field_list {
                    if let Some(v) = fm.get(f) {
                        obj.insert(f.clone(), v.clone());
                    }
                }
            }
            None => {
                if let serde_json::Value::Object(map) = fm {
                    for (k, v) in map {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }
    }
    serde_json::Value::Object(obj)
}

/// Sort list entries by a frontmatter field. If `reverse`, sort descending.
fn sort_by_field(entries: &mut [ListEntry], field: &str, reverse: bool) {
    entries.sort_by(|a, b| {
        let val_a = a
            .frontmatter_json
            .as_ref()
            .and_then(|f| f.get(field))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let val_b = b
            .frontmatter_json
            .as_ref()
            .and_then(|f| f.get(field))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if reverse {
            val_b.cmp(val_a)
        } else {
            val_a.cmp(val_b)
        }
    });
}

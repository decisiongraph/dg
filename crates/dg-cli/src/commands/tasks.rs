//! `dg tasks` command — show in-progress and unstarted decision documents.

use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use markdown_tui::RenderOptions;
use md_db::discovery;
use md_db::frontmatter::Frontmatter;
use md_db::graph;
use md_db::output::ListEntry;
use md_db::schema::Schema;

/// Statuses that are considered "done" — documents with these are hidden.
const DONE_STATUSES: &[&str] = &[
    "completed",
    "declined",
    "resolved",
    "implemented",
    "deprecated",
    "superseded",
    "rejected",
];

#[derive(Args)]
pub struct TasksArgs {
    /// Filter by document type (e.g. adr, pol, opp, inc, spec, proc)
    #[arg(long = "type", short = 't')]
    pub doc_type: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn run(root: &Path, schema: &Schema, args: &TasksArgs) -> Result<()> {
    let filters: Vec<discovery::Filter> = Vec::new();

    let discover_dir = if let Some(doc_type) = &args.doc_type {
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
        let fm_json = match Frontmatter::try_parse(&content) {
            Ok((Some(fm), _)) => Some(fm.to_json()),
            _ => None,
        };

        // Only include typed documents
        let doc_type = fm_json
            .as_ref()
            .and_then(|f| f.get("type"))
            .and_then(|v| v.as_str());
        if doc_type.is_none() || doc_type.unwrap().is_empty() {
            continue;
        }

        // Filter by --type if given
        if let Some(filter_type) = &args.doc_type {
            let canonical = schema
                .get_type(filter_type)
                .map(|td| td.name.as_str())
                .unwrap_or(filter_type.as_str());
            if doc_type.unwrap() != canonical {
                continue;
            }
        }

        // Exclude terminal statuses
        let status = fm_json
            .as_ref()
            .and_then(|f| f.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if DONE_STATUSES.contains(&status) {
            continue;
        }

        entries.push(ListEntry {
            path: path
                .strip_prefix(root)
                .unwrap_or(path)
                .display()
                .to_string(),
            frontmatter_json: fm_json,
            heading: None,
        });
    }

    // Sort by date descending
    entries.sort_by(|a, b| {
        let date_a = a
            .frontmatter_json
            .as_ref()
            .and_then(|f| f.get("date"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let date_b = b
            .frontmatter_json
            .as_ref()
            .and_then(|f| f.get("date"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        date_b.cmp(date_a)
    });

    if args.json {
        let arr: Vec<serde_json::Value> = entries.iter().map(|e| entry_to_json(e, root)).collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
    } else {
        if entries.is_empty() {
            println!("No pending tasks found. All documents are in terminal state.");
            return Ok(());
        }

        let width = crossterm::terminal::size()
            .map(|(cols, _)| cols as usize)
            .unwrap_or(80);
        let options = RenderOptions {
            width,
            ..Default::default()
        };

        let headers = &["Document", "Status", "Date"];
        let rows: Vec<Vec<String>> = entries
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
            .collect();

        let rendered = markdown_tui::render_table(headers, &rows, &options);
        print!("{rendered}");
    }

    Ok(())
}

fn entry_to_json(entry: &ListEntry, root: &Path) -> serde_json::Value {
    let full_path = root.join(&entry.path);
    let id = graph::path_to_id(&full_path);
    let fm = &entry.frontmatter_json;
    serde_json::json!({
        "id": id,
        "path": entry.path,
        "title": fm.as_ref().and_then(|f| f.get("title")).and_then(|v| v.as_str()).unwrap_or("-"),
        "status": fm.as_ref().and_then(|f| f.get("status")).and_then(|v| v.as_str()).unwrap_or("-"),
        "type": fm.as_ref().and_then(|f| f.get("type")).and_then(|v| v.as_str()).unwrap_or("-"),
        "date": fm.as_ref().and_then(|f| f.get("date")).and_then(|v| v.as_str()).unwrap_or("-"),
    })
}

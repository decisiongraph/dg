use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use md_db::graph;
use md_db::history;
use md_db::schema::Schema;

use super::show::resolve_id_to_path;

#[derive(Args)]
pub struct HistoryArgs {
    /// Document ID (e.g. ADR-001)
    #[arg(name = "ID")]
    pub id: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn run(root: &Path, schema: &Schema, args: &HistoryArgs) -> Result<()> {
    let path = resolve_id_to_path(root, schema, &args.id)?;
    let doc_id = graph::path_to_id(&path);

    let doc_paths = vec![(doc_id.clone(), path)];
    let result =
        history::collect_status_history(root, &doc_paths).context("failed to read git history")?;

    let transitions = result.get(&doc_id);

    if args.json {
        let json = match transitions {
            Some(t) => serde_json::to_string_pretty(t)?,
            None => "[]".to_string(),
        };
        println!("{json}");
        return Ok(());
    }

    match transitions {
        Some(transitions) if !transitions.is_empty() => {
            println!("{doc_id} status history:");
            for t in transitions {
                let from = t.from_status.as_deref().unwrap_or("(created)");
                let sha_short = &t.commit_sha[..7.min(t.commit_sha.len())];
                println!(
                    "  {} {} \u{2192} {} ({})",
                    t.date, from, t.to_status, sha_short
                );
            }
        }
        _ => {
            eprintln!("{doc_id}: no status transitions found in git history.");
        }
    }

    Ok(())
}

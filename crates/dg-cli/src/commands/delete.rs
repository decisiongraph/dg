use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use md_db::graph;
use md_db::schema::Schema;

#[derive(Args)]
pub struct DeleteArgs {
    /// Document ID (e.g. ADR-001) or file path
    #[arg(name = "ID")]
    pub id: String,
}

pub fn run(root: &Path, schema: &Schema, args: &DeleteArgs) -> Result<()> {
    let path = super::show::resolve_id_to_path(root, schema, &args.id)?;
    let doc_id = graph::path_to_id_with_schema(&path, schema);
    std::fs::remove_file(&path).with_context(|| format!("failed to delete {}", path.display()))?;

    println!(
        "Deleted {doc_id}: {}",
        path.strip_prefix(root).unwrap_or(&path).display()
    );
    Ok(())
}

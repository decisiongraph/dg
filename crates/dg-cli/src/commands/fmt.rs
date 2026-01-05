use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use md_db::format;
use md_db::schema::Schema;

#[derive(Args)]
pub struct FmtArgs {
    /// Glob pattern to filter files
    #[arg(long)]
    pub pattern: Option<String>,

    /// Preview without writing
    #[arg(long)]
    pub dry_run: bool,

    /// Exit with error if formatting needed (CI mode)
    #[arg(long)]
    pub check: bool,
}

pub fn run(root: &Path, schema: &Schema, args: &FmtArgs) -> Result<()> {
    let dry_run = args.dry_run || args.check;

    let result = format::format_directory(root, schema, args.pattern.as_deref(), dry_run)
        .context("formatting failed")?;

    for err in &result.errors {
        eprintln!("error: {}: {}", err.0.display(), err.1);
    }

    if result.changes.is_empty() {
        println!("no formatting changes needed");
        return Ok(());
    }

    for change in &result.changes {
        let path = change
            .path
            .strip_prefix(root)
            .unwrap_or(&change.path)
            .display();
        if dry_run {
            println!("would fix: {path} > {}", change.description);
        } else {
            println!("fixed: {path} > {}", change.description);
        }
    }

    println!(
        "\n{} change(s){}",
        result.changes.len(),
        if dry_run { " needed" } else { " applied" }
    );

    if args.check && !result.changes.is_empty() {
        anyhow::bail!(
            "formatting check failed: {} change(s) needed",
            result.changes.len()
        );
    }

    Ok(())
}

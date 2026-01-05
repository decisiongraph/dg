use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use md_db::diff::{self, FieldChangeKind, SectionChangeKind};
use md_db::graph;
use md_db::schema::Schema;

use super::show::resolve_id_to_path;

#[derive(Args)]
pub struct DiffArgs {
    /// Document ID (e.g. ADR-001)
    #[arg(name = "ID")]
    pub id: String,

    /// Compare against this git commit (default: HEAD)
    #[arg(long, default_value = "HEAD")]
    pub commit: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn run(root: &Path, schema: &Schema, args: &DiffArgs) -> Result<()> {
    let path = resolve_id_to_path(root, schema, &args.id)?;
    let doc_id = graph::path_to_id(&path);

    // Get relative path for git show
    let rel_path = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();

    // Get old version from git
    let output = std::process::Command::new("git")
        .args(["show", &format!("{}:{}", args.commit, rel_path)])
        .current_dir(root)
        .output()
        .context("failed to run git")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("does not exist") || stderr.contains("fatal:") {
            eprintln!("{doc_id}: new file (not in {}).", args.commit);
            return Ok(());
        }
        anyhow::bail!("git show failed: {stderr}");
    }

    let old_content =
        String::from_utf8(output.stdout).context("git show output is not valid UTF-8")?;
    let new_content = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let mut result = diff::diff_documents(&old_content, &new_content)?;
    result.path = Some(path.display().to_string());
    result.id = Some(doc_id.clone());

    if result.is_empty() {
        eprintln!("{doc_id}: no changes vs {}.", args.commit);
        return Ok(());
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    // Text output
    let title = result
        .field_changes
        .iter()
        .find(|c| c.field == "title")
        .and_then(|c| c.new.as_deref())
        .unwrap_or("");
    if title.is_empty() {
        println!("{doc_id}");
    } else {
        println!("{doc_id}: {title}");
    }

    if !result.field_changes.is_empty() {
        println!("  Fields:");
        for c in &result.field_changes {
            match c.kind {
                FieldChangeKind::Added => {
                    println!("    + {}: {}", c.field, c.new.as_deref().unwrap_or(""));
                }
                FieldChangeKind::Removed => {
                    println!("    - {}: {}", c.field, c.old.as_deref().unwrap_or(""));
                }
                FieldChangeKind::Changed => {
                    println!(
                        "    ~ {}: {} \u{2192} {}",
                        c.field,
                        c.old.as_deref().unwrap_or(""),
                        c.new.as_deref().unwrap_or("")
                    );
                }
            }
        }
    }

    if !result.section_changes.is_empty() {
        println!("  Sections:");
        for c in &result.section_changes {
            match c.kind {
                SectionChangeKind::Added => {
                    println!("    + {}", c.section);
                }
                SectionChangeKind::Removed => {
                    println!("    - {}", c.section);
                }
                SectionChangeKind::Modified => {
                    let added = c.lines_added.unwrap_or(0);
                    let removed = c.lines_removed.unwrap_or(0);
                    println!("    ~ {} (+{} -{} lines)", c.section, added, removed);
                }
            }
        }
    }

    Ok(())
}

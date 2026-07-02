use std::path::Path;

use anyhow::{bail, Result};
use clap::Args;
use md_db::renumber;
use md_db::schema::Schema;

#[derive(Args)]
pub struct RenumberArgs {
    /// Only renumber documents of this type (e.g. adr, opp)
    #[arg(long = "type", short = 't')]
    pub doc_type: Option<String>,

    /// Preview mapping without applying changes
    #[arg(long)]
    pub dry_run: bool,

    /// Skip git remote tracking branch warning
    #[arg(long)]
    pub force: bool,
}

pub fn run(root: &Path, schema: &Schema, args: &RenumberArgs) -> Result<()> {
    // Git remote check: warn if branch tracks a remote (destructive op)
    if !args.force && !args.dry_run {
        if let Some(warning) = check_git_remote(root) {
            bail!(
                "{warning}\n\
                 use --force to proceed anyway, or --dry-run to preview"
            );
        }
    }

    // Resolve alias to canonical type name (e.g. "opportunity" → "opp")
    let canonical_type = args.doc_type.as_ref().map(|t| {
        schema
            .get_type(t)
            .map(|td| td.name.clone())
            .unwrap_or_else(|| t.clone())
    });

    let plan = renumber::compute_renumber_plan(root, schema, canonical_type.as_deref())?;

    if plan.is_empty() {
        println!("Already in chronological order. Nothing to renumber.");
        return Ok(());
    }

    if args.dry_run {
        print!("{}", plan.to_report(root));
        return Ok(());
    }

    // Apply
    renumber::apply_renumber_plan(&plan)?;

    // Summary
    let total_refs: usize = plan.file_updates.iter().map(|f| f.matches.len()).sum();
    println!(
        "Renumbered {} file(s), updated {} reference(s).",
        plan.renames.len(),
        total_refs,
    );
    for r in &plan.renames {
        println!("  {} -> {}", r.old_id, r.new_id);
    }

    Ok(())
}

/// Check if the current branch tracks a remote. Returns a warning message if so.
fn check_git_remote(root: &Path) -> Option<String> {
    let repo = git2::Repository::open(root).ok()?;
    let head = repo.head().ok()?;
    let branch_name = head.shorthand().ok()?;
    let branch = repo
        .find_branch(branch_name, git2::BranchType::Local)
        .ok()?;
    let upstream = branch.upstream().ok()?;
    let upstream_name = upstream.name().ok()??;
    Some(format!(
        "Branch \"{branch_name}\" tracks remote \"{upstream_name}\".\n\
         Renumbering rewrites file names and references — \
         this will cause conflicts for others working on this branch."
    ))
}

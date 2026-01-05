use std::path::Path;

use anyhow::Result;
use clap::Args;
use md_db::search::{self, SearchOptions};

#[derive(Args)]
pub struct SearchArgs {
    /// Search query
    pub query: String,

    /// Only search within this section
    #[arg(long)]
    pub section: Option<String>,

    /// Only search within this frontmatter field
    #[arg(long)]
    pub field: Option<String>,

    /// Case-sensitive search
    #[arg(long)]
    pub case_sensitive: bool,

    /// Maximum results
    #[arg(long, short = 'n')]
    pub max_results: Option<usize>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn run(root: &Path, args: &SearchArgs) -> Result<()> {
    let opts = SearchOptions {
        case_sensitive: args.case_sensitive,
        section_filter: args.section.clone(),
        field_filter: args.field.clone(),
        max_results: args.max_results,
    };

    let results = search::search_documents(root, &args.query, &opts)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }

    if results.is_empty() {
        eprintln!("No matches found.");
        return Ok(());
    }

    // Make paths relative to root for display
    let root_str = root.display().to_string();

    for result in &results {
        let display_path = result
            .path
            .strip_prefix(&root_str)
            .map(|s| s.strip_prefix('/').unwrap_or(s))
            .unwrap_or(&result.path);

        let title = result.id.as_deref().unwrap_or("");
        if title.is_empty() {
            println!("{display_path}");
        } else {
            println!("{display_path} ({title})");
        }

        for m in &result.matches {
            println!("  {}:{}  {}", m.section, m.line, m.context);
        }
        println!();
    }

    Ok(())
}

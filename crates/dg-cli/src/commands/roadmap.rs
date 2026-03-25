use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;

use md_db::roadmap::{self, RoadmapConfig};
use md_db::schema::Schema;
use md_db::users::OrgConfig;

#[derive(Args)]
pub struct RoadmapArgs {
    /// Output directory
    #[arg(short, long, default_value = "site")]
    output: PathBuf,

    /// Skip git history (use frontmatter dates only)
    #[arg(long)]
    no_git: bool,

    /// Output format
    #[arg(long, default_value = "html")]
    format: OutputFormat,

    /// Number of past quarters to show
    #[arg(long)]
    past: Option<u8>,

    /// Number of future quarters to show
    #[arg(long)]
    future: Option<u8>,
}

#[derive(Clone, clap::ValueEnum)]
enum OutputFormat {
    Html,
    Json,
}

pub fn run(
    root: &Path,
    schema: &Schema,
    users: Option<&OrgConfig>,
    args: &RoadmapArgs,
    cache: &mut md_db::cache::DocCache,
) -> Result<()> {
    // Load roadmap config if present
    let config_path = root.join(".dg").join("roadmap.yaml");
    let config =
        if config_path.is_file() {
            Some(RoadmapConfig::from_file(&config_path).with_context(|| {
                format!("failed to load roadmap config: {}", config_path.display())
            })?)
        } else {
            None
        };

    let past = args
        .past
        .or(config.as_ref().map(|c| c.display.past_quarters))
        .unwrap_or(4);
    let future = args
        .future
        .or(config.as_ref().map(|c| c.display.future_quarters))
        .unwrap_or(4);

    // Get today's date
    let today = today_date();

    // Collect git history if not disabled
    let status_history = if args.no_git {
        None
    } else {
        collect_git_history(root, schema, cache).ok()
    };

    // Build roadmap data
    let data = roadmap::build_roadmap(
        root,
        schema,
        config.as_ref(),
        users,
        &today,
        past,
        future,
        status_history.as_ref(),
    )?;

    match args.format {
        OutputFormat::Json => {
            let json =
                serde_json::to_string_pretty(&data).context("failed to serialize roadmap data")?;
            println!("{json}");
        }
        OutputFormat::Html => {
            let output = &args.output;

            // Generate doc pages + index via export_site
            let count = md_db::export::export_site(root, Some(schema), output)
                .context("failed to export site")?;

            // Generate roadmap page
            let current_q =
                roadmap::Quarter::from_date(&today).unwrap_or(roadmap::Quarter::new(2026, 1));
            let html = roadmap::render_roadmap_html(&data, &current_q, schema);
            let roadmap_path = output.join("roadmap.html");
            std::fs::write(&roadmap_path, &html)
                .with_context(|| format!("failed to write {}", roadmap_path.display()))?;

            eprintln!(
                "Exported {count} documents + roadmap to {}",
                output.display()
            );
        }
    }

    Ok(())
}

fn today_date() -> String {
    // Use system clock
    let output = std::process::Command::new("date").arg("+%Y-%m-%d").output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "2026-01-01".to_string(),
    }
}

fn collect_git_history(
    root: &Path,
    schema: &Schema,
    cache: &mut md_db::cache::DocCache,
) -> anyhow::Result<std::collections::BTreeMap<String, Vec<md_db::history::StatusTransition>>> {
    use md_db::graph::DocGraph;

    let graph = DocGraph::build_cached(root, schema, cache)?;

    // Collect OPP file paths
    let opp_paths: Vec<(String, PathBuf)> = graph
        .nodes
        .values()
        .filter(|n| {
            n.doc_type
                .as_deref()
                .map(|t| t.eq_ignore_ascii_case("opp"))
                .unwrap_or(false)
        })
        .map(|n| (n.id.clone(), n.path.clone()))
        .collect();

    Ok(md_db::history::collect_status_history(root, &opp_paths)?)
}

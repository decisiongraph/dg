use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use md_db::graph::DocGraph;
use md_db::schema::Schema;
use md_db::users::OrgConfig;
use md_db::validation;

#[derive(Args)]
pub struct LintArgs {
    /// Glob pattern to filter files
    #[arg(long)]
    pub pattern: Option<String>,
}

pub fn run(
    root: &Path,
    schema: &Schema,
    users: Option<&OrgConfig>,
    args: &LintArgs,
    cache: &mut md_db::cache::DocCache,
) -> Result<()> {
    let mut has_errors = false;

    // 1. Schema validation
    let mut result = validation::validate_directory(root, schema, args.pattern.as_deref(), users)
        .context("validation failed")?;

    // 1b. Run detected linters and test suites for services/apps/infra
    let lint_results = validation::validate_service_linters(root);
    result.file_results.extend(lint_results);
    let test_results = validation::validate_service_tests(root);
    result.file_results.extend(test_results);

    if !result.is_ok() {
        has_errors = true;
    }
    print!("{}", result.to_report());

    // 2. Graph health checks
    let graph = DocGraph::build_cached(root, schema, cache).context("failed to build doc graph")?;
    let diagnostics = graph.check_health(schema);

    if !diagnostics.is_empty() {
        eprintln!("\n--- graph health ---");
        for d in &diagnostics {
            eprintln!("[{}] {}: {}", d.code, d.severity, d.message);
            if d.severity == "error" {
                has_errors = true;
            }
        }
    }

    let node_count = graph.nodes.len();
    let edge_count = graph.edges.len();
    eprintln!(
        "\ngraph: {node_count} documents, {edge_count} references, {} diagnostics",
        diagnostics.len()
    );

    if has_errors {
        anyhow::bail!("lint failed: errors found");
    }

    Ok(())
}

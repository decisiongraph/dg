use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use md_db::schema::Schema;
use md_db::users::OrgConfig;
use md_db::validation;

#[derive(Args)]
pub struct ValidateArgs {
    /// Glob pattern to filter files
    #[arg(long)]
    pub pattern: Option<String>,

    /// Output format (text, compact, json)
    #[arg(long, default_value = "text")]
    pub format: String,

    /// Diagnostic codes to skip (comma-separated, e.g. --skip U012,F020)
    #[arg(long, value_delimiter = ',')]
    pub skip: Vec<String>,
}

pub fn run(
    root: &Path,
    schema: &Schema,
    users: Option<&OrgConfig>,
    args: &ValidateArgs,
) -> Result<()> {
    let mut result = validation::validate_directory(root, schema, args.pattern.as_deref(), users)
        .context("validation failed")?;

    // Run detected linters and test suites only when not filtering by pattern
    if args.pattern.is_none() {
        let lint_results = validation::validate_service_linters(root);
        result.file_results.extend(lint_results);
        let test_results = validation::validate_service_tests(root);
        result.file_results.extend(test_results);
    }

    // Filter out skipped diagnostic codes
    if !args.skip.is_empty() {
        for fr in &mut result.file_results {
            fr.diagnostics
                .retain(|d| !args.skip.iter().any(|s| s.eq_ignore_ascii_case(&d.code)));
        }
    }

    match args.format.as_str() {
        "compact" => print!("{}", result.to_compact_report()),
        "json" => {
            let json = serde_json::json!({
                "errors": result.total_errors(),
                "warnings": result.total_warnings(),
                "files": result.file_results.iter().filter(|f| !f.diagnostics.is_empty()).map(|f| {
                    serde_json::json!({
                        "path": f.path,
                        "diagnostics": f.diagnostics.iter().map(|d| {
                            serde_json::json!({
                                "severity": format!("{}", d.severity),
                                "code": d.code,
                                "message": d.message,
                                "location": d.location,
                                "hint": d.hint,
                            })
                        }).collect::<Vec<_>>(),
                    })
                }).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        _ => print!("{}", result.to_report()),
    }

    if !result.is_ok() {
        anyhow::bail!("validation failed: {} error(s)", result.total_errors());
    }

    Ok(())
}

use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use md_db::schema::Schema;
use md_db::users::OrgConfig;
use md_db::validation;

#[derive(Args)]
pub struct ValidateArgs {
    /// Document ID to validate (e.g. ADR-001). Validates all if omitted.
    pub doc_id: Option<String>,

    /// Glob pattern to filter files
    #[arg(long)]
    pub pattern: Option<String>,

    /// Output format (text, compact, json)
    #[arg(long, default_value = "text")]
    pub format: String,

    /// Diagnostic codes to skip (comma-separated, e.g. --skip U012,F020)
    #[arg(long, value_delimiter = ',')]
    pub skip: Vec<String>,

    /// Skip auto-installing JS dependencies before running tests/linters
    #[arg(long)]
    pub no_install: bool,
}

pub fn run(
    root: &Path,
    schema: &Schema,
    users: Option<&OrgConfig>,
    args: &ValidateArgs,
) -> Result<()> {
    // If a doc ID is given, convert it to a glob pattern
    let pattern = if let Some(ref doc_id) = args.doc_id {
        let id_lower = doc_id.to_lowercase();
        Some(format!("**/{id_lower}*.md"))
    } else {
        args.pattern.clone()
    };

    let progress = std::io::stderr().is_terminal();
    if progress {
        eprintln!("dg: validating markdown documents…");
    }
    let mut result = validation::validate_directory(root, schema, pattern.as_deref(), users)
        .context("validation failed")?;

    // Run detected linters and test suites only when not filtering by pattern or doc ID
    if args.pattern.is_none() && args.doc_id.is_none() {
        let opts = validation::ServiceCheckOptions {
            no_install: args.no_install,
            progress,
        };
        result
            .file_results
            .extend(validation::validate_service_checks(root, &opts));
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

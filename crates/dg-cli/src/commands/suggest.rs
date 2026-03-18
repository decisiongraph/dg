use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use md_db::schema::Schema;
use md_db::suggest;

// ANSI escape helpers (same palette as show.rs)
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const BLUE: &str = "\x1b[1;38;5;39m";
const DIM: &str = "\x1b[38;5;245m";
const YELLOW: &str = "\x1b[38;5;179m";
const GREEN_BG: &str = "\x1b[1;30;42m";
const YELLOW_BG: &str = "\x1b[1;30;43m";
const RED_BG: &str = "\x1b[1;30;41m";
const GRAY_BG: &str = "\x1b[1;37;100m";
const BLUE_BG: &str = "\x1b[1;37;44m";

#[derive(Args)]
pub struct SuggestArgs {
    /// Only show suggestions for this document type
    #[arg(long = "type")]
    pub doc_type: Option<String>,

    /// Output format (text, json)
    #[arg(long, default_value = "text")]
    pub format: String,

    /// Glob pattern to filter files
    #[arg(long)]
    pub pattern: Option<String>,
}

pub fn run(root: &Path, schema: &Schema, args: &SuggestArgs) -> Result<()> {
    let today = today_str();

    let result = suggest::suggest_directory(root, schema, args.pattern.as_deref(), &today)
        .context("suggest failed")?;

    // Resolve alias to canonical type name for filtering
    let canonical_type = args
        .doc_type
        .as_ref()
        .map(|t| schema.get_type(t).map(|td| td.name.as_str()).unwrap_or(t));

    // Filter by type if requested
    let filtered: Vec<&suggest::FileSuggestions> = result
        .file_results
        .iter()
        .filter(|f| !f.suggestions.is_empty())
        .filter(|f| canonical_type.is_none_or(|t| f.doc_type.as_deref() == Some(t)))
        .collect();

    match args.format.as_str() {
        "json" => print_json(&filtered),
        _ => print_text(&filtered),
    }

    Ok(())
}

fn print_text(files: &[&suggest::FileSuggestions]) {
    let color = std::io::stdout().is_terminal();

    for f in files {
        let title_part = f
            .title
            .as_ref()
            .map(|t| format!(": {t}"))
            .unwrap_or_default();
        let status_pill = f
            .status
            .as_deref()
            .map(|s| {
                if color {
                    format!(" {}", format_status_pill(s))
                } else {
                    format!(" ({s})")
                }
            })
            .unwrap_or_default();

        if color {
            println!("{BLUE}{}{title_part}{RESET}{status_pill}", f.doc_id);
        } else {
            println!("{}{title_part}{status_pill}:", f.doc_id);
        }

        for s in &f.suggestions {
            let (icon, msg) = if color {
                match s.severity {
                    suggest::SuggestSeverity::Info => {
                        (format!("{DIM}\u{2139}{RESET}"), s.message.clone())
                    }
                    suggest::SuggestSeverity::Warning => (
                        format!("{YELLOW}\u{26a0}{RESET}"),
                        format!("{BOLD}{}{RESET}", s.message),
                    ),
                }
            } else {
                let icon = match s.severity {
                    suggest::SuggestSeverity::Info => "\u{2139}".to_string(),
                    suggest::SuggestSeverity::Warning => "\u{26a0}".to_string(),
                };
                (icon, s.message.clone())
            };
            println!("  {icon} {msg}");
            // Show multi-line hints inline (e.g. action item lists)
            if let Some(ref hint) = s.hint {
                if hint.contains('\n') {
                    for line in hint.lines() {
                        if color {
                            println!("    {DIM}{line}{RESET}");
                        } else {
                            println!("    {line}");
                        }
                    }
                }
            }
        }
        println!();
    }

    let total: usize = files.iter().map(|f| f.suggestions.len()).sum();
    let doc_count = files.len();
    if total > 0 {
        if color {
            println!("{DIM}summary: {total} suggestion(s) across {doc_count} document(s){RESET}");
        } else {
            println!("summary: {total} suggestion(s) across {doc_count} document(s)");
        }
    } else {
        println!("no suggestions");
    }
}

/// Colored status pill matching show.rs palette.
fn format_status_pill(status: &str) -> String {
    let upper = status.to_uppercase();
    let bg = match status.to_lowercase().as_str() {
        "accepted" | "active" | "approved" | "resolved" | "completed" | "delivered" => GREEN_BG,
        "proposed" | "validating" | "draft" | "review" | "pursuing" | "exploring"
        | "investigating" | "identified" | "open" => YELLOW_BG,
        "parked" => BLUE_BG,
        "deprecated" | "rejected" | "superseded" | "ongoing" | "declined" | "mitigated"
        | "retired" => RED_BG,
        _ => GRAY_BG,
    };
    format!("{bg} {upper} {RESET}")
}

fn print_json(files: &[&suggest::FileSuggestions]) {
    let total: usize = files.iter().map(|f| f.suggestions.len()).sum();
    let warnings: usize = files
        .iter()
        .flat_map(|f| &f.suggestions)
        .filter(|s| s.severity == suggest::SuggestSeverity::Warning)
        .count();
    let info = total - warnings;

    let json = serde_json::json!({
        "total": total,
        "warnings": warnings,
        "info": info,
        "files": files.iter().map(|f| {
            serde_json::json!({
                "path": f.path,
                "doc_id": f.doc_id,
                "title": f.title,
                "doc_type": f.doc_type,
                "status": f.status,
                "suggestions": f.suggestions.iter().map(|s| {
                    serde_json::json!({
                        "severity": format!("{}", s.severity),
                        "category": format!("{}", s.category),
                        "message": s.message,
                        "hint": s.hint,
                    })
                }).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    });
    // unwrap is safe here — we're building from known-good JSON values
    println!(
        "{}",
        serde_json::to_string_pretty(&json).expect("json serialization")
    );
}

fn today_str() -> String {
    let now = std::time::SystemTime::now();
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    epoch_secs_to_date(secs)
}

/// Convert unix epoch seconds to "YYYY-MM-DD" string.
fn epoch_secs_to_date(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    // Algorithm from Howard Hinnant's civil_from_days
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_secs_to_date() {
        // 2025-01-01 00:00:00 UTC = 1735689600
        assert_eq!(epoch_secs_to_date(1735689600), "2025-01-01");
        // 2025-06-16 = 1750032000
        assert_eq!(epoch_secs_to_date(1750032000), "2025-06-16");
    }
}

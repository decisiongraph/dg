use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use md_db::coverage;
use md_db::schema::Schema;

#[derive(Args)]
pub struct CoverageArgs {
    /// Output format (text, json)
    #[arg(long, default_value = "text")]
    pub format: String,
}

pub fn run(root: &Path, schema: &Schema, args: &CoverageArgs) -> Result<()> {
    let today = today_str();
    let report = coverage::coverage_report(root, schema, &today).context("coverage failed")?;

    match args.format.as_str() {
        "json" => print_json(&report),
        _ => print_text(&report),
    }

    Ok(())
}

fn print_text(report: &coverage::CoverageReport) {
    println!("DecisionGraph Coverage Report");
    println!("{}", "=".repeat(40));

    // Type breakdown
    println!("\nDocuments: {}", report.total_docs);
    for tc in &report.type_counts {
        println!("  {}: {}", tc.doc_type.to_uppercase(), tc.count);
    }

    // Metrics
    println!("\nMetrics:");
    println!("  Completeness:  {:.0}%", report.completeness_pct);
    println!("  Linkage:       {:.0}%", report.linkage_pct);
    println!("  Stale docs:    {}", report.stale_count);

    // Stale docs detail
    let stale: Vec<_> = report.files.iter().filter(|f| f.is_stale).collect();
    if !stale.is_empty() {
        println!("\nStale documents:");
        for f in &stale {
            let dt = f.doc_type.as_deref().unwrap_or("?");
            println!("  {} ({dt}): {}", f.doc_id, f.path);
        }
    }

    // Low completeness
    let low: Vec<_> = report
        .files
        .iter()
        .filter(|f| (f.field_completeness + f.section_completeness) / 2.0 < 50.0)
        .collect();
    if !low.is_empty() {
        println!("\nLow completeness (<50%):");
        for f in &low {
            let avg = (f.field_completeness + f.section_completeness) / 2.0;
            println!("  {} ({:.0}%): {}", f.doc_id, avg, f.path);
        }
    }

    // Orphaned docs (no refs)
    let orphans: Vec<_> = report.files.iter().filter(|f| !f.has_refs).collect();
    if !orphans.is_empty() {
        println!("\nOrphaned documents (no references):");
        for f in &orphans {
            println!("  {}: {}", f.doc_id, f.path);
        }
    }
}

fn print_json(report: &coverage::CoverageReport) {
    let json = serde_json::json!({
        "total_docs": report.total_docs,
        "type_counts": report.type_counts.iter().map(|tc| {
            serde_json::json!({
                "type": tc.doc_type,
                "count": tc.count,
            })
        }).collect::<Vec<_>>(),
        "completeness_pct": (report.completeness_pct * 10.0).round() / 10.0,
        "linkage_pct": (report.linkage_pct * 10.0).round() / 10.0,
        "stale_count": report.stale_count,
        "files": report.files.iter().map(|f| {
            serde_json::json!({
                "path": f.path,
                "doc_id": f.doc_id,
                "doc_type": f.doc_type,
                "field_completeness": (f.field_completeness * 10.0).round() / 10.0,
                "section_completeness": (f.section_completeness * 10.0).round() / 10.0,
                "has_refs": f.has_refs,
                "is_stale": f.is_stale,
            })
        }).collect::<Vec<_>>(),
    });
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

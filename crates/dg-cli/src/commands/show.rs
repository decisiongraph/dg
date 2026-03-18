use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use clap::Args;
use markdown_tui::options::OverdueHighlight;
use markdown_tui::{Color, RenderOptions};
use md_db::ast_util;
use md_db::discovery;
use md_db::document::Document;
use md_db::frontmatter::yaml_value_to_string;
use md_db::graph::{self, DocGraph};
use md_db::schema::Schema;

// ANSI escape helpers
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const BLUE: &str = "\x1b[1;38;5;39m";
const GREEN_BG: &str = "\x1b[1;30;42m";
const YELLOW_BG: &str = "\x1b[1;30;43m";
const RED_BG: &str = "\x1b[1;30;41m";
const GRAY_BG: &str = "\x1b[1;37;100m";
const GREEN: &str = "\x1b[38;5;40m";
const CYAN: &str = "\x1b[36m";
const WHITE: &str = "\x1b[37m";
const DARK_WHITE: &str = "\x1b[38;5;249m";
const DIM_YELLOW: &str = "\x1b[38;5;179m";
const DIM_RED: &str = "\x1b[38;5;131m";
const BLUE_BG: &str = "\x1b[1;37;44m";
const CYAN_BG: &str = "\x1b[1;30;46m";
const GREEN_FG_BG: &str = "\x1b[1;30;42m";
const YELLOW_FG_BG: &str = "\x1b[1;30;43m";
const RED_FG_BG: &str = "\x1b[1;30;41m";

#[derive(Args)]
pub struct ShowArgs {
    /// Document ID (e.g. ADR-001) or file path
    #[arg(name = "ID")]
    pub id: String,

    /// Output raw markdown instead of rendered
    #[arg(long)]
    pub raw: bool,

    /// Output frontmatter as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn run(
    root: &Path,
    schema: &Schema,
    args: &ShowArgs,
    cache: &mut md_db::cache::DocCache,
) -> Result<()> {
    let path = resolve_id_to_path(root, schema, &args.id)?;

    if args.json {
        let doc = Document::from_file(&path)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let mut json = doc.to_json();

        let doc_id = graph::path_to_id(&path);
        if let Ok(g) = DocGraph::build_cached(root, schema, cache) {
            if let Some(obj) = json.as_object_mut() {
                obj.insert("refs".to_string(), refs_to_json(&g, &doc_id, false));
                obj.insert("backlinks".to_string(), refs_to_json(&g, &doc_id, true));
            }
        }

        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    if args.raw {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        print!("{content}");
        return Ok(());
    }

    // Structured output: ANSI header + markdown body
    let doc = Document::from_file(&path)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    let doc_id = graph::path_to_id(&path);
    let width = crossterm::terminal::size()
        .map(|(cols, _)| cols as usize)
        .unwrap_or(80);

    print_header(&doc, &doc_id, root, schema, width, &doc.body, cache);

    let today = today_string();
    let options = RenderOptions {
        width,
        cell_highlights: vec![
            ("completed".into(), Color::Indexed(71)),    // dim green
            ("in-progress".into(), Color::Indexed(179)), // dim yellow
            ("pending".into(), Color::Indexed(167)),     // red
        ],
        overdue_highlight: Some(OverdueHighlight {
            statuses: vec!["pending".into(), "in-progress".into()],
            color: Color::Indexed(131), // muted red (#af5f5f)
            today,
        }),
        highlight_doc_ids: true,
        doc_id_prefixes: schema.type_prefixes(),
        auto_number_sections: vec!["action items".into()],
        ..Default::default()
    };
    let rendered = markdown_tui::render_markdown_with_options(&doc.body, &options);
    print!("{rendered}");

    Ok(())
}

/// Print the structured header: title, ID + status pill, metadata, rule, relationships.
fn print_header(
    doc: &Document,
    doc_id: &str,
    root: &Path,
    schema: &Schema,
    width: usize,
    body: &str,
    cache: &mut md_db::cache::DocCache,
) {
    let relation_fields: Vec<&str> = schema.all_relation_field_names();
    let skip_fields: Vec<&str> = {
        let mut s = vec!["title", "type", "status"];
        s.extend(&relation_fields);
        s
    };

    // Type badge + ID: Title  STATUS on one line
    let title = doc
        .frontmatter
        .as_ref()
        .and_then(|fm| fm.get_display("title"))
        .or_else(|| doc.title())
        .unwrap_or_default();
    let status = doc
        .frontmatter
        .as_ref()
        .and_then(|fm| fm.get_display("status"));
    let pill = status.as_deref().map(status_pill).unwrap_or_default();
    // Determine type: explicit field, schema lookup, or infer from doc_id prefix (OPP-001 → opp)
    let doc_type = doc
        .frontmatter
        .as_ref()
        .and_then(|fm| fm.get_display("type"))
        .or_else(|| schema.type_name_for_doc_id(doc_id))
        .unwrap_or_else(|| doc_id.split('-').next().unwrap_or("").to_lowercase());
    let badge = type_badge(&doc_type);
    println!("{badge} {BLUE}{doc_id}: {title}{RESET}  {pill}");
    println!();

    if let Some(ref fm) = doc.frontmatter {
        // Aligned key-value metadata (date fields get relative time + age color)
        let meta: Vec<(String, String)> = fm
            .data()
            .iter()
            .filter(|(k, _)| !skip_fields.contains(&k.as_str()))
            .map(|(k, v)| {
                let display = format_meta_value(v);
                let enriched = if is_date_value(&display) {
                    match relative_time(&display) {
                        Some(rel) => {
                            let color = date_age_color(&display);
                            format!("{display} {color}({rel}){RESET}")
                        }
                        None => display,
                    }
                } else {
                    display
                };
                (capitalize(k), enriched)
            })
            .collect();

        if !meta.is_empty() {
            let max_key = meta.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
            let max_val_width = width.saturating_sub(max_key + 4);
            for (k, v) in &meta {
                let display_v = truncate_display(v, max_val_width);
                println!("{DARK_WHITE}{k:<max_key$}{RESET}  {display_v}");
            }
            println!();
        }
    }

    // Horizontal rule
    println!("{DARK_WHITE}{}{RESET}", "─".repeat(width));
    println!();

    // Relationships (deduplicate inverse pairs) + body mentions
    if let Ok(graph) = DocGraph::build_cached(root, schema, cache) {
        let outgoing = graph.refs_from(doc_id);
        let incoming = graph.refs_to(doc_id);

        // Collect outgoing target IDs to skip redundant incoming inverses
        let outgoing_targets: std::collections::HashSet<&str> =
            outgoing.iter().map(|e| e.to.as_str()).collect();

        // Only show incoming edges from nodes we don't already have an outgoing edge to
        let unique_incoming: Vec<_> = incoming
            .iter()
            .filter(|e| !outgoing_targets.contains(e.from.as_str()))
            .collect();

        // Detect body mentions not covered by any existing edge
        let all_edge_ids: std::collections::HashSet<String> = outgoing
            .iter()
            .map(|e| e.to.clone())
            .chain(incoming.iter().map(|e| e.from.clone()))
            .collect();
        let doc_id_upper = doc_id.to_uppercase();
        let prefixes = schema.type_prefixes();
        let mentions: Vec<String> = ast_util::extract_doc_id_mentions(body, &prefixes)
            .into_iter()
            .filter(|id| *id != doc_id_upper && !all_edge_ids.contains(id))
            .collect();

        let has_relations =
            !outgoing.is_empty() || !unique_incoming.is_empty() || !mentions.is_empty();

        if has_relations {
            let max_rel = outgoing
                .iter()
                .map(|e| e.relation.len())
                .chain(unique_incoming.iter().map(|e| e.relation.len()))
                .chain(if mentions.is_empty() {
                    None
                } else {
                    Some("mentions".len())
                })
                .max()
                .unwrap_or(0);

            for edge in &outgoing {
                let title = graph
                    .nodes
                    .get(edge.to.as_str())
                    .and_then(|n| n.title.as_deref());
                let label = match title {
                    Some(t) => format!("{BOLD}{}{RESET}: {t}", edge.to),
                    None => format!("{BOLD}{}{RESET}", edge.to),
                };
                println!(
                    "  {GREEN}\u{2192}{RESET} {CYAN}{:<max_rel$}{RESET}  {label}",
                    edge.relation
                );
            }

            for edge in &unique_incoming {
                let title = graph
                    .nodes
                    .get(edge.from.as_str())
                    .and_then(|n| n.title.as_deref());
                let label = match title {
                    Some(t) => format!("{BOLD}{}{RESET}: {t}", edge.from),
                    None => format!("{BOLD}{}{RESET}", edge.from),
                };
                println!(
                    "  {WHITE}\u{2190}{RESET} {CYAN}{:<max_rel$}{RESET}  {label}",
                    edge.relation
                );
            }

            for mention_id in &mentions {
                let title = graph
                    .nodes
                    .get(mention_id.as_str())
                    .and_then(|n| n.title.as_deref());
                let label = match title {
                    Some(t) => format!("{BOLD}{mention_id}{RESET}: {t}"),
                    None => format!("{BOLD}{mention_id}{RESET}"),
                };
                println!(
                    "  {GREEN}\u{2192}{RESET} {CYAN}{:<max_rel$}{RESET}  {label}",
                    "mentions"
                );
            }

            println!();
        }
    }
}

/// Render a colored status pill like ` ACCEPTED `.
fn status_pill(status: &str) -> String {
    let upper = status.to_uppercase();
    let bg = match status.to_lowercase().as_str() {
        "accepted" | "active" | "approved" | "resolved" | "completed" | "delivered"
        | "implemented" => GREEN_BG,
        "proposed" | "validating" | "draft" | "review" | "pursuing" | "exploring"
        | "investigating" | "identified" | "open" => YELLOW_BG,
        "parked" => BLUE_BG,
        "deprecated" | "rejected" | "superseded" | "ongoing" | "declined" | "mitigated"
        | "retired" => RED_BG,
        _ => GRAY_BG,
    };
    format!("{bg} {upper} {RESET}")
}

/// Colored type badge: `[ADR]`, `[OPP]`, `[INC]`, `[POL]`.
fn type_badge(type_name: &str) -> String {
    let upper = type_name.to_uppercase();
    let bg = match type_name.to_lowercase().as_str() {
        "adr" => BLUE_BG,
        "opp" => GREEN_FG_BG,
        "inc" => RED_FG_BG,
        "pol" => YELLOW_FG_BG,
        "spec" => CYAN_BG,
        _ => GRAY_BG,
    };
    format!("{bg} {upper} {RESET}")
}

/// ANSI color for the `(X ago)` parenthetical based on date age.
/// 0-30 days → green, 31-180 → dim/default, 181+ → dim yellow/red.
fn date_age_color(date_str: &str) -> &'static str {
    let (ty, tm, td) = today_ymd();
    let (dy, dm, dd) = match parse_ymd(date_str) {
        Some(v) => v,
        None => return DARK_WHITE,
    };
    let today_days = ymd_to_days(ty, tm, td);
    let date_days = ymd_to_days(dy, dm, dd);
    let age = (today_days - date_days).unsigned_abs();
    match age {
        0..=30 => GREEN,
        31..=180 => DARK_WHITE,
        181..=365 => DIM_YELLOW,
        _ => DIM_RED,
    }
}

/// Truncate a string to fit within `max_width` visible chars.
/// Counts only non-ANSI characters for width, preserving escape sequences.
fn truncate_display(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    // Count visible characters (skip ANSI escape sequences)
    let mut visible = 0;
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
            continue;
        }
        if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        visible += 1;
    }
    if visible <= max_width {
        return s.to_string();
    }
    // Truncate: rebuild counting visible chars
    let mut result = String::new();
    let mut count = 0;
    in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
            result.push(ch);
            continue;
        }
        if in_escape {
            result.push(ch);
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }
        if count >= max_width.saturating_sub(1) {
            result.push('\u{2026}'); // …
            break;
        }
        result.push(ch);
        count += 1;
    }
    result
}

/// Capitalize first letter and replace underscores with spaces.
fn capitalize(s: &str) -> String {
    let s = s.replace('_', " ");
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Format a YAML value for metadata display (tags as space-separated).
fn format_meta_value(v: &serde_yaml::Value) -> String {
    match v {
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .map(yaml_value_to_string)
            .collect::<Vec<_>>()
            .join(", "),
        other => yaml_value_to_string(other),
    }
}

/// Check if a string looks like a YYYY-MM-DD date.
fn is_date_value(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(|c| c.is_ascii_digit())
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[8..10].iter().all(|c| c.is_ascii_digit())
}

/// Parse YYYY-MM-DD into (year, month, day).
fn parse_ymd(s: &str) -> Option<(i32, u32, u32)> {
    let y: i32 = s[..4].parse().ok()?;
    let m: u32 = s[5..7].parse().ok()?;
    let d: u32 = s[8..10].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some((y, m, d))
}

/// Convert (year, month, day) to days since Unix epoch using Hinnant's algorithm.
fn ymd_to_days(y: i32, m: u32, d: u32) -> i64 {
    let y = y as i64;
    let m = m as i64;
    let d = d as i64;
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    era * 146097 + doe as i64 - 719468
}

/// Today as YYYY-MM-DD string.
fn today_string() -> String {
    let (y, m, d) = today_ymd();
    format!("{y:04}-{m:02}-{d:02}")
}

/// Today as (year, month, day) in UTC.
fn today_ymd() -> (i32, u32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (secs / 86400) as i64;
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

/// Human-readable relative time from a YYYY-MM-DD string.
///
/// - 0 days: "today"
/// - 1 day: "yesterday" / "tomorrow"
/// - 2-14 days: "X days ago/from now"
/// - 3-7 weeks: "X weeks ago/from now"
/// - 2-11 months: "X months ago/from now"
/// - 12+ months: year-based with half-year granularity
fn relative_time(date_str: &str) -> Option<String> {
    let (ty, tm, td) = today_ymd();
    let (dy, dm, dd) = parse_ymd(date_str)?;

    let today_days = ymd_to_days(ty, tm, td);
    let date_days = ymd_to_days(dy, dm, dd);
    let diff = today_days - date_days;

    let (abs_diff, suffix) = if diff >= 0 {
        (diff, "ago")
    } else {
        (-diff, "from now")
    };

    Some(match abs_diff {
        0 => "today".to_string(),
        1 if diff > 0 => "yesterday".to_string(),
        1 => "tomorrow".to_string(),
        2..=14 => format!("{abs_diff} days {suffix}"),
        15..=48 => {
            let w = abs_diff / 7;
            if w == 1 {
                format!("1 week {suffix}")
            } else {
                format!("{w} weeks {suffix}")
            }
        }
        _ => {
            // Use month-based counting for better accuracy
            let total_months = (ty as i64 - dy as i64) * 12 + (tm as i64 - dm as i64);
            let total_months = if diff >= 0 {
                total_months
            } else {
                -total_months
            };

            if total_months < 12 {
                let m = total_months.max(2);
                if m == 1 {
                    format!("1 month {suffix}")
                } else {
                    format!("{m} months {suffix}")
                }
            } else {
                // Half-year granularity: round to nearest 0.5
                let half_years = (total_months as f64 / 6.0).round() as i64;
                let years = half_years as f64 / 2.0;
                if years <= 1.0 {
                    format!("1 year {suffix}")
                } else if years == years.floor() {
                    format!("{} years {suffix}", years as i64)
                } else {
                    format!("{years:.1} years {suffix}")
                }
            }
        }
    })
}

/// Serialize direct refs or backlinks as a JSON array.
fn refs_to_json(graph: &DocGraph, id: &str, backlinks: bool) -> serde_json::Value {
    let edges = if backlinks {
        graph.refs_to(id)
    } else {
        graph.refs_from(id)
    };

    let arr: Vec<serde_json::Value> = edges
        .iter()
        .map(|edge| {
            let target_id = if backlinks { &edge.from } else { &edge.to };
            let node = graph.nodes.get(target_id.as_str());
            serde_json::json!({
                "id": target_id,
                "relation": edge.relation,
                "type": node.and_then(|n| n.doc_type.as_deref()),
                "title": node.and_then(|n| n.title.as_deref()),
            })
        })
        .collect();

    serde_json::Value::Array(arr)
}

/// Resolve a document ID like "ADR-001" to its file path by scanning docs dirs
pub(crate) fn resolve_id_to_path(
    root: &Path,
    _schema: &Schema,
    id: &str,
) -> Result<std::path::PathBuf> {
    // If it looks like a path, use it directly
    let as_path = Path::new(id);
    if as_path.exists() {
        return Ok(as_path.to_path_buf());
    }
    let rooted = root.join(id);
    if rooted.exists() {
        return Ok(rooted);
    }

    // Scan all doc folders for a file matching this ID
    let target_id = id.to_uppercase();
    let files =
        discovery::discover_files(root, None, &[], false).context("failed to discover files")?;

    for file in &files {
        let file_id = graph::path_to_id(file);
        if file_id == target_id {
            return Ok(file.clone());
        }
    }

    anyhow::bail!("document '{id}' not found\nhint: use `dg list` to see available documents")
}

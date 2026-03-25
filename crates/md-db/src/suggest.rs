use std::fmt;
use std::path::Path;
use std::sync::LazyLock;

use rayon::prelude::*;
use regex::RegexBuilder;

use crate::document::Document;
use crate::graph::DocGraph;
use crate::schema::{Schema, SectionDef, TypeDef};

// ─── Types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestSeverity {
    Info,
    Warning,
}

impl fmt::Display for SuggestSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SuggestSeverity::Info => write!(f, "info"),
            SuggestSeverity::Warning => write!(f, "warning"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestCategory {
    IncompleteMarker,
    OpenActionItem,
    StaleDocument,
    MissingCrossRef,
    MissingOptionalSection,
    MissingDiagram,
    LowQualityContent,
}

impl fmt::Display for SuggestCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SuggestCategory::IncompleteMarker => write!(f, "incomplete_marker"),
            SuggestCategory::OpenActionItem => write!(f, "open_action_item"),
            SuggestCategory::StaleDocument => write!(f, "stale_document"),
            SuggestCategory::MissingCrossRef => write!(f, "missing_cross_ref"),
            SuggestCategory::MissingOptionalSection => write!(f, "missing_optional_section"),
            SuggestCategory::MissingDiagram => write!(f, "missing_diagram"),
            SuggestCategory::LowQualityContent => write!(f, "low_quality_content"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub severity: SuggestSeverity,
    pub category: SuggestCategory,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileSuggestions {
    pub path: String,
    pub doc_id: String,
    pub title: Option<String>,
    pub doc_type: Option<String>,
    pub status: Option<String>,
    pub suggestions: Vec<Suggestion>,
}

#[derive(Debug)]
pub struct SuggestResult {
    pub file_results: Vec<FileSuggestions>,
}

impl SuggestResult {
    pub fn total(&self) -> usize {
        self.file_results.iter().map(|f| f.suggestions.len()).sum()
    }

    pub fn total_warnings(&self) -> usize {
        self.file_results
            .iter()
            .flat_map(|f| &f.suggestions)
            .filter(|s| s.severity == SuggestSeverity::Warning)
            .count()
    }

    pub fn total_info(&self) -> usize {
        self.file_results
            .iter()
            .flat_map(|f| &f.suggestions)
            .filter(|s| s.severity == SuggestSeverity::Info)
            .count()
    }

    pub fn files_with_suggestions(&self) -> usize {
        self.file_results
            .iter()
            .filter(|f| !f.suggestions.is_empty())
            .count()
    }
}

// ─── Lazy regex for TBD/FIXME markers ──────────────────────────────────────

static MARKER_RE: LazyLock<Option<regex::Regex>> = LazyLock::new(|| {
    RegexBuilder::new(r"\b(TBD|FIXME|TODO|XXX)\b|\[TBD\]")
        .case_insensitive(true)
        .size_limit(1 << 20)
        .build()
        .ok()
});

// ─── Recommended team doc sections ──────────────────────────────────────────

const RECOMMENDED_TEAM_SECTIONS: &[&str] = &[
    "Charter",
    "Communication",
    "On-Call",
    "Getting Started",
    "Processes",
    "Key Contacts",
];

// ─── Known diagram languages ───────────────────────────────────────────────

const DIAGRAM_LANGUAGES: &[&str] = &["mermaid", "d2", "plantuml", "graphviz", "dot"];

// ─── Date math (epoch days, no chrono dep) ─────────────────────────────────

/// Parse "YYYY-MM-DD" into (year, month, day). Returns None on bad format.
fn parse_date(s: &str) -> Option<(i64, i64, i64)> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() < 3 {
        return None;
    }
    let y = parts[0].parse::<i64>().ok()?;
    let m = parts[1].parse::<i64>().ok()?;
    let d = parts[2].parse::<i64>().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some((y, m, d))
}

/// Convert (year, month, day) to an epoch-day count (days since some fixed epoch).
/// Good enough for difference calculations.
fn epoch_days(y: i64, m: i64, d: i64) -> i64 {
    // Adjust for months Jan/Feb -> previous year
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    365 * y + y / 4 - y / 100 + y / 400 + (m * 306 + 5) / 10 + d - 1
}

/// Number of days between two date strings. Positive if `a` is after `b`.
fn days_between(a: &str, b: &str) -> Option<i64> {
    let (ay, am, ad) = parse_date(a)?;
    let (by, bm, bd) = parse_date(b)?;
    Some(epoch_days(ay, am, ad) - epoch_days(by, bm, bd))
}

// ─── Category checks ───────────────────────────────────────────────────────

/// 1. TBD/FIXME/TODO/XXX markers in document body
fn check_incomplete_markers(body: &str, suggestions: &mut Vec<Suggestion>) {
    let re = match MARKER_RE.as_ref() {
        Some(re) => re,
        None => return,
    };
    let matches: Vec<&str> = re.find_iter(body).map(|m| m.as_str()).collect();
    if matches.is_empty() {
        return;
    }
    // Deduplicate marker names for the message
    let mut unique: Vec<String> = matches
        .iter()
        .map(|m| {
            m.to_uppercase()
                .trim_matches('[')
                .trim_matches(']')
                .to_string()
        })
        .collect();
    unique.sort();
    unique.dedup();
    suggestions.push(Suggestion {
        severity: SuggestSeverity::Info,
        category: SuggestCategory::IncompleteMarker,
        message: format!(
            "{} incomplete marker(s) ({})",
            matches.len(),
            unique.join(", ")
        ),
        hint: Some("replace placeholder text with actual content".into()),
    });
}

/// 2. Open action items — find tables with a "Status" column
fn check_open_action_items(doc: &Document, today: &str, suggestions: &mut Vec<Suggestion>) {
    let parsed = doc.parse_body();
    for section in all_sections(&parsed.sections) {
        for table in &section.tables {
            let status_col = match table.get_column("Status") {
                Some(col) => col,
                None => continue,
            };
            let due_col = table.get_column("Due Date");

            // Find the first column that isn't "Status"/"Due Date"/"Owner" to use as item label
            let label_col_name = table
                .headers()
                .iter()
                .find(|h| {
                    let l = h.to_lowercase();
                    l != "status" && l != "due date" && l != "owner"
                })
                .cloned();
            let label_col = label_col_name
                .as_deref()
                .and_then(|name| table.get_column(name));

            let mut pending_items: Vec<String> = Vec::new();
            let mut in_progress_items: Vec<String> = Vec::new();
            let mut overdue = 0usize;

            for (i, status) in status_col.iter().enumerate() {
                let s = status.trim().to_lowercase();
                if s == "pending" || s == "in-progress" || s == "in progress" {
                    let label = label_col
                        .as_ref()
                        .and_then(|col| col.get(i).copied())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());

                    if s == "pending" {
                        pending_items.push(label.unwrap_or_else(|| "(untitled)".into()));
                    } else {
                        in_progress_items.push(label.unwrap_or_else(|| "(untitled)".into()));
                    }
                    // Check if overdue
                    if let Some(ref dues) = due_col {
                        if let Some(due) = dues.get(i) {
                            let due = due.trim();
                            if !due.is_empty() {
                                if let Some(diff) = days_between(today, due) {
                                    if diff > 0 {
                                        overdue += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let total = pending_items.len() + in_progress_items.len();
            if total == 0 {
                continue;
            }

            let severity = if overdue > 0 {
                SuggestSeverity::Warning
            } else {
                SuggestSeverity::Info
            };

            let mut parts = Vec::new();
            if !pending_items.is_empty() {
                parts.push(format!("{} pending", pending_items.len()));
            }
            if !in_progress_items.is_empty() {
                parts.push(format!("{} in-progress", in_progress_items.len()));
            }

            let msg = if overdue > 0 {
                format!(
                    "{total} open action item(s), {overdue} overdue ({})",
                    parts.join(", ")
                )
            } else {
                format!("{total} open action item(s) ({})", parts.join(", "))
            };

            // Collect all item titles for the hint
            let mut all_items: Vec<String> = Vec::new();
            for item in &pending_items {
                all_items.push(format!("- [pending] {item}"));
            }
            for item in &in_progress_items {
                all_items.push(format!("- [in-progress] {item}"));
            }

            suggestions.push(Suggestion {
                severity,
                category: SuggestCategory::OpenActionItem,
                message: msg,
                hint: Some(all_items.join("\n")),
            });
        }
    }
}

/// 3. Stale documents — check dates in frontmatter
fn check_stale_document(
    doc: &Document,
    doc_type: Option<&str>,
    today: &str,
    suggestions: &mut Vec<Suggestion>,
) {
    let fm = match &doc.frontmatter {
        Some(fm) => fm,
        None => return,
    };

    let status = fm.get_display("status").unwrap_or_default().to_lowercase();

    match doc_type {
        Some("pol") => {
            if let Some(review_date) = fm.get_display("review_date") {
                if let Some(diff) = days_between(today, &review_date) {
                    if diff > 0 {
                        suggestions.push(Suggestion {
                            severity: SuggestSeverity::Warning,
                            category: SuggestCategory::StaleDocument,
                            message: format!("review_date {review_date} has passed"),
                            hint: Some("schedule a policy review".into()),
                        });
                    } else if diff > -30 {
                        suggestions.push(Suggestion {
                            severity: SuggestSeverity::Info,
                            category: SuggestCategory::StaleDocument,
                            message: format!("review_date {review_date} is within 30 days"),
                            hint: Some("consider scheduling a policy review soon".into()),
                        });
                    }
                }
            }
        }
        Some("opp") => {
            if matches!(
                status.as_str(),
                "identified" | "validating" | "draft" | "proposed" | "exploring"
            ) {
                if let Some(date) = fm.get_display("date") {
                    if let Some(diff) = days_between(today, &date) {
                        if diff > 90 {
                            suggestions.push(Suggestion {
                                severity: SuggestSeverity::Warning,
                                category: SuggestCategory::StaleDocument,
                                message: format!(
                                    "opportunity in \"{status}\" status since {date} (>{diff} days)"
                                ),
                                hint: Some("consider advancing or parking this opportunity".into()),
                            });
                        }
                    }
                }
            }
        }
        Some("inc") => {
            if status != "resolved" && status != "postmortem" {
                suggestions.push(Suggestion {
                    severity: SuggestSeverity::Info,
                    category: SuggestCategory::StaleDocument,
                    message: format!("unresolved incident (status: {status})"),
                    hint: Some("resolve the incident and complete the postmortem".into()),
                });
            }
        }
        _ => {}
    }
}

/// 4. Missing cross-references — check graph connectivity
fn check_missing_cross_refs(
    doc_id: &str,
    doc_type: Option<&str>,
    graph: &DocGraph,
    suggestions: &mut Vec<Suggestion>,
) {
    let outgoing = graph.refs_from(doc_id);
    let incoming = graph.refs_to(doc_id);

    if outgoing.is_empty() && incoming.is_empty() {
        suggestions.push(Suggestion {
            severity: SuggestSeverity::Info,
            category: SuggestCategory::MissingCrossRef,
            message: "no linked documents".into(),
            hint: Some("consider linking related ADRs, POLs, OPPs, or INCs".into()),
        });
        return;
    }

    // INC without follow-up ADR or POL
    if doc_type == Some("inc") {
        let has_followup = outgoing.iter().any(|e| {
            let target_upper = e.to.to_uppercase();
            target_upper.starts_with("ADR-") || target_upper.starts_with("POL-")
        });
        if !has_followup {
            suggestions.push(Suggestion {
                severity: SuggestSeverity::Info,
                category: SuggestCategory::MissingCrossRef,
                message: "no follow-up ADR/POL linked".into(),
                hint: Some("consider creating an ADR or POL to address incident findings".into()),
            });
        }
    }
}

/// 5. Missing optional sections — walk schema sections
fn check_missing_optional_sections(
    doc: &Document,
    type_def: &TypeDef,
    suggestions: &mut Vec<Suggestion>,
) {
    let parsed = doc.parse_body();
    walk_optional_sections(&parsed.sections, &type_def.sections, true, suggestions);
}

/// Recursively search for a section by name (case-insensitive) in the parsed tree.
/// Mirrors `ParsedBody::find_section` so H2 sections under an H1 title are found.
fn find_section_recursive<'a>(
    sections: &'a [crate::document::ParsedSection],
    name: &str,
) -> Option<&'a crate::document::ParsedSection> {
    for s in sections {
        if s.heading.eq_ignore_ascii_case(name) {
            return Some(s);
        }
        if let Some(found) = find_section_recursive(&s.children, name) {
            return Some(found);
        }
    }
    None
}

fn walk_optional_sections(
    parsed_sections: &[crate::document::ParsedSection],
    schema_sections: &[SectionDef],
    is_top_level: bool,
    suggestions: &mut Vec<Suggestion>,
) {
    for sec_def in schema_sections {
        // Search recursively (like validation does) so H2 sections nested under
        // an H1 title heading are found at top level.
        let found = find_section_recursive(parsed_sections, &sec_def.name);

        match found {
            Some(parsed_sec) => {
                // Section exists — check optional children
                if !sec_def.children.is_empty() {
                    walk_optional_sections(
                        &parsed_sec.children,
                        &sec_def.children,
                        false,
                        suggestions,
                    );
                }
            }
            None => {
                if !sec_def.required {
                    // Only suggest if parent exists (or it's top-level)
                    if is_top_level {
                        let hint = sec_def
                            .description
                            .as_ref()
                            .map(|d| format!("add section: {d}"))
                            .unwrap_or_else(|| format!("add a \"## {}\" section", sec_def.name));
                        suggestions.push(Suggestion {
                            severity: SuggestSeverity::Info,
                            category: SuggestCategory::MissingOptionalSection,
                            message: format!("missing optional section \"{}\"", sec_def.name),
                            hint: Some(hint),
                        });
                    }
                }
                // If required section is missing, validate catches it — skip children
            }
        }
    }
}

/// 6. Missing diagrams — check optional diagram defs where section exists
fn check_missing_diagrams(doc: &Document, type_def: &TypeDef, suggestions: &mut Vec<Suggestion>) {
    let parsed = doc.parse_body();
    walk_diagram_sections(&parsed.sections, &type_def.sections, suggestions);
}

fn walk_diagram_sections(
    parsed_sections: &[crate::document::ParsedSection],
    schema_sections: &[SectionDef],
    suggestions: &mut Vec<Suggestion>,
) {
    for sec_def in schema_sections {
        let found = find_section_recursive(parsed_sections, &sec_def.name);

        if let Some(parsed_sec) = found {
            // Check for optional diagram
            if let Some(ref diagram_def) = sec_def.diagram {
                if !diagram_def.required {
                    let has_diagram = if let Some(ref expected_type) = diagram_def.diagram_type {
                        let expected = expected_type.to_lowercase();
                        parsed_sec
                            .code_block_languages
                            .iter()
                            .any(|lang| lang == &expected)
                    } else {
                        parsed_sec
                            .code_block_languages
                            .iter()
                            .any(|lang| DIAGRAM_LANGUAGES.iter().any(|dl| lang == dl))
                    };

                    if !has_diagram {
                        let hint = if let Some(ref dt) = diagram_def.diagram_type {
                            format!("add a ```{dt} code block")
                        } else {
                            format!(
                                "add a diagram code block ({})",
                                DIAGRAM_LANGUAGES.join(", ")
                            )
                        };
                        suggestions.push(Suggestion {
                            severity: SuggestSeverity::Info,
                            category: SuggestCategory::MissingDiagram,
                            message: format!(
                                "section \"{}\" allows a diagram but has none",
                                sec_def.name
                            ),
                            hint: Some(hint),
                        });
                    }
                }
            }

            // Recurse into children
            if !sec_def.children.is_empty() {
                walk_diagram_sections(&parsed_sec.children, &sec_def.children, suggestions);
            }
        }
    }
}

/// 7. Low-quality content — sections that are too short or only contain markers
fn check_low_quality_content(
    doc: &Document,
    type_def: &TypeDef,
    suggestions: &mut Vec<Suggestion>,
) {
    let parsed = doc.parse_body();
    check_section_quality(&parsed.sections, &type_def.sections, suggestions);
}

fn check_section_quality(
    parsed_sections: &[crate::document::ParsedSection],
    schema_sections: &[SectionDef],
    suggestions: &mut Vec<Suggestion>,
) {
    let re = match MARKER_RE.as_ref() {
        Some(re) => re,
        None => return,
    };

    for sec_def in schema_sections {
        let found = parsed_sections
            .iter()
            .find(|s| s.heading.eq_ignore_ascii_case(&sec_def.name));

        if let Some(parsed_sec) = found {
            // Check if section content is only markers
            let content = &parsed_sec.content;
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                // Count words (split on whitespace)
                let word_count = trimmed.split_whitespace().count();
                let marker_count = re.find_iter(trimmed).count();

                // If all meaningful words are markers, flag it
                if marker_count > 0 && word_count <= marker_count * 2 {
                    suggestions.push(Suggestion {
                        severity: SuggestSeverity::Warning,
                        category: SuggestCategory::LowQualityContent,
                        message: format!(
                            "section \"{}\" contains only placeholder text",
                            sec_def.name
                        ),
                        hint: Some("replace placeholder content with actual substance".into()),
                    });
                } else if word_count < 5 && sec_def.required {
                    // Very short required section (< 5 words)
                    suggestions.push(Suggestion {
                        severity: SuggestSeverity::Info,
                        category: SuggestCategory::LowQualityContent,
                        message: format!(
                            "section \"{}\" has very little content ({} words)",
                            sec_def.name, word_count
                        ),
                        hint: Some("consider expanding this section".into()),
                    });
                }
            }

            // Recurse into children
            if !sec_def.children.is_empty() {
                check_section_quality(&parsed_sec.children, &sec_def.children, suggestions);
            }
        }
    }
}

/// Check team doc for recommended sections and quality issues.
fn check_team_doc(body: &str) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();

    // Parse h2 headings from the body
    let doc = match Document::from_str(body) {
        Ok(d) => d,
        Err(_) => return suggestions,
    };
    let parsed = doc.parse_body();
    let headings: Vec<String> = parsed.sections.iter().map(|s| s.heading.clone()).collect();

    // Check for missing recommended sections
    for &section in RECOMMENDED_TEAM_SECTIONS {
        if !headings.iter().any(|h| h.eq_ignore_ascii_case(section)) {
            suggestions.push(Suggestion {
                severity: SuggestSeverity::Info,
                category: SuggestCategory::MissingOptionalSection,
                message: format!("missing recommended section \"{section}\""),
                hint: Some(format!("add a \"## {section}\" section")),
            });
        }
    }

    // Check for empty first paragraph (needed for description)
    let first_line = body.lines().next().unwrap_or("").trim();
    if first_line.is_empty() || first_line.starts_with('#') {
        suggestions.push(Suggestion {
            severity: SuggestSeverity::Info,
            category: SuggestCategory::LowQualityContent,
            message: "missing team description (first paragraph)".into(),
            hint: Some("add a brief description paragraph before the first heading".into()),
        });
    }

    // Reuse incomplete markers check
    check_incomplete_markers(body, &mut suggestions);

    suggestions
}

/// Flatten all sections (including children) into an iterator.
fn all_sections(
    sections: &[crate::document::ParsedSection],
) -> Vec<&crate::document::ParsedSection> {
    let mut result = Vec::new();
    for sec in sections {
        result.push(sec);
        result.extend(all_sections(&sec.children));
    }
    result
}

// ─── Main entry point ──────────────────────────────────────────────────────

/// Discover and analyze documents, returning improvement suggestions.
///
/// `today` is "YYYY-MM-DD" for testability. CLI passes the system date.
pub fn suggest_directory(
    dir: impl AsRef<Path>,
    schema: &Schema,
    pattern: Option<&str>,
    today: &str,
) -> crate::error::Result<SuggestResult> {
    let dir = dir.as_ref();
    let files = crate::discovery::discover_files(dir, pattern, &[], false)?;

    // Build graph once for cross-ref checks
    let graph = DocGraph::build(dir, schema)?;

    // Process files in parallel
    let mut file_results: Vec<FileSuggestions> = files
        .par_iter()
        .filter_map(|path| {
            let doc = Document::from_file(path).ok()?;

            // Skip singletons
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if schema
                .types
                .iter()
                .any(|t| t.singleton && t.match_pattern.as_deref() == Some(filename))
            {
                return None;
            }

            let fm = doc.frontmatter.as_ref()?;
            let doc_type = fm
                .get_display("type")
                .or_else(|| crate::validation::infer_type_from_path(path, dir, schema));

            let type_def = doc_type.as_deref().and_then(|t| schema.get_type(t));

            let doc_id = crate::graph::path_to_id(path);
            let title = fm
                .get_display("title")
                .or_else(|| crate::ast_util::first_heading_text(&doc.body));
            let status = fm.get_display("status");

            let mut suggestions = Vec::new();

            // 1. Incomplete markers
            check_incomplete_markers(&doc.body, &mut suggestions);

            // 2. Open action items
            check_open_action_items(&doc, today, &mut suggestions);

            // 3. Stale documents
            check_stale_document(&doc, doc_type.as_deref(), today, &mut suggestions);

            // 4. Missing cross-references
            check_missing_cross_refs(&doc_id, doc_type.as_deref(), &graph, &mut suggestions);

            // 5. Missing optional sections
            if let Some(td) = type_def {
                check_missing_optional_sections(&doc, td, &mut suggestions);
            }

            // 6. Missing diagrams
            if let Some(td) = type_def {
                check_missing_diagrams(&doc, td, &mut suggestions);
            }

            // 7. Low-quality content
            if let Some(td) = type_def {
                check_low_quality_content(&doc, td, &mut suggestions);
            }

            let display_path = path.strip_prefix(dir).unwrap_or(path).display().to_string();

            Some(FileSuggestions {
                path: display_path,
                doc_id,
                title,
                doc_type,
                status,
                suggestions,
            })
        })
        .collect();

    // Team docs: discover docs/teams/*.md and check for recommended sections
    let teams_dir = dir.join("docs/teams");
    if teams_dir.is_dir() {
        let team_files: Vec<_> = std::fs::read_dir(&teams_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
            .collect();

        let team_suggestions: Vec<FileSuggestions> = team_files
            .par_iter()
            .filter_map(|entry| {
                let path = entry.path();
                let content = std::fs::read_to_string(&path).ok()?;
                let suggestions = check_team_doc(&content);
                if suggestions.is_empty() {
                    return None;
                }
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                let display_path = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                Some(FileSuggestions {
                    path: display_path,
                    doc_id: format!("team/{stem}"),
                    title: Some(stem.to_string()),
                    doc_type: Some("team".to_string()),
                    status: None,
                    suggestions,
                })
            })
            .collect();

        file_results.extend(team_suggestions);
    }

    Ok(SuggestResult { file_results })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Date math ─────────────────────────────────────────────────────────

    #[test]
    fn test_parse_date() {
        assert_eq!(parse_date("2025-01-15"), Some((2025, 1, 15)));
        assert_eq!(parse_date("bad"), None);
        assert_eq!(parse_date("2025-13-01"), None);
    }

    #[test]
    fn test_days_between() {
        // Same day
        assert_eq!(days_between("2025-01-15", "2025-01-15"), Some(0));
        // 1 day later
        assert_eq!(days_between("2025-01-16", "2025-01-15"), Some(1));
        // 1 day earlier
        assert_eq!(days_between("2025-01-14", "2025-01-15"), Some(-1));
        // ~90 days
        let diff = days_between("2025-04-15", "2025-01-15").unwrap();
        assert!(diff >= 89 && diff <= 91);
    }

    // ─── Incomplete markers ────────────────────────────────────────────────

    #[test]
    fn test_incomplete_markers_found() {
        let mut suggestions = Vec::new();
        check_incomplete_markers("This is TBD and also has a FIXME here.", &mut suggestions);
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].message.contains('2'));
        assert_eq!(suggestions[0].category, SuggestCategory::IncompleteMarker);
    }

    #[test]
    fn test_incomplete_markers_none() {
        let mut suggestions = Vec::new();
        check_incomplete_markers("All content is complete.", &mut suggestions);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_incomplete_markers_case_insensitive() {
        let mut suggestions = Vec::new();
        check_incomplete_markers("todo: fix this", &mut suggestions);
        assert_eq!(suggestions.len(), 1);
    }

    #[test]
    fn test_incomplete_markers_bracketed() {
        let mut suggestions = Vec::new();
        check_incomplete_markers("Status: [TBD]", &mut suggestions);
        assert_eq!(suggestions.len(), 1);
    }

    // ─── Open action items ─────────────────────────────────────────────────

    #[test]
    fn test_open_action_items() {
        let doc = Document::from_str(
            "---\ntype: inc\ntitle: T\nstatus: resolved\n---\n\n# Action Items\n\n| Action | Status | Due Date |\n|---|---|---|\n| Fix bug | pending | 2025-01-01 |\n| Test | completed | 2025-01-02 |\n",
        )
        .unwrap();
        let mut suggestions = Vec::new();
        check_open_action_items(&doc, "2025-02-01", &mut suggestions);
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].message.contains("1 open"));
        assert!(suggestions[0].message.contains("overdue"));
        assert_eq!(suggestions[0].severity, SuggestSeverity::Warning);
        let hint = suggestions[0].hint.as_ref().unwrap();
        assert!(
            hint.contains("Fix bug"),
            "hint should contain item title: {hint}"
        );
    }

    #[test]
    fn test_open_action_items_not_overdue() {
        let doc = Document::from_str(
            "---\ntype: inc\ntitle: T\nstatus: resolved\n---\n\n# Items\n\n| Action | Status | Due Date |\n|---|---|---|\n| Fix bug | pending | 2099-01-01 |\n",
        )
        .unwrap();
        let mut suggestions = Vec::new();
        check_open_action_items(&doc, "2025-02-01", &mut suggestions);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].severity, SuggestSeverity::Info);
        let hint = suggestions[0].hint.as_ref().unwrap();
        assert!(
            hint.contains("[pending] Fix bug"),
            "hint should list item: {hint}"
        );
    }

    #[test]
    fn test_no_open_action_items() {
        let doc = Document::from_str(
            "---\ntype: inc\ntitle: T\nstatus: resolved\n---\n\n# Items\n\n| Action | Status |\n|---|---|\n| Done | completed |\n",
        )
        .unwrap();
        let mut suggestions = Vec::new();
        check_open_action_items(&doc, "2025-02-01", &mut suggestions);
        assert!(suggestions.is_empty());
    }

    // ─── Stale documents ───────────────────────────────────────────────────

    #[test]
    fn test_stale_pol_review_passed() {
        let doc = Document::from_str(
            "---\ntype: pol\ntitle: T\nstatus: active\nreview_date: \"2025-01-01\"\n---\n\n# Body\n",
        )
        .unwrap();
        let mut suggestions = Vec::new();
        check_stale_document(&doc, Some("pol"), "2025-02-01", &mut suggestions);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].severity, SuggestSeverity::Warning);
        assert!(suggestions[0].message.contains("review_date"));
    }

    #[test]
    fn test_stale_pol_review_upcoming() {
        let doc = Document::from_str(
            "---\ntype: pol\ntitle: T\nstatus: active\nreview_date: \"2025-02-15\"\n---\n\n# Body\n",
        )
        .unwrap();
        let mut suggestions = Vec::new();
        check_stale_document(&doc, Some("pol"), "2025-02-01", &mut suggestions);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].severity, SuggestSeverity::Info);
        assert!(suggestions[0].message.contains("within 30 days"));
    }

    #[test]
    fn test_stale_opp_old() {
        let doc = Document::from_str(
            "---\ntype: opp\ntitle: T\nstatus: exploring\ndate: \"2024-01-01\"\n---\n\n# Body\n",
        )
        .unwrap();
        let mut suggestions = Vec::new();
        check_stale_document(&doc, Some("opp"), "2025-06-01", &mut suggestions);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].severity, SuggestSeverity::Warning);
    }

    #[test]
    fn test_stale_inc_unresolved() {
        let doc =
            Document::from_str("---\ntype: inc\ntitle: T\nstatus: open\n---\n\n# Body\n").unwrap();
        let mut suggestions = Vec::new();
        check_stale_document(&doc, Some("inc"), "2025-02-01", &mut suggestions);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].severity, SuggestSeverity::Info);
        assert!(suggestions[0].message.contains("unresolved"));
    }

    #[test]
    fn test_stale_inc_resolved_no_suggestion() {
        let doc = Document::from_str("---\ntype: inc\ntitle: T\nstatus: resolved\n---\n\n# Body\n")
            .unwrap();
        let mut suggestions = Vec::new();
        check_stale_document(&doc, Some("inc"), "2025-02-01", &mut suggestions);
        assert!(suggestions.is_empty());
    }

    // ─── Missing cross-references ──────────────────────────────────────────

    #[test]
    fn test_missing_cross_refs_orphan() {
        use std::collections::BTreeMap;
        use std::path::PathBuf;
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "ADR-001".into(),
            crate::graph::DocNode {
                id: "ADR-001".into(),
                path: PathBuf::from("adr-001.md"),
                doc_type: Some("adr".into()),
                title: Some("Test".into()),
                status: None,
            },
        );
        let graph = DocGraph {
            nodes,
            edges: vec![],
        };
        let mut suggestions = Vec::new();
        check_missing_cross_refs("ADR-001", Some("adr"), &graph, &mut suggestions);
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].message.contains("no linked"));
    }

    #[test]
    fn test_missing_cross_refs_inc_no_followup() {
        use std::collections::BTreeMap;
        use std::path::PathBuf;
        let mut nodes = BTreeMap::new();
        nodes.insert(
            "INC-001".into(),
            crate::graph::DocNode {
                id: "INC-001".into(),
                path: PathBuf::from("inc-001.md"),
                doc_type: Some("inc".into()),
                title: Some("Test".into()),
                status: None,
            },
        );
        nodes.insert(
            "INC-002".into(),
            crate::graph::DocNode {
                id: "INC-002".into(),
                path: PathBuf::from("inc-002.md"),
                doc_type: Some("inc".into()),
                title: Some("Test 2".into()),
                status: None,
            },
        );
        let graph = DocGraph {
            nodes,
            edges: vec![crate::graph::DocEdge {
                from: "INC-001".into(),
                to: "INC-002".into(),
                relation: "related".into(),
            }],
        };
        let mut suggestions = Vec::new();
        check_missing_cross_refs("INC-001", Some("inc"), &graph, &mut suggestions);
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].message.contains("no follow-up ADR/POL"));
    }

    // ─── Missing optional sections ─────────────────────────────────────────

    #[test]
    fn test_missing_optional_section() {
        let doc =
            Document::from_str("---\ntype: doc\ntitle: T\n---\n\n# Required\n\nContent\n").unwrap();
        let type_def = TypeDef {
            name: "doc".into(),
            description: None,
            aliases: vec![],
            folder: None,
            max_count: None,
            singleton: false,
            match_pattern: None,
            fields: vec![],
            sections: vec![
                SectionDef {
                    name: "Required".into(),
                    required: true,
                    description: None,
                    children: vec![],
                    table: None,
                    content: None,
                    list: None,
                    diagram: None,
                    min_subsections: None,
                    callout_required: false,
                },
                SectionDef {
                    name: "Optional".into(),
                    required: false,
                    description: Some("Extra details".into()),
                    children: vec![],
                    table: None,
                    content: None,
                    list: None,
                    diagram: None,
                    min_subsections: None,
                    callout_required: false,
                },
            ],
            rules: vec![],
            nav_label: None,
        };
        let mut suggestions = Vec::new();
        check_missing_optional_sections(&doc, &type_def, &mut suggestions);
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].message.contains("Optional"));
        assert_eq!(
            suggestions[0].category,
            SuggestCategory::MissingOptionalSection
        );
    }

    #[test]
    fn test_optional_section_present_no_suggestion() {
        let doc =
            Document::from_str("---\ntype: doc\ntitle: T\n---\n\n# Optional\n\nContent\n").unwrap();
        let type_def = TypeDef {
            name: "doc".into(),
            description: None,
            aliases: vec![],
            folder: None,
            max_count: None,
            singleton: false,
            match_pattern: None,
            fields: vec![],
            sections: vec![SectionDef {
                name: "Optional".into(),
                required: false,
                description: None,
                children: vec![],
                table: None,
                content: None,
                list: None,
                diagram: None,
                min_subsections: None,
                callout_required: false,
            }],
            rules: vec![],
            nav_label: None,
        };
        let mut suggestions = Vec::new();
        check_missing_optional_sections(&doc, &type_def, &mut suggestions);
        assert!(suggestions.is_empty());
    }

    // ─── Missing diagrams ──────────────────────────────────────────────────

    #[test]
    fn test_missing_optional_diagram() {
        let doc =
            Document::from_str("---\ntype: doc\ntitle: T\n---\n\n# Arch\n\nJust text.\n").unwrap();
        let type_def = TypeDef {
            name: "doc".into(),
            description: None,
            aliases: vec![],
            folder: None,
            max_count: None,
            singleton: false,
            match_pattern: None,
            fields: vec![],
            sections: vec![SectionDef {
                name: "Arch".into(),
                required: true,
                description: None,
                children: vec![],
                table: None,
                content: None,
                list: None,
                diagram: Some(crate::schema::DiagramDef {
                    required: false,
                    diagram_type: Some("d2".into()),
                }),
                min_subsections: None,
                callout_required: false,
            }],
            rules: vec![],
            nav_label: None,
        };
        let mut suggestions = Vec::new();
        check_missing_diagrams(&doc, &type_def, &mut suggestions);
        assert_eq!(suggestions.len(), 1);
        assert!(suggestions[0].message.contains("Arch"));
        assert!(suggestions[0].hint.as_ref().unwrap().contains("d2"));
    }

    #[test]
    fn test_diagram_present_no_suggestion() {
        let doc = Document::from_str(
            "---\ntype: doc\ntitle: T\n---\n\n# Arch\n\n```d2\nshape: oval\n```\n",
        )
        .unwrap();
        let type_def = TypeDef {
            name: "doc".into(),
            description: None,
            aliases: vec![],
            folder: None,
            max_count: None,
            singleton: false,
            match_pattern: None,
            fields: vec![],
            sections: vec![SectionDef {
                name: "Arch".into(),
                required: true,
                description: None,
                children: vec![],
                table: None,
                content: None,
                list: None,
                diagram: Some(crate::schema::DiagramDef {
                    required: false,
                    diagram_type: Some("d2".into()),
                }),
                min_subsections: None,
                callout_required: false,
            }],
            rules: vec![],
            nav_label: None,
        };
        let mut suggestions = Vec::new();
        check_missing_diagrams(&doc, &type_def, &mut suggestions);
        assert!(suggestions.is_empty());
    }

    // ─── Directory-level integration test ──────────────────────────────────

    #[test]
    fn test_suggest_directory_fixtures() {
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = crate::schema::Schema::from_str(&schema_content).unwrap();
        let result =
            suggest_directory("../../tests/fixtures", &schema, None, "2025-06-01").unwrap();

        // Should have results for fixture docs
        assert!(!result.file_results.is_empty(), "should have file results");

        // Should find suggestions across fixtures
        let total = result.total();
        assert!(total > 0, "should find at least one suggestion");
    }
}

use std::path::Path;

use crate::document::Document;
use crate::graph::DocGraph;
use crate::schema::{Schema, TypeDef};

/// Coverage metrics for a set of decision documents.
#[derive(Debug, Clone)]
pub struct CoverageReport {
    /// Total documents analyzed
    pub total_docs: usize,
    /// Documents by type
    pub type_counts: Vec<TypeCount>,
    /// Completeness: % of optional fields/sections filled (per doc, averaged)
    pub completeness_pct: f64,
    /// Linkage: % of docs with at least one inbound or outbound reference
    pub linkage_pct: f64,
    /// Freshness: number of stale docs (proposed >30 days, expired review_date)
    pub stale_count: usize,
    /// code_paths coverage: % of ADR/POL docs with non-empty code_paths
    pub code_paths_pct: f64,
    /// Per-file details
    pub files: Vec<FileCoverage>,
}

/// Document count for a single type (e.g. "adr": 5).
#[derive(Debug, Clone)]
pub struct TypeCount {
    /// Type name (e.g. "adr", "pol")
    pub doc_type: String,
    /// Number of documents of this type
    pub count: usize,
}

/// Coverage metrics for a single document.
#[derive(Debug, Clone)]
pub struct FileCoverage {
    /// Relative path to the document
    pub path: String,
    /// Document ID (e.g. "ADR-001")
    pub doc_id: String,
    /// Document type (e.g. "adr", "pol")
    pub doc_type: Option<String>,
    /// % of optional fields present (0.0-100.0)
    pub field_completeness: f64,
    /// % of optional sections present (0.0-100.0)
    pub section_completeness: f64,
    /// Has at least one cross-reference
    pub has_refs: bool,
    /// Has code_paths set
    pub has_code_paths: bool,
    /// Is stale (proposed >30 days or expired review_date)
    pub is_stale: bool,
}

/// Analyze coverage of decision documents in a directory.
pub fn coverage_report(
    dir: impl AsRef<Path>,
    schema: &Schema,
    today: &str,
) -> crate::error::Result<CoverageReport> {
    let dir = dir.as_ref();
    let files = crate::discovery::discover_files(dir, None, &[], false)?;
    let graph = DocGraph::build(dir, schema)?;

    let mut file_coverages = Vec::new();
    let mut type_map: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut code_paths_eligible = 0usize;
    let mut code_paths_present = 0usize;

    for path in &files {
        let doc = match Document::from_file(path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Skip singletons
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if schema
            .types
            .iter()
            .any(|t| t.singleton && t.match_pattern.as_deref() == Some(filename))
        {
            continue;
        }

        let fm = match &doc.frontmatter {
            Some(fm) => fm,
            None => continue,
        };

        let doc_type = fm
            .get_display("type")
            .or_else(|| crate::validation::infer_type_from_path(path, dir, schema));

        let type_def = doc_type.as_deref().and_then(|t| schema.get_type(t));

        let doc_id = crate::graph::path_to_id(path);

        if let Some(ref dt) = doc_type {
            *type_map.entry(dt.clone()).or_default() += 1;
        }

        // Field completeness: % of optional fields present
        let field_completeness = type_def
            .map(|td| field_completeness_pct(fm, td))
            .unwrap_or(100.0);

        // Section completeness: % of optional sections present
        let section_completeness = type_def
            .map(|td| section_completeness_pct(&doc, td))
            .unwrap_or(100.0);

        // Linkage
        let outgoing = graph.refs_from(&doc_id);
        let incoming = graph.refs_to(&doc_id);
        let has_refs = !outgoing.is_empty() || !incoming.is_empty();

        // code_paths
        let has_code_paths = fm.get("code_paths").is_some_and(|v| match v {
            serde_yaml::Value::Sequence(seq) => !seq.is_empty(),
            serde_yaml::Value::String(s) => !s.is_empty(),
            _ => false,
        });

        // Track code_paths for ADR/POL
        if matches!(doc_type.as_deref(), Some("adr") | Some("pol")) {
            code_paths_eligible += 1;
            if has_code_paths {
                code_paths_present += 1;
            }
        }

        // Staleness
        let is_stale = check_staleness(fm, doc_type.as_deref(), today);

        let display_path = path.strip_prefix(dir).unwrap_or(path).display().to_string();

        file_coverages.push(FileCoverage {
            path: display_path,
            doc_id,
            doc_type: doc_type.clone(),
            field_completeness,
            section_completeness,
            has_refs,
            has_code_paths,
            is_stale,
        });
    }

    let total_docs = file_coverages.len();

    let completeness_pct = if total_docs > 0 {
        let sum: f64 = file_coverages
            .iter()
            .map(|f| (f.field_completeness + f.section_completeness) / 2.0)
            .sum();
        sum / total_docs as f64
    } else {
        100.0
    };

    let linkage_pct = if total_docs > 0 {
        let linked = file_coverages.iter().filter(|f| f.has_refs).count();
        (linked as f64 / total_docs as f64) * 100.0
    } else {
        100.0
    };

    let stale_count = file_coverages.iter().filter(|f| f.is_stale).count();

    let code_paths_pct = if code_paths_eligible > 0 {
        (code_paths_present as f64 / code_paths_eligible as f64) * 100.0
    } else {
        100.0
    };

    let type_counts: Vec<TypeCount> = type_map
        .into_iter()
        .map(|(doc_type, count)| TypeCount { doc_type, count })
        .collect();

    Ok(CoverageReport {
        total_docs,
        type_counts,
        completeness_pct,
        linkage_pct,
        stale_count,
        code_paths_pct,
        files: file_coverages,
    })
}

/// Calculate % of optional fields that are present.
fn field_completeness_pct(fm: &crate::frontmatter::Frontmatter, type_def: &TypeDef) -> f64 {
    let optional_fields: Vec<_> = type_def.fields.iter().filter(|f| !f.required).collect();

    if optional_fields.is_empty() {
        return 100.0;
    }

    let present = optional_fields
        .iter()
        .filter(|f| fm.get(&f.name).is_some())
        .count();

    (present as f64 / optional_fields.len() as f64) * 100.0
}

/// Calculate % of optional sections that are present.
fn section_completeness_pct(doc: &Document, type_def: &TypeDef) -> f64 {
    let parsed = doc.parse_body();
    let optional_sections = count_optional_sections(&type_def.sections);

    if optional_sections == 0 {
        return 100.0;
    }

    let present = count_present_optional_sections(&parsed.sections, &type_def.sections);
    (present as f64 / optional_sections as f64) * 100.0
}

fn count_optional_sections(defs: &[crate::schema::SectionDef]) -> usize {
    let mut count = 0;
    for def in defs {
        if !def.required {
            count += 1;
        }
        count += count_optional_sections(&def.children);
    }
    count
}

fn count_present_optional_sections(
    parsed: &[crate::document::ParsedSection],
    defs: &[crate::schema::SectionDef],
) -> usize {
    let mut count = 0;
    for def in defs {
        if !def.required
            && parsed
                .iter()
                .any(|s| s.heading.eq_ignore_ascii_case(&def.name))
        {
            count += 1;
        }
        // Recurse for children of sections that exist
        if let Some(ps) = parsed
            .iter()
            .find(|s| s.heading.eq_ignore_ascii_case(&def.name))
        {
            count += count_present_optional_sections(&ps.children, &def.children);
        }
    }
    count
}

/// Check if a document is stale based on its type and dates.
fn check_staleness(
    fm: &crate::frontmatter::Frontmatter,
    doc_type: Option<&str>,
    today: &str,
) -> bool {
    let status = fm.get_display("status").unwrap_or_default().to_lowercase();

    match doc_type {
        Some("pol") => {
            // Expired review_date
            if let Some(review_date) = fm.get_display("review_date") {
                if let Some(diff) = days_between(today, &review_date) {
                    if diff > 0 {
                        return true;
                    }
                }
            }
            false
        }
        Some("adr") => {
            // Proposed for >30 days
            if status == "proposed" {
                if let Some(date) = fm.get_display("date") {
                    if let Some(diff) = days_between(today, &date) {
                        return diff > 30;
                    }
                }
            }
            false
        }
        Some("opp") => {
            // Early stages >90 days
            if matches!(
                status.as_str(),
                "identified" | "validating" | "proposed" | "exploring"
            ) {
                if let Some(date) = fm.get_display("date") {
                    if let Some(diff) = days_between(today, &date) {
                        return diff > 90;
                    }
                }
            }
            false
        }
        Some("inc") => {
            // Unresolved incidents
            !matches!(status.as_str(), "resolved" | "postmortem")
        }
        _ => false,
    }
}

/// Parse "YYYY-MM-DD" and compute day difference.
fn days_between(a: &str, b: &str) -> Option<i64> {
    let parse = |s: &str| -> Option<(i64, i64, i64)> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() < 3 {
            return None;
        }
        let y = parts[0].parse::<i64>().ok()?;
        let m = parts[1].parse::<i64>().ok()?;
        let d = parts[2].parse::<i64>().ok()?;
        Some((y, m, d))
    };

    let epoch_days = |y: i64, m: i64, d: i64| -> i64 {
        let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
        365 * y + y / 4 - y / 100 + y / 400 + (m * 306 + 5) / 10 + d - 1
    };

    let (ay, am, ad) = parse(a)?;
    let (by, bm, bd) = parse(b)?;
    Some(epoch_days(ay, am, ad) - epoch_days(by, bm, bd))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_days_between() {
        assert_eq!(days_between("2025-02-01", "2025-01-01"), Some(31));
        assert_eq!(days_between("2025-01-01", "2025-01-01"), Some(0));
        assert_eq!(days_between("2025-01-01", "2025-02-01"), Some(-31));
    }

    #[test]
    fn test_coverage_fixtures() {
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = crate::schema::Schema::from_str(&schema_content).unwrap();
        let report = coverage_report("../../tests/fixtures", &schema, "2025-06-01").unwrap();

        assert!(report.total_docs > 0, "should find fixture docs");
        // Completeness should be between 0 and 100
        assert!(
            (0.0..=100.0).contains(&report.completeness_pct),
            "completeness should be 0-100, got {}",
            report.completeness_pct
        );
        assert!(
            (0.0..=100.0).contains(&report.linkage_pct),
            "linkage should be 0-100, got {}",
            report.linkage_pct
        );
    }
}

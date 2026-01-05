//! Open Questions: extract, add, resolve, remove, replace task-list questions
//! from the "Open Questions" section of decision documents.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::document::Document;
use crate::error::{Error, Result};
use crate::graph;
use crate::schema::Schema;
use crate::template;

/// Section heading used for open questions.
const SECTION_HEADING: &str = "Open Questions";

/// Regex matching a task list item: `- [ ] text` or `- [x] text`
static TASK_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^(\s*-\s+\[([ xX])\]\s+)(.+)$").ok());

/// Regex matching a bold label: `**Label:** rest`
static LABEL_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r"^\*\*([^*]+?):\*\*\s*(.*)$").ok());

/// A single question extracted from the Open Questions section.
#[derive(Debug, Clone)]
pub struct Question {
    /// 1-based position in section
    pub index: usize,
    /// `[x]` = true
    pub done: bool,
    /// Bold text before colon (if present)
    pub label: Option<String>,
    /// Full text after checkbox
    pub text: String,
    /// Original line
    pub raw_line: String,
}

/// Questions for a single document.
#[derive(Debug, Clone)]
pub struct DocQuestions {
    pub doc_id: String,
    pub path: PathBuf,
    pub title: Option<String>,
    pub questions: Vec<Question>,
}

/// Extract questions from a document's "Open Questions" section.
pub fn extract_questions(doc: &Document) -> Vec<Question> {
    let task_re = match TASK_RE.as_ref() {
        Some(re) => re,
        None => return Vec::new(),
    };

    let section = match doc.get_section(SECTION_HEADING) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut questions = Vec::new();
    let mut index = 0usize;

    for line in section.content.lines() {
        if let Some(caps) = task_re.captures(line) {
            index += 1;
            let check = &caps[2];
            let done = check == "x" || check == "X";
            let text = caps[3].to_string();
            let label = LABEL_RE
                .as_ref()
                .and_then(|re| re.captures(&text))
                .map(|c| c[1].to_string());

            questions.push(Question {
                index,
                done,
                label,
                text,
                raw_line: line.to_string(),
            });
        }
    }

    questions
}

/// Add a question to a document's "Open Questions" section.
/// Creates the section if missing.
pub fn add_question(doc: &mut Document, text: &str) -> Result<()> {
    let item = format!("- [ ] {text}\n");

    // Try appending to existing section
    if doc.get_section(SECTION_HEADING).is_ok() {
        doc.append_to_section(SECTION_HEADING, &item)?;
        return Ok(());
    }

    // Section doesn't exist — append it
    let section_md = format!("\n## {SECTION_HEADING}\n\n{item}");

    // For readme types, insert before License section if it exists
    if let Ok(license) = doc.get_section("License") {
        let _ = license; // just checking existence
        let insert_pos = find_section_start(&doc.body, "License");
        if let Some(pos) = insert_pos {
            let mut new_body = String::with_capacity(doc.body.len() + section_md.len());
            new_body.push_str(&doc.body[..pos]);
            new_body.push_str(&section_md);
            new_body.push('\n');
            new_body.push_str(&doc.body[pos..]);
            doc.body = new_body;
            doc.rebuild_raw();
            return Ok(());
        }
    }

    // For service-readme, insert before Decisions section if it exists
    if let Ok(decisions) = doc.get_section("Decisions") {
        let _ = decisions;
        let insert_pos = find_section_start(&doc.body, "Decisions");
        if let Some(pos) = insert_pos {
            let mut new_body = String::with_capacity(doc.body.len() + section_md.len());
            new_body.push_str(&doc.body[..pos]);
            new_body.push_str(&section_md);
            new_body.push('\n');
            new_body.push_str(&doc.body[pos..]);
            doc.body = new_body;
            doc.rebuild_raw();
            return Ok(());
        }
    }

    // Default: append to end
    doc.body.push_str(&section_md);
    doc.rebuild_raw();
    Ok(())
}

/// Mark a question as resolved (`[x]`) by fuzzy text match.
pub fn resolve_question(doc: &mut Document, match_text: &str) -> Result<()> {
    let (line, _) = find_question(doc, match_text)?;
    let new_line = line.replacen("[ ]", "[x]", 1);
    doc.body = doc.body.replacen(&line, &new_line, 1);
    doc.rebuild_raw();
    Ok(())
}

/// Remove a question by fuzzy text match.
pub fn remove_question(doc: &mut Document, match_text: &str) -> Result<()> {
    let (line, _) = find_question(doc, match_text)?;

    // Remove the line (and trailing newline)
    let target = format!("{line}\n");
    if doc.body.contains(&target) {
        doc.body = doc.body.replacen(&target, "", 1);
    } else {
        doc.body = doc.body.replacen(&line, "", 1);
    }
    doc.rebuild_raw();
    Ok(())
}

/// Replace a question's text by fuzzy text match.
pub fn replace_question(doc: &mut Document, match_text: &str, new_text: &str) -> Result<()> {
    let (line, q) = find_question(doc, match_text)?;
    let checkbox = if q.done { "[x]" } else { "[ ]" };
    let new_line = format!(
        "{}- {checkbox} {new_text}",
        &line[..line.len() - line.trim_start().len()]
    );
    doc.body = doc.body.replacen(&line, &new_line, 1);
    doc.rebuild_raw();
    Ok(())
}

/// Scan all documents in a directory for open questions.
pub fn scan_questions(
    dir: &Path,
    schema: &Schema,
    filter_type: Option<&str>,
) -> Result<Vec<DocQuestions>> {
    let files = crate::discovery::discover_files(dir, None, &[], false)?;
    let mut results = Vec::new();

    for path in &files {
        let doc = match Document::from_file(path) {
            Ok(d) => d,
            Err(_) => continue,
        };

        // Skip docs without frontmatter
        let fm = match &doc.frontmatter {
            Some(fm) => fm,
            None => continue,
        };

        // Type filter
        if let Some(ft) = filter_type {
            let doc_type = fm
                .get_display("type")
                .or_else(|| crate::validation::infer_type_from_path(path, dir, schema));
            let canonical = schema
                .get_type(ft)
                .map(|td| td.name.clone())
                .unwrap_or_else(|| ft.to_string());
            if doc_type.as_deref() != Some(&canonical) {
                continue;
            }
        }

        let questions = extract_questions(&doc);
        if questions.is_empty() {
            continue;
        }

        let doc_id = graph::path_to_id(path);
        let title = fm
            .get_display("title")
            .or_else(|| crate::ast_util::first_heading_text(&doc.body));

        results.push(DocQuestions {
            doc_id,
            path: path.clone(),
            title,
            questions,
        });
    }

    Ok(results)
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Find a question by fuzzy text match. Returns (original_line, Question).
fn find_question(doc: &Document, match_text: &str) -> Result<(String, Question)> {
    let questions = extract_questions(doc);
    if questions.is_empty() {
        return Err(Error::Other("no questions found in document".into()));
    }

    // Build candidates: labels and full text
    let candidates: Vec<&str> = questions
        .iter()
        .map(|q| q.label.as_deref().unwrap_or(&q.text))
        .collect();

    // Try exact substring match first (case-insensitive)
    let match_lower = match_text.to_lowercase();
    let matches: Vec<&Question> = questions
        .iter()
        .filter(|q| {
            let label_match = q
                .label
                .as_ref()
                .map(|l| l.to_lowercase().contains(&match_lower))
                .unwrap_or(false);
            let text_match = q.text.to_lowercase().contains(&match_lower);
            label_match || text_match
        })
        .collect();

    if matches.len() == 1 {
        let q = matches[0].clone();
        return Ok((q.raw_line.clone(), q));
    }

    if matches.len() > 1 {
        let labels: Vec<String> = matches
            .iter()
            .map(|q| q.label.as_deref().unwrap_or(&q.text).to_string())
            .collect();
        return Err(Error::Other(format!(
            "ambiguous match '{}', candidates: {}",
            match_text,
            labels.join(", ")
        )));
    }

    // Fall back to fuzzy match via Levenshtein
    if let Some(best) = template::closest_match(match_text, &candidates, 10) {
        let q = questions
            .iter()
            .find(|q| q.label.as_deref() == Some(best) || q.text == best)
            .cloned();
        if let Some(q) = q {
            return Ok((q.raw_line.clone(), q));
        }
    }

    Err(Error::Other(format!(
        "no question matching '{}' found",
        match_text
    )))
}

/// Find the byte offset where a section heading starts in the body.
fn find_section_start(body: &str, heading: &str) -> Option<usize> {
    let arena = comrak::Arena::new();
    let root = crate::ast_util::parse_md(&arena, body);
    let node = crate::ast_util::find_heading_by_text(root, heading)?;
    let range = crate::ast_util::section_byte_range(node, body);
    Some(range.start)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(body: &str) -> Document {
        let raw = format!("---\ntype: opp\ntitle: Test\nstatus: identified\nauthors:\n  - \"@alice\"\ndate: \"2025-01-01\"\n---\n\n{body}");
        Document::from_str(&raw).unwrap()
    }

    #[test]
    fn test_extract_questions_basic() {
        let doc = make_doc(
            "## Description\n\nSome text.\n\n## Open Questions\n\n- [ ] **Auth:** Which auth provider?\n- [x] **DB:** Use Postgres\n- [ ] Simple question\n",
        );
        let qs = extract_questions(&doc);
        assert_eq!(qs.len(), 3);
        assert!(!qs[0].done);
        assert_eq!(qs[0].label.as_deref(), Some("Auth"));
        assert!(qs[1].done);
        assert_eq!(qs[1].label.as_deref(), Some("DB"));
        assert!(!qs[2].done);
        assert!(qs[2].label.is_none());
    }

    #[test]
    fn test_extract_questions_no_section() {
        let doc = make_doc("## Description\n\nNo questions here.\n");
        let qs = extract_questions(&doc);
        assert!(qs.is_empty());
    }

    #[test]
    fn test_add_question_existing_section() {
        let mut doc = make_doc("## Open Questions\n\n- [ ] Existing?\n");
        add_question(&mut doc, "**New:** What about this?").unwrap();
        let qs = extract_questions(&doc);
        assert_eq!(qs.len(), 2);
        assert_eq!(qs[1].label.as_deref(), Some("New"));
    }

    #[test]
    fn test_add_question_creates_section() {
        let mut doc = make_doc("## Description\n\nSome text.\n");
        add_question(&mut doc, "**First:** Is this working?").unwrap();
        let qs = extract_questions(&doc);
        assert_eq!(qs.len(), 1);
        assert!(doc.body.contains("## Open Questions"));
    }

    #[test]
    fn test_resolve_question() {
        let mut doc = make_doc("## Open Questions\n\n- [ ] **Auth:** Which provider?\n");
        resolve_question(&mut doc, "Auth").unwrap();
        let qs = extract_questions(&doc);
        assert_eq!(qs.len(), 1);
        assert!(qs[0].done);
    }

    #[test]
    fn test_remove_question() {
        let mut doc = make_doc(
            "## Open Questions\n\n- [ ] **Auth:** Which provider?\n- [ ] **DB:** Which DB?\n",
        );
        remove_question(&mut doc, "Auth").unwrap();
        let qs = extract_questions(&doc);
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].label.as_deref(), Some("DB"));
    }

    #[test]
    fn test_replace_question() {
        let mut doc = make_doc("## Open Questions\n\n- [ ] **Auth:** Which provider?\n");
        replace_question(&mut doc, "Auth", "**Auth:** Use OAuth2 (decided)").unwrap();
        let qs = extract_questions(&doc);
        assert_eq!(qs.len(), 1);
        assert!(qs[0].text.contains("OAuth2"));
    }
}

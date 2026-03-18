use std::ops::Range;
use std::path::{Path, PathBuf};

use comrak::Arena;
use serde_yaml::Value;

use crate::ast_util;
use crate::error::{Error, Result};
use crate::frontmatter::Frontmatter;
use crate::section::Section;
use crate::table::Table;

/// Pre-parsed document body: all sections extracted in a single AST pass.
#[derive(Debug, Clone)]
pub struct ParsedBody {
    pub sections: Vec<ParsedSection>,
    /// Number of paragraphs before the first heading (preamble).
    pub preamble_paragraphs: usize,
}

/// A single section extracted from the parsed AST.
#[derive(Debug, Clone)]
pub struct ParsedSection {
    pub heading: String,
    pub level: u8,
    pub content: String,
    pub tables: Vec<Table>,
    pub paragraph_count: usize,
    pub list_has_list: bool,
    pub list_total_items: usize,
    pub list_is_ordered: bool,
    pub code_block_languages: Vec<String>,
    /// Code blocks as (language, content) pairs for diagram validation.
    pub code_blocks: Vec<(String, String)>,
    /// Whether section content contains a GFM callout (`> [!NOTE]`, `> [!WARNING]`, etc.).
    pub has_callout: bool,
    pub children: Vec<ParsedSection>,
}

impl ParsedBody {
    /// Find a section by heading (case-insensitive), searching the entire tree.
    /// Matches behavior of Document::get_section which finds any heading in the doc.
    pub fn find_section(&self, heading: &str) -> Option<&ParsedSection> {
        let target = heading.trim().to_lowercase();
        fn search<'a>(sections: &'a [ParsedSection], target: &str) -> Option<&'a ParsedSection> {
            for s in sections {
                if s.heading.trim().to_lowercase() == target {
                    return Some(s);
                }
                if let Some(found) = search(&s.children, target) {
                    return Some(found);
                }
            }
            None
        }
        search(&self.sections, &target)
    }
}

impl ParsedSection {
    /// Find a child section by heading (case-insensitive).
    pub fn find_child(&self, heading: &str) -> Option<&ParsedSection> {
        let target = heading.trim().to_lowercase();
        self.children
            .iter()
            .find(|s| s.heading.trim().to_lowercase() == target)
    }

    /// Find a nested section by path, e.g. ["Child", "Grandchild"].
    pub fn find_by_path(&self, path: &[&str]) -> Option<&ParsedSection> {
        if path.is_empty() {
            return Some(self);
        }
        let child = self.find_child(path[0])?;
        if path.len() == 1 {
            Some(child)
        } else {
            child.find_by_path(&path[1..])
        }
    }
}

#[derive(Debug, Clone)]
pub struct Document {
    pub path: Option<PathBuf>,
    pub raw: String,
    pub frontmatter: Option<Frontmatter>,
    pub body: String,
}

impl Document {
    /// Load a document from a file path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::FileNotFound(path.to_path_buf()));
        }
        let raw = std::fs::read_to_string(path)?;
        let mut doc = Self::from_str(&raw)?;
        doc.path = Some(path.to_path_buf());
        Ok(doc)
    }

    /// Parse a document from a string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(content: &str) -> Result<Self> {
        let (frontmatter, body) = Frontmatter::try_parse(content)?;
        Ok(Self {
            path: None,
            raw: content.to_string(),
            frontmatter,
            body,
        })
    }

    /// Get the frontmatter, returning error if absent.
    pub fn frontmatter(&self) -> Result<&Frontmatter> {
        self.frontmatter.as_ref().ok_or(Error::NoFrontmatter)
    }

    /// Extract the document title from the first H1 heading in the body.
    pub fn title(&self) -> Option<String> {
        ast_util::first_heading_text(&self.body)
    }

    /// Get a section by heading text (case-insensitive exact match).
    pub fn get_section(&self, heading: &str) -> Result<Section> {
        let arena = Arena::new();
        let root = ast_util::parse_md(&arena, &self.body);

        let heading_node = ast_util::find_heading_by_text(root, heading)
            .ok_or_else(|| Error::SectionNotFound(heading.to_string()))?;

        let level = ast_util::heading_level(heading_node).unwrap_or(1);
        let range = ast_util::section_byte_range(heading_node, &self.body);
        let raw = self.body[range.clone()].to_string();
        let content_range = ast_util::section_content_byte_range(heading_node, &self.body);
        let content = self.body[content_range].to_string();

        Ok(Section::new(
            ast_util::collect_text(heading_node),
            level,
            raw,
            content,
        ))
    }

    /// Get a nested section by path, e.g. ["Consequences", "Positive"].
    pub fn get_section_by_path(&self, path: &[&str]) -> Result<Section> {
        if path.is_empty() {
            return Err(Error::SectionNotFound("(empty path)".to_string()));
        }

        let mut section = self.get_section(path[0])?;
        for &name in &path[1..] {
            let sub = section
                .subsections()
                .into_iter()
                .find(|s| s.heading.trim().eq_ignore_ascii_case(name))
                .ok_or_else(|| Error::SectionNotFound(name.to_string()))?;
            section = sub;
        }
        Ok(section)
    }

    /// Get all top-level sections (headings at the minimum level found in the doc).
    pub fn sections(&self) -> Vec<Section> {
        let arena = Arena::new();
        let root = ast_util::parse_md(&arena, &self.body);

        // Find minimum heading level to determine "top-level"
        let all_headings = ast_util::find_headings(root, None);
        let min_level = all_headings
            .iter()
            .filter_map(|n| ast_util::heading_level(n))
            .min()
            .unwrap_or(1);

        let mut sections = Vec::new();
        for node in &all_headings {
            let level = ast_util::heading_level(node).unwrap_or(1);
            if level == min_level {
                let heading_text = ast_util::collect_text(node);
                let range = ast_util::section_byte_range(node, &self.body);
                let raw = self.body[range.clone()].to_string();
                let content_range = ast_util::section_content_byte_range(node, &self.body);
                let content = self.body[content_range].to_string();
                sections.push(Section::new(heading_text, level, raw, content));
            }
        }

        sections
    }

    /// Parse the document body once, extracting all sections with their
    /// validation-relevant data (tables, paragraphs, lists, code blocks).
    /// This avoids repeated AST parsing during validation.
    pub fn parse_body(&self) -> ParsedBody {
        let arena = Arena::new();
        let root = ast_util::parse_md(&arena, &self.body);

        let preamble_paragraphs = ast_util::preamble_paragraph_count(root);

        let all_headings = ast_util::find_headings(root, None);
        if all_headings.is_empty() {
            return ParsedBody {
                sections: Vec::new(),
                preamble_paragraphs,
            };
        }

        // Collect flat (heading, level, content_string) from the single AST
        let mut flat: Vec<(String, u8, String)> = Vec::new();
        for node in &all_headings {
            let level = ast_util::heading_level(node).unwrap_or(1);
            let heading_text = ast_util::collect_text(node);
            let content_range = ast_util::section_content_byte_range(node, &self.body);
            flat.push((heading_text, level, self.body[content_range].to_string()));
        }

        // Parse each section's content once (extracts tables, paragraphs, etc.)
        let parsed: Vec<ParsedSection> = flat
            .into_iter()
            .map(|(heading, level, content)| {
                let sec_arena = Arena::new();
                let sec_root = ast_util::parse_md(&sec_arena, &content);

                let tables = ast_util::find_tables(sec_root)
                    .into_iter()
                    .map(|n| ast_util::parse_table_node(n))
                    .collect();
                let paragraph_count = ast_util::count_paragraphs(sec_root);
                let (list_has_list, list_total_items, list_is_ordered) =
                    ast_util::count_list_info(sec_root);
                let code_block_languages = ast_util::collect_code_block_languages(sec_root);
                let code_blocks = ast_util::collect_code_blocks(sec_root);
                let has_callout = content.lines().any(|l| l.trim_start().starts_with("> [!"));

                ParsedSection {
                    heading,
                    level,
                    content,
                    tables,
                    paragraph_count,
                    list_has_list,
                    list_total_items,
                    list_is_ordered,
                    code_block_languages,
                    code_blocks,
                    has_callout,
                    children: Vec::new(),
                }
            })
            .collect();

        // Build tree from flat ordered list using a stack
        let min_level = parsed.iter().map(|s| s.level).min().unwrap_or(1);
        let mut top_sections: Vec<ParsedSection> = Vec::new();
        let mut stack: Vec<ParsedSection> = Vec::new();

        for section in parsed {
            let level = section.level;
            // Pop sections at same or deeper level
            while let Some(top) = stack.last() {
                if top.level >= level {
                    let popped = stack.pop().unwrap();
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(popped);
                    } else if popped.level == min_level {
                        top_sections.push(popped);
                    }
                } else {
                    break;
                }
            }
            stack.push(section);
        }

        // Flush remaining stack
        while let Some(popped) = stack.pop() {
            if let Some(parent) = stack.last_mut() {
                parent.children.push(popped);
            } else if popped.level == min_level {
                top_sections.push(popped);
            }
        }

        ParsedBody {
            sections: top_sections,
            preamble_paragraphs,
        }
    }

    /// Convert entire document to JSON.
    pub fn to_json(&self) -> serde_json::Value {
        let mut obj = serde_json::Map::new();

        if let Some(ref fm) = self.frontmatter {
            obj.insert("frontmatter".to_string(), fm.to_json());
        }

        if let Some(ref p) = self.path {
            obj.insert(
                "path".to_string(),
                serde_json::Value::String(p.display().to_string()),
            );
        }

        let sections: Vec<serde_json::Value> = self
            .sections()
            .iter()
            .map(|s| {
                let mut sec = serde_json::Map::new();
                sec.insert(
                    "heading".to_string(),
                    serde_json::Value::String(s.heading.clone()),
                );
                sec.insert(
                    "level".to_string(),
                    serde_json::Value::Number(s.level.into()),
                );
                sec.insert(
                    "content".to_string(),
                    serde_json::Value::String(s.content.clone()),
                );
                serde_json::Value::Object(sec)
            })
            .collect();

        obj.insert("sections".to_string(), serde_json::Value::Array(sections));
        obj.insert(
            "body".to_string(),
            serde_json::Value::String(self.body.clone()),
        );

        serde_json::Value::Object(obj)
    }

    // ─── Mutation methods ─────────────────────────────────────────────────

    /// Set a frontmatter field, creating frontmatter if absent.
    pub fn set_field(&mut self, key: &str, value: Value) {
        self.frontmatter
            .get_or_insert_with(|| Frontmatter::from_data(std::collections::BTreeMap::new()))
            .set(key, value);
        self.rebuild_raw();
    }

    /// Parse a string value and set the frontmatter field.
    pub fn set_field_from_str(&mut self, key: &str, raw: &str) {
        let value = crate::frontmatter::parse_yaml_value(raw);
        self.set_field(key, value);
    }

    /// Append a parsed string value to a sequence frontmatter field.
    pub fn append_field_from_str(&mut self, key: &str, raw: &str) {
        let value = crate::frontmatter::parse_yaml_value(raw);
        self.frontmatter
            .get_or_insert_with(|| Frontmatter::from_data(std::collections::BTreeMap::new()))
            .append(key, value);
        self.rebuild_raw();
    }

    /// Remove a frontmatter field and rebuild raw content.
    pub fn remove_field(&mut self, key: &str) -> Option<Value> {
        let removed = self.frontmatter.as_mut().and_then(|fm| fm.remove(key));
        if removed.is_some() {
            self.rebuild_raw();
        }
        removed
    }

    /// Replace the content of a section (everything between heading and next heading).
    pub fn replace_section_content(&mut self, heading: &str, new_content: &str) -> Result<()> {
        let range = self.section_content_range(heading)?;
        self.replace_body_range(range, new_content);
        Ok(())
    }

    /// Append content at the end of a section (before the next same-or-higher-level heading).
    pub fn append_to_section(&mut self, heading: &str, content: &str) -> Result<()> {
        let range = self.section_content_range(heading)?;
        let existing = self.body[range.clone()].to_string();
        let mut new = existing.trim_end().to_string();
        if !new.is_empty() {
            new.push_str("\n\n");
        }
        new.push_str(content);
        new.push('\n');
        self.replace_body_range(range, &new);
        Ok(())
    }

    /// Update a table cell within a section.
    pub fn set_table_cell(
        &mut self,
        heading: &str,
        table_idx: usize,
        col: &str,
        row: usize,
        value: &str,
    ) -> Result<()> {
        let (range, mut table) = self.find_table_byte_range(heading, table_idx)?;
        table.set_cell(col, row, value.to_string())?;
        self.replace_body_range(range, &table.to_markdown());
        Ok(())
    }

    /// Add a row to a table within a section.
    pub fn add_table_row(
        &mut self,
        heading: &str,
        table_idx: usize,
        values: Vec<String>,
    ) -> Result<()> {
        let (range, mut table) = self.find_table_byte_range(heading, table_idx)?;
        table.add_row(values);
        self.replace_body_range(range, &table.to_markdown());
        Ok(())
    }

    /// Reorder table columns to match schema order.
    /// Returns Ok(Some(old_headers)) if changed, Ok(None) if already correct.
    pub fn reorder_table(
        &mut self,
        heading: &str,
        table_idx: usize,
        order: &[String],
    ) -> Result<Option<Vec<String>>> {
        let (range, mut table) = self.find_table_byte_range(heading, table_idx)?;
        let old_headers = table.headers().to_vec();
        if table.reorder_columns(order) {
            self.replace_body_range(range, &table.to_markdown());
            Ok(Some(old_headers))
        } else {
            Ok(None)
        }
    }

    /// Convert bullet↔numbered list in a section. Returns true if changed.
    pub fn fix_list_ordering(&mut self, heading: &str, to_ordered: bool) -> Result<bool> {
        let range = self.section_content_range(heading)?;
        let content = self.body[range.clone()].to_string();

        let mut result = String::new();
        let mut changed = false;
        let mut counter = 1u32;

        for line in content.lines() {
            let trimmed = line.trim_start();
            if to_ordered {
                // Bullet → numbered: match `- `, `* `, `+ ` at start
                if let Some(rest) = trimmed
                    .strip_prefix("- ")
                    .or_else(|| trimmed.strip_prefix("* "))
                    .or_else(|| trimmed.strip_prefix("+ "))
                {
                    let indent = &line[..line.len() - trimmed.len()];
                    result.push_str(&format!("{indent}{counter}. {rest}\n"));
                    counter += 1;
                    changed = true;
                    continue;
                }
            } else {
                // Numbered → bullet: match `N. ` or `N) ` at start
                let after_num = trimmed
                    .find(". ")
                    .and_then(|pos| {
                        if trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
                            Some(&trimmed[pos + 2..])
                        } else {
                            None
                        }
                    })
                    .or_else(|| {
                        trimmed.find(") ").and_then(|pos| {
                            if trimmed[..pos].chars().all(|c| c.is_ascii_digit()) {
                                Some(&trimmed[pos + 2..])
                            } else {
                                None
                            }
                        })
                    });
                if let Some(rest) = after_num {
                    let indent = &line[..line.len() - trimmed.len()];
                    result.push_str(&format!("{indent}- {rest}\n"));
                    changed = true;
                    continue;
                }
            }
            result.push_str(line);
            result.push('\n');
        }

        if changed {
            self.replace_body_range(range, &result);
        }
        Ok(changed)
    }

    /// Format "Five Whys" style list items: split `N. Question? Answer` into
    /// `N. Question?\n   └► Answer` with blank lines between items.
    /// Returns true if any changes were made.
    pub fn format_whys(&mut self, heading: &str) -> Result<bool> {
        let range = self.section_content_range(heading)?;
        let content = self.body[range.clone()].to_string();

        let mut result = String::new();
        let mut changed = false;

        for line in content.lines() {
            let trimmed = line.trim_start();

            // Match numbered list item: `N. text`
            let item_text = trimmed.find(". ").and_then(|pos| {
                if !trimmed[..pos].is_empty() && trimmed[..pos].chars().all(|c| c.is_ascii_digit())
                {
                    Some((pos, &trimmed[pos + 2..]))
                } else {
                    None
                }
            });

            if let Some((dot_pos, text)) = item_text {
                let num = &trimmed[..dot_pos];
                let indent = &line[..line.len() - trimmed.len()];

                // Already formatted: line is just the question (└► on next line)
                if text.contains("? ") && !text.starts_with("└► ") {
                    // Check if there's a "Because"/"because" answer after the question mark
                    if let Some(q_pos) = text.find("? ") {
                        let question = &text[..q_pos + 1];
                        let answer = text[q_pos + 2..].trim();
                        if !answer.is_empty() {
                            // Split into question + └► answer (no indent on answer)
                            result.push_str(&format!("{indent}{num}. {question}\n"));
                            result.push_str(&format!("{indent}└► {answer}\n"));
                            result.push('\n');
                            changed = true;
                            continue;
                        }
                    }
                }
            }

            // Skip blank lines that we'll regenerate between items
            // (only if we're making changes and line is empty)
            result.push_str(line);
            result.push('\n');
        }

        if changed {
            // Clean up: remove double blank lines at end
            while result.ends_with("\n\n\n") {
                result.pop();
            }
            self.replace_body_range(range, &result);
        }
        Ok(changed)
    }

    /// Save to the document's path (errors if no path set).
    pub fn save(&self) -> Result<()> {
        let path = self.path.as_ref().ok_or(Error::NoPath)?;
        std::fs::write(path, &self.raw).map_err(|_| Error::WriteFailed(path.clone()))?;
        Ok(())
    }

    /// Save to an explicit path.
    pub fn save_to(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        std::fs::write(path, &self.raw).map_err(|_| Error::WriteFailed(path.to_path_buf()))?;
        Ok(())
    }

    /// Get the content byte range for a section by heading (excludes heading line).
    fn section_content_range(&self, heading: &str) -> Result<Range<usize>> {
        let arena = Arena::new();
        let root = ast_util::parse_md(&arena, &self.body);
        let heading_node = ast_util::find_heading_by_text(root, heading)
            .ok_or_else(|| Error::SectionNotFound(heading.to_string()))?;
        Ok(ast_util::section_content_byte_range(
            heading_node,
            &self.body,
        ))
    }

    /// Reconstruct raw from frontmatter + body.
    pub(crate) fn rebuild_raw(&mut self) {
        let mut raw = String::new();
        if let Some(ref fm) = self.frontmatter {
            raw.push_str("---\n");
            raw.push_str(&fm.to_yaml_string());
            raw.push_str("---\n");
        }
        raw.push_str(&self.body);
        self.raw = raw;
    }

    /// Splice body string then rebuild_raw.
    fn replace_body_range(&mut self, range: Range<usize>, replacement: &str) {
        self.body.replace_range(range, replacement);
        self.rebuild_raw();
    }

    /// Find the byte range and parsed Table for the nth table in a section.
    fn find_table_byte_range(
        &self,
        heading: &str,
        table_idx: usize,
    ) -> Result<(Range<usize>, Table)> {
        let arena = Arena::new();
        let root = ast_util::parse_md(&arena, &self.body);

        let heading_node = ast_util::find_heading_by_text(root, heading)
            .ok_or_else(|| Error::SectionNotFound(heading.to_string()))?;

        let section_range = ast_util::section_byte_range(heading_node, &self.body);

        // Find all tables in the section range
        let all_tables = ast_util::find_tables(root);
        let section_tables: Vec<_> = all_tables
            .into_iter()
            .filter(|t| {
                let tr = ast_util::table_byte_range(t, &self.body);
                tr.start >= section_range.start && tr.end <= section_range.end
            })
            .collect();

        let table_node = section_tables
            .get(table_idx)
            .ok_or(Error::TableNotFound(table_idx))?;

        let range = ast_util::table_byte_range(table_node, &self.body);
        let table = ast_util::parse_table_node(table_node);
        Ok((range, table))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
---
title: Use PostgreSQL
status: accepted
---

# Decision

We will use PostgreSQL.

## Rationale

It's reliable.

# Consequences

Some consequences here.

## Positive

Good things.

## Negative

Bad things.
";

    #[test]
    fn test_from_str() {
        let doc = Document::from_str(SAMPLE).unwrap();
        assert!(doc.frontmatter.is_some());
        assert_eq!(
            doc.frontmatter().unwrap().get_display("status").unwrap(),
            "accepted"
        );
    }

    #[test]
    fn test_get_section() {
        let doc = Document::from_str(SAMPLE).unwrap();
        let section = doc.get_section("Decision").unwrap();
        assert!(section.content.contains("PostgreSQL"));
        assert!(section.content.contains("Rationale"));
    }

    #[test]
    fn test_get_section_by_path() {
        let doc = Document::from_str(SAMPLE).unwrap();
        let section = doc
            .get_section_by_path(&["Consequences", "Positive"])
            .unwrap();
        assert!(section.content.contains("Good things"));
    }

    #[test]
    fn test_sections() {
        let doc = Document::from_str(SAMPLE).unwrap();
        let sections = doc.sections();
        assert_eq!(sections.len(), 2); // Decision, Consequences (top-level = h1)
    }

    #[test]
    fn test_to_json() {
        let doc = Document::from_str(SAMPLE).unwrap();
        let json = doc.to_json();
        assert!(json["frontmatter"]["title"] == "Use PostgreSQL");
    }

    #[test]
    fn test_set_field() {
        let mut doc = Document::from_str(SAMPLE).unwrap();
        doc.set_field("status", serde_yaml::Value::String("deprecated".into()));
        assert_eq!(
            doc.frontmatter().unwrap().get_display("status").unwrap(),
            "deprecated"
        );
        // raw should contain the new value
        assert!(doc.raw.contains("deprecated"));
    }

    #[test]
    fn test_set_field_from_str() {
        let mut doc = Document::from_str(SAMPLE).unwrap();
        doc.set_field_from_str("status", "rejected");
        assert_eq!(
            doc.frontmatter().unwrap().get_display("status").unwrap(),
            "rejected"
        );
    }

    #[test]
    fn test_append_field_from_str() {
        let content = "---\ntitle: Test\ntags:\n  - silver\n---\n\n# Body\n";
        let mut doc = Document::from_str(content).unwrap();
        doc.append_field_from_str("tags", "backend");
        let tags = doc.frontmatter().unwrap().get("tags").unwrap();
        match tags {
            serde_yaml::Value::Sequence(seq) => {
                assert_eq!(seq.len(), 2);
                assert_eq!(seq[1], serde_yaml::Value::String("backend".into()));
            }
            _ => panic!("expected sequence"),
        }
        assert!(doc.raw.contains("backend"));
    }

    #[test]
    fn test_replace_section_content() {
        let mut doc = Document::from_str(SAMPLE).unwrap();
        doc.replace_section_content("Decision", "New decision text.\n")
            .unwrap();
        let section = doc.get_section("Decision").unwrap();
        assert!(section.content.contains("New decision text"));
        assert!(!section.content.contains("PostgreSQL"));
    }

    #[test]
    fn test_append_to_section() {
        let mut doc = Document::from_str(SAMPLE).unwrap();
        doc.append_to_section("Decision", "Extra note.").unwrap();
        let section = doc.get_section("Decision").unwrap();
        assert!(section.content.contains("PostgreSQL"));
        assert!(section.content.contains("Extra note."));
    }

    const TABLE_DOC: &str = "\
---
title: Tables
---

# Data

| A | B |
|---|---|
| 1 | 2 |
| 3 | 4 |

# Other

Done.
";

    #[test]
    fn test_set_table_cell() {
        let mut doc = Document::from_str(TABLE_DOC).unwrap();
        doc.set_table_cell("Data", 0, "B", 0, "99").unwrap();
        let section = doc.get_section("Data").unwrap();
        let tables = section.tables();
        assert_eq!(tables[0].get_cell("B", 0), Some("99"));
    }

    #[test]
    fn test_add_table_row() {
        let mut doc = Document::from_str(TABLE_DOC).unwrap();
        doc.add_table_row("Data", 0, vec!["5".into(), "6".into()])
            .unwrap();
        let section = doc.get_section("Data").unwrap();
        let tables = section.tables();
        assert_eq!(tables[0].rows().len(), 3);
        assert_eq!(tables[0].get_cell("A", 2), Some("5"));
    }

    #[test]
    fn test_save_to() {
        let doc = Document::from_str(SAMPLE).unwrap();
        let dir = std::env::temp_dir();
        let path = dir.join("md_db_test_save.md");
        doc.save_to(&path).unwrap();
        let loaded = Document::from_file(&path).unwrap();
        assert_eq!(
            loaded.frontmatter().unwrap().get_display("title").unwrap(),
            "Use PostgreSQL"
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_save_no_path_errors() {
        let doc = Document::from_str(SAMPLE).unwrap();
        assert!(doc.save().is_err());
    }

    const REORDER_DOC: &str = "\
---
title: Reorder
---

# Data

| B | A | C |
|---|---|---|
| b1 | a1 | c1 |

# Other

Done.
";

    #[test]
    fn test_reorder_table() {
        let mut doc = Document::from_str(REORDER_DOC).unwrap();
        let old = doc
            .reorder_table("Data", 0, &["A".into(), "B".into(), "C".into()])
            .unwrap();
        assert!(old.is_some());
        assert_eq!(old.unwrap(), vec!["B", "A", "C"]);
        let section = doc.get_section("Data").unwrap();
        let tables = section.tables();
        assert_eq!(tables[0].headers(), &["A", "B", "C"]);
        assert_eq!(tables[0].get_cell("A", 0), Some("a1"));
    }

    #[test]
    fn test_reorder_table_noop() {
        let mut doc = Document::from_str(TABLE_DOC).unwrap();
        let result = doc
            .reorder_table("Data", 0, &["A".into(), "B".into()])
            .unwrap();
        assert!(result.is_none());
    }

    const LIST_DOC: &str = "\
---
title: Lists
---

# Bullet Section

- first item
- second item
- third item

# Numbered Section

1. alpha
2. beta
3. gamma

# Other

Done.
";

    #[test]
    fn test_fix_list_ordering_bullet_to_numbered() {
        let mut doc = Document::from_str(LIST_DOC).unwrap();
        let changed = doc.fix_list_ordering("Bullet Section", true).unwrap();
        assert!(changed);
        let section = doc.get_section("Bullet Section").unwrap();
        assert!(section.content.contains("1. first item"));
        assert!(section.content.contains("2. second item"));
        assert!(section.content.contains("3. third item"));
    }

    #[test]
    fn test_fix_list_ordering_numbered_to_bullet() {
        let mut doc = Document::from_str(LIST_DOC).unwrap();
        let changed = doc.fix_list_ordering("Numbered Section", false).unwrap();
        assert!(changed);
        let section = doc.get_section("Numbered Section").unwrap();
        assert!(section.content.contains("- alpha"));
        assert!(section.content.contains("- beta"));
    }

    #[test]
    fn test_fix_list_ordering_noop() {
        let mut doc = Document::from_str(LIST_DOC).unwrap();
        // Bullet section is already bullet, asking for bullet → no change
        let changed = doc.fix_list_ordering("Bullet Section", false).unwrap();
        assert!(!changed);
    }

    const WHYS_DOC: &str = "\
---
title: Whys
---

# Five Whys

1. Why did X fail? Because Y was broken.
2. Why was Y broken? Because Z was missing.

# Other

Done.
";

    #[test]
    fn test_format_whys() {
        let mut doc = Document::from_str(WHYS_DOC).unwrap();
        let changed = doc.format_whys("Five Whys").unwrap();
        assert!(changed);
        let section = doc.get_section("Five Whys").unwrap();
        assert!(section.content.contains("1. Why did X fail?"));
        assert!(section.content.contains("└► Because Y was broken."));
        assert!(section.content.contains("2. Why was Y broken?"));
        assert!(section.content.contains("└► Because Z was missing."));
        // Should NOT contain the combined single-line form
        assert!(!section.content.contains("? Because"));
    }

    #[test]
    fn test_format_whys_already_formatted() {
        let already = "\
---
title: Whys
---

# Five Whys

1. Why did X fail?
└► Because Y was broken.

# Other

Done.
";
        let mut doc = Document::from_str(already).unwrap();
        let changed = doc.format_whys("Five Whys").unwrap();
        assert!(!changed);
    }
}

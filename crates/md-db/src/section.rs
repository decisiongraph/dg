use crate::ast_util;
use crate::table::Table;
use comrak::Arena;

#[derive(Debug, Clone)]
pub struct Section {
    pub heading: String,
    pub level: u8,
    /// Raw markdown content of the section (including heading).
    pub raw: String,
    /// Content below the heading (excluding the heading line).
    pub content: String,
}

impl Section {
    pub fn new(heading: String, level: u8, raw: String, content: String) -> Self {
        Self {
            heading,
            level,
            raw,
            content,
        }
    }

    /// Parse tables within this section's content.
    pub fn tables(&self) -> Vec<Table> {
        let arena = Arena::new();
        let root = ast_util::parse_md(&arena, &self.content);
        ast_util::find_tables(root)
            .into_iter()
            .map(|n| ast_util::parse_table_node(n))
            .collect()
    }

    /// Get subsections (headings one level deeper within this section).
    pub fn subsections(&self) -> Vec<Section> {
        let arena = Arena::new();
        let root = ast_util::parse_md(&arena, &self.content);

        let sub_level = self.level + 1;
        let headings = ast_util::find_headings(root, Some(sub_level));

        headings
            .into_iter()
            .map(|h| {
                let heading_text = ast_util::collect_text(h);
                let range = ast_util::section_byte_range(h, &self.content);
                let raw = self.content[range.clone()].to_string();
                let content_range = ast_util::section_content_byte_range(h, &self.content);
                let content = self.content[content_range].to_string();
                Section::new(heading_text, sub_level, raw, content)
            })
            .collect()
    }

    /// Strip markdown syntax and return plain text with block structure preserved.
    pub fn text(&self) -> String {
        let arena = Arena::new();
        let root = ast_util::parse_md(&arena, &self.content);
        ast_util::collect_text_blocks(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_text() {
        let s = Section::new(
            "Test".into(),
            2,
            "## Test\n\nSome **bold** text.\n".into(),
            "Some **bold** text.\n".into(),
        );
        assert_eq!(s.text(), "Some bold text.");
    }

    #[test]
    fn test_section_tables() {
        let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";
        let s = Section::new(
            "Test".into(),
            2,
            format!("## Test\n\n{content}"),
            content.to_string(),
        );
        let tables = s.tables();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].get_cell("A", 0), Some("1"));
    }

    #[test]
    fn test_subsections() {
        let content = "### Sub1\n\nContent 1\n\n### Sub2\n\nContent 2\n";
        let s = Section::new(
            "Parent".into(),
            2,
            format!("## Parent\n\n{content}"),
            content.to_string(),
        );
        let subs = s.subsections();
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].heading.trim(), "Sub1");
        assert_eq!(subs[1].heading.trim(), "Sub2");
    }
}

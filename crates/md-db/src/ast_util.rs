use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options};

use crate::table::Table;

/// Shared comrak options with table extension enabled.
/// Most markdown parsing in the codebase needs this configuration.
pub fn comrak_opts() -> Options<'static> {
    let mut opts = Options::default();
    opts.extension.table = true;
    opts
}

/// Parse markdown body into a comrak AST using standard options.
/// Caller provides the arena so the returned node borrows from it.
pub fn parse_md<'a>(arena: &'a Arena<AstNode<'a>>, body: &str) -> &'a AstNode<'a> {
    comrak::parse_document(arena, body, &comrak_opts())
}

/// Collect plain text from a node (inline only, for heading text etc).
pub fn collect_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut text = String::new();
    collect_text_inner(node, &mut text);
    text
}

fn collect_text_inner<'a>(node: &'a AstNode<'a>, out: &mut String) {
    match &node.data.borrow().value {
        NodeValue::Text(t) => out.push_str(t),
        NodeValue::Code(c) => out.push_str(&c.literal),
        NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
        _ => {}
    }
    for child in node.children() {
        collect_text_inner(child, out);
    }
}

/// Collect plain text with block structure preserved (newlines between paragraphs/headings).
pub fn collect_text_blocks<'a>(node: &'a AstNode<'a>) -> String {
    let mut parts = Vec::new();
    for child in node.children() {
        let text = collect_text(child);
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
    }
    parts.join("\n\n")
}

/// Find all heading nodes, optionally filtered by level.
pub fn find_headings<'a>(root: &'a AstNode<'a>, level: Option<u8>) -> Vec<&'a AstNode<'a>> {
    let mut headings = Vec::new();
    for node in root.descendants() {
        if let NodeValue::Heading(h) = &node.data.borrow().value {
            if level.is_none() || level == Some(h.level) {
                headings.push(node);
            }
        }
    }
    headings
}

/// Find a heading node by exact text match (case-insensitive).
pub fn find_heading_by_text<'a>(root: &'a AstNode<'a>, text: &str) -> Option<&'a AstNode<'a>> {
    let target = text.trim().to_lowercase();
    for node in root.descendants() {
        if let NodeValue::Heading(_) = &node.data.borrow().value {
            let heading_text = collect_text(node).trim().to_lowercase();
            if heading_text == target {
                return Some(node);
            }
        }
    }
    None
}

/// Get the heading level of a node. Returns None if not a heading.
pub fn heading_level<'a>(node: &'a AstNode<'a>) -> Option<u8> {
    if let NodeValue::Heading(h) = &node.data.borrow().value {
        Some(h.level)
    } else {
        None
    }
}

/// Get the byte range of a section (from heading to next same-or-higher-level heading).
/// Returns (start_byte, end_byte) into the body string.
/// The start includes the heading line itself.
pub fn section_byte_range<'a>(heading_node: &'a AstNode<'a>, body: &str) -> std::ops::Range<usize> {
    let sourcepos = heading_node.data.borrow().sourcepos;
    let level = heading_level(heading_node).unwrap_or(1);

    // Start at the beginning of the heading line (convert 1-based line to byte offset)
    let start = line_col_to_byte(body, sourcepos.start.line, 1);

    // Walk siblings to find the next heading at same or higher level
    let mut next = heading_node.next_sibling();
    while let Some(sibling) = next {
        if let NodeValue::Heading(h) = &sibling.data.borrow().value {
            if h.level <= level {
                let end_pos = sibling.data.borrow().sourcepos;
                let end = line_col_to_byte(body, end_pos.start.line, 1);
                return start..end;
            }
        }
        next = sibling.next_sibling();
    }

    // No next heading found — section extends to end of body
    start..body.len()
}

/// Get byte range of section content (excluding the heading line itself).
pub fn section_content_byte_range<'a>(
    heading_node: &'a AstNode<'a>,
    body: &str,
) -> std::ops::Range<usize> {
    let full_range = section_byte_range(heading_node, body);

    // Skip past the heading line
    let content_start = body[full_range.start..]
        .find('\n')
        .map(|i| full_range.start + i + 1)
        .unwrap_or(full_range.end);

    content_start..full_range.end
}

/// Convert 1-based line number and 1-based column to byte offset.
fn line_col_to_byte(text: &str, line: usize, _col: usize) -> usize {
    let mut current_line = 1;
    for (i, c) in text.char_indices() {
        if current_line == line {
            return i;
        }
        if c == '\n' {
            current_line += 1;
        }
    }
    text.len()
}

/// Find all table nodes in the AST.
pub fn find_tables<'a>(root: &'a AstNode<'a>) -> Vec<&'a AstNode<'a>> {
    let mut tables = Vec::new();
    for node in root.descendants() {
        if let NodeValue::Table(_) = &node.data.borrow().value {
            tables.push(node);
        }
    }
    tables
}

/// Get the byte range of a table node in the body string (sourcepos-based).
pub fn table_byte_range<'a>(table_node: &'a AstNode<'a>, body: &str) -> std::ops::Range<usize> {
    let sourcepos = table_node.data.borrow().sourcepos;
    let start = line_col_to_byte(body, sourcepos.start.line, 1);
    // End at the end of the last line of the table
    let end_line = sourcepos.end.line;
    // Find the byte position at the end of end_line (after the newline)
    let mut current_line = 1;
    let mut end = body.len();
    for (i, c) in body.char_indices() {
        if c == '\n' {
            if current_line == end_line {
                end = i + 1;
                break;
            }
            current_line += 1;
        }
    }
    start..end
}

/// Parse a comrak Table node into our Table struct.
pub fn parse_table_node<'a>(table_node: &'a AstNode<'a>) -> Table {
    let mut headers = Vec::new();
    let mut rows = Vec::new();
    let mut is_header = true;

    for row_node in table_node.children() {
        if let NodeValue::TableRow(header) = &row_node.data.borrow().value {
            is_header = *header;
        }

        let cells: Vec<String> = row_node
            .children()
            .filter(|n| matches!(n.data.borrow().value, NodeValue::TableCell))
            .map(|cell| collect_text(cell).trim().to_string())
            .collect();

        if is_header && headers.is_empty() {
            headers = cells;
        } else {
            rows.push(cells);
        }
    }

    Table::new(headers, rows)
}

/// Count paragraph nodes in an AST subtree.
pub fn count_paragraphs<'a>(node: &'a AstNode<'a>) -> usize {
    node.descendants()
        .filter(|n| matches!(n.data.borrow().value, NodeValue::Paragraph))
        .count()
}

/// Count list info in an AST subtree: (has_list, total_items, is_ordered).
/// `is_ordered` reflects the first list found in the subtree.
pub fn count_list_info<'a>(node: &'a AstNode<'a>) -> (bool, usize, bool) {
    let mut has_list = false;
    let mut total_items = 0;
    let mut is_ordered = false;
    for n in node.descendants() {
        if let NodeValue::List(ref list) = n.data.borrow().value {
            if !has_list {
                is_ordered = list.list_type == comrak::nodes::ListType::Ordered;
            }
            has_list = true;
            total_items += n
                .children()
                .filter(|c| matches!(c.data.borrow().value, NodeValue::Item(_)))
                .count();
        }
    }
    (has_list, total_items, is_ordered)
}

/// Collect code block languages (lowercased, trimmed) from an AST subtree.
pub fn collect_code_block_languages<'a>(node: &'a AstNode<'a>) -> Vec<String> {
    node.descendants()
        .filter_map(|n| {
            if let NodeValue::CodeBlock(ref cb) = n.data.borrow().value {
                Some(cb.info.trim().to_lowercase())
            } else {
                None
            }
        })
        .collect()
}

/// Collect code blocks as (language, content) pairs from an AST subtree.
/// Language is lowercased and trimmed; content is the literal code text.
pub fn collect_code_blocks<'a>(node: &'a AstNode<'a>) -> Vec<(String, String)> {
    node.descendants()
        .filter_map(|n| {
            if let NodeValue::CodeBlock(ref cb) = n.data.borrow().value {
                Some((cb.info.trim().to_lowercase(), cb.literal.clone()))
            } else {
                None
            }
        })
        .collect()
}

/// Count paragraph nodes that appear before the first heading in the AST.
/// This captures the "preamble" — introductory text before any ## section.
pub fn preamble_paragraph_count<'a>(root: &'a AstNode<'a>) -> usize {
    let mut count = 0;
    let mut past_h1 = false;
    for child in root.children() {
        if let NodeValue::Heading(h) = child.data.borrow().value {
            if h.level == 1 {
                past_h1 = true;
                continue;
            }
            break;
        }
        // Only count paragraphs after H1 (between H1 and first H2+)
        if past_h1 && matches!(child.data.borrow().value, NodeValue::Paragraph) {
            count += 1;
        }
    }
    count
}

/// Extract list item text from an AST node. Returns each top-level list item's text.
pub fn extract_list_items<'a>(node: &'a AstNode<'a>) -> Vec<String> {
    let mut items = Vec::new();
    for n in node.descendants() {
        if let NodeValue::List(_) = n.data.borrow().value {
            for child in n.children() {
                if let NodeValue::Item(_) = child.data.borrow().value {
                    let text = collect_text(child).trim().to_string();
                    if !text.is_empty() {
                        items.push(text);
                    }
                }
            }
            // Only extract from the first list found
            break;
        }
    }
    items
}

/// Parse markdown body and return all link URLs found in the AST.
pub fn extract_links(body: &str) -> Vec<String> {
    let arena = Arena::new();
    let root = parse_md(&arena, body);
    let mut links = Vec::new();
    for node in root.descendants() {
        if let NodeValue::Link(ref link) = node.data.borrow().value {
            links.push(link.url.clone());
        }
    }
    links
}

/// Extract text of the first heading, preferring the lowest level (h1 > h2).
/// Returns `None` if no h1 or h2 heading is found.
pub fn first_heading_text(body: &str) -> Option<String> {
    let arena = Arena::new();
    let root = parse_md(&arena, body);
    for level in 1..=2 {
        let headings = find_headings(root, Some(level));
        if let Some(h) = headings.first() {
            let text = collect_text(h).trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

/// Extract document ID mentions (e.g. "ADR-001", "OPP-002") from raw markdown text.
/// Returns unique uppercase IDs found as bare text (not just inside links).
///
/// When `prefixes` is non-empty, only IDs whose prefix matches are returned
/// (e.g. `["ADR", "OPP"]` filters out `ERC-20`, `ISO-8601`, etc.).
pub fn extract_doc_id_mentions(body: &str, prefixes: &[String]) -> Vec<String> {
    let re = match regex::Regex::new(r"\b([A-Za-z]{2,})-(\d+)\b") {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut seen = std::collections::HashSet::new();
    let mut ids = Vec::new();
    for cap in re.captures_iter(body) {
        let id = cap[0].to_uppercase();
        if !prefixes.is_empty() {
            let prefix = cap[1].to_uppercase();
            if !prefixes.iter().any(|p| p == &prefix) {
                continue;
            }
        }
        if seen.insert(id.clone()) {
            ids.push(id);
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use comrak::Arena;

    use super::*;

    #[test]
    fn test_find_headings() {
        let md = "# H1\n\ntext\n\n## H2\n\nmore\n\n# H1b\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);

        assert_eq!(find_headings(root, None).len(), 3);
        assert_eq!(find_headings(root, Some(1)).len(), 2);
        assert_eq!(find_headings(root, Some(2)).len(), 1);
    }

    #[test]
    fn test_find_heading_by_text() {
        let md = "# Introduction\n\ntext\n\n## Details\n\nmore\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);

        assert!(find_heading_by_text(root, "Introduction").is_some());
        assert!(find_heading_by_text(root, "introduction").is_some());
        assert!(find_heading_by_text(root, "details").is_some());
        assert!(find_heading_by_text(root, "missing").is_none());
    }

    #[test]
    fn test_section_byte_range() {
        let md = "# First\n\nContent 1\n\n# Second\n\nContent 2\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);

        let h = find_heading_by_text(root, "First").unwrap();
        let range = section_byte_range(h, md);
        let section = &md[range];
        assert!(section.contains("Content 1"));
        assert!(!section.contains("Content 2"));
    }

    #[test]
    fn test_parse_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);

        let tables = find_tables(root);
        assert_eq!(tables.len(), 1);

        let table = parse_table_node(tables[0]);
        assert_eq!(table.headers(), &["A", "B"]);
        assert_eq!(table.get_cell("A", 0), Some("1"));
        assert_eq!(table.get_cell("B", 1), Some("4"));
    }

    #[test]
    fn test_table_byte_range() {
        let md = "# Section\n\nSome text.\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nMore text.\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);

        let tables = find_tables(root);
        assert_eq!(tables.len(), 1);

        let range = table_byte_range(tables[0], md);
        let table_text = &md[range];
        assert!(table_text.contains("| A | B |"));
        assert!(table_text.contains("| 1 | 2 |"));
        assert!(!table_text.contains("Some text"));
        assert!(!table_text.contains("More text"));
    }

    #[test]
    fn test_extract_links() {
        let md = "See [ADR-001](./adr-001.md) and [OPP](./opp-001.md) for details.\n\nAlso [external](https://example.com).\n";
        let links = super::extract_links(md);
        assert_eq!(links.len(), 3);
        assert_eq!(links[0], "./adr-001.md");
        assert_eq!(links[1], "./opp-001.md");
        assert_eq!(links[2], "https://example.com");
    }

    #[test]
    fn test_extract_links_empty() {
        let md = "No links here, just plain text.\n";
        let links = super::extract_links(md);
        assert!(links.is_empty());
    }

    #[test]
    fn test_extract_doc_id_mentions() {
        let prefixes = vec!["ADR".into(), "OPP".into()];
        let body = "This relates to ADR-001 and see OPP-002 for context.\nAlso adr-001 again.";
        let ids = super::extract_doc_id_mentions(body, &prefixes);
        assert!(ids.contains(&"ADR-001".to_string()));
        assert!(ids.contains(&"OPP-002".to_string()));
        // Deduplicated: adr-001 and ADR-001 are same
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_extract_doc_id_mentions_no_false_positives() {
        let prefixes = vec!["ADR".into(), "OPP".into(), "POL".into(), "INC".into()];
        let body = "Use https://example.com and file.txt, no IDs here.";
        let ids = super::extract_doc_id_mentions(body, &prefixes);
        assert!(ids.is_empty());
    }

    #[test]
    fn test_extract_doc_id_mentions_filters_non_doc_ids() {
        let prefixes = vec!["ADR".into(), "OPP".into()];
        let body = "Implements ERC-20 token standard per ADR-001. Also uses ISO-8601 dates and SHA-256 hashes.";
        let ids = super::extract_doc_id_mentions(body, &prefixes);
        assert_eq!(ids, vec!["ADR-001"]);
    }

    #[test]
    fn test_extract_doc_id_mentions_empty_prefixes_allows_all() {
        let body = "ERC-20 and ADR-001";
        let ids = super::extract_doc_id_mentions(body, &[]);
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_extract_list_items_basic() {
        let md = "- foo\n- bar\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);
        let items = extract_list_items(root);
        assert_eq!(items, vec!["foo", "bar"]);
    }

    #[test]
    fn test_extract_list_items_empty() {
        let md = "Just a paragraph, no list.\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);
        let items = extract_list_items(root);
        assert!(items.is_empty());
    }

    #[test]
    fn test_extract_list_items_only_first_list() {
        let md = "- a\n- b\n\nSome text.\n\n- c\n- d\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);
        let items = extract_list_items(root);
        assert_eq!(items, vec!["a", "b"]);
    }

    #[test]
    fn test_extract_list_items_ordered() {
        let md = "1. first\n2. second\n3. third\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);
        let items = extract_list_items(root);
        assert_eq!(items, vec!["first", "second", "third"]);
    }

    #[test]
    fn test_first_heading_text_h1() {
        let md = "# My Title\n\nSome content\n\n## Subtitle\n";
        assert_eq!(first_heading_text(md), Some("My Title".to_string()));
    }

    #[test]
    fn test_first_heading_text_h2_fallback() {
        let md = "Some intro text\n\n## Subtitle\n\nMore content\n";
        assert_eq!(first_heading_text(md), Some("Subtitle".to_string()));
    }

    #[test]
    fn test_first_heading_text_none() {
        let md = "Just a paragraph with no headings.\n";
        assert_eq!(first_heading_text(md), None);
    }

    #[test]
    fn test_first_heading_text_prefers_h1_over_h2() {
        let md = "## H2 first\n\n# H1 later\n";
        // h1 is preferred even if h2 appears first in the document
        assert_eq!(first_heading_text(md), Some("H1 later".to_string()));
    }

    #[test]
    fn test_preamble_paragraph_count_text_before_h1_not_counted() {
        // Text before H1 should NOT count as preamble
        let md = "Stray text.\n\n# Title\n\n## Heading\n\nContent.\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);
        assert_eq!(preamble_paragraph_count(root), 0);
    }

    #[test]
    fn test_preamble_paragraph_count_no_preamble() {
        let md = "## Heading\n\nContent.\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);
        assert_eq!(preamble_paragraph_count(root), 0);
    }

    #[test]
    fn test_preamble_paragraph_count_no_headings() {
        // No H1 at all → preamble is 0
        let md = "Just text.\n\nMore text.\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);
        assert_eq!(preamble_paragraph_count(root), 0);
    }

    #[test]
    fn test_preamble_paragraph_count_after_h1() {
        // Service README: H1, description paragraph, then H2
        let md = "# My Service\n\nDescription of the service.\n\n## Architecture\n\nContent.\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);
        assert_eq!(preamble_paragraph_count(root), 1);
    }

    #[test]
    fn test_preamble_paragraph_count_h1_no_description() {
        // H1 but no description paragraph before H2
        let md = "# My Service\n\n## Architecture\n\nContent.\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);
        assert_eq!(preamble_paragraph_count(root), 0);
    }

    #[test]
    fn test_preamble_paragraph_count_h1_two_paragraphs() {
        let md = "# Title\n\nFirst paragraph.\n\nSecond paragraph.\n\n## Section\n";
        let arena = Arena::new();
        let root = parse_md(&arena, md);
        assert_eq!(preamble_paragraph_count(root), 2);
    }
}

//! AST walker — converts comrak AST into RenderedDocument

use comrak::nodes::{AstNode, NodeCode, NodeValue};
use ratatui::style::Color;

use crate::blocks;
use crate::inline;
use crate::options::RenderOptions;
use crate::types::{
    RenderedBlock, RenderedDocument, SectionHint, SegmentStyle, StyledLine, StyledSegment,
};

/// Walk the entire document and produce rendered blocks
pub fn walk_document<'a>(root: &'a AstNode<'a>, options: &RenderOptions) -> RenderedDocument {
    let mut doc = RenderedDocument::new();
    let mut section = SectionHint::Neutral;
    let mut current_heading = String::new();

    for child in root.children() {
        // Detect section type from headings before rendering
        let ast = child.data.borrow();
        let is_heading = matches!(ast.value, NodeValue::Heading(_));
        let is_table = matches!(ast.value, NodeValue::Table(_));
        drop(ast);

        if is_heading {
            section = detect_section_hint(child);
            current_heading = extract_plain_text(child).to_lowercase();
        }

        // Tables in auto-numbered sections get a "#" column
        if is_table && options.auto_number_sections.contains(&current_heading) {
            doc.blocks
                .push(blocks::table::render_table(child, options, true));
            continue;
        }

        if let Some(block) = walk_block(child, options, 0, section) {
            doc.blocks.push(block);
        }
    }

    // Collect footnotes
    collect_footnotes(root, options, &mut doc);

    doc
}

/// Extract plain text from a heading node's inline children
fn extract_plain_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut text = String::new();
    for child in node.children() {
        let ast = child.data.borrow();
        match &ast.value {
            NodeValue::Text(t) => text.push_str(t),
            NodeValue::SoftBreak | NodeValue::LineBreak => text.push(' '),
            NodeValue::Code(NodeCode { literal, .. }) => text.push_str(literal),
            _ => {
                drop(ast);
                text.push_str(&extract_plain_text(child));
                continue;
            }
        }
        drop(ast);
    }
    text
}

/// Determine section hint from a heading node's text
fn detect_section_hint<'a>(node: &'a AstNode<'a>) -> SectionHint {
    let heading_text = extract_plain_text(node).to_lowercase();
    if heading_text.contains("positive")
        || heading_text.contains("pros")
        || heading_text.contains("benefits")
        || heading_text.contains("advantages")
    {
        SectionHint::Positive
    } else if heading_text.contains("negative")
        || heading_text.contains("cons")
        || heading_text.contains("risks")
        || heading_text.contains("drawbacks")
        || heading_text.contains("disadvantages")
    {
        SectionHint::Negative
    } else {
        SectionHint::Neutral
    }
}

/// Walk a block-level node
pub fn walk_block<'a>(
    node: &'a AstNode<'a>,
    options: &RenderOptions,
    depth: usize,
    section: SectionHint,
) -> Option<RenderedBlock> {
    let ast = node.data.borrow();
    match &ast.value {
        NodeValue::Paragraph => {
            let mut line = StyledLine::new();
            drop(ast);
            walk_inlines(node, &mut line, &SegmentStyle::default(), options);
            Some(RenderedBlock::Lines(wrap_line(line, options.width)))
        }
        NodeValue::Heading(heading) => {
            let level = heading.level;
            drop(ast);
            Some(blocks::heading::render_heading(node, level, options))
        }
        NodeValue::CodeBlock(cb) => {
            let info = cb.info.clone();
            let literal = cb.literal.clone();
            drop(ast);
            Some(blocks::code::render_code_block(&info, &literal, options))
        }
        NodeValue::List(list) => {
            let list_type = list.list_type;
            let start = list.start;
            let tight = list.tight;
            drop(ast);
            Some(blocks::list::render_list(
                node, list_type, start, tight, options, depth, section,
            ))
        }
        NodeValue::Item(_) => {
            // Handled by list rendering
            None
        }
        NodeValue::BlockQuote => {
            drop(ast);
            Some(blocks::blockquote::render_blockquote(
                node, options, depth, section,
            ))
        }
        NodeValue::ThematicBreak => {
            drop(ast);
            Some(blocks::rule::render_rule(options))
        }
        NodeValue::Table(_) => {
            drop(ast);
            Some(blocks::table::render_table(node, options, false))
        }
        NodeValue::TableRow(_) | NodeValue::TableCell => {
            // Handled by table rendering
            None
        }
        NodeValue::HtmlBlock(hb) => {
            let literal = hb.literal.clone();
            drop(ast);
            Some(crate::html::render_html_block(&literal, options))
        }
        NodeValue::FootnoteDefinition(_) => {
            // Collected separately
            None
        }
        NodeValue::SoftBreak => None,
        NodeValue::LineBreak => None,
        _ => {
            // Try to render children
            drop(ast);
            let mut blocks = Vec::new();
            for child in node.children() {
                if let Some(block) = walk_block(child, options, depth, section) {
                    blocks.push(block);
                }
            }
            if blocks.is_empty() {
                None
            } else if blocks.len() == 1 {
                Some(blocks.into_iter().next().unwrap())
            } else {
                // Flatten multiple blocks as consecutive lines
                let mut all_lines = Vec::new();
                for block in blocks {
                    match block {
                        RenderedBlock::Lines(lines) => all_lines.extend(lines),
                        other => return Some(other),
                    }
                }
                Some(RenderedBlock::Lines(all_lines))
            }
        }
    }
}

/// Walk inline nodes and append styled segments to the current line
pub fn walk_inlines<'a>(
    node: &'a AstNode<'a>,
    line: &mut StyledLine,
    parent_style: &SegmentStyle,
    options: &RenderOptions,
) {
    for child in node.children() {
        let ast = child.data.borrow();
        match &ast.value {
            NodeValue::Text(text) => {
                let t = text.clone();
                drop(ast);
                push_text_with_doc_ids(&t, line, parent_style, options);
            }
            NodeValue::SoftBreak => {
                drop(ast);
                line.push(" ", parent_style.clone());
            }
            NodeValue::LineBreak => {
                drop(ast);
                line.push("\n", parent_style.clone());
            }
            NodeValue::Code(NodeCode { literal, .. }) => {
                let code_text = literal.clone();
                drop(ast);
                inline::render_code_span(&code_text, line, options);
            }
            NodeValue::Emph => {
                let mut style = parent_style.clone();
                style.italic = true;
                drop(ast);
                walk_inlines(child, line, &style, options);
            }
            NodeValue::Strong => {
                let mut style = parent_style.clone();
                style.bold = true;
                drop(ast);
                walk_inlines(child, line, &style, options);
            }
            NodeValue::Strikethrough => {
                let mut style = parent_style.clone();
                style.strikethrough = true;
                drop(ast);
                walk_inlines(child, line, &style, options);
            }
            NodeValue::Link(link) => {
                let url = link.url.clone();
                drop(ast);
                inline::render_link(child, &url, line, parent_style, options);
            }
            NodeValue::Image(link) => {
                let alt = link.title.clone();
                let url = link.url.clone();
                drop(ast);
                inline::render_image_inline(&alt, &url, line, options);
            }
            NodeValue::HtmlInline(raw) => {
                let html = raw.clone();
                drop(ast);
                crate::html::render_html_inline(&html, line, parent_style, options);
            }
            NodeValue::FootnoteReference(fref) => {
                let name_str = fref.name.clone();
                drop(ast);
                inline::render_footnote_ref(&name_str, line, options);
            }
            NodeValue::Superscript => {
                drop(ast);
                inline::render_superscript(child, line, parent_style, options);
            }
            _ => {
                drop(ast);
                walk_inlines(child, line, parent_style, options);
            }
        }
    }
}

/// Word-wrap a StyledLine into multiple lines to fit within width
pub fn wrap_line(line: StyledLine, width: usize) -> Vec<StyledLine> {
    if width == 0 {
        return vec![line];
    }

    use unicode_width::UnicodeWidthStr;

    // Flatten all text into a single string to find wrap points
    let total_width = line.content_width();
    if total_width <= width {
        return vec![line];
    }

    // When wrapping is needed, leave a 2-char right margin for readability
    let wrap_width = width.saturating_sub(2).max(1);

    // Simple character-level wrapping with style tracking
    let mut result = Vec::new();
    let mut current = StyledLine::with_indent(line.indent);
    let mut col = 0;

    for segment in &line.segments {
        let mut remaining = segment.text.as_str();
        while !remaining.is_empty() {
            // Find next word boundary
            let (word, rest) = next_word(remaining);
            let word_width = word.width();

            if col + word_width > wrap_width && col > 0 {
                // Wrap to next line
                result.push(current);
                current = StyledLine::with_indent(line.indent);
                col = 0;
                // Skip leading space on new line
                let trimmed = word.trim_start();
                if !trimmed.is_empty() {
                    let w = trimmed.width();
                    current.push(trimmed.to_string(), segment.style.clone());
                    col += w;
                }
            } else {
                current.push(word.to_string(), segment.style.clone());
                col += word_width;
            }
            remaining = rest;
        }
    }

    if !current.is_empty() {
        result.push(current);
    }

    if result.is_empty() {
        result.push(StyledLine::new());
    }

    result
}

/// Split off the next word (or whitespace run) from the string
fn next_word(s: &str) -> (&str, &str) {
    if s.is_empty() {
        return ("", "");
    }
    let start_is_space = s.chars().next().unwrap().is_whitespace();
    let split_pos = s
        .char_indices()
        .find(|(_, c)| {
            if start_is_space {
                !c.is_whitespace()
            } else {
                c.is_whitespace()
            }
        })
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    (&s[..split_pos], &s[split_pos..])
}

/// Push text to a line, highlighting doc ID patterns (e.g. ADR-001) as bold+white
/// when `options.highlight_doc_ids` is true.
fn push_text_with_doc_ids(
    text: &str,
    line: &mut StyledLine,
    parent_style: &SegmentStyle,
    options: &RenderOptions,
) {
    for seg in split_text_doc_ids(text, parent_style, options) {
        line.push_styled(seg);
    }
}

/// Split text into styled segments, highlighting doc ID patterns as bold+white.
/// Used by both the walker and the list renderer.
pub fn split_text_doc_ids(
    text: &str,
    parent_style: &SegmentStyle,
    options: &RenderOptions,
) -> Vec<StyledSegment> {
    if !options.highlight_doc_ids {
        return vec![StyledSegment {
            text: text.to_string(),
            style: parent_style.clone(),
        }];
    }

    let bytes = text.as_bytes();
    let mut result = Vec::new();
    let mut last_end = 0;
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i].is_ascii_alphabetic() && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric()) {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            let alpha_len = i - start;
            if alpha_len >= 2 && i < bytes.len() && bytes[i] == b'-' {
                i += 1;
                let num_start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if i > num_start && (i >= bytes.len() || !bytes[i].is_ascii_alphanumeric()) {
                    // Filter by known prefixes when configured
                    let prefix_ok = options.doc_id_prefixes.is_empty() || {
                        let prefix = text[start..start + alpha_len].to_uppercase();
                        options.doc_id_prefixes.iter().any(|p| p == &prefix)
                    };
                    if prefix_ok {
                        if start > last_end {
                            result.push(StyledSegment {
                                text: text[last_end..start].to_string(),
                                style: parent_style.clone(),
                            });
                        }
                        result.push(StyledSegment {
                            text: text[start..i].to_string(),
                            style: SegmentStyle {
                                bold: true,
                                underline: true,
                                fg: Some(Color::White),
                                ..parent_style.clone()
                            },
                        });
                        last_end = i;
                        continue;
                    }
                }
            }
            continue;
        }
        i += 1;
    }

    if last_end < text.len() {
        result.push(StyledSegment {
            text: text[last_end..].to_string(),
            style: parent_style.clone(),
        });
    }

    result
}

/// Collect footnote definitions from the document
fn collect_footnotes<'a>(
    root: &'a AstNode<'a>,
    options: &RenderOptions,
    doc: &mut RenderedDocument,
) {
    for child in root.children() {
        let ast = child.data.borrow();
        if let NodeValue::FootnoteDefinition(fdef) = &ast.value {
            let name = fdef.name.clone();
            drop(ast);
            let mut blocks = Vec::new();
            for grandchild in child.children() {
                if let Some(block) = walk_block(grandchild, options, 0, SectionHint::Neutral) {
                    blocks.push(block);
                }
            }
            doc.footnotes.push((name, blocks));
        } else {
            drop(ast);
        }
    }
}

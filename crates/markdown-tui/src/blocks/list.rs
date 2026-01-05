//! List rendering: ordered, unordered, task lists with nesting

use comrak::nodes::{AstNode, ListType, NodeValue};

use crate::options::RenderOptions;
use crate::types::{RenderedBlock, SectionHint, SegmentStyle, StyledLine, StyledSegment};
use crate::walker::{split_text_doc_ids, walk_block, walk_inlines, wrap_line};

const BULLETS: &[&str] = &["●", "○", "▪"];

/// Render a list (ordered or unordered)
pub fn render_list<'a>(
    node: &'a AstNode<'a>,
    list_type: ListType,
    start: usize,
    tight: bool,
    options: &RenderOptions,
    depth: usize,
    section: SectionHint,
) -> RenderedBlock {
    let mut all_lines = Vec::new();
    let indent = depth * 2;

    for (idx, child) in node.children().enumerate() {
        let ast = child.data.borrow();
        if let NodeValue::Item(_) = ast.value {
            drop(ast);

            // Detect task list from first child paragraph
            let task_status = detect_task_status(child);

            // Build bullet/number prefix with section-aware styling
            let (prefix, prefix_style) = match list_type {
                ListType::Bullet => {
                    if let Some(checked) = task_status {
                        let marker = if checked { "☑ " } else { "☐ " };
                        (
                            format!("{:indent$}{}", "", marker, indent = indent),
                            SegmentStyle::default(),
                        )
                    } else {
                        match section {
                            SectionHint::Positive => (
                                format!("{:indent$}+ ", "", indent = indent),
                                SegmentStyle {
                                    fg: Some(options.theme.positive_bullet_fg),
                                    bold: true,
                                    ..Default::default()
                                },
                            ),
                            SectionHint::Negative => (
                                format!("{:indent$}- ", "", indent = indent),
                                SegmentStyle {
                                    fg: Some(options.theme.negative_bullet_fg),
                                    bold: true,
                                    ..Default::default()
                                },
                            ),
                            SectionHint::Neutral => {
                                let bullet_idx = depth.min(BULLETS.len() - 1);
                                (
                                    format!(
                                        "{:indent$}{} ",
                                        "",
                                        BULLETS[bullet_idx],
                                        indent = indent
                                    ),
                                    SegmentStyle::default(),
                                )
                            }
                        }
                    }
                }
                ListType::Ordered => {
                    let num = start + idx;
                    (
                        format!("{:indent$}{}. ", "", num, indent = indent),
                        SegmentStyle::default(),
                    )
                }
            };

            // Render item content
            let mut first_line = true;
            let prefix_width = prefix.chars().count();

            for item_child in child.children() {
                let item_ast = item_child.data.borrow();
                match &item_ast.value {
                    NodeValue::Paragraph => {
                        drop(item_ast);
                        // Split paragraph at SoftBreak/LineBreak for terminal readability
                        let para_lines =
                            collect_paragraph_lines(item_child, &SegmentStyle::default(), options);
                        for (li, segments) in para_lines.into_iter().enumerate() {
                            if first_line && li == 0 {
                                // Build content-only line, wrap at width minus prefix
                                let content_width = options.width.saturating_sub(prefix_width);
                                let task_style = task_status.map(|checked| {
                                    if checked {
                                        SegmentStyle {
                                            fg: Some(options.theme.checkbox_done_fg),
                                            strikethrough: checked,
                                            ..Default::default()
                                        }
                                    } else {
                                        SegmentStyle {
                                            fg: Some(options.theme.checkbox_todo_fg),
                                            ..Default::default()
                                        }
                                    }
                                });

                                let mut content = StyledLine::new();
                                if let Some(ref style) = task_style {
                                    for seg in segments {
                                        let mut s = seg;
                                        s.style = style.clone();
                                        content.push_styled(s);
                                    }
                                } else {
                                    content.segments.extend(segments);
                                }

                                first_line = false;
                                let wrapped = wrap_line(content, content_width);
                                for (wi, wl) in wrapped.into_iter().enumerate() {
                                    let mut out = StyledLine::new();
                                    if wi == 0 {
                                        if task_style.is_some() {
                                            out.push_plain(&prefix);
                                        } else {
                                            out.push(prefix.clone(), prefix_style.clone());
                                        }
                                    } else {
                                        out.push_plain(" ".repeat(prefix_width));
                                    }
                                    out.segments.extend(wl.segments);
                                    all_lines.push(out);
                                }
                            } else {
                                // Don't indent continuation lines starting with └►
                                let starts_with_arrow =
                                    segments.first().is_some_and(|s| s.text.starts_with("└►"));
                                let cont_indent = if starts_with_arrow {
                                    " ".repeat(3)
                                } else {
                                    " ".repeat(prefix_width)
                                };
                                // Wrap content at width minus indent
                                let content_width = options.width.saturating_sub(cont_indent.len());
                                let mut content = StyledLine::new();
                                content.segments.extend(segments);
                                let wrapped = wrap_line(content, content_width);
                                for (wi, wl) in wrapped.into_iter().enumerate() {
                                    let mut out = StyledLine::new();
                                    // First └► line keeps its arrow; all others get indent
                                    if !(wi == 0 && starts_with_arrow) {
                                        out.push_plain(&cont_indent);
                                    }
                                    out.segments.extend(wl.segments);
                                    all_lines.push(out);
                                }
                            }
                        }
                    }
                    NodeValue::List(sub_list) => {
                        let sub_type = sub_list.list_type;
                        let sub_start = sub_list.start;
                        let sub_tight = sub_list.tight;
                        drop(item_ast);
                        if let RenderedBlock::Lines(sub_lines) = render_list(
                            item_child,
                            sub_type,
                            sub_start,
                            sub_tight,
                            options,
                            depth + 1,
                            section,
                        ) {
                            all_lines.extend(sub_lines);
                        }
                    }
                    _ => {
                        drop(item_ast);
                        if let Some(RenderedBlock::Lines(lines)) =
                            walk_block(item_child, options, depth, section)
                        {
                            for l in lines {
                                if first_line {
                                    let mut prefixed = StyledLine::new();
                                    prefixed.push(prefix.clone(), prefix_style.clone());
                                    prefixed.segments.extend(l.segments);
                                    all_lines.push(prefixed);
                                    first_line = false;
                                } else {
                                    let mut prefixed = StyledLine::new();
                                    prefixed.push_plain(" ".repeat(prefix_width));
                                    prefixed.segments.extend(l.segments);
                                    all_lines.push(prefixed);
                                }
                            }
                        }
                    }
                }
            }
            // Loose lists: add blank line between items
            if !tight {
                all_lines.push(StyledLine::new());
            }
        } else {
            drop(ast);
        }
    }

    // Remove trailing blank line from loose list
    if !tight && all_lines.last().is_some_and(|l| l.is_empty()) {
        all_lines.pop();
    }

    RenderedBlock::Lines(all_lines)
}

/// Detect if a list item is a task list item, returns Some(checked) or None
fn detect_task_status<'a>(item_node: &'a AstNode<'a>) -> Option<bool> {
    // Look at the first child paragraph's first child for TaskItem
    if let Some(first_child) = item_node.children().next() {
        let ast = first_child.data.borrow();
        if let NodeValue::Paragraph = ast.value {
            drop(ast);
            if let Some(inline) = first_child.children().next() {
                let iast = inline.data.borrow();
                if let NodeValue::TaskItem(checked) = &iast.value {
                    let c = checked.is_some();
                    drop(iast);
                    return Some(c);
                }
            }
        }
    }
    None
}

/// Walk a paragraph's children, splitting at SoftBreak/LineBreak into separate lines.
/// Returns Vec of segment groups — each group becomes one StyledLine.
fn collect_paragraph_lines<'a>(
    para_node: &'a AstNode<'a>,
    parent_style: &SegmentStyle,
    options: &RenderOptions,
) -> Vec<Vec<StyledSegment>> {
    let mut lines: Vec<Vec<StyledSegment>> = vec![Vec::new()];

    for child in para_node.children() {
        let ast = child.data.borrow();
        match &ast.value {
            NodeValue::SoftBreak | NodeValue::LineBreak => {
                drop(ast);
                lines.push(Vec::new());
            }
            NodeValue::TaskItem(_) => {
                drop(ast);
                // Skip — handled by caller
            }
            NodeValue::Text(text) => {
                let t = text.clone();
                drop(ast);
                if let Some(current) = lines.last_mut() {
                    current.extend(split_text_doc_ids(&t, parent_style, options));
                }
            }
            _ => {
                drop(ast);
                // Container inline (Emph, Strong, Link, etc.) — walk its children
                let mut tmp = StyledLine::new();
                walk_inlines(child, &mut tmp, parent_style, options);
                if let Some(current) = lines.last_mut() {
                    current.extend(tmp.segments);
                }
            }
        }
    }

    // Remove trailing empty lines
    while lines.last().is_some_and(|l| l.is_empty()) && lines.len() > 1 {
        lines.pop();
    }

    lines
}

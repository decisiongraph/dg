//! Blockquote rendering with │ prefix and callout detection

use comrak::nodes::AstNode;

use crate::options::RenderOptions;
use crate::types::{RenderedBlock, SectionHint, SegmentStyle, StyledLine};
use crate::walker::walk_block;

/// GFM-style callout types
#[derive(Debug, Clone)]
enum Callout {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

/// Render a blockquote, detecting callouts like [!NOTE]
pub fn render_blockquote<'a>(
    node: &'a AstNode<'a>,
    options: &RenderOptions,
    depth: usize,
    section: SectionHint,
) -> RenderedBlock {
    // First render children
    let mut inner_blocks = Vec::new();
    for child in node.children() {
        if let Some(block) = walk_block(child, options, depth, section) {
            inner_blocks.push(block);
        }
    }

    // Check first block for callout marker
    let callout = detect_callout(&inner_blocks);
    let (callout_type, inner_blocks) = if let Some((ct, modified)) = callout {
        (Some(ct), modified)
    } else {
        (None, inner_blocks)
    };

    // Flatten to lines and add │ prefix
    let mut result_lines = Vec::new();

    // Add callout header if detected
    if let Some(ref ct) = callout_type {
        let (icon, label, color) = callout_info(ct, options);
        let mut header = StyledLine::new();
        header.push(
            "│ ",
            SegmentStyle {
                fg: Some(color),
                ..Default::default()
            },
        );
        header.push(
            format!("{} {}", icon, label),
            SegmentStyle {
                fg: Some(color),
                bold: true,
                ..Default::default()
            },
        );
        result_lines.push(header);
    }

    let prefix_color = if let Some(ref ct) = callout_type {
        callout_info(ct, options).2
    } else {
        options.theme.blockquote_prefix_fg
    };

    for block in &inner_blocks {
        match block {
            RenderedBlock::Lines(lines) => {
                for line in lines {
                    let mut prefixed = StyledLine::new();
                    prefixed.push(
                        "│ ",
                        SegmentStyle {
                            fg: Some(prefix_color),
                            ..Default::default()
                        },
                    );
                    let default_style = if callout_type.is_none() {
                        SegmentStyle {
                            fg: Some(options.theme.blockquote_fg),
                            italic: true,
                            ..Default::default()
                        }
                    } else {
                        SegmentStyle::default()
                    };
                    if line.segments.is_empty() {
                        prefixed.push("", default_style);
                    } else {
                        for seg in &line.segments {
                            // Merge blockquote style with existing style
                            let mut merged = seg.style.clone();
                            if callout_type.is_none() {
                                if merged.fg.is_none() {
                                    merged.fg = Some(options.theme.blockquote_fg);
                                }
                                merged.italic = true;
                            }
                            prefixed.push(seg.text.clone(), merged);
                        }
                    }
                    result_lines.push(prefixed);
                }
            }
            RenderedBlock::Grid { lines } => {
                for grid_line in lines {
                    let mut prefixed = StyledLine::new();
                    prefixed.push(
                        "│ ",
                        SegmentStyle {
                            fg: Some(prefix_color),
                            ..Default::default()
                        },
                    );
                    prefixed.push_plain(grid_line);
                    result_lines.push(prefixed);
                }
            }
            RenderedBlock::Blank => {
                let mut prefixed = StyledLine::new();
                prefixed.push(
                    "│",
                    SegmentStyle {
                        fg: Some(prefix_color),
                        ..Default::default()
                    },
                );
                result_lines.push(prefixed);
            }
            _ => {}
        }
    }

    RenderedBlock::Lines(result_lines)
}

/// Detect if the first block starts with [!NOTE], [!TIP], etc.
fn detect_callout(blocks: &[RenderedBlock]) -> Option<(Callout, Vec<RenderedBlock>)> {
    if let Some(RenderedBlock::Lines(lines)) = blocks.first() {
        if let Some(first_line) = lines.first() {
            let text: String = first_line
                .segments
                .iter()
                .map(|s| s.text.as_str())
                .collect();
            let trimmed = text.trim();

            let (callout, rest_text) = if let Some(rest) = trimmed.strip_prefix("[!NOTE]") {
                (Callout::Note, rest.trim_start().to_string())
            } else if let Some(rest) = trimmed.strip_prefix("[!TIP]") {
                (Callout::Tip, rest.trim_start().to_string())
            } else if let Some(rest) = trimmed.strip_prefix("[!IMPORTANT]") {
                (Callout::Important, rest.trim_start().to_string())
            } else if let Some(rest) = trimmed.strip_prefix("[!WARNING]") {
                (Callout::Warning, rest.trim_start().to_string())
            } else if let Some(rest) = trimmed.strip_prefix("[!CAUTION]") {
                (Callout::Caution, rest.trim_start().to_string())
            } else {
                return None;
            };

            // Rebuild blocks without the callout marker
            let mut modified = Vec::new();
            if !rest_text.is_empty() || lines.len() > 1 {
                let mut new_lines = Vec::new();
                if !rest_text.is_empty() {
                    let mut new_line = StyledLine::new();
                    new_line.push_plain(&rest_text);
                    new_lines.push(new_line);
                }
                for line in &lines[1..] {
                    new_lines.push(line.clone());
                }
                if !new_lines.is_empty() {
                    modified.push(RenderedBlock::Lines(new_lines));
                }
            }
            modified.extend(blocks[1..].iter().cloned());

            return Some((callout, modified));
        }
    }
    None
}

/// Get icon, label, and color for a callout type
fn callout_info(
    callout: &Callout,
    options: &RenderOptions,
) -> (&'static str, &'static str, ratatui::style::Color) {
    match callout {
        Callout::Note => ("ℹ", "NOTE", options.theme.callout_note_fg),
        Callout::Tip => ("💡", "TIP", options.theme.callout_tip_fg),
        Callout::Important => ("❗", "IMPORTANT", options.theme.callout_important_fg),
        Callout::Warning => ("⚠", "WARNING", options.theme.callout_warning_fg),
        Callout::Caution => ("🔥", "CAUTION", options.theme.callout_caution_fg),
    }
}

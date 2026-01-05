//! Raw HTML handling: <details>, <summary>, <sup>, <sub>, <br>, <kbd>

use crate::options::RenderOptions;
use crate::types::{RenderedBlock, SegmentStyle, StyledLine};

/// Render an HTML block (e.g., <details>)
pub fn render_html_block(html: &str, options: &RenderOptions) -> RenderedBlock {
    let trimmed = html.trim();

    // Detect <details> block
    if trimmed.starts_with("<details") {
        return render_details_block(trimmed, options);
    }

    // Fallback: render as plain text
    let mut lines = Vec::new();
    for line in trimmed.lines() {
        let mut styled = StyledLine::new();
        styled.push_plain(line);
        lines.push(styled);
    }
    RenderedBlock::Lines(lines)
}

/// Render inline HTML tags
pub fn render_html_inline(
    html: &str,
    line: &mut StyledLine,
    parent_style: &SegmentStyle,
    _options: &RenderOptions,
) {
    let trimmed = html.trim();

    match trimmed {
        "<br>" | "<br/>" | "<br />" => {
            line.push("\n", parent_style.clone());
        }
        "<sup>" => {
            // Superscript start — handled inline
        }
        "</sup>" => {}
        "<sub>" => {}
        "</sub>" => {}
        "<kbd>" => {
            line.push("[", parent_style.clone());
        }
        "</kbd>" => {
            line.push("]", parent_style.clone());
        }
        "<mark>" => {
            // Could use bg color, skip for now
        }
        "</mark>" => {}
        _ => {
            // Unknown HTML — render as-is in dim style
            let style = SegmentStyle {
                fg: Some(ratatui::style::Color::DarkGray),
                ..parent_style.clone()
            };
            line.push(trimmed, style);
        }
    }
}

/// Render a <details> block (always expanded in string mode)
fn render_details_block(html: &str, _options: &RenderOptions) -> RenderedBlock {
    // Simple parsing: extract summary and body
    let summary = extract_between(html, "<summary>", "</summary>").unwrap_or("Details");
    let body = extract_after_tag(html, "</summary>");

    let mut lines = Vec::new();

    // Summary line with disclosure triangle
    let mut summary_line = StyledLine::new();
    summary_line.push(
        format!("▼ {}", summary.trim()),
        SegmentStyle {
            bold: true,
            ..Default::default()
        },
    );
    lines.push(summary_line);

    // Body content
    if let Some(body) = body {
        let body = body.trim().trim_end_matches("</details>").trim();
        if !body.is_empty() {
            for body_line in body.lines() {
                let mut styled = StyledLine::new();
                styled.push_plain(format!("  {}", body_line));
                lines.push(styled);
            }
        }
    }

    RenderedBlock::Lines(lines)
}

fn extract_between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start_idx = s.find(start)? + start.len();
    let end_idx = s[start_idx..].find(end)? + start_idx;
    Some(&s[start_idx..end_idx])
}

fn extract_after_tag<'a>(s: &'a str, tag: &str) -> Option<&'a str> {
    let idx = s.find(tag)? + tag.len();
    Some(&s[idx..])
}

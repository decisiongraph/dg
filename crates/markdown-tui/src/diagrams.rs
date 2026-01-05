//! Diagram rendering via graphs-tui (feature-gated)

#[cfg(feature = "diagrams")]
use crate::types::{RenderedBlock, SegmentStyle, StyledLine};
#[cfg(feature = "diagrams")]
use ratatui::style::Color;

/// Render a diagram (mermaid/d2) to Unicode box-drawing art.
///
/// Uses `graphs_tui` to parse and layout the diagram, then wraps
/// the output in a labeled box matching the code block style.
#[cfg(feature = "diagrams")]
pub fn render_diagram(lang: &str, code: &str, width: usize) -> RenderedBlock {
    let max_width = if width > 4 { Some(width - 4) } else { None };
    let opts = graphs_tui::RenderOptions {
        max_width,
        ..Default::default()
    };

    let result = graphs_tui::render(lang, code, opts);

    match result {
        Ok(rendered) => render_diagram_box(lang, &rendered.output, width, &rendered.warnings),
        Err(_) => {
            // Fallback: show raw code in a labeled box
            render_fallback_box(lang, code, width)
        }
    }
}

/// Wrap rendered diagram output in a labeled box with vertical padding.
///
/// Unlike code blocks, diagrams are NOT truncated — the box uses the
/// diagram's natural width so labels and edges remain intact. The terminal
/// clips anything beyond the screen edge.
#[cfg(feature = "diagrams")]
fn render_diagram_box(
    lang: &str,
    rendered: &str,
    _width: usize,
    warnings: &[graphs_tui::DiagramWarning],
) -> RenderedBlock {
    let border_style = SegmentStyle {
        fg: Some(crate::options::Theme::default().code_block_border_fg),
        ..Default::default()
    };
    let diagram_style = SegmentStyle::default();

    // Trim trailing whitespace — graphs-tui pads lines uniformly
    let diagram_lines: Vec<&str> = rendered.lines().map(|l| l.trim_end()).collect();
    let max_line_width = diagram_lines
        .iter()
        .map(|l| {
            use unicode_width::UnicodeWidthStr;
            l.width()
        })
        .max()
        .unwrap_or(0);

    let label = format!(" {} ", lang);
    // 1-char horizontal padding on each side between border and content
    let box_content = max_line_width.max(label.len() + 2);
    let box_inner = box_content + 2; // +2 for padding spaces

    let mut lines = Vec::new();

    // Top border: ┌─ d2 ──────────────────┐
    let mut top = StyledLine::new();
    let right_dashes = box_inner.saturating_sub(label.len() + 1);
    top.push(
        format!("┌─{}{}┐", label, "─".repeat(right_dashes)),
        border_style.clone(),
    );
    lines.push(top);

    // Top padding (single empty line)
    let mut pad_top = StyledLine::new();
    pad_top.push("│", border_style.clone());
    pad_top.push_plain(" ".repeat(box_inner));
    pad_top.push("│", border_style.clone());
    lines.push(pad_top);

    // Diagram content with horizontal padding
    for dl in &diagram_lines {
        use unicode_width::UnicodeWidthStr;
        let mut styled = StyledLine::new();
        styled.push("│ ", border_style.clone());
        let line_w = dl.width();
        styled.push(*dl, diagram_style.clone());
        if line_w < box_content {
            styled.push_plain(" ".repeat(box_content - line_w));
        }
        styled.push(" │", border_style.clone());
        lines.push(styled);
    }

    // Bottom border (no extra padding line — removes wasted whitespace)
    let mut bottom = StyledLine::new();
    bottom.push(format!("└{}┘", "─".repeat(box_inner)), border_style);
    lines.push(bottom);

    // Render warnings as amber text below the box with ⚠ prefix
    let warn_style = SegmentStyle {
        fg: Some(Color::Indexed(179)), // dim amber/yellow
        ..Default::default()
    };
    for warning in warnings {
        let text = warning.to_string();
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            let mut styled = StyledLine::new();
            styled.push(format!(" ⚠ {trimmed}"), warn_style.clone());
            lines.push(styled);
        }
    }

    RenderedBlock::Lines(lines)
}

/// Fallback: show raw code in a box when parsing fails.
#[cfg(feature = "diagrams")]
fn render_fallback_box(lang: &str, code: &str, width: usize) -> RenderedBlock {
    crate::blocks::code::render_code_block(
        lang,
        code,
        &crate::options::RenderOptions {
            width,
            ..Default::default()
        },
    )
}

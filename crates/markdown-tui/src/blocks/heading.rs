//! Heading rendering with level-based styling (gh-style: `## Heading`)

use comrak::nodes::AstNode;

use crate::options::RenderOptions;
use crate::types::{RenderedBlock, SegmentStyle, StyledLine};
use crate::walker::walk_inlines;

/// Render a heading (H1-H6) — gh CLI style with `#` prefix, no underlines
pub fn render_heading<'a>(
    node: &'a AstNode<'a>,
    level: u8,
    options: &RenderOptions,
) -> RenderedBlock {
    let fg = match level {
        1 => options.theme.h1_fg,
        2 => options.theme.h2_fg,
        _ => options.theme.h3_fg,
    };

    let style = SegmentStyle {
        bold: true,
        fg: Some(fg),
        ..Default::default()
    };

    let mut line = StyledLine::new();

    // Add `#` prefix like gh CLI
    let prefix = "#".repeat(level as usize);
    line.push(format!("{} ", prefix), style.clone());

    walk_inlines(node, &mut line, &style, options);

    RenderedBlock::Lines(vec![line])
}

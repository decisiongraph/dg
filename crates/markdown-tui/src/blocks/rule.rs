//! Horizontal rule rendering

use crate::options::RenderOptions;
use crate::types::{RenderedBlock, SegmentStyle, StyledLine};

/// Render a thematic break / horizontal rule
pub fn render_rule(options: &RenderOptions) -> RenderedBlock {
    let width = if options.width > 0 { options.width } else { 80 };
    let mut line = StyledLine::new();
    line.push(
        "─".repeat(width),
        SegmentStyle {
            fg: Some(options.theme.rule_fg),
            ..Default::default()
        },
    );
    RenderedBlock::Lines(vec![line])
}

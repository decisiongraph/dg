//! Stateless ratatui widget for rendering markdown

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

use crate::options::RenderOptions;
use crate::parser::parse_markdown;
use crate::types::{RenderedBlock, RenderedDocument};

/// A ratatui widget that renders markdown content
#[derive(Clone)]
pub struct MarkdownWidget<'a> {
    source: &'a str,
    style: Style,
    block: Option<Block<'a>>,
    options: RenderOptions,
}

impl<'a> MarkdownWidget<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            style: Style::default(),
            block: None,
            options: RenderOptions::default(),
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn width(mut self, width: usize) -> Self {
        self.options.width = width;
        self
    }

    pub fn options(mut self, options: RenderOptions) -> Self {
        self.options = options;
        self
    }
}

impl Widget for MarkdownWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut opts = self.options.clone();
        // Use area width if not explicitly set
        if opts.width == 80 {
            opts.width = area.width as usize;
        }

        let doc = parse_markdown(self.source, &opts);
        let ratatui_lines = doc_to_ratatui_lines(&doc, &self.style);

        let mut paragraph = Paragraph::new(ratatui_lines);

        if let Some(block) = self.block {
            paragraph = paragraph.block(block);
        }

        paragraph.render(area, buf);
    }
}

/// Convert a RenderedDocument to ratatui Lines
pub fn doc_to_ratatui_lines<'a>(doc: &RenderedDocument, base_style: &Style) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    for block in &doc.blocks {
        block_to_ratatui_lines(block, base_style, &mut lines);
        // Add blank line between blocks
        lines.push(Line::from(""));
    }

    // Footnotes
    if !doc.footnotes.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "───",
            Style::default().fg(Color::DarkGray),
        )));
        for (name, blocks) in &doc.footnotes {
            let mut spans = vec![Span::styled(
                format!("[^{}]: ", name),
                Style::default().fg(Color::Blue),
            )];
            // Simple: just render first block inline
            if let Some(RenderedBlock::Lines(ref blines)) = blocks.first() {
                if let Some(first_line) = blines.first() {
                    for seg in &first_line.segments {
                        spans.push(segment_to_span(seg, base_style));
                    }
                }
            }
            lines.push(Line::from(spans));
        }
    }

    // Remove trailing empty lines
    while lines.last().is_some_and(|l| l.spans.is_empty()) {
        lines.pop();
    }

    lines
}

fn block_to_ratatui_lines<'a>(
    block: &RenderedBlock,
    base_style: &Style,
    lines: &mut Vec<Line<'a>>,
) {
    match block {
        RenderedBlock::Lines(styled_lines) => {
            for styled_line in styled_lines {
                let mut spans: Vec<Span> = Vec::new();

                // Add indent
                if styled_line.indent > 0 {
                    spans.push(Span::raw(" ".repeat(styled_line.indent)));
                }

                for seg in &styled_line.segments {
                    spans.push(segment_to_span(seg, base_style));
                }

                lines.push(Line::from(spans));
            }
        }
        RenderedBlock::Grid { lines: grid_lines } => {
            for grid_line in grid_lines {
                lines.push(Line::from(Span::styled(grid_line.clone(), *base_style)));
            }
        }
        RenderedBlock::Collapsible {
            summary,
            body,
            expanded,
        } => {
            // Render disclosure triangle and summary
            let icon = if *expanded { "▼" } else { "▶" };
            let mut spans = vec![Span::styled(
                format!("{} ", icon),
                Style::default().add_modifier(Modifier::BOLD),
            )];
            for seg in summary {
                spans.push(segment_to_span(seg, base_style));
            }
            lines.push(Line::from(spans));

            if *expanded {
                for block in body {
                    block_to_ratatui_lines(block, base_style, lines);
                }
            }
        }
        RenderedBlock::Image { alt, url: _ } => {
            lines.push(Line::from(Span::styled(
                format!("[Image: {}]", alt),
                Style::default().fg(Color::Blue),
            )));
        }
        RenderedBlock::Blank => {
            lines.push(Line::from(""));
        }
    }
}

fn segment_to_span<'a>(seg: &crate::types::StyledSegment, base_style: &Style) -> Span<'a> {
    let mut style = *base_style;

    if seg.style.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if seg.style.italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if seg.style.underline {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if seg.style.strikethrough {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }
    if let Some(fg) = seg.style.fg {
        style = style.fg(fg);
    }
    if let Some(bg) = seg.style.bg {
        style = style.bg(bg);
    }

    Span::styled(seg.text.clone(), style)
}

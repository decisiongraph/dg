//! Stateful markdown widget with scroll and collapse support

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, Paragraph, Widget},
};

use crate::options::RenderOptions;
use crate::parser::parse_markdown;
use crate::types::RenderedDocument;
use crate::widget::doc_to_ratatui_lines;

/// Persistent state for a markdown widget
pub struct MarkdownState {
    /// Pre-parsed document
    doc: Option<RenderedDocument>,
    /// Vertical scroll offset (in lines)
    pub scroll: u16,
    /// Total rendered line count
    pub total_lines: usize,
}

impl MarkdownState {
    pub fn new() -> Self {
        Self {
            doc: None,
            scroll: 0,
            total_lines: 0,
        }
    }

    /// Parse and cache the markdown document
    pub fn set_content(&mut self, source: &str, options: &RenderOptions) {
        let doc = parse_markdown(source, options);
        self.doc = Some(doc);
        self.scroll = 0;
    }

    /// Scroll down by n lines
    pub fn scroll_down(&mut self, n: u16) {
        self.scroll = self.scroll.saturating_add(n);
        let max = self.total_lines.saturating_sub(1) as u16;
        if self.scroll > max {
            self.scroll = max;
        }
    }

    /// Scroll up by n lines
    pub fn scroll_up(&mut self, n: u16) {
        self.scroll = self.scroll.saturating_sub(n);
    }

    /// Get the cached document
    pub fn document(&self) -> Option<&RenderedDocument> {
        self.doc.as_ref()
    }
}

impl Default for MarkdownState {
    fn default() -> Self {
        Self::new()
    }
}

/// Stateful markdown widget that uses cached MarkdownState
pub struct StatefulMarkdownWidget<'a> {
    style: Style,
    block: Option<Block<'a>>,
}

impl<'a> StatefulMarkdownWidget<'a> {
    pub fn new() -> Self {
        Self {
            style: Style::default(),
            block: None,
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

    pub fn render(self, area: Rect, buf: &mut Buffer, state: &mut MarkdownState) {
        let lines = if let Some(doc) = &state.doc {
            doc_to_ratatui_lines(doc, &self.style)
        } else {
            vec![]
        };

        state.total_lines = lines.len();

        let mut paragraph = Paragraph::new(lines).scroll((state.scroll, 0));

        if let Some(block) = self.block {
            paragraph = paragraph.block(block);
        }

        paragraph.render(area, buf);
    }
}

impl Default for StatefulMarkdownWidget<'_> {
    fn default() -> Self {
        Self::new()
    }
}

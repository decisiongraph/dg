//! Ratatui widget for rendering math expressions (backward compat from tui-math)

use crate::math::{MathRenderer, RenderError};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget, Wrap},
};

/// A ratatui widget for rendering LaTeX math expressions
#[derive(Clone)]
pub struct MathWidget<'a> {
    latex: &'a str,
    style: Style,
    block: Option<Block<'a>>,
    use_unicode_scripts: bool,
    wrap: bool,
}

impl<'a> MathWidget<'a> {
    pub fn new(latex: &'a str) -> Self {
        Self {
            latex,
            style: Style::default(),
            block: None,
            use_unicode_scripts: true,
            wrap: false,
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.style = self.style.fg(color);
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.style = self.style.bg(color);
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn use_unicode_scripts(mut self, use_unicode: bool) -> Self {
        self.use_unicode_scripts = use_unicode;
        self
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn render_to_string(&self) -> Result<String, RenderError> {
        let renderer = MathRenderer::new().use_unicode_scripts(self.use_unicode_scripts);
        renderer.render_latex(self.latex)
    }
}

impl Widget for MathWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let renderer = MathRenderer::new().use_unicode_scripts(self.use_unicode_scripts);

        let rendered = match renderer.render_latex(self.latex) {
            Ok(s) => s,
            Err(e) => format!("Error: {}", e),
        };

        let lines: Vec<Line> = rendered
            .lines()
            .map(|line| Line::from(Span::styled(line.to_string(), self.style)))
            .collect();

        let mut paragraph = Paragraph::new(lines);

        if let Some(block) = self.block {
            paragraph = paragraph.block(block);
        }

        if self.wrap {
            paragraph = paragraph.wrap(Wrap { trim: false });
        }

        paragraph.render(area, buf);
    }
}

/// Stateful math widget state (caching)
pub struct MathWidgetState {
    rendered: Option<String>,
    error: Option<String>,
}

impl MathWidgetState {
    pub fn new() -> Self {
        Self {
            rendered: None,
            error: None,
        }
    }

    pub fn update(&mut self, latex: &str, use_unicode_scripts: bool) {
        let renderer = MathRenderer::new().use_unicode_scripts(use_unicode_scripts);
        match renderer.render_latex(latex) {
            Ok(s) => {
                self.rendered = Some(s);
                self.error = None;
            }
            Err(e) => {
                self.rendered = None;
                self.error = Some(e.to_string());
            }
        }
    }

    pub fn rendered(&self) -> Option<&str> {
        self.rendered.as_deref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl Default for MathWidgetState {
    fn default() -> Self {
        Self::new()
    }
}

/// Stateful math widget using cached rendering
pub struct StatefulMathWidget<'a> {
    style: Style,
    block: Option<Block<'a>>,
    wrap: bool,
}

impl<'a> StatefulMathWidget<'a> {
    pub fn new() -> Self {
        Self {
            style: Style::default(),
            block: None,
            wrap: false,
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

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn render(self, area: Rect, buf: &mut Buffer, state: &MathWidgetState) {
        let text = state
            .rendered
            .as_deref()
            .or(state.error.as_deref())
            .unwrap_or("");

        let lines: Vec<Line> = text
            .lines()
            .map(|line| Line::from(Span::styled(line.to_string(), self.style)))
            .collect();

        let mut paragraph = Paragraph::new(lines);

        if let Some(block) = self.block {
            paragraph = paragraph.block(block);
        }

        if self.wrap {
            paragraph = paragraph.wrap(Wrap { trim: false });
        }

        paragraph.render(area, buf);
    }
}

impl Default for StatefulMathWidget<'_> {
    fn default() -> Self {
        Self::new()
    }
}

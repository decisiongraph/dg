#![warn(missing_docs)]

//! # markdown-tui
//!
//! Render GitHub Flavored Markdown beautifully in terminal UIs with ratatui.
//!
//! ## Features
//!
//! - Full GFM rendering: headings, lists, tables, code blocks, blockquotes
//! - Syntax highlighting via syntect
//! - Unicode box-drawing for tables and code blocks
//! - Callout support ([!NOTE], [!WARNING], etc.)
//! - Task lists with checkboxes
//! - Math rendering (LaTeX to Unicode)
//! - Inline styles: bold, italic, strikethrough, links
//! - Both string output (ANSI) and ratatui widget output
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use markdown_tui::render_markdown;
//!
//! let output = render_markdown("# Hello\n\nThis is **bold** and *italic*.");
//! println!("{}", output);
//! ```
//!
//! ## As a ratatui Widget
//!
//! ```rust,no_run
//! use markdown_tui::MarkdownWidget;
//! use ratatui::widgets::Block;
//!
//! let widget = MarkdownWidget::new("# Hello World")
//!     .block(Block::bordered().title("README"));
//! ```

/// Block-level rendering (headings, tables, code blocks, lists, blockquotes).
#[allow(missing_docs)]
pub mod blocks;
#[cfg(feature = "diagrams")]
/// Diagram rendering (D2, Mermaid to terminal).
#[allow(missing_docs)]
pub mod diagrams;
/// HTML tag handling (kbd, details, etc.).
#[allow(missing_docs)]
pub mod html;
/// Image rendering support.
#[allow(missing_docs)]
pub mod images;
/// Inline element rendering (bold, italic, code, links).
#[allow(missing_docs)]
pub mod inline;
/// LaTeX/MathML to Unicode rendering.
#[allow(missing_docs)]
pub mod math;
/// Rendering configuration options.
#[allow(missing_docs)]
pub mod options;
/// Markdown AST parser (comrak wrapper).
#[allow(missing_docs)]
pub mod parser;
/// Stateful widget with scrolling support.
#[allow(missing_docs)]
pub mod stateful;
/// ANSI/plain string output renderer.
#[allow(missing_docs)]
pub mod string_renderer;
/// Core types (styled lines, segments, rendered blocks).
#[allow(missing_docs)]
pub mod types;
/// AST tree walker for rendering.
#[allow(missing_docs)]
pub mod walker;
/// Ratatui widget implementation.
#[allow(missing_docs)]
pub mod widget;

// Re-export main types
pub use options::RenderOptions;
pub use ratatui::style::Color;
pub use stateful::{MarkdownState, StatefulMarkdownWidget};
pub use types::{
    RenderedBlock, RenderedDocument, SectionHint, SegmentStyle, StyledLine, StyledSegment,
};
pub use widget::MarkdownWidget;

// Re-export math types for backward compat
pub use math::{CanvasMathWidget, MathBox, MathRenderer, MathWidget, MathWidgetState, RenderError};

/// Render markdown to an ANSI-colored string
pub fn render_markdown(source: &str) -> String {
    let options = RenderOptions::default();
    let doc = parser::parse_markdown(source, &options);
    string_renderer::render_to_ansi_string(&doc)
}

/// Render markdown to a plain string (no ANSI codes)
pub fn render_markdown_plain(source: &str) -> String {
    let options = RenderOptions::default();
    let doc = parser::parse_markdown(source, &options);
    string_renderer::render_to_plain_string(&doc)
}

/// Render markdown with custom options
pub fn render_markdown_with_options(source: &str, options: &RenderOptions) -> String {
    let doc = parser::parse_markdown(source, options);
    if options.ansi_colors {
        string_renderer::render_to_ansi_string(&doc)
    } else {
        string_renderer::render_to_plain_string(&doc)
    }
}

/// Render a data table to an ANSI-colored string with box-drawing borders.
///
/// `headers` — column header labels
/// `rows` — each row is a Vec of cell strings
pub fn render_table(headers: &[&str], rows: &[Vec<String>], options: &RenderOptions) -> String {
    let block = blocks::table::render_table_from_data(headers, rows, options);
    let doc = RenderedDocument {
        blocks: vec![block],
        footnotes: vec![],
    };
    if options.ansi_colors {
        string_renderer::render_to_ansi_string(&doc)
    } else {
        string_renderer::render_to_plain_string(&doc)
    }
}

/// Render LaTeX math to a Unicode string (backward compat from tui-math)
pub fn render_latex(latex: &str) -> Result<String, RenderError> {
    let renderer = MathRenderer::new();
    renderer.render_latex(latex)
}

/// Render MathML to a Unicode string (backward compat from tui-math)
pub fn render_mathml(mathml: &str) -> Result<String, RenderError> {
    let renderer = MathRenderer::new();
    renderer.render_mathml(mathml)
}

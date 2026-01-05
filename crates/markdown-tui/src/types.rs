//! Intermediate representation types for rendered markdown

use ratatui::style::Color;

/// Section context hint for list bullet styling.
///
/// When a heading contains words like "Pros" or "Benefits", subsequent lists
/// render with green `+` markers. Headings with "Cons" or "Risks" produce red `-`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SectionHint {
    #[default]
    Neutral,
    Positive,
    Negative,
}

/// A single styled text segment within a line
#[derive(Debug, Clone)]
pub struct StyledSegment {
    pub text: String,
    pub style: SegmentStyle,
}

/// Visual style for a text segment
#[derive(Debug, Clone, Default)]
pub struct SegmentStyle {
    pub bold: bool,
    pub italic: bool,
    pub strikethrough: bool,
    pub underline: bool,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub link_url: Option<String>,
}

/// A rendered line composed of styled segments
#[derive(Debug, Clone)]
pub struct StyledLine {
    pub segments: Vec<StyledSegment>,
    pub indent: usize,
}

impl StyledLine {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            indent: 0,
        }
    }

    pub fn with_indent(indent: usize) -> Self {
        Self {
            segments: Vec::new(),
            indent,
        }
    }

    pub fn push(&mut self, text: impl Into<String>, style: SegmentStyle) {
        self.segments.push(StyledSegment {
            text: text.into(),
            style,
        });
    }

    pub fn push_plain(&mut self, text: impl Into<String>) {
        self.segments.push(StyledSegment {
            text: text.into(),
            style: SegmentStyle::default(),
        });
    }

    pub fn push_styled(&mut self, segment: StyledSegment) {
        self.segments.push(segment);
    }

    /// Total display width of this line (excluding indent)
    pub fn content_width(&self) -> usize {
        use unicode_width::UnicodeWidthStr;
        self.segments.iter().map(|s| s.text.as_str().width()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty() || self.segments.iter().all(|s| s.text.is_empty())
    }
}

impl Default for StyledLine {
    fn default() -> Self {
        Self::new()
    }
}

/// A rendered block of markdown content
#[derive(Debug, Clone)]
pub enum RenderedBlock {
    /// Paragraphs, headings, list items — styled lines
    Lines(Vec<StyledLine>),
    /// Pre-rendered grid (math, diagrams) — already formatted as plain strings
    Grid { lines: Vec<String> },
    /// Collapsible section from <details>
    Collapsible {
        summary: Vec<StyledSegment>,
        body: Vec<RenderedBlock>,
        expanded: bool,
    },
    /// Image reference
    Image { alt: String, url: String },
    /// Blank line separator
    Blank,
}

/// A fully rendered markdown document
#[derive(Debug, Clone)]
pub struct RenderedDocument {
    pub blocks: Vec<RenderedBlock>,
    pub footnotes: Vec<(String, Vec<RenderedBlock>)>,
}

impl RenderedDocument {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            footnotes: Vec::new(),
        }
    }
}

impl Default for RenderedDocument {
    fn default() -> Self {
        Self::new()
    }
}

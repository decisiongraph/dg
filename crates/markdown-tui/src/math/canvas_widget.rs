//! Canvas-based math widget using Braille markers for sub-cell resolution
//!
//! Uses Braille characters for smooth lines (fraction bars, sqrt) while
//! rendering text normally for better readability.

use crate::math::{MathBox, MathRenderer};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    symbols::Marker,
    widgets::{
        canvas::{Canvas, Line},
        Block, Widget,
    },
};

/// A high-resolution math widget using Canvas with Braille markers for lines
#[derive(Clone)]
pub struct CanvasMathWidget<'a> {
    latex: &'a str,
    style: Style,
    block: Option<Block<'a>>,
    color: Color,
}

impl<'a> CanvasMathWidget<'a> {
    /// Create a new CanvasMathWidget from a LaTeX expression
    pub fn new(latex: &'a str) -> Self {
        Self {
            latex,
            style: Style::default(),
            block: None,
            color: Color::White,
        }
    }

    /// Set the style
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set the drawing color
    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Wrap in a block
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

/// Line segment to draw with Braille
struct BrailleLine {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

/// Extract line segments and text positions from MathBox
/// area_height is used to flip y coordinates for Canvas (which has y=0 at bottom)
fn extract_elements(
    mbox: &MathBox,
    area_height: f64,
) -> (Vec<BrailleLine>, Vec<(usize, usize, char)>) {
    let mut lines = Vec::new();
    let mut text_chars = Vec::new();

    let content = mbox.to_lines();

    for (row, line) in content.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            // Convert screen row (0=top) to canvas y (0=bottom)
            // For row r, we want the line in the middle of that cell
            // Screen row r is at canvas y = area_height - r - 0.5 (middle of cell)
            let canvas_y_mid = area_height - row as f64 - 0.5;
            let _canvas_y_top = area_height - row as f64;
            let _canvas_y_bot = area_height - row as f64 - 1.0;

            match ch {
                // Horizontal line for fractions - draw with Braille for smoothness
                '─' => {
                    let x1 = col as f64;
                    let x2 = (col + 1) as f64;
                    lines.push(BrailleLine {
                        x1,
                        y1: canvas_y_mid,
                        x2,
                        y2: canvas_y_mid,
                    });
                }
                // Keep box-drawing characters as text for better visual connection
                // with adjacent symbols like √
                '╱' | '╲' | '│' => {
                    text_chars.push((col, row, ch));
                }
                // Everything else is text
                ' ' => {} // skip spaces
                _ => {
                    text_chars.push((col, row, ch));
                }
            }
        }
    }

    (lines, text_chars)
}

impl Widget for CanvasMathWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // First render to MathBox using existing renderer
        let renderer = MathRenderer::new();
        let mbox = match renderer.render_to_box(self.latex) {
            Ok(b) => b,
            Err(e) => {
                buf.set_string(area.x, area.y, format!("Error: {}", e), self.style);
                return;
            }
        };

        // Calculate content area (accounting for block borders)
        let content_area = if let Some(ref block) = self.block {
            let inner = block.inner(area);
            block.clone().render(area, buf);
            inner
        } else {
            area
        };

        // Extract line segments and text
        // Use MathBox height for coordinate mapping to ensure alignment
        let mbox_height_f = mbox.height as f64;
        let (braille_lines, text_chars) = extract_elements(&mbox, mbox_height_f);

        // Render Canvas FIRST (so text can overlay it)
        if !braille_lines.is_empty() {
            let mbox_width = mbox.width as u16;
            let mbox_height = mbox.height as u16;
            let color = self.color;

            // Create canvas area that matches MathBox size
            let canvas_area = Rect::new(
                content_area.x,
                content_area.y,
                mbox_width.min(content_area.width),
                mbox_height.min(content_area.height),
            );

            let canvas_width = canvas_area.width as f64;
            let canvas_height = canvas_area.height as f64;

            let canvas = Canvas::default()
                .marker(Marker::Braille)
                .x_bounds([0.0, canvas_width])
                .y_bounds([0.0, canvas_height])
                .paint(move |ctx| {
                    for line in &braille_lines {
                        // Add 0.5 to x coordinates to align Braille dots with character cells
                        ctx.draw(&Line {
                            x1: line.x1 + 0.5,
                            y1: line.y1,
                            x2: line.x2 + 0.5,
                            y2: line.y2,
                            color,
                        });
                    }
                });

            canvas.render(canvas_area, buf);
        }

        // Render text characters AFTER canvas (so text overlays Braille)
        for (col, row, ch) in &text_chars {
            let x = content_area.x + *col as u16;
            let y = content_area.y + *row as u16;
            if x < content_area.right() && y < content_area.bottom() {
                buf.set_string(x, y, ch.to_string(), self.style);
            }
        }
    }
}

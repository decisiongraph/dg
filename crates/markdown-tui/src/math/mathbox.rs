//! MathBox - A 2D character grid for math rendering

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Represents a box of grapheme clusters for rendering math expressions.
/// Uses a 2D grid with baseline tracking for proper vertical alignment.
/// Each cell holds a grapheme cluster (base char + combining marks).
#[derive(Clone, Debug)]
pub struct MathBox {
    content: Vec<Vec<String>>,
    pub width: usize,
    pub height: usize,
    /// The baseline row (0-indexed from top)
    pub baseline: usize,
}

impl MathBox {
    /// Create a MathBox from a single-line string
    pub fn from_text(text: &str) -> Self {
        let graphemes: Vec<String> = text.graphemes(true).map(|g| g.to_string()).collect();
        let width = text.width();

        // Pad to match display width (handles wide chars)
        let mut cells = Vec::with_capacity(width);
        for g in graphemes {
            let g_width = g.width();
            cells.push(g);
            // Add empty cells for wide characters
            for _ in 1..g_width {
                cells.push(String::new());
            }
        }
        // Ensure we have exactly 'width' cells
        while cells.len() < width {
            cells.push(" ".to_string());
        }

        Self {
            content: vec![cells],
            width,
            height: 1,
            baseline: 0,
        }
    }

    /// Create an empty MathBox with specified dimensions
    pub fn empty(width: usize, height: usize, baseline: usize) -> Self {
        Self {
            content: vec![vec![" ".to_string(); width]; height],
            width,
            height,
            baseline: baseline.min(height.saturating_sub(1)),
        }
    }

    /// Create a MathBox from multiple lines
    pub fn from_lines(lines: Vec<String>, baseline: usize) -> Self {
        let height = lines.len();
        let width = lines.iter().map(|l| l.width()).max().unwrap_or(0);
        let mut content = vec![vec![" ".to_string(); width]; height];

        for (y, line) in lines.iter().enumerate() {
            let mut x = 0;
            for g in line.graphemes(true) {
                if x < width {
                    let g_width = g.width();
                    content[y][x] = g.to_string();
                    // Mark continuation cells for wide chars
                    for i in 1..g_width {
                        if x + i < width {
                            content[y][x + i] = String::new();
                        }
                    }
                    x += g_width;
                }
            }
        }

        Self {
            content,
            width,
            height,
            baseline: baseline.min(height.saturating_sub(1)),
        }
    }

    /// Get grapheme at position (returns space if out of bounds or empty)
    pub fn get(&self, x: usize, y: usize) -> char {
        if y < self.height && x < self.width {
            self.content[y][x].chars().next().unwrap_or(' ')
        } else {
            ' '
        }
    }

    /// Get full grapheme cluster at position
    pub fn get_grapheme(&self, x: usize, y: usize) -> &str {
        if y < self.height && x < self.width {
            &self.content[y][x]
        } else {
            " "
        }
    }

    /// Set character at position
    pub fn set(&mut self, x: usize, y: usize, ch: char) {
        if y < self.height && x < self.width {
            self.content[y][x] = ch.to_string();
        }
    }

    /// Set grapheme cluster at position
    pub fn set_grapheme(&mut self, x: usize, y: usize, g: &str) {
        if y < self.height && x < self.width {
            self.content[y][x] = g.to_string();
        }
    }

    /// Copy another MathBox into this one at the specified offset
    pub fn blit(&mut self, other: &MathBox, x_offset: usize, y_offset: usize) {
        for y in 0..other.height {
            for x in 0..other.width {
                let target_x = x_offset + x;
                let target_y = y_offset + y;
                if target_y < self.height && target_x < self.width {
                    let g = other.get_grapheme(x, y);
                    if !g.is_empty() && g != " " {
                        self.set_grapheme(target_x, target_y, g);
                    }
                }
            }
        }
    }

    /// Concatenate horizontally, aligning by baseline
    pub fn concat_horizontal(boxes: &[MathBox]) -> MathBox {
        if boxes.is_empty() {
            return MathBox::empty(0, 1, 0);
        }

        // Find max ascent (baseline) and max descent (height - baseline - 1)
        let max_ascent = boxes.iter().map(|b| b.baseline).max().unwrap_or(0);
        let max_descent = boxes
            .iter()
            .map(|b| b.height.saturating_sub(b.baseline + 1))
            .max()
            .unwrap_or(0);

        let total_width: usize = boxes.iter().map(|b| b.width).sum();
        let total_height = max_ascent + 1 + max_descent;

        let mut result = MathBox::empty(total_width, total_height, max_ascent);
        let mut x_pos = 0;

        for b in boxes {
            let y_offset = max_ascent - b.baseline;
            result.blit(b, x_pos, y_offset);
            x_pos += b.width;
        }

        result
    }

    /// Stack vertically, centered horizontally
    pub fn stack_vertical(boxes: &[MathBox]) -> MathBox {
        if boxes.is_empty() {
            return MathBox::empty(0, 1, 0);
        }

        let max_width = boxes.iter().map(|b| b.width).max().unwrap_or(0);
        let total_height: usize = boxes.iter().map(|b| b.height).sum();

        let mut result = MathBox::empty(max_width, total_height, 0);
        let mut y_pos = 0;

        for b in boxes {
            let x_offset = (max_width - b.width) / 2;
            result.blit(b, x_offset, y_pos);
            y_pos += b.height;
        }

        // Baseline at middle
        result.baseline = total_height / 2;
        result
    }

    /// Fill a row with a character
    pub fn fill_row(&mut self, y: usize, ch: char) {
        if y < self.height {
            for x in 0..self.width {
                self.set(x, y, ch);
            }
        }
    }

    /// Fill a column with a character
    pub fn fill_col(&mut self, x: usize, ch: char) {
        if x < self.width {
            for y in 0..self.height {
                self.set(x, y, ch);
            }
        }
    }

    /// Get lines as vector of strings
    pub fn to_lines(&self) -> Vec<String> {
        self.content.iter().map(|row| row.join("")).collect()
    }
}

impl std::fmt::Display for MathBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self
            .content
            .iter()
            .map(|row| row.join("").trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        f.write_str(&s)
    }
}

impl Default for MathBox {
    fn default() -> Self {
        Self::empty(0, 1, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_text() {
        let mb = MathBox::from_text("abc");
        assert_eq!(mb.width, 3);
        assert_eq!(mb.height, 1);
        assert_eq!(mb.get(0, 0), 'a');
        assert_eq!(mb.get(2, 0), 'c');
    }

    #[test]
    fn test_combining_chars() {
        // T with combining macron (T̄) should be width 1
        let mb = MathBox::from_text("T\u{0304}");
        assert_eq!(mb.width, 1);
        assert_eq!(mb.get_grapheme(0, 0), "T\u{0304}");
    }

    #[test]
    fn test_concat_horizontal() {
        let a = MathBox::from_text("x");
        let b = MathBox::from_text("+");
        let c = MathBox::from_text("y");
        let result = MathBox::concat_horizontal(&[a, b, c]);
        assert_eq!(result.to_string(), "x+y");
    }
}

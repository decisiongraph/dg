//! MathML to Unicode terminal renderer

use crate::math::mathbox::MathBox;
use crate::math::unicode_maps::{get_greek, get_symbol, to_subscript, to_superscript, BRACKETS};
use latex2mathml::{latex_to_mathml, DisplayStyle};
use roxmltree::{Document, Node};
use std::fmt;

/// Errors that can occur during math rendering
#[derive(Debug)]
pub enum RenderError {
    LatexConversion(String),
    MathMLParse(String),
    InvalidStructure(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::LatexConversion(e) => write!(f, "LaTeX conversion error: {}", e),
            RenderError::MathMLParse(e) => write!(f, "MathML parse error: {}", e),
            RenderError::InvalidStructure(e) => write!(f, "Invalid math structure: {}", e),
        }
    }
}

impl std::error::Error for RenderError {}

/// Math renderer that converts LaTeX/MathML to Unicode terminal output
pub struct MathRenderer {
    use_unicode_scripts: bool,
}

impl MathRenderer {
    pub fn new() -> Self {
        Self {
            use_unicode_scripts: true,
        }
    }

    /// Set whether to use Unicode superscript/subscript characters when possible
    pub fn use_unicode_scripts(mut self, use_unicode: bool) -> Self {
        self.use_unicode_scripts = use_unicode;
        self
    }

    /// Render LaTeX math to Unicode string
    pub fn render_latex(&self, latex: &str) -> Result<String, RenderError> {
        let mathml = latex_to_mathml(latex, DisplayStyle::Inline)
            .map_err(|e| RenderError::LatexConversion(e.to_string()))?;
        self.render_mathml(&mathml)
    }

    /// Render MathML to Unicode string
    pub fn render_mathml(&self, mathml: &str) -> Result<String, RenderError> {
        let doc = Document::parse(mathml).map_err(|e| RenderError::MathMLParse(e.to_string()))?;
        let root = doc.root_element();
        let math_box = self.process_element(&root, 0)?;
        Ok(math_box.to_string())
    }

    /// Render to MathBox (for advanced usage)
    pub fn render_to_box(&self, latex: &str) -> Result<MathBox, RenderError> {
        let mathml = latex_to_mathml(latex, DisplayStyle::Inline)
            .map_err(|e| RenderError::LatexConversion(e.to_string()))?;
        let doc = Document::parse(&mathml).map_err(|e| RenderError::MathMLParse(e.to_string()))?;
        let root = doc.root_element();
        self.process_element(&root, 0)
    }

    /// Maximum recursion depth to prevent stack overflow on pathological input
    const MAX_DEPTH: usize = 64;

    fn process_element(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        if depth > Self::MAX_DEPTH {
            return Err(RenderError::InvalidStructure(
                "maximum recursion depth exceeded".to_string(),
            ));
        }

        let tag = node.tag_name().name();

        match tag {
            "math" | "mrow" | "mstyle" | "mpadded" | "mphantom" => self.process_row(node, depth),
            "mi" | "mn" | "mtext" => self.process_text(node),
            "mo" => self.process_operator(node),
            "msup" => self.process_superscript(node, depth),
            "msub" => self.process_subscript(node, depth),
            "msubsup" => self.process_subsup(node, depth),
            "mfrac" => self.process_fraction(node, depth),
            "msqrt" => self.process_sqrt(node, depth),
            "mroot" => self.process_nthroot(node, depth),
            "mover" => self.process_over(node, depth),
            "munder" => self.process_under(node, depth),
            "munderover" => self.process_underover(node, depth),
            "mtable" => self.process_table(node, depth),
            "mtr" => self.process_table_row(node, depth),
            "mtd" => self.process_row(node, depth),
            "mfenced" => self.process_fenced(node, depth),
            "menclose" => {
                self.process_row(node, depth) // Simplified
            }
            "mspace" => Ok(MathBox::from_text(" ")),
            "semantics" => {
                // Process first child only
                if let Some(child) = node.children().find(|n| n.is_element()) {
                    self.process_element(&child, depth + 1)
                } else {
                    Ok(MathBox::empty(0, 1, 0))
                }
            }
            "annotation" | "annotation-xml" => {
                // Skip annotations
                Ok(MathBox::empty(0, 1, 0))
            }
            _ => {
                // Unknown element, try to process children
                self.process_row(node, depth)
            }
        }
    }

    fn process_row(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        self.process_row_inner(node, true, depth)
    }

    fn process_row_compact(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        self.process_row_inner(node, false, depth)
    }

    fn process_row_inner(
        &self,
        node: &Node,
        add_spacing: bool,
        depth: usize,
    ) -> Result<MathBox, RenderError> {
        let child_nodes: Vec<_> = node.children().filter(|n| n.is_element()).collect();

        if child_nodes.is_empty() {
            let text = self.get_text_content(node);
            if !text.is_empty() {
                return Ok(MathBox::from_text(&text));
            }
            return Ok(MathBox::empty(0, 1, 0));
        }

        let mut boxes = Vec::new();
        let mut prev_multiline = false;

        for (i, child) in child_nodes.iter().enumerate() {
            let child_box = self.process_element(child, depth + 1)?;
            let is_multiline = child_box.height > 1;

            // Add spacing between multi-line elements
            if add_spacing && i > 0 && (prev_multiline || is_multiline) {
                boxes.push(MathBox::from_text(" "));
            }

            // Add spacing around binary operators in row context (not in compact mode)
            if add_spacing && child.tag_name().name() == "mo" {
                let op = self.get_text_content(child);
                let is_first = i == 0;
                let is_binary_op = !is_first && matches!(op.as_str(), "+" | "-" | "±" | "∓");
                let is_relation = matches!(
                    op.as_str(),
                    "=" | "≤" | "≥" | "≠" | "≈" | "≡" | "→" | "⇒" | "⟹" | "×" | "÷" | "·"
                );

                if is_binary_op || is_relation {
                    // Don't add extra space if we just added one for multiline
                    if !prev_multiline && !is_multiline {
                        boxes.push(MathBox::from_text(" "));
                    }
                    boxes.push(child_box);
                    boxes.push(MathBox::from_text(" "));
                    prev_multiline = is_multiline;
                    continue;
                }
            }
            boxes.push(child_box);
            prev_multiline = is_multiline;
        }

        Ok(MathBox::concat_horizontal(&boxes))
    }

    fn process_text(&self, node: &Node) -> Result<MathBox, RenderError> {
        let text = self.get_text_content(node);

        // Handle Greek letters and special identifiers
        if let Some(greek) = get_greek(&text) {
            return Ok(MathBox::from_text(&greek.to_string()));
        }

        Ok(MathBox::from_text(&text))
    }

    fn process_operator(&self, node: &Node) -> Result<MathBox, RenderError> {
        let text = self.get_text_content(node);

        // Handle special operators
        let rendered = match text.as_str() {
            "∑" | "∏" | "∫" | "∬" | "∭" | "∮" | "⋃" | "⋂" => {
                // Big operators - keep as is
                text
            }
            _ => {
                // Check if it's a LaTeX command
                if let Some(cmd) = text.strip_prefix('\\') {
                    if let Some(sym) = get_symbol(cmd) {
                        sym.to_string()
                    } else if let Some(greek) = get_greek(cmd) {
                        greek.to_string()
                    } else {
                        text
                    }
                } else {
                    text
                }
            }
        };

        // Spacing is handled in process_row for context-aware operator spacing
        Ok(MathBox::from_text(&rendered))
    }

    fn process_superscript(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        let children: Vec<_> = node.children().filter(|n| n.is_element()).collect();
        if children.len() != 2 {
            return Err(RenderError::InvalidStructure(
                "msup requires exactly 2 children".to_string(),
            ));
        }

        let base = self.process_element(&children[0], depth + 1)?;
        // Use compact mode for superscript content (no spacing around operators)
        let sup = if children[1].tag_name().name() == "mrow" {
            self.process_row_compact(&children[1], depth + 1)?
        } else {
            self.process_element(&children[1], depth + 1)?
        };

        // Try Unicode superscript for simple cases
        if self.use_unicode_scripts && base.height == 1 && sup.height == 1 {
            let sup_text = sup.to_string();
            if let Some(unicode_sup) = to_superscript(sup_text.trim()) {
                let combined = format!("{}{}", base, unicode_sup);
                return Ok(MathBox::from_text(&combined));
            }
        }

        // Fall back to 2D rendering
        let width = base.width + sup.width;
        let height = base.height + 1;
        let mut result = MathBox::empty(width, height, base.baseline + 1);

        // Place base at bottom
        result.blit(&base, 0, 1);
        // Place superscript at top-right
        result.blit(&sup, base.width, 0);

        Ok(result)
    }

    fn process_subscript(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        let children: Vec<_> = node.children().filter(|n| n.is_element()).collect();
        if children.len() != 2 {
            return Err(RenderError::InvalidStructure(
                "msub requires exactly 2 children".to_string(),
            ));
        }

        let base = self.process_element(&children[0], depth + 1)?;
        // Use compact mode for subscript content (no spacing around operators)
        let sub = if children[1].tag_name().name() == "mrow" {
            self.process_row_compact(&children[1], depth + 1)?
        } else {
            self.process_element(&children[1], depth + 1)?
        };

        // Try Unicode subscript for simple cases
        if self.use_unicode_scripts && base.height == 1 && sub.height == 1 {
            let sub_text = sub.to_string();
            if let Some(unicode_sub) = to_subscript(sub_text.trim()) {
                let combined = format!("{}{}", base, unicode_sub);
                return Ok(MathBox::from_text(&combined));
            }
        }

        // Fall back to 2D rendering
        let width = base.width + sub.width;
        let height = base.height + 1;
        let mut result = MathBox::empty(width, height, base.baseline);

        // Place base at top
        result.blit(&base, 0, 0);
        // Place subscript at bottom-right
        result.blit(&sub, base.width, base.height);

        Ok(result)
    }

    fn process_subsup(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        let children: Vec<_> = node.children().filter(|n| n.is_element()).collect();
        if children.len() != 3 {
            return Err(RenderError::InvalidStructure(
                "msubsup requires exactly 3 children".to_string(),
            ));
        }

        // Check if base is a big operator (integral, sum, etc.)
        let base_text = self.get_text_content(&children[0]);
        let is_big_operator = matches!(
            base_text.as_str(),
            "∫" | "∬" | "∭" | "∮" | "∑" | "∏" | "⋃" | "⋂"
        );

        let base = self.process_element(&children[0], depth + 1)?;
        let sub = self.process_element(&children[1], depth + 1)?;
        let sup = self.process_element(&children[2], depth + 1)?;

        // For big operators, stack limits vertically (centered)
        if is_big_operator {
            return Ok(MathBox::stack_vertical(&[sup, base, sub]));
        }

        // Try Unicode scripts for simple cases
        if self.use_unicode_scripts && base.height == 1 && sub.height == 1 && sup.height == 1 {
            let sub_text = sub.to_string();
            let sup_text = sup.to_string();
            if let (Some(unicode_sub), Some(unicode_sup)) = (
                to_subscript(sub_text.trim()),
                to_superscript(sup_text.trim()),
            ) {
                let combined = format!("{}{}{}", base, unicode_sub, unicode_sup);
                return Ok(MathBox::from_text(&combined));
            }
        }

        // 2D rendering with both
        let script_width = sub.width.max(sup.width);
        let width = base.width + script_width;
        let height = base.height + 2;
        let mut result = MathBox::empty(width, height, base.baseline + 1);

        result.blit(&base, 0, 1);
        result.blit(&sup, base.width, 0);
        result.blit(&sub, base.width, height - 1);

        Ok(result)
    }

    fn process_fraction(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        let children: Vec<_> = node.children().filter(|n| n.is_element()).collect();
        if children.len() != 2 {
            return Err(RenderError::InvalidStructure(
                "mfrac requires exactly 2 children".to_string(),
            ));
        }

        let num = self.process_element(&children[0], depth + 1)?;
        let den = self.process_element(&children[1], depth + 1)?;

        let width = num.width.max(den.width);
        let height = num.height + 1 + den.height;
        let baseline = num.height;

        let mut result = MathBox::empty(width, height, baseline);

        // Center numerator
        let num_offset = (width - num.width) / 2;
        result.blit(&num, num_offset, 0);

        // Draw fraction line using box-drawing character
        result.fill_row(num.height, '─');

        // Center denominator
        let den_offset = (width - den.width) / 2;
        result.blit(&den, den_offset, num.height + 1);

        Ok(result)
    }

    fn process_sqrt(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        let inner = self.process_row(node, depth)?;

        // Simple sqrt rendering: √ followed by content with overline
        // Layout: ___
        //        √abc

        if inner.height == 1 {
            let inner_text = inner.to_string();
            let inner_width = inner_text.chars().count();

            // Single line: √ + content, with overline above content
            let width = 1 + inner_width;
            let height = 2;
            let mut result = MathBox::empty(width, height, 1);

            // Draw bar above the content (not above √)
            for x in 1..width {
                result.set(x, 0, '_');
            }

            // Draw √ and content
            result.set(0, 1, '√');
            for (i, ch) in inner_text.chars().enumerate() {
                result.set(1 + i, 1, ch);
            }

            return Ok(result);
        }

        // Multi-line sqrt - use simple bracket approach
        let width = inner.width + 1;
        let height = inner.height + 1;
        let mut result = MathBox::empty(width, height, inner.baseline + 1);

        // Draw bar
        for x in 1..width {
            result.set(x, 0, '_');
        }

        // Draw √ at the left
        result.set(0, 1, '√');

        // Place content
        result.blit(&inner, 1, 1);

        Ok(result)
    }

    fn process_nthroot(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        let children: Vec<_> = node.children().filter(|n| n.is_element()).collect();
        if children.len() != 2 {
            return Err(RenderError::InvalidStructure(
                "mroot requires exactly 2 children".to_string(),
            ));
        }

        let inner = self.process_element(&children[0], depth + 1)?;
        let index = self.process_element(&children[1], depth + 1)?;

        // Try Unicode superscript for index
        let index_text = index.to_string();
        if let Some(unicode_idx) = to_superscript(index_text.trim()) {
            let text = format!("{}√{}", unicode_idx, inner);
            return Ok(MathBox::from_text(&text));
        }

        // 2D rendering
        let width = index.width + inner.width + 2;
        let height = (inner.height + 1).max(index.height);
        let mut result = MathBox::empty(width, height, height / 2);

        // Place index
        result.blit(&index, 0, 0);

        // Draw sqrt and content
        result.set(index.width, height - 1, '√');
        for x in (index.width + 1)..width {
            result.set(x, 0, '─');
        }
        result.blit(&inner, index.width + 2, 1);

        Ok(result)
    }

    fn process_over(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        let children: Vec<_> = node.children().filter(|n| n.is_element()).collect();
        if children.len() != 2 {
            return Err(RenderError::InvalidStructure(
                "mover requires exactly 2 children".to_string(),
            ));
        }

        let base = self.process_element(&children[0], depth + 1)?;
        let over = self.process_element(&children[1], depth + 1)?;

        let over_text = over.to_string().trim().to_string();

        // Handle common accents on single-height bases
        if base.height == 1 {
            let accent = match over_text.as_str() {
                "^" | "ˆ" => Some("̂"),  // Combining circumflex
                "~" | "˜" => Some("̃"),  // Combining tilde
                "¯" | "-" => Some("̄"),  // Combining macron (bar)
                "." => Some("̇"),        // Combining dot above
                ".." | "¨" => Some("̈"), // Combining diaeresis
                "→" => Some("⃗"),        // Combining right arrow
                _ => None,
            };
            if let Some(combining) = accent {
                let base_text = base.to_string();
                let text = format!("{}{}", base_text, combining);
                return Ok(MathBox::from_text(&text));
            }
        }

        // Stack vertically
        Ok(MathBox::stack_vertical(&[over, base]))
    }

    fn process_under(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        let children: Vec<_> = node.children().filter(|n| n.is_element()).collect();
        if children.len() != 2 {
            return Err(RenderError::InvalidStructure(
                "munder requires exactly 2 children".to_string(),
            ));
        }

        let base_text = self.get_text_content(&children[0]);
        let base = self.process_element(&children[0], depth + 1)?;
        let under = self.process_element(&children[1], depth + 1)?;

        // For "lim" and similar operators, render subscript inline
        if base_text == "lim"
            || base_text == "max"
            || base_text == "min"
            || base_text == "sup"
            || base_text == "inf"
        {
            // Try to convert to Unicode subscript, fallback to parentheses
            let under_text = under.to_string();
            let under_trimmed = under_text.trim();

            // Try full Unicode subscript conversion
            if let Some(subscript) = to_subscript(under_trimmed) {
                let combined = format!("{}{}", base_text, subscript);
                return Ok(MathBox::from_text(&combined));
            }

            // Fallback: use parentheses notation
            let combined = format!("{}({})", base_text, under_trimmed);
            return Ok(MathBox::from_text(&combined));
        }

        // For other elements, stack with baseline at the base element
        let width = base.width.max(under.width);
        let height = base.height + under.height;
        let mut result = MathBox::empty(width, height, base.baseline);

        // Center base
        let base_offset = (width - base.width) / 2;
        result.blit(&base, base_offset, 0);

        // Center under below base
        let under_offset = (width - under.width) / 2;
        result.blit(&under, under_offset, base.height);

        Ok(result)
    }

    fn process_underover(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        let children: Vec<_> = node.children().filter(|n| n.is_element()).collect();
        if children.len() != 3 {
            return Err(RenderError::InvalidStructure(
                "munderover requires exactly 3 children".to_string(),
            ));
        }

        let base = self.process_element(&children[0], depth + 1)?;
        let under = self.process_element(&children[1], depth + 1)?;
        let over = self.process_element(&children[2], depth + 1)?;

        Ok(MathBox::stack_vertical(&[over, base, under]))
    }

    fn process_table(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        let rows: Vec<Vec<MathBox>> = node
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "mtr")
            .map(|row| {
                row.children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "mtd")
                    .map(|cell| self.process_row(&cell, depth + 1))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;

        if rows.is_empty() {
            return Ok(MathBox::empty(0, 1, 0));
        }

        // Calculate column widths and row heights
        let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut col_widths = vec![0; num_cols];
        let mut row_heights = vec![0; rows.len()];

        for (i, row) in rows.iter().enumerate() {
            for (j, cell) in row.iter().enumerate() {
                col_widths[j] = col_widths[j].max(cell.width);
                row_heights[i] = row_heights[i].max(cell.height);
            }
        }

        // Add spacing
        let spacing = 2;
        let total_width: usize =
            col_widths.iter().sum::<usize>() + spacing * (num_cols.saturating_sub(1));
        let total_height: usize = row_heights.iter().sum();

        let mut result = MathBox::empty(total_width, total_height, total_height / 2);

        let mut y_pos = 0;
        for (i, row) in rows.iter().enumerate() {
            let mut x_pos = 0;
            for (j, cell) in row.iter().enumerate() {
                // Center cell in its column
                let x_offset = (col_widths[j] - cell.width) / 2;
                result.blit(cell, x_pos + x_offset, y_pos);
                x_pos += col_widths[j] + spacing;
            }
            y_pos += row_heights[i];
        }

        Ok(result)
    }

    fn process_table_row(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        let cells: Vec<MathBox> = node
            .children()
            .filter(|n| n.is_element())
            .map(|n| self.process_row(&n, depth + 1))
            .collect::<Result<Vec<_>, _>>()?;

        // Join cells with spacing
        let spacing = MathBox::from_text("  ");
        let mut parts = Vec::new();
        for (i, cell) in cells.into_iter().enumerate() {
            if i > 0 {
                parts.push(spacing.clone());
            }
            parts.push(cell);
        }

        Ok(MathBox::concat_horizontal(&parts))
    }

    fn process_fenced(&self, node: &Node, depth: usize) -> Result<MathBox, RenderError> {
        let open = node.attribute("open").unwrap_or("(");
        let close = node.attribute("close").unwrap_or(")");

        let inner = self.process_row(node, depth)?;

        if inner.height <= 1 {
            // Simple case
            let text = format!("{}{}{}", open, inner, close);
            return Ok(MathBox::from_text(&text));
        }

        // Scaled brackets
        let left_chars = BRACKETS.get_left(open, inner.height);
        let right_chars = BRACKETS.get_right(close, inner.height);

        let width = 1 + inner.width + 1;
        let height = inner.height;
        let mut result = MathBox::empty(width, height, inner.baseline);

        // Draw brackets
        for (y, &ch) in left_chars.iter().enumerate() {
            result.set(0, y, ch);
        }
        for (y, &ch) in right_chars.iter().enumerate() {
            result.set(width - 1, y, ch);
        }

        // Place content
        result.blit(&inner, 1, 0);

        Ok(result)
    }

    fn get_text_content(&self, node: &Node) -> String {
        let mut text = String::new();
        for child in node.children() {
            if child.is_text() {
                text.push_str(child.text().unwrap_or(""));
            }
        }
        text.trim().to_string()
    }
}

impl Default for MathRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_expression() {
        let renderer = MathRenderer::new();
        let result = renderer.render_latex("x + y").unwrap();
        assert!(result.contains('x'));
        assert!(result.contains('y'));
    }

    #[test]
    fn test_superscript() {
        let renderer = MathRenderer::new();
        let result = renderer.render_latex("x^2").unwrap();
        // Should contain Unicode superscript
        assert!(result.contains('²') || result.contains('2'));
    }

    #[test]
    fn test_fraction() {
        let renderer = MathRenderer::new();
        let result = renderer.render_latex(r"\frac{a}{b}").unwrap();
        assert!(result.contains('a'));
        assert!(result.contains('b'));
        assert!(result.contains('─'));
    }
}

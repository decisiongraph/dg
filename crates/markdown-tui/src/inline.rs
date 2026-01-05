//! Inline element rendering: bold, italic, code spans, links, images

use comrak::nodes::AstNode;

use crate::options::RenderOptions;
use crate::types::{SegmentStyle, StyledLine};
use crate::walker::walk_inlines;

/// Render an inline code span: `code`
pub fn render_code_span(text: &str, line: &mut StyledLine, options: &RenderOptions) {
    let style = SegmentStyle {
        fg: Some(options.theme.code_fg),
        bg: Some(options.theme.code_bg),
        ..Default::default()
    };
    line.push(format!("`{}`", text), style);
}

/// Render a link: [text](url)
pub fn render_link<'a>(
    node: &'a AstNode<'a>,
    url: &str,
    line: &mut StyledLine,
    parent_style: &SegmentStyle,
    options: &RenderOptions,
) {
    let mut style = parent_style.clone();
    style.fg = Some(options.theme.link_fg);
    style.underline = true;
    style.link_url = Some(url.to_string());
    walk_inlines(node, line, &style, options);
}

/// Render an inline image reference
pub fn render_image_inline(alt: &str, _url: &str, line: &mut StyledLine, options: &RenderOptions) {
    let style = SegmentStyle {
        fg: Some(options.theme.link_fg),
        ..Default::default()
    };
    // Get alt text from the node text or fallback to title
    let display = if alt.is_empty() { "image" } else { alt };
    line.push(format!("[Image: {}]", display), style);
}

/// Render a footnote reference [^name]
pub fn render_footnote_ref(name: &str, line: &mut StyledLine, options: &RenderOptions) {
    let style = SegmentStyle {
        fg: Some(options.theme.link_fg),
        ..Default::default()
    };
    line.push(format!("[^{}]", name), style);
}

/// Render superscript text
pub fn render_superscript<'a>(
    node: &'a AstNode<'a>,
    line: &mut StyledLine,
    parent_style: &SegmentStyle,
    options: &RenderOptions,
) {
    // Try to use Unicode superscript characters
    let mut text = String::new();
    collect_text(node, &mut text);

    if let Some(sup) = crate::math::unicode_maps::to_superscript(&text) {
        line.push(sup, parent_style.clone());
    } else {
        line.push("^(", parent_style.clone());
        walk_inlines(node, line, parent_style, options);
        line.push(")", parent_style.clone());
    }
}

/// Collect plain text from a node and its children
fn collect_text<'a>(node: &'a AstNode<'a>, buf: &mut String) {
    use comrak::nodes::NodeValue;
    for child in node.children() {
        let ast = child.data.borrow();
        match &ast.value {
            NodeValue::Text(t) => {
                buf.push_str(t);
                drop(ast);
            }
            _ => {
                drop(ast);
                collect_text(child, buf);
            }
        }
    }
}

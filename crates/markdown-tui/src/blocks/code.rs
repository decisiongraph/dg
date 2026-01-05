//! Code block rendering with syntect syntax highlighting

use once_cell::sync::Lazy;
use syntect::highlighting::{Color as SynColor, ThemeSet};
use syntect::parsing::SyntaxSet;

use crate::options::RenderOptions;
use crate::types::{RenderedBlock, SegmentStyle, StyledLine};

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

/// Render a fenced code block with syntax highlighting
pub fn render_code_block(info: &str, code: &str, options: &RenderOptions) -> RenderedBlock {
    let lang = info.split_whitespace().next().unwrap_or("");

    // Dispatch special languages
    match lang {
        "math" | "latex" | "tex" => {
            return render_math_block(code, options);
        }
        #[cfg(feature = "diagrams")]
        "mermaid" | "d2" => {
            return render_diagram_block(lang, code, options);
        }
        _ => {}
    }

    let width = if options.width > 0 { options.width } else { 80 };
    let code = code.trim_end_matches('\n');
    let code_lines: Vec<&str> = code.lines().collect();

    // Find max line width for the box
    let max_line_width = code_lines
        .iter()
        .map(|l| {
            use unicode_width::UnicodeWidthStr;
            l.width()
        })
        .max()
        .unwrap_or(0);
    let box_inner_width = max_line_width
        .max(lang.len() + 2)
        .min(width.saturating_sub(2));

    let border_style = SegmentStyle {
        fg: Some(options.theme.code_block_border_fg),
        ..Default::default()
    };

    let mut lines = Vec::new();

    // Top border: ┌─ lang ─┐
    let mut top = StyledLine::new();
    if lang.is_empty() {
        top.push(
            format!("┌{}┐", "─".repeat(box_inner_width)),
            border_style.clone(),
        );
    } else {
        let label = format!(" {} ", lang);
        let right_pad = box_inner_width.saturating_sub(label.len() + 1);
        top.push(
            format!("┌─{}{}┐", label, "─".repeat(right_pad)),
            border_style.clone(),
        );
    }
    lines.push(top);

    // Try syntax highlighting
    let highlighted = highlight_code(code, lang);

    match highlighted {
        Some(highlighted_lines) => {
            for hl_line in highlighted_lines {
                let mut styled = StyledLine::new();
                styled.push("│", border_style.clone());
                // Truncate segments to fit box_inner_width
                let mut remaining = box_inner_width;
                for seg in hl_line {
                    use unicode_width::UnicodeWidthStr;
                    let seg_w = seg.text.as_str().width();
                    if remaining == 0 {
                        break;
                    }
                    if seg_w <= remaining {
                        styled.push_styled(seg);
                        remaining -= seg_w;
                    } else {
                        // Truncate this segment
                        let truncated = truncate_str(&seg.text, remaining.saturating_sub(1));
                        styled.push(format!("{truncated}\u{2026}"), seg.style);
                        remaining = 0;
                    }
                }
                if remaining > 0 {
                    styled.push_plain(" ".repeat(remaining));
                }
                styled.push("│", border_style.clone());
                lines.push(styled);
            }
        }
        None => {
            // Plain rendering without highlighting
            for code_line in &code_lines {
                use unicode_width::UnicodeWidthStr;
                let line_width = code_line.width();
                let mut styled = StyledLine::new();
                styled.push("│", border_style.clone());
                if line_width <= box_inner_width {
                    styled.push_plain(*code_line);
                    styled.push_plain(" ".repeat(box_inner_width - line_width));
                } else {
                    let truncated = truncate_str(code_line, box_inner_width.saturating_sub(1));
                    styled.push_plain(format!("{truncated}\u{2026}"));
                    let tw = truncated.width() + 1; // +1 for ellipsis
                    if tw < box_inner_width {
                        styled.push_plain(" ".repeat(box_inner_width - tw));
                    }
                }
                styled.push("│", border_style.clone());
                lines.push(styled);
            }
        }
    }

    // Bottom border: └───┘
    let mut bottom = StyledLine::new();
    bottom.push(format!("└{}┘", "─".repeat(box_inner_width)), border_style);
    lines.push(bottom);

    RenderedBlock::Lines(lines)
}

/// Highlight code using syntect, returns styled segments per line
fn highlight_code(code: &str, lang: &str) -> Option<Vec<Vec<crate::types::StyledSegment>>> {
    let syntax = if lang.is_empty() {
        return None;
    } else {
        SYNTAX_SET
            .find_syntax_by_token(lang)
            .or_else(|| SYNTAX_SET.find_syntax_by_extension(lang))?
    };

    let theme = &THEME_SET.themes["base16-ocean.dark"];
    let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);

    let mut result = Vec::new();

    for line in code.lines() {
        let regions = highlighter.highlight_line(line, &SYNTAX_SET).ok()?;

        let mut segments = Vec::new();
        for (style, text) in regions {
            let fg = syn_color_to_ratatui(style.foreground);
            segments.push(crate::types::StyledSegment {
                text: text.to_string(),
                style: SegmentStyle {
                    fg: Some(fg),
                    ..Default::default()
                },
            });
        }
        result.push(segments);
    }

    Some(result)
}

/// Convert syntect color to ratatui color
fn syn_color_to_ratatui(c: SynColor) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(c.r, c.g, c.b)
}

/// Render a math code block (```math or ```latex)
fn render_math_block(code: &str, _options: &RenderOptions) -> RenderedBlock {
    let renderer = crate::math::MathRenderer::new();
    match renderer.render_latex(code.trim()) {
        Ok(rendered) => RenderedBlock::Grid {
            lines: rendered.lines().map(String::from).collect(),
        },
        Err(e) => {
            let mut line = StyledLine::new();
            line.push_plain(format!("[Math error: {}]", e));
            RenderedBlock::Lines(vec![line])
        }
    }
}

/// Truncate a string to fit within `max_chars` display width (char-based).
fn truncate_str(s: &str, max_chars: usize) -> &str {
    use unicode_width::UnicodeWidthStr;
    if s.width() <= max_chars {
        return s;
    }
    let mut end = 0;
    let mut w = 0;
    for (i, ch) in s.char_indices() {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if w + cw > max_chars {
            break;
        }
        w += cw;
        end = i + ch.len_utf8();
    }
    &s[..end]
}

/// Render a diagram code block (mermaid/d2) — feature-gated
#[cfg(feature = "diagrams")]
fn render_diagram_block(lang: &str, code: &str, options: &RenderOptions) -> RenderedBlock {
    let width = if options.width > 0 { options.width } else { 80 };
    crate::diagrams::render_diagram(lang, code, width)
}

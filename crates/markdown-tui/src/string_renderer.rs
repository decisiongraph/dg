//! Convert RenderedDocument to String output (plain or ANSI)

use crate::types::{RenderedBlock, RenderedDocument, StyledLine};

/// Render a document to a plain string (no ANSI codes)
pub fn render_to_plain_string(doc: &RenderedDocument) -> String {
    let mut output = String::new();
    let mut first = true;

    for block in &doc.blocks {
        if !first {
            output.push('\n');
        }
        first = false;
        render_block_plain(&mut output, block);
    }

    // Render footnotes
    if !doc.footnotes.is_empty() {
        output.push_str("\n\n───\n");
        for (name, blocks) in &doc.footnotes {
            output.push_str(&format!("[^{}]: ", name));
            for block in blocks {
                render_block_plain(&mut output, block);
            }
            output.push('\n');
        }
    }

    output
}

/// Render a document to a string with ANSI color codes
pub fn render_to_ansi_string(doc: &RenderedDocument) -> String {
    let mut output = String::new();
    let mut first = true;

    for block in &doc.blocks {
        if !first {
            output.push('\n');
        }
        first = false;
        render_block_ansi(&mut output, block);
    }

    // Render footnotes
    if !doc.footnotes.is_empty() {
        output.push_str("\n\n───\n");
        for (name, blocks) in &doc.footnotes {
            output.push_str(&format!("\x1b[34m[^{}]\x1b[0m: ", name));
            for block in blocks {
                render_block_ansi(&mut output, block);
            }
            output.push('\n');
        }
    }

    output
}

fn render_block_plain(output: &mut String, block: &RenderedBlock) {
    match block {
        RenderedBlock::Lines(lines) => {
            for line in lines {
                render_line_plain(output, line);
                output.push('\n');
            }
        }
        RenderedBlock::Grid { lines } => {
            for line in lines {
                output.push_str(line);
                output.push('\n');
            }
        }
        RenderedBlock::Collapsible {
            summary,
            body,
            expanded: _,
        } => {
            // Always expanded in string mode
            output.push_str("▼ ");
            for seg in summary {
                output.push_str(&seg.text);
            }
            output.push('\n');
            for block in body {
                render_block_plain(output, block);
            }
        }
        RenderedBlock::Image { alt, url } => {
            output.push_str(&format!("[Image: {}]({})\n", alt, url));
        }
        RenderedBlock::Blank => {
            output.push('\n');
        }
    }
}

fn render_line_plain(output: &mut String, line: &StyledLine) {
    // Add indent
    for _ in 0..line.indent {
        output.push(' ');
    }
    for seg in &line.segments {
        output.push_str(&seg.text);
    }
}

fn render_block_ansi(output: &mut String, block: &RenderedBlock) {
    match block {
        RenderedBlock::Lines(lines) => {
            for line in lines {
                render_line_ansi(output, line);
                output.push('\n');
            }
        }
        RenderedBlock::Grid { lines } => {
            for line in lines {
                output.push_str(line);
                output.push('\n');
            }
        }
        RenderedBlock::Collapsible {
            summary,
            body,
            expanded: _,
        } => {
            output.push_str("\x1b[1m▼ ");
            for seg in summary {
                render_segment_ansi(output, seg);
            }
            output.push_str("\x1b[0m\n");
            for block in body {
                render_block_ansi(output, block);
            }
        }
        RenderedBlock::Image { alt, url } => {
            output.push_str(&format!("\x1b[34m[Image: {}]\x1b[0m({})\n", alt, url));
        }
        RenderedBlock::Blank => {
            output.push('\n');
        }
    }
}

fn render_line_ansi(output: &mut String, line: &StyledLine) {
    for _ in 0..line.indent {
        output.push(' ');
    }
    for seg in &line.segments {
        render_segment_ansi(output, seg);
    }
}

fn render_segment_ansi(output: &mut String, seg: &crate::types::StyledSegment) {
    let style = &seg.style;
    let mut codes: Vec<String> = Vec::new();

    if style.bold {
        codes.push("1".into());
    }
    if style.italic {
        codes.push("3".into());
    }
    if style.underline {
        codes.push("4".into());
    }
    if style.strikethrough {
        codes.push("9".into());
    }

    if let Some(fg) = &style.fg {
        if let Some(code) = color_to_ansi_fg(fg) {
            codes.push(code);
        }
    }

    if let Some(bg) = &style.bg {
        if let Some(code) = color_to_ansi_bg(bg) {
            codes.push(code);
        }
    }

    if !codes.is_empty() {
        output.push_str(&format!("\x1b[{}m", codes.join(";")));
        output.push_str(&seg.text);
        output.push_str("\x1b[0m");
    } else {
        output.push_str(&seg.text);
    }
}

fn color_to_ansi_fg(color: &ratatui::style::Color) -> Option<String> {
    use ratatui::style::Color;
    match color {
        Color::Black => Some("30".into()),
        Color::Red => Some("31".into()),
        Color::Green => Some("32".into()),
        Color::Yellow => Some("33".into()),
        Color::Blue => Some("34".into()),
        Color::Magenta => Some("35".into()),
        Color::Cyan => Some("36".into()),
        Color::Gray => Some("37".into()),
        Color::DarkGray => Some("90".into()),
        Color::LightRed => Some("91".into()),
        Color::LightGreen => Some("92".into()),
        Color::LightYellow => Some("93".into()),
        Color::LightBlue => Some("94".into()),
        Color::LightMagenta => Some("95".into()),
        Color::LightCyan => Some("96".into()),
        Color::White => Some("97".into()),
        Color::Indexed(n) => Some(format!("38;5;{n}")),
        Color::Rgb(r, g, b) => Some(format!("38;2;{r};{g};{b}")),
        _ => None,
    }
}

fn color_to_ansi_bg(color: &ratatui::style::Color) -> Option<String> {
    use ratatui::style::Color;
    match color {
        Color::Black => Some("40".into()),
        Color::Red => Some("41".into()),
        Color::Green => Some("42".into()),
        Color::Yellow => Some("43".into()),
        Color::Blue => Some("44".into()),
        Color::Magenta => Some("45".into()),
        Color::Cyan => Some("46".into()),
        Color::Gray => Some("47".into()),
        Color::DarkGray => Some("100".into()),
        Color::LightRed => Some("101".into()),
        Color::LightGreen => Some("102".into()),
        Color::LightYellow => Some("103".into()),
        Color::LightBlue => Some("104".into()),
        Color::LightMagenta => Some("105".into()),
        Color::LightCyan => Some("106".into()),
        Color::White => Some("107".into()),
        Color::Indexed(n) => Some(format!("48;5;{n}")),
        Color::Rgb(r, g, b) => Some(format!("48;2;{r};{g};{b}")),
        _ => None,
    }
}

//! Rendering options and theme configuration

use ratatui::style::Color;

/// Options controlling how markdown is rendered
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Maximum width for rendered output (0 = no limit)
    pub width: usize,
    /// Whether to output ANSI color codes in string mode
    pub ansi_colors: bool,
    /// Theme colors
    pub theme: Theme,
    /// Whether to render images (requires feature)
    pub render_images: bool,
    /// Whether to render diagrams (requires feature)
    pub render_diagrams: bool,
    /// Case-insensitive cell text to foreground color for table cells.
    /// When a table cell's trimmed text matches a key, the color is applied.
    pub cell_highlights: Vec<(String, Color)>,
    /// Highlight entire table rows when a status cell is overdue.
    /// If a row contains a cell matching one of `statuses` (case-insensitive)
    /// AND a cell with a YYYY-MM-DD date before `today`, apply `color` to all cells.
    pub overdue_highlight: Option<OverdueHighlight>,
    /// When true, highlight document ID patterns (e.g. ADR-001) in body text as bold+white.
    pub highlight_doc_ids: bool,
    /// Uppercase type prefixes to restrict doc ID highlighting (e.g. ["ADR", "OPP"]).
    /// When empty, all `LETTERS-DIGITS` patterns are highlighted (legacy behavior).
    pub doc_id_prefixes: Vec<String>,
    /// Section headings (lowercase) whose tables get an auto-generated "#" row-number column.
    pub auto_number_sections: Vec<String>,
}

/// Configuration for overdue row highlighting in tables.
#[derive(Debug, Clone)]
pub struct OverdueHighlight {
    /// Status values that can be overdue (lowercase, e.g. "pending", "in-progress")
    pub statuses: Vec<String>,
    /// Color to apply to overdue rows
    pub color: Color,
    /// Today's date as YYYY-MM-DD for comparison
    pub today: String,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            width: 80,
            ansi_colors: true,
            theme: Theme::default(),
            render_images: false,
            render_diagrams: false,
            cell_highlights: Vec::new(),
            overdue_highlight: None,
            highlight_doc_ids: false,
            doc_id_prefixes: Vec::new(),
            auto_number_sections: Vec::new(),
        }
    }
}

/// Color theme for rendered markdown
#[derive(Debug, Clone)]
pub struct Theme {
    pub h1_fg: Color,
    pub h2_fg: Color,
    pub h3_fg: Color,
    pub code_fg: Color,
    pub code_bg: Color,
    pub link_fg: Color,
    pub blockquote_fg: Color,
    pub blockquote_prefix_fg: Color,
    pub callout_note_fg: Color,
    pub callout_warning_fg: Color,
    pub callout_important_fg: Color,
    pub callout_tip_fg: Color,
    pub callout_caution_fg: Color,
    pub rule_fg: Color,
    pub table_border_fg: Color,
    pub code_block_border_fg: Color,
    pub checkbox_done_fg: Color,
    pub checkbox_todo_fg: Color,
    pub positive_bullet_fg: Color,
    pub negative_bullet_fg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            h1_fg: Color::Indexed(39), // gh blue (#00afff)
            h2_fg: Color::Indexed(39),
            h3_fg: Color::Indexed(39),
            code_fg: Color::Indexed(203), // gh salmon/red
            code_bg: Color::Indexed(236), // gh dark gray (#303030)
            link_fg: Color::Blue,
            blockquote_fg: Color::Gray,
            blockquote_prefix_fg: Color::DarkGray,
            callout_note_fg: Color::Blue,
            callout_warning_fg: Color::Yellow,
            callout_important_fg: Color::Magenta,
            callout_tip_fg: Color::Green,
            callout_caution_fg: Color::Red,
            rule_fg: Color::DarkGray,
            table_border_fg: Color::DarkGray,
            code_block_border_fg: Color::DarkGray,
            checkbox_done_fg: Color::Green,
            checkbox_todo_fg: Color::DarkGray,
            positive_bullet_fg: Color::Indexed(40), // bright green (#00d700)
            negative_bullet_fg: Color::Indexed(203), // red, same as code fg
        }
    }
}

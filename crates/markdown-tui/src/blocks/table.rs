//! Table rendering with Unicode box-drawing characters

use comrak::nodes::{AstNode, NodeValue, TableAlignment};

use crate::options::RenderOptions;
use crate::types::{RenderedBlock, SegmentStyle, StyledLine, StyledSegment};
use crate::walker::walk_inlines;

/// Internal alignment enum so the shared helper doesn't depend on comrak
#[derive(Clone, Copy)]
enum Alignment {
    Left,
    Center,
    Right,
}

impl From<TableAlignment> for Alignment {
    fn from(a: TableAlignment) -> Self {
        match a {
            TableAlignment::Center => Alignment::Center,
            TableAlignment::Right => Alignment::Right,
            _ => Alignment::Left,
        }
    }
}

/// Apply cell_highlights: if the cell's trimmed text matches a highlight key (case-insensitive),
/// set the foreground color on all segments.
fn apply_cell_highlights(segments: &mut [StyledSegment], options: &RenderOptions) {
    if options.cell_highlights.is_empty() {
        return;
    }
    let text: String = segments.iter().map(|s| s.text.as_str()).collect();
    let lower = text.trim().to_lowercase();
    if let Some((_, color)) = options.cell_highlights.iter().find(|(k, _)| *k == lower) {
        for seg in segments.iter_mut() {
            seg.style.fg = Some(*color);
        }
    }
}

/// Check if a row is overdue and apply the overdue color to all cells.
/// A row is overdue when it has a matching status AND a date before today.
fn apply_overdue_highlight(row: &mut [Vec<StyledSegment>], options: &RenderOptions) {
    let highlight = match &options.overdue_highlight {
        Some(h) => h,
        None => return,
    };

    let mut has_status = false;
    let mut has_past_date = false;

    for cell in row.iter() {
        let text: String = cell.iter().map(|s| s.text.as_str()).collect();
        let trimmed = text.trim().to_lowercase();

        if highlight.statuses.contains(&trimmed) {
            has_status = true;
        }

        if is_date(&trimmed) && trimmed < highlight.today {
            has_past_date = true;
        }
    }

    if has_status && has_past_date {
        for cell in row.iter_mut() {
            for seg in cell.iter_mut() {
                seg.style.fg = Some(highlight.color);
            }
        }
    }
}

/// Check if a string looks like YYYY-MM-DD.
fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(|c| c.is_ascii_digit())
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[8..10].iter().all(|c| c.is_ascii_digit())
}

/// Shrink column widths to fit within `available` total content width.
///
/// Iteratively locks columns whose natural width fits within a fair share,
/// then distributes remaining space proportionally among wide columns.
/// Minimum width per column is 3.
fn shrink_columns(widths: &mut [usize], available: usize) {
    let total: usize = widths.iter().sum();
    if total <= available || widths.is_empty() {
        return;
    }

    let n = widths.len();
    let mut locked = vec![false; n];

    loop {
        let unlocked_count = locked.iter().filter(|l| !**l).count();
        if unlocked_count == 0 {
            break;
        }

        let locked_sum: usize = widths
            .iter()
            .zip(locked.iter())
            .filter(|(_, l)| **l)
            .map(|(w, _)| *w)
            .sum();
        let remaining = available.saturating_sub(locked_sum);
        let fair_share = remaining / unlocked_count;

        let mut changed = false;
        for i in 0..n {
            if !locked[i] && widths[i] <= fair_share {
                locked[i] = true;
                changed = true;
            }
        }

        if !changed {
            // Distribute remaining space proportionally among unlocked columns
            let unlocked_total: usize = widths
                .iter()
                .zip(locked.iter())
                .filter(|(_, l)| !**l)
                .map(|(w, _)| *w)
                .sum();

            let mut distributed = 0usize;
            let unlocked_indices: Vec<usize> = (0..n).filter(|i| !locked[*i]).collect();

            for (idx, &i) in unlocked_indices.iter().enumerate() {
                if idx == unlocked_indices.len() - 1 {
                    // Last unlocked column gets whatever is left (avoids rounding drift)
                    widths[i] = remaining.saturating_sub(distributed).max(3);
                } else {
                    let proportion = widths[i] as f64 / unlocked_total as f64;
                    let new_w = (remaining as f64 * proportion).floor() as usize;
                    widths[i] = new_w.max(3);
                    distributed += widths[i];
                }
            }
            break;
        }
    }
}

/// Word-wrap a cell's styled segments into multiple physical lines,
/// each fitting within `max_width` display characters.
fn wrap_cell_segments(segments: &[StyledSegment], max_width: usize) -> Vec<Vec<StyledSegment>> {
    use unicode_width::UnicodeWidthStr;

    if max_width == 0 {
        return vec![segments.to_vec()];
    }

    let mut result_lines: Vec<Vec<StyledSegment>> = Vec::new();
    let mut current_line: Vec<StyledSegment> = Vec::new();
    let mut current_width: usize = 0;

    for seg in segments {
        let style = seg.style.clone();
        let mut remaining = seg.text.as_str();

        while !remaining.is_empty() {
            let remaining_space = max_width.saturating_sub(current_width);

            if remaining_space == 0 {
                result_lines.push(std::mem::take(&mut current_line));
                current_width = 0;
                continue;
            }

            let rem_width = remaining.width();
            if rem_width <= remaining_space {
                // Whole remaining text fits on this line
                current_line.push(StyledSegment {
                    text: remaining.to_string(),
                    style: style.clone(),
                });
                current_width += rem_width;
                break;
            }

            // Need to wrap — find a break point
            let (chunk, rest) = find_wrap_point(remaining, remaining_space);
            if chunk.is_empty() {
                if current_width == 0 {
                    // Force at least one char to avoid infinite loop
                    let mut char_end = 0;
                    for (i, ch) in remaining.char_indices() {
                        let next = i + ch.len_utf8();
                        if next > remaining.len() {
                            break;
                        }
                        let w = remaining[..next].width();
                        if w > max_width && char_end > 0 {
                            break;
                        }
                        char_end = next;
                        if w >= max_width {
                            break;
                        }
                    }
                    if char_end == 0 {
                        char_end = remaining
                            .char_indices()
                            .nth(1)
                            .map(|(i, _)| i)
                            .unwrap_or(remaining.len());
                    }
                    current_line.push(StyledSegment {
                        text: remaining[..char_end].to_string(),
                        style: style.clone(),
                    });
                    remaining = &remaining[char_end..];
                    result_lines.push(std::mem::take(&mut current_line));
                    current_width = 0;
                } else {
                    // Start a new line and retry
                    result_lines.push(std::mem::take(&mut current_line));
                    current_width = 0;
                }
            } else {
                current_line.push(StyledSegment {
                    text: chunk.to_string(),
                    style: style.clone(),
                });
                current_width += chunk.width();
                remaining = rest;
                if !remaining.is_empty() {
                    result_lines.push(std::mem::take(&mut current_line));
                    current_width = 0;
                    // Skip leading whitespace on the new line
                    remaining = remaining.trim_start();
                }
            }
        }
    }

    if !current_line.is_empty() {
        result_lines.push(current_line);
    }

    if result_lines.is_empty() {
        result_lines.push(Vec::new());
    }

    result_lines
}

/// Find a word-boundary break point that fits within `max_width` display chars.
/// Returns `(chunk_that_fits, remainder)`.
fn find_wrap_point(text: &str, max_width: usize) -> (&str, &str) {
    // Find the last whitespace boundary that fits
    let mut last_space = 0;
    let mut current_width = 0;

    for (i, ch) in text.char_indices() {
        let char_w = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width + char_w > max_width {
            break;
        }
        current_width += char_w;
        if ch.is_whitespace() {
            // Include the space in the chunk, break after it
            last_space = i + ch.len_utf8();
        }
    }

    if last_space > 0 {
        // Trim trailing whitespace from chunk
        let chunk = text[..last_space].trim_end();
        (&text[..chunk.len()], &text[last_space..])
    } else {
        // No whitespace found — can't break at word boundary
        ("", text)
    }
}

/// Shared box-drawing renderer used by both AST-based and data-based table functions.
///
/// `rows` — first row is treated as header (rendered bold)
/// `alignments` — per-column alignment (padded with Left if shorter than num_cols)
fn render_table_inner(
    rows: &[Vec<Vec<StyledSegment>>],
    alignments: &[Alignment],
    options: &RenderOptions,
) -> Vec<StyledLine> {
    if rows.is_empty() {
        return vec![];
    }

    // Calculate column widths
    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut col_widths = vec![0usize; num_cols];

    for row in rows {
        for (j, cell) in row.iter().enumerate() {
            use unicode_width::UnicodeWidthStr;
            let cell_width: usize = cell.iter().map(|s| s.text.as_str().width()).sum();
            if j < col_widths.len() {
                col_widths[j] = col_widths[j].max(cell_width);
            }
        }
    }

    // Ensure minimum column width
    for w in &mut col_widths {
        *w = (*w).max(3);
    }

    // Constrain column widths to fit terminal when width is set
    if options.width > 0 && num_cols > 0 {
        let border_overhead = 1 + 3 * num_cols; // │ + ( space + space + │ ) per col
        let available = options.width.saturating_sub(border_overhead);
        let total: usize = col_widths.iter().sum();
        if total > available {
            shrink_columns(&mut col_widths, available);
        }
    }

    let border_style = SegmentStyle {
        fg: Some(options.theme.table_border_fg),
        ..Default::default()
    };

    let mut lines = Vec::new();

    // Top border: ┌───┬───┐
    let mut top = StyledLine::new();
    top.push("┌", border_style.clone());
    for (i, w) in col_widths.iter().enumerate() {
        top.push("─".repeat(w + 2), border_style.clone());
        if i < num_cols - 1 {
            top.push("┬", border_style.clone());
        }
    }
    top.push("┐", border_style.clone());
    lines.push(top);

    for (row_idx, row) in rows.iter().enumerate() {
        // Wrap each cell's content and compute row height
        let wrapped_cells: Vec<Vec<Vec<StyledSegment>>> = (0..num_cols)
            .map(|j| {
                let col_w = col_widths.get(j).copied().unwrap_or(3);
                if let Some(cell) = row.get(j) {
                    wrap_cell_segments(cell, col_w)
                } else {
                    vec![Vec::new()]
                }
            })
            .collect();

        let row_height = wrapped_cells.iter().map(|c| c.len()).max().unwrap_or(1);

        for line_idx in 0..row_height {
            let mut data_line = StyledLine::new();
            data_line.push("│", border_style.clone());

            for j in 0..num_cols {
                use unicode_width::UnicodeWidthStr;
                let col_w = col_widths.get(j).copied().unwrap_or(3);
                let cell_line = wrapped_cells.get(j).and_then(|lines| lines.get(line_idx));

                let cell_width: usize = cell_line
                    .map(|segs| segs.iter().map(|s| s.text.as_str().width()).sum())
                    .unwrap_or(0);
                let padding = col_w.saturating_sub(cell_width);

                data_line.push(" ", SegmentStyle::default());

                // Apply alignment
                let align = alignments.get(j).copied().unwrap_or(Alignment::Left);
                let (left_pad, right_pad) = match align {
                    Alignment::Center => (padding / 2, padding - padding / 2),
                    Alignment::Right => (padding, 0),
                    Alignment::Left => (0, padding),
                };

                if left_pad > 0 {
                    data_line.push_plain(" ".repeat(left_pad));
                }

                // Bold header row (first row)
                if let Some(segs) = cell_line {
                    for seg in segs {
                        if row_idx == 0 {
                            let mut header_style = seg.style.clone();
                            header_style.bold = true;
                            data_line.push(seg.text.clone(), header_style);
                        } else {
                            data_line.push_styled(seg.clone());
                        }
                    }
                }

                if right_pad > 0 {
                    data_line.push_plain(" ".repeat(right_pad));
                }

                data_line.push(" ", SegmentStyle::default());
                data_line.push("│", border_style.clone());
            }
            lines.push(data_line);
        }

        // Separator after header (first row)
        if row_idx == 0 {
            let mut sep = StyledLine::new();
            sep.push("├", border_style.clone());
            for (i, w) in col_widths.iter().enumerate() {
                sep.push("─".repeat(w + 2), border_style.clone());
                if i < num_cols - 1 {
                    sep.push("┼", border_style.clone());
                }
            }
            sep.push("┤", border_style.clone());
            lines.push(sep);
        }
    }

    // Bottom border: └───┴───┘
    let mut bottom = StyledLine::new();
    bottom.push("└", border_style.clone());
    for (i, w) in col_widths.iter().enumerate() {
        bottom.push("─".repeat(w + 2), border_style.clone());
        if i < num_cols - 1 {
            bottom.push("┴", border_style.clone());
        }
    }
    bottom.push("┘", border_style.clone());
    lines.push(bottom);

    lines
}

/// Render a GFM table with box-drawing borders (from comrak AST).
/// When `numbered` is true, a "#" column with row numbers is prepended.
pub fn render_table<'a>(
    node: &'a AstNode<'a>,
    options: &RenderOptions,
    numbered: bool,
) -> RenderedBlock {
    // Collect all rows and cells
    let mut rows: Vec<Vec<Vec<StyledSegment>>> = Vec::new();
    let mut comrak_alignments: Vec<TableAlignment> = Vec::new();

    // Get column alignments from Table node
    {
        let ast = node.data.borrow();
        if let NodeValue::Table(ref table_data) = ast.value {
            comrak_alignments = table_data.alignments.clone();
        }
    }

    for row_node in node.children() {
        let ast = row_node.data.borrow();
        if let NodeValue::TableRow(_) = ast.value {
            drop(ast);
            let mut row = Vec::new();
            for cell_node in row_node.children() {
                let cell_ast = cell_node.data.borrow();
                if let NodeValue::TableCell = cell_ast.value {
                    drop(cell_ast);
                    let mut line = StyledLine::new();
                    walk_inlines(cell_node, &mut line, &SegmentStyle::default(), options);
                    apply_cell_highlights(&mut line.segments, options);
                    row.push(line.segments);
                } else {
                    drop(cell_ast);
                }
            }
            rows.push(row);
        } else {
            drop(ast);
        }
    }

    // Apply overdue highlighting to data rows (skip header at index 0)
    for row in rows.iter_mut().skip(1) {
        apply_overdue_highlight(row, options);
    }

    // Prepend "#" column with row numbers
    if numbered {
        prepend_number_column(&mut rows);
        comrak_alignments.insert(0, TableAlignment::Right);
    }

    let alignments: Vec<Alignment> = comrak_alignments.into_iter().map(Alignment::from).collect();
    let lines = render_table_inner(&rows, &alignments, options);
    RenderedBlock::Lines(lines)
}

/// Prepend a "#" header + row numbers (1, 2, 3...) as the first column.
fn prepend_number_column(rows: &mut [Vec<Vec<StyledSegment>>]) {
    let dim_style = SegmentStyle {
        fg: Some(ratatui::style::Color::DarkGray),
        ..Default::default()
    };
    for (i, row) in rows.iter_mut().enumerate() {
        let cell = if i == 0 {
            vec![StyledSegment {
                text: "#".to_string(),
                style: SegmentStyle::default(),
            }]
        } else {
            vec![StyledSegment {
                text: i.to_string(),
                style: dim_style.clone(),
            }]
        };
        row.insert(0, cell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SegmentStyle, StyledSegment};

    fn plain_cell(text: &str) -> Vec<StyledSegment> {
        vec![StyledSegment {
            text: text.to_string(),
            style: SegmentStyle::default(),
        }]
    }

    #[test]
    fn test_prepend_number_column() {
        let mut rows = vec![
            vec![plain_cell("Name"), plain_cell("Score")],
            vec![plain_cell("Alice"), plain_cell("8")],
            vec![plain_cell("Bob"), plain_cell("6")],
        ];
        prepend_number_column(&mut rows);

        assert_eq!(rows[0][0][0].text, "#");
        assert_eq!(rows[1][0][0].text, "1");
        assert_eq!(rows[2][0][0].text, "2");
        assert_eq!(rows[0].len(), 3);
        assert_eq!(rows[1].len(), 3);
    }

    #[test]
    fn test_prepend_number_column_empty() {
        let mut rows: Vec<Vec<Vec<StyledSegment>>> = vec![];
        prepend_number_column(&mut rows);
        assert!(rows.is_empty());
    }
}

/// Render a table from raw string data (not from markdown AST).
///
/// `headers` — column header labels
/// `rows` — each row is a Vec of cell strings
///
/// Returns `RenderedBlock::Lines` with box-drawn styled table.
pub fn render_table_from_data(
    headers: &[&str],
    rows: &[Vec<String>],
    options: &RenderOptions,
) -> RenderedBlock {
    // Convert headers into first row of styled segments
    let header_row: Vec<Vec<StyledSegment>> = headers
        .iter()
        .map(|h| {
            vec![StyledSegment {
                text: h.to_string(),
                style: SegmentStyle::default(),
            }]
        })
        .collect();

    // Convert data rows
    let mut styled_rows: Vec<Vec<Vec<StyledSegment>>> = Vec::with_capacity(rows.len() + 1);
    styled_rows.push(header_row);

    for row in rows {
        let styled_row: Vec<Vec<StyledSegment>> = row
            .iter()
            .map(|cell| {
                let mut segs = vec![StyledSegment {
                    text: cell.clone(),
                    style: SegmentStyle::default(),
                }];
                apply_cell_highlights(&mut segs, options);
                segs
            })
            .collect();
        styled_rows.push(styled_row);
    }

    // Apply overdue highlighting to data rows (skip header at index 0)
    for row in styled_rows.iter_mut().skip(1) {
        apply_overdue_highlight(row, options);
    }

    // All columns left-aligned for data tables
    let alignments = vec![Alignment::Left; headers.len()];
    let lines = render_table_inner(&styled_rows, &alignments, options);
    RenderedBlock::Lines(lines)
}

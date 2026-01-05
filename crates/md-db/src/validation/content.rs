use regex::Regex;

use super::{Diagnostic, Severity};

/// Detect markdown table syntax that comrak failed to parse as a `<table>`.
///
/// Looks for GFM pipe-table patterns (header + separator + data rows) in the body
/// and verifies they actually produce `<table>` in the rendered HTML. If they don't,
/// the table syntax is likely malformed (e.g. missing blank line before table).
pub(crate) fn check_broken_tables(body: &str, diags: &mut Vec<Diagnostic>) {
    // Quick reject: no pipe characters → no tables
    if !body.contains('|') {
        return;
    }

    // Find table-like blocks: consecutive lines starting/containing pipes with a separator row
    let lines: Vec<&str> = body.lines().collect();
    let mut i = 0;
    while i + 1 < lines.len() {
        let line = lines[i].trim();
        let next = lines[i + 1].trim();

        // A table needs: header row (|...|), separator row (|---|...|)
        if line.starts_with('|')
            && line.ends_with('|')
            && next.starts_with('|')
            && next.contains("---")
        {
            // Count data rows after separator
            let mut end = i + 2;
            while end < lines.len() {
                let row = lines[end].trim();
                if row.starts_with('|') && row.ends_with('|') {
                    end += 1;
                } else {
                    break;
                }
            }

            // We found table-like syntax from lines i..end.
            // Check if comrak renders it as a real table.
            let table_block: String = lines[i..end].join("\n");
            // Wrap with blank lines to give comrak the best chance
            let test_md = format!("\n\n{table_block}\n\n");
            let html = crate::export::render_markdown_to_html(&test_md);

            if !html.contains("<table") {
                let preview = if table_block.len() > 80 {
                    format!("{}...", &table_block[..80])
                } else {
                    table_block.clone()
                };
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "C001".into(),
                    message: "broken markdown table (not parsed as HTML table)".to_string(),
                    location: format!("body line {}", i + 1),
                    hint: Some(format!(
                        "ensure a blank line before the table. Preview: {preview}"
                    )),
                });
            }

            i = end;
            continue;
        }
        i += 1;
    }
}

/// Check that local image references use docs/assets/ as their path.
///
/// Matches `![alt](path)` and `<img src="path"` patterns, skipping
/// external URLs (http://, https://) and absolute paths.
pub(crate) fn check_image_paths(body: &str, diags: &mut Vec<Diagnostic>) {
    // Match markdown images: ![...](path)
    let md_re = Regex::new(r"!\[[^\]]*\]\(([^)]+)\)").unwrap();
    let html_re = Regex::new(r#"<img\s[^>]*src=["']([^"']+)["']"#).unwrap();
    for (line_num, line) in body.lines().enumerate() {
        for cap in md_re.captures_iter(line) {
            let path = cap[1].split_whitespace().next().unwrap_or("");
            check_single_image_path(path, line_num + 1, diags);
        }
        // Match HTML images: <img src="path" or <img src='path'
        if line.contains("<img") {
            for cap in html_re.captures_iter(line) {
                check_single_image_path(&cap[1], line_num + 1, diags);
            }
        }
    }
}

fn check_single_image_path(path: &str, line_num: usize, diags: &mut Vec<Diagnostic>) {
    // Skip external URLs
    if path.starts_with("http://") || path.starts_with("https://") {
        return;
    }
    // Normalize: strip leading ./ and resolve ../
    let normalized = path.trim_start_matches("./");
    // Valid paths: docs/assets/... or ../assets/... (relative from docs subdir)
    let is_valid = normalized.starts_with("docs/assets/")
        || normalized.starts_with("../assets/")
        || normalized.starts_with("assets/");
    if !is_valid {
        diags.push(Diagnostic {
            severity: Severity::Error,
            code: "C002".into(),
            message: format!("image path \"{path}\" is not in docs/assets/"),
            location: format!("body line {line_num}"),
            hint: Some("move the image to docs/assets/ and update the reference".into()),
        });
    }
}

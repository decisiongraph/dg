use std::collections::BTreeMap;
use std::fmt;

/// Severity of a validation diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
        }
    }
}

/// A single validation diagnostic.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub location: String,
    pub hint: Option<String>,
}

impl Diagnostic {
    /// One-liner format: `code:severity:location:message`
    pub fn to_compact(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.code, self.severity, self.location, self.message
        )
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "  {}[{}]: {}", self.severity, self.code, self.message)?;
        write!(f, "\n    --> {}", self.location)?;
        if let Some(ref hint) = self.hint {
            write!(f, "\n    = hint: {hint}")?;
        }
        Ok(())
    }
}

/// Result of validating one or more documents.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub file_results: Vec<FileResult>,
}

#[derive(Debug, Clone)]
pub struct FileResult {
    pub path: String,
    pub diagnostics: Vec<Diagnostic>,
}

impl FileResult {
    pub fn errors(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    pub fn warnings(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count()
    }
}

impl ValidationResult {
    pub fn total_errors(&self) -> usize {
        self.file_results.iter().map(|f| f.errors()).sum()
    }

    pub fn total_warnings(&self) -> usize {
        self.file_results.iter().map(|f| f.warnings()).sum()
    }

    pub fn is_ok(&self) -> bool {
        self.total_errors() == 0
    }

    /// Merge file_results with the same path into single entries, then group
    /// repeated per-row diagnostics (e.g. multiple U012 warnings for different
    /// rows in the same table column) into a single diagnostic.
    fn merged(&self) -> Vec<FileResult> {
        let mut map: BTreeMap<&str, Vec<&Diagnostic>> = BTreeMap::new();
        for fr in &self.file_results {
            for d in &fr.diagnostics {
                map.entry(&fr.path).or_default().push(d);
            }
        }
        map.into_iter()
            .map(|(path, diags)| FileResult {
                path: path.to_string(),
                diagnostics: group_row_diagnostics(diags.into_iter().cloned().collect()),
            })
            .collect()
    }

    /// Compact format: one line per diagnostic `path:code:severity:location:message`
    pub fn to_compact_report(&self) -> String {
        let mut out = String::new();
        for fr in &self.merged() {
            for d in &fr.diagnostics {
                out.push_str(&fr.path);
                out.push(':');
                out.push_str(&d.to_compact());
                out.push('\n');
            }
        }
        out
    }

    /// Format as human-readable report.
    pub fn to_report(&self) -> String {
        let mut out = String::new();
        let merged = self.merged();

        for fr in &merged {
            if fr.diagnostics.is_empty() {
                continue;
            }
            out.push_str(&fr.path);
            out.push_str(":\n");
            for d in &fr.diagnostics {
                out.push_str(&format!("{d}\n"));
            }
            out.push('\n');
        }

        let errors: usize = merged.iter().map(|f| f.errors()).sum();
        let warnings: usize = merged.iter().map(|f| f.warnings()).sum();
        out.push_str(&format!(
            "result: {errors} error(s), {warnings} warning(s)\n"
        ));
        out
    }
}

/// Group diagnostics whose locations differ only by a `.rowN` suffix.
///
/// For example, multiple U012 warnings for different rows in the same table
/// column get merged into a single diagnostic listing all affected rows,
/// grouped by the extracted value (e.g. the departed user name).
fn group_row_diagnostics(diags: Vec<Diagnostic>) -> Vec<Diagnostic> {
    // Key: (code, location_prefix) where prefix is location without `.rowN`
    // Value: vec of (row_index, original_diagnostic)
    let mut groups: BTreeMap<(String, String), Vec<(usize, Diagnostic)>> = BTreeMap::new();
    let mut result: Vec<Diagnostic> = Vec::new();

    for d in diags {
        if let Some((prefix, row_idx)) = split_row_suffix(&d.location) {
            groups
                .entry((d.code.clone(), prefix.to_string()))
                .or_default()
                .push((row_idx, d));
        } else {
            result.push(d);
        }
    }

    for ((_code, loc_prefix), mut entries) in groups {
        if entries.len() == 1 {
            // Single entry, keep as-is
            result.push(entries.remove(0).1);
            continue;
        }

        entries.sort_by_key(|(idx, _)| *idx);

        // Group rows by the distinguishing value extracted from the message.
        // e.g. for U012: extract the departed user name from the message.
        // We group by everything in the message after a common prefix pattern.
        let mut by_value: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (row_idx, d) in &entries {
            let value = extract_message_value(&d.message);
            by_value.entry(value).or_default().push(*row_idx);
        }

        // Build grouped message
        let first = &entries[0].1;
        // Extract the field base (location prefix without "frontmatter.")
        let field_base = loc_prefix
            .strip_prefix("frontmatter.")
            .unwrap_or(&loc_prefix);

        let value_parts: Vec<String> = by_value
            .iter()
            .map(|(value, rows)| {
                let row_list = format_row_list(rows);
                format!("{value} ({row_list})")
            })
            .collect();

        let message = format!("{field_base} references {}", value_parts.join(", "));

        result.push(Diagnostic {
            severity: first.severity,
            code: first.code.clone(),
            message,
            location: format!("frontmatter.{field_base}"),
            hint: first.hint.clone(),
        });
    }

    result
}

/// Split a location like `frontmatter.table:Foo.Bar.row3` into
/// `("frontmatter.table:Foo.Bar", 3)`.
fn split_row_suffix(location: &str) -> Option<(&str, usize)> {
    let dot_row = location.rfind(".row")?;
    let row_str = &location[dot_row + 4..];
    let row_idx: usize = row_str.parse().ok()?;
    Some((&location[..dot_row], row_idx))
}

/// Extract the distinguishing value from a diagnostic message.
/// e.g. `field "table:Requirements.Owner.row0" references departed user "@jiikonen"`
///   → `departed user "@jiikonen"`
fn extract_message_value(message: &str) -> String {
    // Look for "references X" pattern
    if let Some(idx) = message.find("references ") {
        return message[idx + "references ".len()..].to_string();
    }
    // Fallback: use the whole message
    message.to_string()
}

/// Format row indices as a compact list: `rows 0,1,2,4`
fn format_row_list(rows: &[usize]) -> String {
    if rows.len() == 1 {
        format!("row {}", rows[0])
    } else {
        let nums: Vec<String> = rows.iter().map(|r| r.to_string()).collect();
        format!("rows {}", nums.join(","))
    }
}

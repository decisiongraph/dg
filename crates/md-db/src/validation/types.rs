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

    /// Merge file_results with the same path into single entries.
    fn merged(&self) -> Vec<FileResult> {
        let mut map: std::collections::BTreeMap<&str, Vec<&Diagnostic>> =
            std::collections::BTreeMap::new();
        for fr in &self.file_results {
            for d in &fr.diagnostics {
                map.entry(&fr.path).or_default().push(d);
            }
        }
        map.into_iter()
            .map(|(path, diags)| FileResult {
                path: path.to_string(),
                diagnostics: diags.into_iter().cloned().collect(),
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

        for fr in &self.merged() {
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

        let errors = self.total_errors();
        let warnings = self.total_warnings();
        out.push_str(&format!(
            "result: {errors} error(s), {warnings} warning(s)\n"
        ));
        out
    }
}

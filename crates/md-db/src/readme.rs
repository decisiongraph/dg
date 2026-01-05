use regex::Regex;
use std::sync::LazyLock;

use crate::error::Result;

/// Find marker section in content.
/// Returns (start_offset, end_offset) if markers are found.
pub fn find_markers(content: &str, name: &str) -> Option<(usize, usize)> {
    static MARKER_RE_CACHE: LazyLock<std::sync::Mutex<std::collections::HashMap<String, Regex>>> =
        LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

    let pattern_key = format!("{}_{}", name, "marker");
    let regex_pattern = format!(
        r"(?s)({}).*?({})[\r\n]*",
        regex::escape(&format!("<!-- dg:{}:start -->", name)),
        regex::escape(&format!("<!-- dg:{}:end -->", name))
    );

    let re = {
        let mut cache = MARKER_RE_CACHE.lock().unwrap();
        cache
            .entry(pattern_key)
            .or_insert_with(|| Regex::new(&regex_pattern).unwrap())
            .clone()
    };

    if let Some(captures) = re.captures(content) {
        if let Some(full_match) = captures.get(0) {
            return Some((full_match.start(), full_match.end()));
        }
    }

    None
}

/// Replace content between markers.
/// Preserves the markers themselves.
pub fn replace_marker_section(content: &str, name: &str, new_content: &str) -> Result<String> {
    if let Some((start, end)) = find_markers(content, name) {
        let mut result = String::new();
        result.push_str(&content[..start]);
        result.push_str(&format!("<!-- dg:{}:start -->\n", name));
        result.push_str(new_content);
        if !new_content.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(&format!("<!-- dg:{}:end -->\n", name));
        result.push_str(&content[end..]);
        Ok(result)
    } else {
        Err(crate::error::Error::SectionNotFound(format!(
            "Markers <!-- dg:{}:start --> and <!-- dg:{}:end --> not found",
            name, name
        )))
    }
}

/// Suggest insertion location for service catalog markers in README.
/// Returns None if no suitable location found.
pub fn suggest_marker_location(content: &str) -> Option<String> {
    // Look for Architecture section (case-insensitive)
    let lines: Vec<&str> = content.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        if line.to_lowercase().contains("## architecture")
            || line.to_lowercase().contains("# architecture")
        {
            // Suggest inserting after the Architecture section
            let suggestion = if idx + 1 < lines.len() {
                format!("After line {}: {}", idx + 1, line)
            } else {
                "At the end of the file".to_string()
            };
            return Some(suggestion);
        }
    }

    // Fallback: suggest after first heading
    for (idx, line) in lines.iter().enumerate() {
        if line.starts_with('#') {
            return Some(format!("After line {}: {}", idx + 1, line));
        }
    }

    Some("At the beginning of the file".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_markers() {
        let content = r#"
# README

<!-- dg:services:start -->
old content
<!-- dg:services:end -->

More text
"#;

        let result = find_markers(content, "services");
        assert!(result.is_some());
    }

    #[test]
    fn test_replace_marker_section() {
        let content = r#"# README

<!-- dg:services:start -->
old content
<!-- dg:services:end -->

More text"#;

        let new_content = "new table content";
        let result = replace_marker_section(content, "services", new_content).unwrap();

        assert!(result.contains("new table content"));
        assert!(result.contains("<!-- dg:services:start -->"));
        assert!(result.contains("<!-- dg:services:end -->"));
        assert!(!result.contains("old content"));
    }

    #[test]
    fn test_suggest_marker_location() {
        let content = r#"# README

Some intro text.

## Architecture

Architecture content here.

## More sections
"#;

        let suggestion = suggest_marker_location(content);
        assert!(suggestion.is_some());
        assert!(suggestion.unwrap().contains("Architecture"));
    }
}

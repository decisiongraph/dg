/// Parse raw gherkin strings into Feature ASTs.
use gherkin::GherkinEnv;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GherkinError {
    #[error("failed to parse Gherkin in {filename}: {message}")]
    ParseError {
        filename: String,
        message: String,
        /// Byte offset within the block where the error occurred.
        offset: usize,
        /// Which block (0-indexed) failed.
        block_index: usize,
    },
}

/// Parse gherkin content strings into Feature ASTs.
/// Each string is a raw gherkin block (e.g. extracted via `collect_code_blocks`).
/// Returns Ok with parsed features, or Err with the first error encountered.
pub fn parse_gherkin_blocks(
    blocks: &[String],
    filename: &str,
) -> Result<Vec<gherkin::Feature>, GherkinError> {
    let mut features = Vec::new();

    for (i, content) in blocks.iter().enumerate() {
        let env = GherkinEnv::default();
        match gherkin::Feature::parse(content, env) {
            Ok(feature) => features.push(feature),
            Err(e) => {
                let err_msg = format!("{e}");
                let offset = extract_error_offset(content, &err_msg);
                return Err(GherkinError::ParseError {
                    filename: filename.to_string(),
                    message: err_msg,
                    offset,
                    block_index: i,
                });
            }
        }
    }

    Ok(features)
}

/// Try to figure out a byte offset for the error within the block content.
fn extract_error_offset(content: &str, error_msg: &str) -> usize {
    if let Some(line_num) = extract_line_from_error(error_msg) {
        let mut offset = 0;
        for (i, line) in content.lines().enumerate() {
            if i + 1 == line_num {
                return offset;
            }
            offset += line.len() + 1; // +1 for newline
        }
    }
    0 // fallback to start of block
}

fn extract_line_from_error(msg: &str) -> Option<usize> {
    let lower = msg.to_lowercase();
    if let Some(idx) = lower.find("line ") {
        let rest = &lower[idx + 5..];
        let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        num_str.parse().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_feature() {
        let blocks = vec!["Feature: Test\n  Scenario: A\n    Given step".to_string()];
        let result = parse_gherkin_blocks(&blocks, "test.md");
        assert!(result.is_ok());
        let features = result.unwrap();
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].name, "Test");
    }

    #[test]
    fn parse_invalid_feature() {
        let blocks = vec!["Not valid gherkin at all\nrandom text".to_string()];
        let result = parse_gherkin_blocks(&blocks, "test.md");
        assert!(result.is_err());
    }

    #[test]
    fn parse_multiple_blocks() {
        let blocks = vec![
            "Feature: First\n  Scenario: A\n    Given step".to_string(),
            "Feature: Second\n  Scenario: B\n    Given other".to_string(),
        ];
        let features = parse_gherkin_blocks(&blocks, "test.md").unwrap();
        assert_eq!(features.len(), 2);
        assert_eq!(features[0].name, "First");
        assert_eq!(features[1].name, "Second");
    }

    #[test]
    fn error_includes_block_index() {
        let blocks = vec![
            "Feature: Good\n  Scenario: A\n    Given step".to_string(),
            "Not valid gherkin".to_string(),
        ];
        let err = parse_gherkin_blocks(&blocks, "test.md").unwrap_err();
        match err {
            GherkinError::ParseError { block_index, .. } => assert_eq!(block_index, 1),
        }
    }
}

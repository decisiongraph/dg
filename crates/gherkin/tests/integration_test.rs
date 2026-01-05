use dg_gherkin::diagram::d2::D2Generator;
use dg_gherkin::diagram::mermaid::MermaidGenerator;
use dg_gherkin::diagram::{DiagramGenerator, DiagramStyle};
use dg_gherkin::validate::Severity;
use dg_gherkin::{generate_diagram, process_blocks, DiagramFormat};

/// Read a fixture file, extract gherkin fenced blocks manually (for test purposes).
fn fixture_gherkin_blocks(name: &str) -> Vec<String> {
    let content = std::fs::read_to_string(format!("tests/fixtures/{name}")).unwrap();
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut current = String::new();

    for line in content.lines() {
        if !in_block
            && (line.trim().starts_with("```gherkin") || line.trim().starts_with("```feature"))
        {
            in_block = true;
            current.clear();
        } else if in_block && line.trim() == "```" {
            in_block = false;
            blocks.push(current.clone());
        } else if in_block {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
    }
    blocks
}

fn parse_features(name: &str) -> Vec<gherkin::Feature> {
    let blocks = fixture_gherkin_blocks(name);
    assert!(!blocks.is_empty(), "No gherkin blocks found in {name}");
    dg_gherkin::parse::parse_gherkin_blocks(&blocks, name).unwrap()
}

// --- Mermaid flow snapshots ---

#[test]
fn simple_mermaid_flow() {
    let features = parse_features("valid_simple.md");
    let output = MermaidGenerator.generate(&features, DiagramStyle::Flow);
    insta::assert_snapshot!(output);
}

#[test]
fn simple_mermaid_overview() {
    let features = parse_features("valid_simple.md");
    let output = MermaidGenerator.generate(&features, DiagramStyle::Overview);
    insta::assert_snapshot!(output);
}

#[test]
fn background_mermaid_flow() {
    let features = parse_features("valid_background.md");
    let output = MermaidGenerator.generate(&features, DiagramStyle::Flow);
    insta::assert_snapshot!(output);
}

#[test]
fn outline_mermaid_flow() {
    let features = parse_features("valid_outline.md");
    let output = MermaidGenerator.generate(&features, DiagramStyle::Flow);
    insta::assert_snapshot!(output);
}

#[test]
fn tags_mermaid_flow() {
    let features = parse_features("valid_tags.md");
    let output = MermaidGenerator.generate(&features, DiagramStyle::Flow);
    insta::assert_snapshot!(output);
}

#[test]
fn rules_mermaid_flow() {
    let features = parse_features("valid_rules.md");
    let output = MermaidGenerator.generate(&features, DiagramStyle::Flow);
    insta::assert_snapshot!(output);
}

#[test]
fn large_auto_selects_overview() {
    let features = parse_features("valid_large.md");
    let output = MermaidGenerator.generate(&features, DiagramStyle::Auto);
    assert!(
        output.starts_with("flowchart LR"),
        "Expected overview (LR) for large feature"
    );
    insta::assert_snapshot!(output);
}

// --- D2 snapshots ---

#[test]
fn simple_d2_flow() {
    let features = parse_features("valid_simple.md");
    let output = D2Generator.generate(&features, DiagramStyle::Flow);
    insta::assert_snapshot!(output);
}

#[test]
fn simple_d2_overview() {
    let features = parse_features("valid_simple.md");
    let output = D2Generator.generate(&features, DiagramStyle::Overview);
    insta::assert_snapshot!(output);
}

// --- Multi-block ---

#[test]
fn multiple_blocks_parsed() {
    let features = parse_features("multiple_blocks.md");
    assert_eq!(features.len(), 2);
    assert_eq!(features[0].name, "Login");
    assert_eq!(features[1].name, "Logout");
}

// --- Error handling ---

#[test]
fn invalid_syntax_fails() {
    let blocks = fixture_gherkin_blocks("invalid_syntax.md");
    assert!(!blocks.is_empty());
    let result = dg_gherkin::parse::parse_gherkin_blocks(&blocks, "invalid_syntax.md");
    assert!(result.is_err(), "Expected parse error for invalid syntax");
}

// --- Public API tests ---

#[test]
fn process_blocks_api() {
    let blocks = fixture_gherkin_blocks("valid_simple.md");
    let result = process_blocks(&blocks, "valid_simple.md").unwrap();
    assert_eq!(result.features.len(), 1);
    assert_eq!(result.features[0].name, "User Login");
    assert!(!result.validation.has_warnings());
}

#[test]
fn process_blocks_empty() {
    let result = process_blocks(&[], "empty.md").unwrap();
    assert!(result.features.is_empty());
}

#[test]
fn process_blocks_invalid() {
    let blocks = fixture_gherkin_blocks("invalid_syntax.md");
    let result = process_blocks(&blocks, "invalid.md");
    assert!(result.is_err());
}

#[test]
fn generate_diagram_api() {
    let blocks = fixture_gherkin_blocks("valid_simple.md");
    let result = process_blocks(&blocks, "test.md").unwrap();
    let mermaid = generate_diagram(&result.features, DiagramFormat::Mermaid, DiagramStyle::Auto);
    assert!(mermaid.starts_with("flowchart"));
    let d2 = generate_diagram(&result.features, DiagramFormat::D2, DiagramStyle::Flow);
    assert!(d2.contains("shape: oval"));
}

// --- Validation tests ---

#[test]
fn validation_warns_no_then() {
    let blocks = vec![
        "Feature: Missing Then\n  Scenario: No outcome\n    Given something\n    When action"
            .to_string(),
    ];
    let result = process_blocks(&blocks, "test.md").unwrap();
    assert!(result.validation.has_warnings());
    assert!(result
        .validation
        .warnings
        .iter()
        .any(|w| w.message.contains("no Then")));
}

#[test]
fn validation_warns_duplicate_scenarios() {
    let blocks = vec![
        "Feature: Dups\n  Scenario: Same name\n    Given a\n    Then b\n  Scenario: Same name\n    Given c\n    Then d"
            .to_string(),
    ];
    let result = process_blocks(&blocks, "test.md").unwrap();
    assert!(result
        .validation
        .warnings
        .iter()
        .any(|w| w.message.contains("Duplicate")));
}

#[test]
fn validation_no_given_is_info_severity() {
    let blocks = vec![
        "Feature: Actions only\n  Scenario: Direct\n    When user clicks\n    Then something happens"
            .to_string(),
    ];
    let result = process_blocks(&blocks, "test.md").unwrap();
    let no_given = result
        .validation
        .warnings
        .iter()
        .find(|w| w.message.contains("no Given"));
    assert!(no_given.is_some());
    assert_eq!(no_given.unwrap().severity, Severity::Info);
}

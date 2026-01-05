pub mod d2;
pub mod mermaid;

use gherkin::Feature;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramStyle {
    Auto,
    Flow,
    Overview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramFormat {
    Mermaid,
    D2,
}

pub trait DiagramGenerator {
    fn generate(&self, features: &[Feature], style: DiagramStyle) -> String;
}

/// Auto-select style based on complexity heuristic.
/// ≤3 scenarios AND ≤15 total steps → Flow, otherwise Overview.
pub fn auto_select_style(features: &[Feature]) -> DiagramStyle {
    let mut total_scenarios = 0usize;
    let mut total_steps = 0usize;

    for f in features {
        total_scenarios += f.scenarios.len();
        for s in &f.scenarios {
            total_steps += s.steps.len();
        }
        if let Some(bg) = &f.background {
            total_steps += bg.steps.len();
        }
        for r in &f.rules {
            total_scenarios += r.scenarios.len();
            for s in &r.scenarios {
                total_steps += s.steps.len();
            }
        }
    }

    if total_scenarios <= 3 && total_steps <= 15 {
        DiagramStyle::Flow
    } else {
        DiagramStyle::Overview
    }
}

/// Resolve Auto to a concrete style.
pub fn resolve_style(style: DiagramStyle, features: &[Feature]) -> DiagramStyle {
    match style {
        DiagramStyle::Auto => auto_select_style(features),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gherkin::GherkinEnv;

    fn parse_feature(gherkin: &str) -> Feature {
        Feature::parse(gherkin, GherkinEnv::default()).unwrap()
    }

    #[test]
    fn auto_selects_flow_for_small() {
        let f =
            parse_feature("Feature: Small\n  Scenario: A\n    Given x\n    When y\n    Then z\n");
        assert_eq!(auto_select_style(&[f]), DiagramStyle::Flow);
    }

    #[test]
    fn auto_selects_overview_for_large() {
        let gherkin = "Feature: Big\n  Scenario: A\n    Given a\n    When b\n    Then c\n  Scenario: B\n    Given d\n    When e\n    Then f\n  Scenario: C\n    Given g\n    When h\n    Then i\n  Scenario: D\n    Given j\n    When k\n    Then l\n";
        let f = parse_feature(gherkin);
        assert_eq!(auto_select_style(&[f]), DiagramStyle::Overview);
    }
}

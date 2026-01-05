/// Semantic validation for parsed Gherkin features.
/// These are warnings beyond syntax correctness — structural best-practice checks.
use gherkin::Feature;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Info,
}

#[derive(Debug, Clone)]
pub struct ValidationWarning {
    pub severity: Severity,
    pub message: String,
    /// Which feature this warning belongs to
    pub feature_name: String,
    /// Scenario name (if applicable)
    pub scenario_name: Option<String>,
}

#[derive(Debug, Default)]
pub struct ValidationResult {
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// Run semantic checks on parsed features.
pub fn validate_features(features: &[Feature]) -> ValidationResult {
    let mut result = ValidationResult::default();

    for feature in features {
        check_feature(&mut result, feature);
    }

    result
}

fn check_feature(result: &mut ValidationResult, feature: &Feature) {
    // Empty feature name
    if feature.name.trim().is_empty() {
        result.warnings.push(ValidationWarning {
            severity: Severity::Warning,
            message: "Feature has no name".to_string(),
            feature_name: "(unnamed)".to_string(),
            scenario_name: None,
        });
    }

    let fname = if feature.name.is_empty() {
        "(unnamed)".to_string()
    } else {
        feature.name.clone()
    };

    // Background without scenarios
    if feature.background.is_some() && feature.scenarios.is_empty() && feature.rules.is_empty() {
        result.warnings.push(ValidationWarning {
            severity: Severity::Warning,
            message: "Background defined but no scenarios exist".to_string(),
            feature_name: fname.clone(),
            scenario_name: None,
        });
    }

    // Check top-level scenarios
    let mut seen_names: Vec<String> = Vec::new();
    for scenario in &feature.scenarios {
        check_scenario(result, &fname, scenario, &mut seen_names);
    }

    // Check rule scenarios
    for rule in &feature.rules {
        for scenario in &rule.scenarios {
            check_scenario(result, &fname, scenario, &mut seen_names);
        }
    }
}

fn check_scenario(
    result: &mut ValidationResult,
    feature_name: &str,
    scenario: &gherkin::Scenario,
    seen_names: &mut Vec<String>,
) {
    let sname = if scenario.name.is_empty() {
        "(unnamed)".to_string()
    } else {
        scenario.name.clone()
    };

    // Empty scenario
    if scenario.steps.is_empty() {
        result.warnings.push(ValidationWarning {
            severity: Severity::Warning,
            message: "Scenario has no steps".to_string(),
            feature_name: feature_name.to_string(),
            scenario_name: Some(sname.clone()),
        });
        return;
    }

    // No Then step (no assertion/outcome)
    let has_then = scenario
        .steps
        .iter()
        .any(|s| s.ty == gherkin::StepType::Then);
    if !has_then {
        result.warnings.push(ValidationWarning {
            severity: Severity::Warning,
            message: "Scenario has no Then step (missing expected outcome)".to_string(),
            feature_name: feature_name.to_string(),
            scenario_name: Some(sname.clone()),
        });
    }

    // No Given step (no precondition)
    let has_given = scenario
        .steps
        .iter()
        .any(|s| s.ty == gherkin::StepType::Given);
    if !has_given {
        result.warnings.push(ValidationWarning {
            severity: Severity::Info,
            message: "Scenario has no Given step (missing precondition)".to_string(),
            feature_name: feature_name.to_string(),
            scenario_name: Some(sname.clone()),
        });
    }

    // Duplicate scenario name
    if !scenario.name.is_empty() && seen_names.contains(&scenario.name) {
        result.warnings.push(ValidationWarning {
            severity: Severity::Warning,
            message: format!("Duplicate scenario name: \"{}\"", scenario.name),
            feature_name: feature_name.to_string(),
            scenario_name: Some(sname.clone()),
        });
    }
    if !scenario.name.is_empty() {
        seen_names.push(scenario.name.clone());
    }
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match self.severity {
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        if let Some(ref s) = self.scenario_name {
            write!(
                f,
                "{prefix}: [{}] Scenario \"{s}\": {}",
                self.feature_name, self.message
            )
        } else {
            write!(f, "{prefix}: [{}]: {}", self.feature_name, self.message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gherkin::GherkinEnv;

    fn parse(gherkin: &str) -> Feature {
        Feature::parse(gherkin, GherkinEnv::default()).unwrap()
    }

    #[test]
    fn valid_feature_no_warnings() {
        let f = parse(
            "Feature: Login\n  Scenario: Success\n    Given on page\n    When enter creds\n    Then see dashboard\n",
        );
        let result = validate_features(&[f]);
        assert!(!result.has_warnings());
    }

    #[test]
    fn warns_no_then_step() {
        let f =
            parse("Feature: Test\n  Scenario: No outcome\n    Given something\n    When action\n");
        let result = validate_features(&[f]);
        assert!(result.has_warnings());
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("no Then")));
    }

    #[test]
    fn warns_empty_scenario() {
        let f = parse("Feature: Test\n  Scenario: Empty\n");
        let result = validate_features(&[f]);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("no steps")));
    }

    #[test]
    fn warns_duplicate_names() {
        let f = parse(
            "Feature: Test\n  Scenario: Dup\n    Given a\n    Then b\n  Scenario: Dup\n    Given c\n    Then d\n",
        );
        let result = validate_features(&[f]);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("Duplicate")));
    }

    #[test]
    fn warns_background_without_scenarios() {
        let f = parse("Feature: Test\n  Background:\n    Given setup\n");
        let result = validate_features(&[f]);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.message.contains("Background defined but no scenarios")));
    }
}

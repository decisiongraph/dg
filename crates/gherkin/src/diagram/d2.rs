use std::fmt::Write;

use gherkin::{Feature, Step, StepType};

use super::{DiagramGenerator, DiagramStyle};

pub struct D2Generator;

impl DiagramGenerator for D2Generator {
    fn generate(&self, features: &[Feature], style: DiagramStyle) -> String {
        let resolved = super::resolve_style(style, features);
        match resolved {
            DiagramStyle::Flow => generate_flow(features),
            DiagramStyle::Overview => generate_overview(features),
            DiagramStyle::Auto => unreachable!("Auto should be resolved"),
        }
    }
}

fn escape_d2(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn step_shape(step: &Step) -> &'static str {
    match step.ty {
        StepType::Given => "oval",
        StepType::When => "diamond",
        StepType::Then => "rectangle",
    }
}

/// Use the raw keyword from the gherkin file (Given/When/Then/And/But).
fn step_keyword(step: &Step) -> &str {
    step.keyword.trim()
}

fn emit_steps_d2(out: &mut String, steps: &[Step], indent: usize) {
    let pad = " ".repeat(indent);
    for (i, step) in steps.iter().enumerate() {
        let shape = step_shape(step);
        let label = format!("{} {}", step_keyword(step), step.value);
        writeln!(
            out,
            "{pad}st{i}: \"{}\" {{ shape: {shape} }}",
            escape_d2(&label)
        )
        .unwrap();
        if i > 0 {
            writeln!(out, "{pad}st{} -> st{i}", i - 1).unwrap();
        }
    }
}

fn generate_flow(features: &[Feature]) -> String {
    let mut out = String::new();

    for (fi, feature) in features.iter().enumerate() {
        let fid = format!("f{fi}");
        writeln!(out, "{fid}: \"{}\" {{", escape_d2(&feature.name)).unwrap();

        // Background
        if let Some(bg) = &feature.background {
            let bg_name = if bg.name.is_empty() {
                "Background".to_string()
            } else {
                format!("Background: {}", bg.name)
            };
            writeln!(out, "  bg: \"{}\" {{", escape_d2(&bg_name)).unwrap();
            emit_steps_d2(&mut out, &bg.steps, 4);
            writeln!(out, "  }}").unwrap();
        }

        // Scenarios
        for (si, scenario) in feature.scenarios.iter().enumerate() {
            let sid = format!("s{si}");
            let label = if scenario.examples.is_empty() {
                escape_d2(&scenario.name)
            } else {
                format!("[Outline] {}", escape_d2(&scenario.name))
            };

            // Tags as comment
            if !scenario.tags.is_empty() {
                let tags: Vec<_> = scenario.tags.iter().map(|t| format!("@{t}")).collect();
                writeln!(out, "  # {}", tags.join(" ")).unwrap();
            }

            writeln!(out, "  {sid}: \"{label}\" {{").unwrap();
            emit_steps_d2(&mut out, &scenario.steps, 4);
            writeln!(out, "  }}").unwrap();

            // Background -> scenario dashed connection
            if let Some(bg) = &feature.background {
                if !bg.steps.is_empty() && !scenario.steps.is_empty() {
                    writeln!(
                        out,
                        "  bg.st{} -> {sid}.st0: {{ style.stroke-dash: 3 }}",
                        bg.steps.len() - 1
                    )
                    .unwrap();
                }
            }
        }

        // Rules
        for (ri, rule) in feature.rules.iter().enumerate() {
            let rid = format!("r{ri}");
            writeln!(out, "  {rid}: \"Rule: {}\" {{", escape_d2(&rule.name)).unwrap();
            for (si, scenario) in rule.scenarios.iter().enumerate() {
                let sid = format!("s{si}");
                writeln!(out, "    {sid}: \"{}\" {{", escape_d2(&scenario.name)).unwrap();
                emit_steps_d2(&mut out, &scenario.steps, 6);
                writeln!(out, "    }}").unwrap();
            }
            writeln!(out, "  }}").unwrap();
        }

        writeln!(out, "}}").unwrap();
    }

    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn generate_overview(features: &[Feature]) -> String {
    let mut out = String::new();

    for (fi, feature) in features.iter().enumerate() {
        let fid = format!("f{fi}");
        writeln!(out, "{fid}: \"{}\"", escape_d2(&feature.name)).unwrap();

        // Rules
        for (ri, rule) in feature.rules.iter().enumerate() {
            let rid = format!("{fid}_r{ri}");
            writeln!(
                out,
                "{rid}: \"Rule: {}\" {{ shape: diamond }}",
                escape_d2(&rule.name)
            )
            .unwrap();
            writeln!(out, "{fid} -> {rid}").unwrap();

            for (si, scenario) in rule.scenarios.iter().enumerate() {
                let sid = format!("{rid}_s{si}");
                let label = scenario_label(scenario);
                writeln!(out, "{sid}: \"{}\" {{ shape: oval }}", escape_d2(&label)).unwrap();
                writeln!(out, "{rid} -> {sid}").unwrap();
            }
        }

        // Top-level scenarios
        for (si, scenario) in feature.scenarios.iter().enumerate() {
            let sid = format!("{fid}_s{si}");
            let label = scenario_label(scenario);
            writeln!(out, "{sid}: \"{}\" {{ shape: oval }}", escape_d2(&label)).unwrap();
            writeln!(out, "{fid} -> {sid}").unwrap();
        }
    }

    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn scenario_label(scenario: &gherkin::Scenario) -> String {
    let mut label = scenario.name.clone();
    if !scenario.tags.is_empty() {
        let tags: Vec<_> = scenario.tags.iter().map(|t| format!("@{t}")).collect();
        label = format!("{} [{}]", label, tags.join(" "));
    }
    if !scenario.examples.is_empty() {
        label = format!("[Outline] {label}");
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use gherkin::GherkinEnv;

    fn parse(gherkin: &str) -> Feature {
        Feature::parse(gherkin, GherkinEnv::default()).unwrap()
    }

    #[test]
    fn d2_flow_simple() {
        let f = parse(
            "Feature: Login\n  Scenario: Success\n    Given on page\n    When enter creds\n    Then see dashboard\n",
        );
        let output = D2Generator.generate(&[f], DiagramStyle::Flow);
        assert!(output.contains("Login"));
        assert!(output.contains("Given on page"));
        assert!(output.contains("shape: oval")); // Given
        assert!(output.contains("shape: diamond")); // When
        assert!(output.contains("shape: rectangle")); // Then
    }

    #[test]
    fn d2_overview_simple() {
        let f = parse(
            "Feature: Login\n  Scenario: Success\n    Given on page\n    When enter creds\n    Then see dashboard\n",
        );
        let output = D2Generator.generate(&[f], DiagramStyle::Overview);
        assert!(output.contains("Login"));
        assert!(output.contains("Success"));
    }

    #[test]
    fn d2_and_but_keywords() {
        let f = parse(
            "Feature: Test\n  Scenario: A\n    Given first\n    And second\n    When action\n    But not this\n    Then result\n",
        );
        let output = D2Generator.generate(&[f], DiagramStyle::Flow);
        assert!(output.contains("And second"));
        assert!(output.contains("But not this"));
    }
}

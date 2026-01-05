use std::collections::BTreeMap;
use std::fmt::Write;

use gherkin::{Feature, Scenario, Step, StepType};

use super::{DiagramGenerator, DiagramStyle};

pub struct MermaidGenerator;

impl DiagramGenerator for MermaidGenerator {
    fn generate(&self, features: &[Feature], style: DiagramStyle) -> String {
        let resolved = super::resolve_style(style, features);
        match resolved {
            DiagramStyle::Flow => generate_flow(features),
            DiagramStyle::Overview => generate_overview(features),
            DiagramStyle::Auto => unreachable!("Auto should be resolved"),
        }
    }
}

fn escape_mermaid(s: &str) -> String {
    // Use Mermaid's HTML entity and #code; syntax to prevent
    // brackets/parens from being interpreted as node shapes.
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('[', "#91;")
        .replace(']', "#93;")
        .replace('(', "#40;")
        .replace(')', "#41;")
        .replace('{', "#123;")
        .replace('}', "#125;")
}

/// Use the raw keyword from the gherkin file (Given/When/Then/And/But).
fn step_keyword(step: &Step) -> &str {
    step.keyword.trim()
}

fn step_node(id: &str, step: &Step, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let label = escape_mermaid(&format!("{} {}", step_keyword(step), step.value));

    match step.ty {
        StepType::Given => format!("{pad}{id}([\"{label}\"])"),
        StepType::When => format!("{pad}{id}{{{{\"{label}\"}}}}"),
        StepType::Then => format!("{pad}{id}[\"{label}\"]"),
    }
}

fn generate_flow(features: &[Feature]) -> String {
    let mut out = String::from("flowchart TD\n");

    for (fi, feature) in features.iter().enumerate() {
        let fp = format!("f{fi}");

        // Feature subgraph
        writeln!(
            out,
            "    subgraph {fp}[\"{}\"]",
            escape_mermaid(&feature.name)
        )
        .unwrap();

        // Background
        if let Some(bg) = &feature.background {
            let bp = format!("{fp}_bg");
            let bg_name = if bg.name.is_empty() {
                "Background".to_string()
            } else {
                format!("Background: {}", bg.name)
            };
            writeln!(
                out,
                "        subgraph {bp}[\"{}\"]",
                escape_mermaid(&bg_name)
            )
            .unwrap();
            emit_steps(&mut out, &bp, &bg.steps, 12);
            writeln!(out, "        end").unwrap();
        }

        // Scenarios — group by primary tag when >4 scenarios and multiple groups
        let groups = group_scenarios_by_tag(&feature.scenarios);
        let use_groups = feature.scenarios.len() > 4 && groups.len() > 1;

        if use_groups {
            for (group_tag, scenario_indices) in &groups {
                let in_group = group_tag.is_some();
                if let Some(tag) = group_tag {
                    let gp = format!("{fp}_g_{tag}");
                    writeln!(out, "        subgraph {gp}[\"@{}\"]", escape_mermaid(tag)).unwrap();
                }
                for &si in scenario_indices {
                    let scenario = &feature.scenarios[si];
                    emit_scenario(
                        &mut out,
                        &fp,
                        si,
                        scenario,
                        feature.background.as_ref(),
                        in_group,
                    );
                }
                if in_group {
                    writeln!(out, "        end").unwrap();
                }
            }
        } else {
            for (si, scenario) in feature.scenarios.iter().enumerate() {
                emit_scenario(
                    &mut out,
                    &fp,
                    si,
                    scenario,
                    feature.background.as_ref(),
                    false,
                );
            }
        }

        // Rules
        for (ri, rule) in feature.rules.iter().enumerate() {
            let rp = format!("{fp}_r{ri}");
            writeln!(
                out,
                "        subgraph {rp}[\"Rule: {}\"]",
                escape_mermaid(&rule.name)
            )
            .unwrap();
            for (si, scenario) in rule.scenarios.iter().enumerate() {
                let sp = format!("{rp}_s{si}");
                writeln!(
                    out,
                    "            subgraph {sp}[\"{}\"]",
                    escape_mermaid(&scenario.name)
                )
                .unwrap();
                emit_steps(&mut out, &sp, &scenario.steps, 16);
                writeln!(out, "            end").unwrap();
            }
            writeln!(out, "        end").unwrap();
        }

        writeln!(out, "    end").unwrap();
    }

    // Remove trailing newline
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Emit step nodes and edges with consistent indentation.
fn emit_steps(out: &mut String, prefix: &str, steps: &[Step], indent: usize) {
    let pad = " ".repeat(indent);
    for (i, step) in steps.iter().enumerate() {
        let id = format!("{prefix}_st{i}");
        writeln!(out, "{}", step_node(&id, step, indent)).unwrap();
        if i > 0 {
            let prev = format!("{prefix}_st{}", i - 1);
            writeln!(out, "{pad}{prev} --> {id}").unwrap();
        }
    }
}

/// Group scenarios by their first (primary) tag.
/// Returns ordered groups: (Some(tag), indices) for tagged, (None, indices) for untagged.
fn group_scenarios_by_tag(scenarios: &[Scenario]) -> Vec<(Option<String>, Vec<usize>)> {
    let mut tag_groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut untagged: Vec<usize> = Vec::new();

    for (i, scenario) in scenarios.iter().enumerate() {
        if let Some(first_tag) = scenario.tags.first() {
            tag_groups.entry(first_tag.clone()).or_default().push(i);
        } else {
            untagged.push(i);
        }
    }

    let mut result: Vec<(Option<String>, Vec<usize>)> = Vec::new();
    for (tag, indices) in tag_groups {
        if indices.len() > 1 {
            result.push((Some(tag), indices));
        } else {
            untagged.extend(indices);
        }
    }
    if !untagged.is_empty() {
        untagged.sort();
        result.push((None, untagged));
    }
    result
}

fn emit_scenario(
    out: &mut String,
    fp: &str,
    si: usize,
    scenario: &Scenario,
    background: Option<&gherkin::Background>,
    suppress_primary_tag: bool,
) {
    let sp = format!("{fp}_s{si}");
    let scenario_label = if scenario.examples.is_empty() {
        escape_mermaid(&scenario.name)
    } else {
        format!("[Outline] {}", escape_mermaid(&scenario.name))
    };

    // Tags as comment (skip primary tag if inside a group subgraph)
    if !scenario.tags.is_empty() {
        let tags: Vec<_> = scenario
            .tags
            .iter()
            .enumerate()
            .filter(|(i, _)| !suppress_primary_tag || *i > 0)
            .map(|(_, t)| format!("@{t}"))
            .collect();
        if !tags.is_empty() {
            writeln!(out, "        %% {}", tags.join(" ")).unwrap();
        }
    }

    writeln!(out, "        subgraph {sp}[\"{scenario_label}\"]").unwrap();
    emit_steps(out, &sp, &scenario.steps, 12);

    // Scenario Outline examples note
    if !scenario.examples.is_empty() {
        let example_cols: Vec<String> = scenario
            .examples
            .iter()
            .flat_map(|ex| ex.table.as_ref())
            .flat_map(|t| t.rows.first())
            .flat_map(|row| row.iter())
            .map(|c| c.to_string())
            .collect();
        if !example_cols.is_empty() {
            let note_id = format!("{sp}_examples");
            writeln!(
                out,
                "            {note_id}[/\"Examples: {}\"/]",
                escape_mermaid(&example_cols.join(", "))
            )
            .unwrap();
        }
    }

    writeln!(out, "        end").unwrap();

    // Connect background to scenario start
    if let Some(bg) = background {
        if !bg.steps.is_empty() && !scenario.steps.is_empty() {
            let bg_last = format!("{fp}_bg_st{}", bg.steps.len() - 1);
            let s_first = format!("{sp}_st0");
            writeln!(out, "        {bg_last} -.-> {s_first}").unwrap();
        }
    }
}

fn generate_overview(features: &[Feature]) -> String {
    let mut out = String::from("flowchart LR\n");

    for (fi, feature) in features.iter().enumerate() {
        let fid = format!("f{fi}");
        writeln!(out, "    {fid}[\"{}\"]", escape_mermaid(&feature.name)).unwrap();

        // Rules as intermediate nodes
        for (ri, rule) in feature.rules.iter().enumerate() {
            let rid = format!("{fid}_r{ri}");
            writeln!(
                out,
                "    {rid}{{{{\"Rule: {}\"}}}}",
                escape_mermaid(&rule.name)
            )
            .unwrap();
            writeln!(out, "    {fid} --> {rid}").unwrap();

            for (si, scenario) in rule.scenarios.iter().enumerate() {
                let sid = format!("{rid}_s{si}");
                let label = scenario_overview_label(scenario);
                writeln!(out, "    {sid}([\"{}\"])", escape_mermaid(&label)).unwrap();
                writeln!(out, "    {rid} --> {sid}").unwrap();
            }
        }

        // Top-level scenarios
        for (si, scenario) in feature.scenarios.iter().enumerate() {
            let sid = format!("{fid}_s{si}");
            let label = scenario_overview_label(scenario);
            writeln!(out, "    {sid}([\"{}\"])", escape_mermaid(&label)).unwrap();
            writeln!(out, "    {fid} --> {sid}").unwrap();
        }
    }

    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn scenario_overview_label(scenario: &gherkin::Scenario) -> String {
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
    fn flow_simple() {
        let f = parse(
            "Feature: Login\n  Scenario: Success\n    Given on page\n    When enter creds\n    Then see dashboard\n",
        );
        let output = MermaidGenerator.generate(&[f], DiagramStyle::Flow);
        assert!(output.starts_with("flowchart TD"));
        assert!(output.contains("Login"));
        assert!(output.contains("Given on page"));
    }

    #[test]
    fn overview_simple() {
        let f = parse(
            "Feature: Login\n  Scenario: Success\n    Given on page\n    When enter creds\n    Then see dashboard\n",
        );
        let output = MermaidGenerator.generate(&[f], DiagramStyle::Overview);
        assert!(output.starts_with("flowchart LR"));
        assert!(output.contains("Login"));
        assert!(output.contains("Success"));
    }

    #[test]
    fn and_but_keywords_preserved() {
        let f = parse(
            "Feature: Test\n  Scenario: A\n    Given first\n    And second\n    When action\n    But not this\n    Then result\n",
        );
        let output = MermaidGenerator.generate(&[f], DiagramStyle::Flow);
        assert!(output.contains("And second"));
        assert!(output.contains("But not this"));
    }
}

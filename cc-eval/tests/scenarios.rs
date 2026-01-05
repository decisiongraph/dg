//! Generic test runner for all eval scenarios.
//!
//! Each scenario is loaded from markdown files in `scenarios/`.
//! Test assertions are driven by the `expect` section in frontmatter.

use cc_eval::llm::LlmProvider;

/// Run a scenario and check its expectations.
async fn run_and_check(scenario_name: &str) {
    let scenarios = cc_eval::scenario::load_scenarios().unwrap();
    let config = scenarios
        .iter()
        .find(|s| s.name == scenario_name)
        .unwrap_or_else(|| panic!("scenario '{}' not found", scenario_name));

    let workspace = cc_eval::setup::create_workspace().unwrap();
    let stats = cc_eval::eval::run_scenario(config, workspace.path(), LlmProvider::default())
        .await
        .unwrap();

    let expect = &config.expect;

    if expect.questions_first {
        assert!(
            stats.asked_questions_first,
            "[{}] expected questions before writes",
            scenario_name
        );
    }

    if expect.min_questions > 0 {
        assert!(
            stats.question_count >= expect.min_questions as usize,
            "[{}] expected at least {} questions, got {}",
            scenario_name,
            expect.min_questions,
            stats.question_count
        );
    }

    if expect.min_tool_calls > 0 {
        assert!(
            stats.tool_call_count >= expect.min_tool_calls as usize,
            "[{}] expected at least {} tool calls, got {}",
            scenario_name,
            expect.min_tool_calls,
            stats.tool_call_count
        );
    }

    if expect.any_doc_created {
        assert!(
            stats.opp_created || stats.pol_created || stats.adr_created || stats.inc_created,
            "[{}] expected at least one doc (OPP/POL/ADR/INC) to be created",
            scenario_name
        );
    }
}

#[tokio::test]
#[ignore] // requires Claude CLI + API key + costs money
async fn scenario_selling_lama_milk() {
    run_and_check("selling-lama-milk").await;
}

#[tokio::test]
#[ignore] // requires Claude CLI + API key + costs money
async fn scenario_accepting_crypto_payments() {
    run_and_check("accepting-crypto-payments").await;
}

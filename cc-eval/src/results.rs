use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use markdown_tui::RenderOptions;
use serde::{Deserialize, Serialize};

use crate::criteria::CriterionResult;
use crate::eval::EvalStats;
use crate::judge::JudgeResults;

/// Result of a single scenario within a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResult {
    pub name: String,
    pub score: u32,
    pub max_score: u32,
    pub status: String,
    pub criteria: Vec<CriterionResult>,
    pub stats: EvalStats,
    pub judge: Option<JudgeResults>,
}

/// A complete eval run containing all scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRun {
    pub run_id: String,
    pub timestamp: String,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
    pub scenarios: Vec<ScenarioResult>,
    pub total_score: u32,
    pub total_max_score: u32,
    pub status: String,
    pub baseline: Baseline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub min_score: u32,
    pub target_score: u32,
}

impl Default for Baseline {
    fn default() -> Self {
        Self {
            min_score: 70,
            target_score: 85,
        }
    }
}

/// Generate a unique run ID: "20260208T143022-a1b2c3d4"
pub fn generate_run_id() -> String {
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let hex: String = (0..4)
        .map(|_| format!("{:02x}", rand::random::<u8>()))
        .collect();
    format!("{ts}-{hex}")
}

/// Get git HEAD commit hash + branch name from a repo path.
pub fn get_git_info(repo_path: &Path) -> (Option<String>, Option<String>) {
    let repo = match git2::Repository::discover(repo_path) {
        Ok(r) => r,
        Err(_) => return (None, None),
    };

    let commit = repo
        .head()
        .ok()
        .and_then(|h| h.peel_to_commit().ok())
        .map(|c| c.id().to_string()[..7].to_string());

    let branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()));

    (commit, branch)
}

/// Save an eval run to disk.
pub fn save_result(run: &EvalRun, results_dir: &Path) -> Result<()> {
    let run_dir = results_dir.join(&run.run_id);
    fs::create_dir_all(&run_dir).context("create run dir")?;

    let json = serde_json::to_string_pretty(run).context("serialize run")?;
    fs::write(run_dir.join("result.json"), &json).context("write result.json")?;

    let line = serde_json::to_string(run).context("serialize run for history")?;
    let history_path = results_dir.join("history.jsonl");

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&history_path)
        .context("open history.jsonl")?;
    writeln!(file, "{line}").context("write history line")?;

    Ok(())
}

/// Load all runs from history.jsonl.
pub fn load_history(results_dir: &Path) -> Result<Vec<EvalRun>> {
    let history_path = results_dir.join("history.jsonl");
    if !history_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&history_path).context("read history.jsonl")?;
    let mut runs = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<EvalRun>(line) {
            Ok(run) => runs.push(run),
            Err(e) => eprintln!("warning: history line {i} parse error: {e}"),
        }
    }
    Ok(runs)
}

/// Print comparison of last two runs.
pub fn compare_last_two(results_dir: &Path, opts: &RenderOptions) -> Result<()> {
    let runs = load_history(results_dir)?;
    if runs.len() < 2 {
        eprintln!("Need at least 2 runs to compare (have {})", runs.len());
        return Ok(());
    }

    let prev = &runs[runs.len() - 2];
    let curr = &runs[runs.len() - 1];

    eprintln!("=== Comparison ===");
    eprintln!("  prev: {} ({})", prev.run_id, prev.timestamp);
    eprintln!("  curr: {} ({})", curr.run_id, curr.timestamp);
    eprintln!();

    for curr_s in &curr.scenarios {
        let prev_s = prev.scenarios.iter().find(|s| s.name == curr_s.name);

        eprintln!("  --- {} ---", curr_s.name);

        let headers: Vec<&str> = vec!["Criterion", "Prev", "Curr", "Delta"];
        let mut rows: Vec<Vec<String>> = Vec::new();

        for curr_c in &curr_s.criteria {
            let prev_score = prev_s
                .and_then(|s| s.criteria.iter().find(|c| c.name == curr_c.name))
                .map(|c| c.score)
                .unwrap_or(0);

            let delta = curr_c.score as i32 - prev_score as i32;
            let delta_str = if delta > 0 {
                format!("+{delta}")
            } else if delta < 0 {
                format!("{delta}")
            } else {
                "=".into()
            };

            rows.push(vec![
                curr_c.name.clone(),
                format!("{}/{}", prev_score, curr_c.max_score),
                format!("{}/{}", curr_c.score, curr_c.max_score),
                delta_str,
            ]);
        }

        // Total row
        let prev_total = prev_s.map(|s| s.score).unwrap_or(0);
        let delta = curr_s.score as i32 - prev_total as i32;
        let delta_str = if delta > 0 {
            format!("+{delta}")
        } else if delta < 0 {
            format!("{delta}")
        } else {
            "=".into()
        };
        rows.push(vec![
            "TOTAL".into(),
            format!("{}/{}", prev_total, curr_s.max_score),
            format!("{}/{}", curr_s.score, curr_s.max_score),
            delta_str,
        ]);

        let table = markdown_tui::render_table(&headers, &rows, opts);
        eprint!("{table}");
        eprintln!();
    }

    Ok(())
}

/// Print insights from recent eval runs: tool errors, missing docs, judge feedback.
pub fn print_insights(results_dir: &Path, run_id: Option<&str>, n: usize) -> Result<()> {
    let runs = load_history(results_dir)?;
    if runs.is_empty() {
        eprintln!("No eval history found.");
        return Ok(());
    }

    // Select runs to display
    let selected_runs: Vec<&EvalRun> = if let Some(id) = run_id {
        runs.iter().filter(|r| r.run_id.contains(id)).collect()
    } else {
        let start = runs.len().saturating_sub(n);
        runs[start..].iter().collect()
    };

    if selected_runs.is_empty() {
        eprintln!("No matching runs found.");
        return Ok(());
    }

    for run in selected_runs {
        print_run_insights(run);
    }

    Ok(())
}

/// Print insights for a single run.
fn print_run_insights(run: &EvalRun) {
    eprintln!("═══ {} ═══", run.run_id);
    if let (Some(branch), Some(commit)) = (&run.git_branch, &run.git_commit) {
        eprintln!("git: {}@{}", branch, commit);
    }
    eprintln!(
        "score: {}/{} ({})",
        run.total_score, run.total_max_score, run.status
    );
    eprintln!();

    for scenario in &run.scenarios {
        eprintln!(
            "─── {} ({}/{}) ───",
            scenario.name, scenario.score, scenario.max_score
        );

        // Tool errors
        let errors: Vec<_> = scenario.stats.tool_calls.iter().filter(|t| t.is_error).collect();
        if !errors.is_empty() {
            eprintln!("  TOOL ERRORS ({}):", errors.len());
            for e in errors.iter().take(5) {
                let summary = if e.input_summary.len() > 80 {
                    format!("{}...", &e.input_summary[..80])
                } else {
                    e.input_summary.clone()
                };
                eprintln!("    • {} — {}", e.name, summary);
            }
            if errors.len() > 5 {
                eprintln!("    ... and {} more", errors.len() - 5);
            }
        }

        // Missing docs
        let expected_docs = infer_expected_docs(&scenario.name);
        let mut missing = Vec::new();
        if expected_docs.contains(&"opp") && !scenario.stats.opp_created {
            missing.push("OPP");
        }
        if expected_docs.contains(&"pol") && !scenario.stats.pol_created {
            missing.push("POL");
        }
        if expected_docs.contains(&"adr") && !scenario.stats.adr_created {
            missing.push("ADR");
        }
        if expected_docs.contains(&"inc") && !scenario.stats.inc_created {
            missing.push("INC");
        }
        if !missing.is_empty() {
            eprintln!("  MISSING DOCS: {}", missing.join(", "));
        }

        // Created docs
        let mut created = Vec::new();
        if scenario.stats.opp_created {
            created.push("OPP");
        }
        if scenario.stats.pol_created {
            created.push("POL");
        }
        if scenario.stats.adr_created {
            created.push("ADR");
        }
        if scenario.stats.inc_created {
            created.push("INC");
        }
        if !created.is_empty() {
            eprintln!("  CREATED: {}", created.join(", "));
        }

        // Criteria with low scores
        let low_criteria: Vec<_> = scenario
            .criteria
            .iter()
            .filter(|c| c.max_score > 0 && c.score * 2 < c.max_score) // <50%
            .collect();
        if !low_criteria.is_empty() {
            eprintln!("  LOW SCORES:");
            for c in low_criteria {
                eprintln!("    • {}: {}/{}", c.name, c.score, c.max_score);
                if let Some(ref fb) = c.feedback {
                    let fb_short = truncate_feedback(fb, 100);
                    eprintln!("      {}", fb_short);
                }
            }
        }

        // Judge feedback
        if let Some(ref judge) = scenario.judge {
            eprintln!("  JUDGE:");
            eprintln!(
                "    Q-Quality: {}/100 — {}",
                judge.question_quality.score,
                truncate_feedback(&judge.question_quality.feedback, 80)
            );
            if let Some(ref dq) = judge.doc_quality {
                eprintln!(
                    "    D-Quality: {}/100 — {}",
                    dq.score,
                    truncate_feedback(&dq.feedback, 80)
                );
            }
        }

        // Questions asked (brief summary)
        if !scenario.stats.questions.is_empty() {
            eprintln!("  QUESTIONS ({}):", scenario.stats.questions.len());
            for q in scenario.stats.questions.iter().take(3) {
                let q_short = if q.len() > 70 {
                    format!("{}...", &q[..70])
                } else {
                    q.clone()
                };
                eprintln!("    • {}", q_short);
            }
            if scenario.stats.questions.len() > 3 {
                eprintln!("    ... and {} more", scenario.stats.questions.len() - 3);
            }
        }

        eprintln!();
    }
}

/// Truncate feedback string for display.
fn truncate_feedback(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', " ");
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s
    }
}

/// Infer expected doc types from scenario name.
fn infer_expected_docs(name: &str) -> Vec<&'static str> {
    let name_lower = name.to_lowercase();
    if name_lower.contains("incident") {
        vec!["inc"]
    } else if name_lower.contains("policy") || name_lower.contains("compliance") {
        vec!["pol"]
    } else if name_lower.contains("adr") || name_lower.contains("decision") {
        vec!["adr"]
    } else {
        // Default: expect OPP for business scenarios
        vec!["opp"]
    }
}

/// Print recent history as a table.
pub fn print_history(results_dir: &Path, n: usize, opts: &RenderOptions) -> Result<()> {
    let runs = load_history(results_dir)?;
    if runs.is_empty() {
        eprintln!("No eval history found.");
        return Ok(());
    }

    let start = runs.len().saturating_sub(n);
    let recent = &runs[start..];

    let headers: Vec<&str> = vec!["Run ID", "Score", "Status", "Cost", "Git"];
    let mut rows: Vec<Vec<String>> = Vec::new();

    for run in recent {
        let total_cost: f64 = run
            .scenarios
            .iter()
            .filter_map(|s| s.stats.total_cost_usd)
            .sum();
        let cost = if total_cost > 0.0 {
            format!("${total_cost:.4}")
        } else {
            "-".into()
        };
        let git = match (&run.git_branch, &run.git_commit) {
            (Some(b), Some(c)) => format!("{b}@{c}"),
            _ => "-".into(),
        };
        rows.push(vec![
            run.run_id.clone(),
            format!("{}/{}", run.total_score, run.total_max_score),
            run.status.clone(),
            cost,
            git,
        ]);
    }

    let table = markdown_tui::render_table(&headers, &rows, opts);
    eprint!("{table}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{EvalStats, EvalTiming};
    use std::collections::HashMap;

    fn mock_scenario(name: &str, score: u32) -> ScenarioResult {
        ScenarioResult {
            name: name.into(),
            score,
            max_score: 100,
            status: if score >= 85 { "pass" } else { "fail" }.into(),
            criteria: vec![CriterionResult {
                name: "test".into(),
                score,
                max_score: 100,
                details: "test".into(),
                feedback: None,
            }],
            stats: EvalStats {
                asked_questions_first: true,
                question_count: 3,
                questions: vec!["Q1?".into()],
                first_question_turn: Some(0),
                first_write_turn: Some(2),
                ask_user_question_count: 0,
                ask_user_questions: Vec::new(),
                ask_user_answers: Vec::new(),
                tool_calls: Vec::new(),
                tool_call_count: 5,
                tool_error_count: 0,
                tool_counts_by_name: HashMap::new(),
                total_input_tokens: 1000,
                total_output_tokens: 500,
                total_cache_creation_tokens: 0,
                total_cache_read_tokens: 0,
                total_cost_usd: Some(0.05),
                duration_ms: Some(10000.0),
                num_turns: Some(3),
                opp_created: true,
                pol_created: false,
                adr_created: false,
                inc_created: false,
                cross_link_count: 0,
                fixme_count: 0,
                files_created: Vec::new(),
                doc_contents: None,
                assistant_text: "test".into(),
                timing: EvalTiming::default(),
            },
            judge: None,
        }
    }

    fn mock_run(id: &str, score: u32) -> EvalRun {
        EvalRun {
            run_id: id.into(),
            timestamp: "2026-01-01T00:00:00Z".into(),
            git_commit: Some("abc1234".into()),
            git_branch: Some("main".into()),
            scenarios: vec![mock_scenario("selling-lama-milk", score)],
            total_score: score,
            total_max_score: 100,
            status: if score >= 85 { "pass" } else { "fail" }.into(),
            baseline: Baseline::default(),
        }
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let run = mock_run("test-001", 75);
        save_result(&run, dir.path()).unwrap();

        assert!(dir.path().join("test-001/result.json").exists());

        let history = load_history(dir.path()).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].run_id, "test-001");
        assert_eq!(history[0].total_score, 75);
    }

    #[test]
    fn history_append() {
        let dir = tempfile::tempdir().unwrap();
        save_result(&mock_run("run-1", 60), dir.path()).unwrap();
        save_result(&mock_run("run-2", 80), dir.path()).unwrap();

        let history = load_history(dir.path()).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].run_id, "run-1");
        assert_eq!(history[1].run_id, "run-2");
    }

    #[test]
    fn empty_history() {
        let dir = tempfile::tempdir().unwrap();
        let history = load_history(dir.path()).unwrap();
        assert!(history.is_empty());
    }

    #[test]
    fn run_id_format() {
        let id = generate_run_id();
        assert!(id.contains('T'));
        assert!(id.contains('-'));
        assert!(id.len() > 20);
    }
}

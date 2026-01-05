use serde::{Deserialize, Serialize};

use crate::eval::EvalStats;
#[cfg(test)]
use crate::eval::EvalTiming;
use crate::judge::JudgeResults;

/// Result of evaluating a single criterion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CriterionResult {
    pub name: String,
    pub score: u32,
    pub max_score: u32,
    pub details: String,
    pub feedback: Option<String>,
}

/// Score all criteria for a given scenario.
///
/// Returns a vec of criterion results. Total max = 100.
/// Uses a single generic scoring rubric for all scenarios.
pub fn score_criteria(
    _scenario: &str,
    stats: &EvalStats,
    judge: Option<&JudgeResults>,
) -> Vec<CriterionResult> {
    // Generic scoring: same rubric for all scenarios
    score_generic(stats, judge)
}

/// Total score from a list of criterion results.
pub fn total_score(criteria: &[CriterionResult]) -> (u32, u32) {
    let score = criteria.iter().map(|c| c.score).sum();
    let max = criteria.iter().map(|c| c.max_score).sum();
    (score, max)
}

/// Determine pass/fail status from score.
pub fn status_from_score(score: u32, min_score: u32, target_score: u32) -> &'static str {
    if score >= target_score {
        "pass"
    } else if score >= min_score {
        "below_target"
    } else {
        "fail"
    }
}

// --- Generic scoring (used for all scenarios) ---

fn score_generic(stats: &EvalStats, judge: Option<&JudgeResults>) -> Vec<CriterionResult> {
    vec![
        score_questions_before_action(stats, 20),
        score_question_quality(judge),
        score_docs_created(stats),
        score_cross_linking(stats),
        score_doc_completeness(stats),
        score_doc_quality(judge),
    ]
}

// --- Shared criteria ---

fn score_questions_before_action(stats: &EvalStats, max: u32) -> CriterionResult {
    let (score, details) = if stats.asked_questions_first {
        let q_turn = stats.first_question_turn.unwrap_or(0);
        let w_turn = stats
            .first_write_turn
            .map(|t| t.to_string())
            .unwrap_or("none".into());
        (
            max,
            format!("Asked questions at turn {q_turn} before write at turn {w_turn}"),
        )
    } else if stats.first_write_turn.is_none() && stats.question_count == 0 {
        (0, "No questions and no writes detected".into())
    } else {
        let w_turn = stats.first_write_turn.unwrap_or(0);
        let q_turn = stats
            .first_question_turn
            .map(|t| t.to_string())
            .unwrap_or("none".into());
        (
            0,
            format!("Wrote at turn {w_turn} before questions at turn {q_turn}"),
        )
    };

    CriterionResult {
        name: "questions_before_action".into(),
        score,
        max_score: max,
        details,
        feedback: None,
    }
}

fn score_question_quality(judge: Option<&JudgeResults>) -> CriterionResult {
    match judge {
        Some(j) => {
            // Scale judge's 0-100 to 0-15 (rounded)
            let score = ((j.question_quality.score * 15 + 50) / 100).min(15);
            CriterionResult {
                name: "question_quality".into(),
                score,
                max_score: 15,
                details: format!("Judge score: {}/100", j.question_quality.score),
                feedback: Some(j.question_quality.feedback.clone()),
            }
        }
        None => CriterionResult {
            name: "question_quality".into(),
            score: 0,
            max_score: 15,
            details: "Skipped (no judge)".into(),
            feedback: None,
        },
    }
}

fn score_doc_completeness(stats: &EvalStats) -> CriterionResult {
    // Score based on FIXME/TBD count: 0 = 10 pts, 1-2 = 7 pts, 3-5 = 4 pts, 6+ = 0 pts
    let any_doc = stats.opp_created || stats.pol_created || stats.adr_created || stats.inc_created;
    let (score, details) = if !any_doc {
        (0, "No docs created".into())
    } else {
        match stats.fixme_count {
            0 => (10, "No TBD/FIXME markers - complete docs".into()),
            1..=2 => (7, format!("{} TBD/FIXME markers", stats.fixme_count)),
            3..=5 => (4, format!("{} TBD/FIXME markers", stats.fixme_count)),
            _ => (0, format!("{} TBD/FIXME markers - too many gaps", stats.fixme_count)),
        }
    };

    CriterionResult {
        name: "doc_completeness".into(),
        score,
        max_score: 10,
        details,
        feedback: None,
    }
}

fn score_doc_quality(judge: Option<&JudgeResults>) -> CriterionResult {
    match judge.and_then(|j| j.doc_quality.as_ref()) {
        Some(dq) => {
            // Scale judge's 0-100 to 0-15 (rounded)
            let score = ((dq.score * 15 + 50) / 100).min(15);
            CriterionResult {
                name: "doc_quality".into(),
                score,
                max_score: 15,
                details: format!("Judge score: {}/100", dq.score),
                feedback: Some(dq.feedback.clone()),
            }
        }
        None => CriterionResult {
            name: "doc_quality".into(),
            score: 0,
            max_score: 15,
            details: "Skipped (no docs or no judge)".into(),
            feedback: None,
        },
    }
}

fn score_docs_created(stats: &EvalStats) -> CriterionResult {
    let mut score = 0u32;
    let mut parts = Vec::new();

    if stats.opp_created {
        score += 5;
        parts.push("OPP");
    }
    if stats.pol_created {
        score += 5;
        parts.push("POL");
    }
    if stats.adr_created {
        score += 5;
        parts.push("ADR");
    }
    if stats.inc_created {
        score += 5;
        parts.push("INC");
    }

    let details = if parts.is_empty() {
        "No docs created".into()
    } else {
        format!("Created: {}", parts.join(", "))
    };

    CriterionResult {
        name: "docs_created".into(),
        score,
        max_score: 20,
        details,
        feedback: None,
    }
}

fn score_cross_linking(stats: &EvalStats) -> CriterionResult {
    // (cross_link_count / 2) * 20, capped at 20
    let score = ((stats.cross_link_count as u32 * 20) / 2).min(20);
    let details = format!("{} cross-links (target: 2)", stats.cross_link_count);

    CriterionResult {
        name: "cross_linking".into(),
        score,
        max_score: 20,
        details,
        feedback: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::JudgeScore;
    use std::collections::HashMap;

    fn mock_stats() -> EvalStats {
        EvalStats {
            asked_questions_first: true,
            question_count: 4,
            questions: vec![
                "What problem?".into(),
                "Who benefits?".into(),
                "What audience?".into(),
                "What budget?".into(),
            ],
            first_question_turn: Some(0),
            first_write_turn: Some(2),
            ask_user_question_count: 0,
            ask_user_questions: Vec::new(),
            ask_user_answers: Vec::new(),
            tool_calls: Vec::new(),
            tool_call_count: 12,
            tool_error_count: 0,
            tool_counts_by_name: HashMap::new(),
            total_input_tokens: 1000,
            total_output_tokens: 500,
            total_cache_creation_tokens: 0,
            total_cache_read_tokens: 0,
            total_cost_usd: Some(0.05),
            duration_ms: Some(10000.0),
            num_turns: Some(4),
            opp_created: true,
            pol_created: false,
            adr_created: false,
            inc_created: false,
            cross_link_count: 0,
            fixme_count: 0,
            files_created: vec!["docs/opportunities/OPP-001.md".into()],
            doc_contents: None,
            assistant_text: "test".into(),
            timing: EvalTiming::default(),
        }
    }

    fn mock_judge() -> JudgeResults {
        JudgeResults {
            question_quality: JudgeScore {
                score: 80,
                feedback: "Good questions".into(),
            },
            doc_quality: Some(JudgeScore {
                score: 75,
                feedback: "Good docs".into(),
            }),
        }
    }

    #[test]
    fn clarify_max_score_is_100() {
        let stats = mock_stats();
        let judge = mock_judge();
        let criteria = score_criteria("selling-lama-milk", &stats, Some(&judge));
        let (_, max) = total_score(&criteria);
        assert_eq!(max, 100);
    }

    #[test]
    fn clarify_score_with_judge() {
        let stats = mock_stats();
        let judge = mock_judge();
        let criteria = score_criteria("selling-lama-milk", &stats, Some(&judge));
        let (score, _) = total_score(&criteria);
        // questions_before_action: 20, question_quality: 80*15/100=12,
        // docs_created: OPP only = 5, cross_linking: 0, doc_completeness: 10,
        // doc_quality: 75*15/100=11
        assert_eq!(score, 20 + 12 + 5 + 0 + 10 + 11);
    }

    #[test]
    fn clarify_no_judge_zeroes_judge_criteria() {
        let stats = mock_stats();
        let criteria = score_criteria("selling-lama-milk", &stats, None);
        let (score, _) = total_score(&criteria);
        // questions_before_action: 20, question_quality: 0,
        // docs_created: OPP only = 5, cross_linking: 0, doc_completeness: 10, doc_quality: 0
        assert_eq!(score, 20 + 0 + 5 + 0 + 10 + 0);
    }

    #[test]
    fn acp_max_score_is_100() {
        let stats = mock_stats();
        let judge = mock_judge();
        let criteria = score_criteria("accepting-crypto-payments", &stats, Some(&judge));
        let (_, max) = total_score(&criteria);
        assert_eq!(max, 100);
    }

    #[test]
    fn acp_all_docs_created() {
        let mut stats = mock_stats();
        stats.opp_created = true;
        stats.pol_created = true;
        stats.adr_created = true;
        stats.inc_created = true;
        stats.cross_link_count = 4;

        let judge = mock_judge();
        let criteria = score_criteria("accepting-crypto-payments", &stats, Some(&judge));
        let (score, _) = total_score(&criteria);
        // questions_before_action: 20, question_quality: 80*15/100=12,
        // docs_created: 5+5+5+5=20, cross_linking: (4*20/2)=40 -> capped 20,
        // doc_completeness: 10 (0 FIXMEs), doc_quality: 75*15/100=11
        assert_eq!(score, 20 + 12 + 20 + 20 + 10 + 11);
    }

    #[test]
    fn acp_no_docs_no_judge() {
        let mut stats = mock_stats();
        stats.opp_created = false;
        stats.pol_created = false;
        stats.adr_created = false;
        stats.cross_link_count = 0;

        let criteria = score_criteria("accepting-crypto-payments", &stats, None);
        let (score, _) = total_score(&criteria);
        // questions_before_action: 20, question_quality: 0,
        // docs_created: 0, cross_linking: 0, doc_completeness: 0 (no docs), doc_quality: 0
        assert_eq!(score, 20);
    }

    #[test]
    fn status_thresholds() {
        assert_eq!(status_from_score(80, 40, 70), "pass");
        assert_eq!(status_from_score(70, 40, 70), "pass");
        assert_eq!(status_from_score(50, 40, 70), "below_target");
        assert_eq!(status_from_score(40, 40, 70), "below_target");
        assert_eq!(status_from_score(30, 40, 70), "fail");
    }

    #[test]
    fn generic_max_score_is_100() {
        let stats = mock_stats();
        let judge = mock_judge();
        let criteria = score_criteria("any-scenario", &stats, Some(&judge));
        let (_, max) = total_score(&criteria);
        assert_eq!(max, 100);
    }

    #[test]
    fn generic_questions_before_action() {
        let mut stats = mock_stats();
        stats.asked_questions_first = true;
        stats.first_question_turn = Some(0);
        stats.first_write_turn = Some(2);
        let criteria = score_criteria("any-scenario", &stats, None);

        let qba_crit = criteria.iter().find(|c| c.name == "questions_before_action").unwrap();
        assert_eq!(qba_crit.score, 20);
    }

    #[test]
    fn generic_with_questions_and_docs() {
        let mut stats = mock_stats();
        stats.asked_questions_first = true;
        stats.adr_created = true;
        stats.pol_created = true;
        stats.inc_created = true;
        stats.cross_link_count = 2;
        let judge = mock_judge();
        let criteria = score_criteria("any-scenario", &stats, Some(&judge));
        let (score, _) = total_score(&criteria);

        // questions_before_action: 20, question_quality: 80*15/100=12,
        // docs_created: 5+5+5+5=20 (OPP+POL+ADR+INC), cross_linking: (2*20/2)=20,
        // doc_completeness: 10 (0 FIXMEs), doc_quality: 75*15/100=11
        assert_eq!(score, 20 + 12 + 20 + 20 + 10 + 11);
    }

    #[test]
    fn generic_docs_scoring() {
        let mut stats = mock_stats();

        // Only OPP = 5
        let criteria = score_criteria("any-scenario", &stats, None);
        let docs_crit = criteria.iter().find(|c| c.name == "docs_created").unwrap();
        assert_eq!(docs_crit.score, 5);

        // OPP + ADR = 5 + 5 = 10
        stats.adr_created = true;
        let criteria = score_criteria("any-scenario", &stats, None);
        let docs_crit = criteria.iter().find(|c| c.name == "docs_created").unwrap();
        assert_eq!(docs_crit.score, 10);

        // OPP + ADR + POL = 5 + 5 + 5 = 15
        stats.pol_created = true;
        let criteria = score_criteria("any-scenario", &stats, None);
        let docs_crit = criteria.iter().find(|c| c.name == "docs_created").unwrap();
        assert_eq!(docs_crit.score, 15);

        // OPP + ADR + POL + INC = 5 + 5 + 5 + 5 = 20
        stats.inc_created = true;
        let criteria = score_criteria("any-scenario", &stats, None);
        let docs_crit = criteria.iter().find(|c| c.name == "docs_created").unwrap();
        assert_eq!(docs_crit.score, 20);
    }
}

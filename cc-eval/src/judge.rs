use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::llm::LlmProvider;
use crate::scenario::ScenarioConfig;

/// Score from an LLM judge call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeScore {
    pub score: u32,
    pub feedback: String,
}

/// Results from all judge evaluations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeResults {
    pub question_quality: JudgeScore,
    pub doc_quality: Option<JudgeScore>,
}

const JUDGE_SYSTEM_PROMPT: &str =
    "You are an eval judge. Score 0-100. Return JSON only: {\"score\": N, \"feedback\": \"...\"}";

/// Run judge evaluations for a scenario using its config.
pub async fn run_judge(
    config: &ScenarioConfig,
    questions: &[String],
    doc_contents: Option<&str>,
    llm_provider: LlmProvider,
) -> Result<JudgeResults> {
    let question_quality = judge_question_quality(config, questions, llm_provider).await?;

    let doc_quality = match doc_contents {
        Some(contents) if !contents.trim().is_empty() => {
            Some(judge_doc_quality(config, contents, llm_provider).await?)
        }
        _ => None,
    };

    Ok(JudgeResults {
        question_quality,
        doc_quality,
    })
}

/// Judge question quality using the scenario's template prompt.
async fn judge_question_quality(
    config: &ScenarioConfig,
    questions: &[String],
    llm_provider: LlmProvider,
) -> Result<JudgeScore> {
    if questions.is_empty() {
        return Ok(JudgeScore {
            score: 0,
            feedback: "No questions were asked".into(),
        });
    }

    let questions_list = questions
        .iter()
        .enumerate()
        .map(|(i, q)| format!("{}. {}", i + 1, q))
        .collect::<Vec<_>>()
        .join("\n");

    // Use template from config, or fall back to default
    let judge_prompt = match &config.judge_question_prompt {
        Some(template) => expand_template(template, &config.prompt, &questions_list, None),
        None => default_question_judge_prompt(&config.prompt, &questions_list),
    };

    call_judge(&judge_prompt, llm_provider).await
}

/// Judge document quality using the scenario's template prompt.
async fn judge_doc_quality(
    config: &ScenarioConfig,
    doc_contents: &str,
    llm_provider: LlmProvider,
) -> Result<JudgeScore> {
    // Use template from config, or fall back to default
    let judge_prompt = match &config.judge_doc_prompt {
        Some(template) => expand_template(template, &config.prompt, "", Some(doc_contents)),
        None => default_doc_judge_prompt(&config.prompt, doc_contents),
    };

    call_judge(&judge_prompt, llm_provider).await
}

/// Expand template variables: {prompt}, {questions_list}, {doc_contents}
fn expand_template(
    template: &str,
    prompt: &str,
    questions_list: &str,
    doc_contents: Option<&str>,
) -> String {
    let mut result = template.replace("{prompt}", prompt);
    result = result.replace("{questions_list}", questions_list);
    if let Some(contents) = doc_contents {
        result = result.replace("{doc_contents}", contents);
    }
    result
}

/// Default question quality judge prompt.
fn default_question_judge_prompt(prompt: &str, questions_list: &str) -> String {
    format!(
        "The user asked: \"{prompt}\"\n\
         Claude asked these clarifying questions:\n{questions_list}\n\n\
         Score the questions on:\n\
         (1) relevance to understanding the problem (0-30)\n\
         (2) challenging assumptions about audience/business model (0-30)\n\
         (3) specificity — not generic filler questions (0-20)\n\
         (4) coverage — asked about different aspects (0-20)\n\n\
         Return: {{\"score\": N, \"feedback\": \"...\"}}"
    )
}

/// Default document quality judge prompt.
fn default_doc_judge_prompt(prompt: &str, doc_contents: &str) -> String {
    format!(
        "Claude created these documents in response to \"{prompt}\":\n\
         {doc_contents}\n\n\
         Score on:\n\
         (1) problem-focused title, not solution-focused (0-25)\n\
         (2) evidence/context captured (0-25)\n\
         (3) FIXMEs for gaps (0-25)\n\
         (4) proper frontmatter and structure (0-25)\n\n\
         Return: {{\"score\": N, \"feedback\": \"...\"}}"
    )
}

async fn call_judge(prompt: &str, llm_provider: LlmProvider) -> Result<JudgeScore> {
    let response = crate::llm::query(llm_provider, prompt, Some(JUDGE_SYSTEM_PROMPT))
        .await
        .context("judge query failed")?;

    parse_judge_response(&response)
}

/// Parse JSON from judge response. Tolerant of markdown code blocks.
pub fn parse_judge_response(response: &str) -> Result<JudgeScore> {
    // Try to extract JSON from response (may be wrapped in ```json...```)
    let json_str = extract_json(response);

    #[derive(Deserialize)]
    struct JudgeResponse {
        score: u32,
        feedback: String,
    }

    match serde_json::from_str::<JudgeResponse>(&json_str) {
        Ok(parsed) => Ok(JudgeScore {
            score: parsed.score.min(100),
            feedback: parsed.feedback,
        }),
        Err(e) => {
            // Fallback: try to find any JSON object in the response
            if let Some(fallback) = try_extract_json_object(response) {
                if let Ok(parsed) = serde_json::from_str::<JudgeResponse>(&fallback) {
                    return Ok(JudgeScore {
                        score: parsed.score.min(100),
                        feedback: parsed.feedback,
                    });
                }
            }
            Ok(JudgeScore {
                score: 0,
                feedback: format!("Failed to parse judge response: {e}"),
            })
        }
    }
}

/// Extract JSON content, stripping markdown code blocks if present.
fn extract_json(text: &str) -> String {
    let trimmed = text.trim();

    // Strip ```json ... ``` wrapper
    if let Some(rest) = trimmed.strip_prefix("```json") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim().to_string();
        }
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        if let Some(inner) = rest.strip_suffix("```") {
            return inner.trim().to_string();
        }
    }

    trimmed.to_string()
}

/// Try to find a JSON object { ... } in text.
fn try_extract_json_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end > start {
        Some(text[start..=end].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clean_json() {
        let resp = r#"{"score": 75, "feedback": "Good questions"}"#;
        let result = parse_judge_response(resp).unwrap();
        assert_eq!(result.score, 75);
        assert_eq!(result.feedback, "Good questions");
    }

    #[test]
    fn parse_json_in_code_block() {
        let resp = "```json\n{\"score\": 80, \"feedback\": \"Nice\"}\n```";
        let result = parse_judge_response(resp).unwrap();
        assert_eq!(result.score, 80);
    }

    #[test]
    fn parse_json_with_surrounding_text() {
        let resp = "Here is my evaluation:\n{\"score\": 60, \"feedback\": \"Ok\"}\nDone.";
        let result = parse_judge_response(resp).unwrap();
        assert_eq!(result.score, 60);
    }

    #[test]
    fn parse_garbage_returns_zero() {
        let resp = "I don't know how to score this";
        let result = parse_judge_response(resp).unwrap();
        assert_eq!(result.score, 0);
        assert!(result.feedback.contains("Failed to parse"));
    }

    #[test]
    fn score_capped_at_100() {
        let resp = r#"{"score": 150, "feedback": "Amazing"}"#;
        let result = parse_judge_response(resp).unwrap();
        assert_eq!(result.score, 100);
    }

    #[test]
    fn expand_template_replaces_vars() {
        let template = "User asked: {prompt}\nQuestions:\n{questions_list}";
        let result = expand_template(template, "Build a website", "1. What is the goal?", None);
        assert!(result.contains("Build a website"));
        assert!(result.contains("What is the goal?"));
    }

    #[test]
    fn expand_template_with_doc_contents() {
        let template = "Prompt: {prompt}\nDocs:\n{doc_contents}";
        let result = expand_template(template, "Test", "", Some("# Document\nContent here"));
        assert!(result.contains("Document"));
        assert!(result.contains("Content here"));
    }
}

//! Gemini API client for LLM answerer and judge.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Default Gemini model for answerer/judge tasks.
/// See: https://ai.google.dev/gemini-api/docs/models
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-3-flash-preview";

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<Content>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Content {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<Part>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Part {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: Content,
}

/// Query Gemini API with a prompt and optional system instruction.
pub async fn query(
    prompt: &str,
    system_prompt: Option<&str>,
    model: Option<&str>,
) -> Result<String> {
    let api_key = std::env::var("GEMINI_API_KEY")
        .or_else(|_| std::env::var("GOOGLE_API_KEY"))
        .context("GEMINI_API_KEY or GOOGLE_API_KEY environment variable not set")?;

    let model = model.unwrap_or(DEFAULT_GEMINI_MODEL);
    let url = format!("{GEMINI_API_BASE}/{model}:generateContent?key={api_key}");

    let request = GeminiRequest {
        contents: vec![Content {
            role: Some("user".into()),
            parts: vec![Part {
                text: prompt.to_string(),
            }],
        }],
        system_instruction: system_prompt.map(|s| Content {
            role: None,
            parts: vec![Part {
                text: s.to_string(),
            }],
        }),
    };

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .context("Gemini API request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Gemini API error {status}: {body}");
    }

    let gemini_response: GeminiResponse = response
        .json()
        .await
        .context("Failed to parse Gemini response")?;

    let text = gemini_response
        .candidates
        .first()
        .and_then(|c| c.content.parts.first())
        .map(|p| p.text.clone())
        .unwrap_or_default();

    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_model_is_set() {
        assert!(!DEFAULT_GEMINI_MODEL.is_empty());
    }
}

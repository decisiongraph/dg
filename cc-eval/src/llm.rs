//! LLM provider abstraction for answerer and judge.

use anyhow::{Context, Result};
use clap::ValueEnum;
use claude_code_rs::{ClaudeAgentOptions, PermissionMode};
use serde::{Deserialize, Serialize};

use crate::gemini;

/// LLM provider selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    /// Claude (via claude-code-rs)
    #[default]
    Claude,
    /// Gemini (via REST API)
    Gemini,
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmProvider::Claude => write!(f, "claude"),
            LlmProvider::Gemini => write!(f, "gemini"),
        }
    }
}

/// Query an LLM with a prompt and optional system prompt.
pub async fn query(
    provider: LlmProvider,
    prompt: &str,
    system_prompt: Option<&str>,
) -> Result<String> {
    match provider {
        LlmProvider::Claude => query_claude(prompt, system_prompt).await,
        LlmProvider::Gemini => gemini::query(prompt, system_prompt, None).await,
    }
}

/// Query Claude via claude-code-rs.
async fn query_claude(prompt: &str, system_prompt: Option<&str>) -> Result<String> {
    let options = ClaudeAgentOptions {
        system_prompt: system_prompt.map(|s| s.to_string()),
        model: Some("claude-sonnet-4-20250514".into()),
        max_turns: Some(1),
        permission_mode: PermissionMode::DenyAll,
        ..Default::default()
    };

    claude_code_rs::query_text(prompt, options)
        .await
        .context("Claude query failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_display() {
        assert_eq!(format!("{}", LlmProvider::Claude), "claude");
        assert_eq!(format!("{}", LlmProvider::Gemini), "gemini");
    }

    #[test]
    fn default_is_claude() {
        assert_eq!(LlmProvider::default(), LlmProvider::Claude);
    }
}

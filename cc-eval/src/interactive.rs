//! Interactive scenario infrastructure for testing multi-turn conversations.
//!
//! This module provides the ability to run eval scenarios where Claude asks
//! clarifying questions as text and receives LLM-generated answers.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use claude_code_rs::client::ClaudeSDKClient;
use claude_code_rs::{
    new_tool, permission_callback, ClaudeAgentOptions, ContentBlock, McpToolResult, Message,
    PermissionMode, PermissionResult, SdkMcpServer,
};
use serde::{Deserialize, Serialize};

use crate::eval::{analyze_messages, EvalStats};
use crate::llm::LlmProvider;
use crate::setup;

const ANSWERER_SYSTEM_PROMPT: &str = "You are a busy user answering questions from a consultant. \
RULES: \
1. Answer EACH question with EXACTLY 1 short sentence (max 10 words). \
2. Be slightly ambiguous - don't over-explain. \
3. Use casual language like a real person would. \
4. Don't ask follow-up questions. \
5. If asked about compliance/regulations/policies, mention them briefly. \
Example good answers: 'Around $5000 total.' 'Other farmers nearby.' 'Need cold storage for milk.'";

/// Configuration for LLM-based question answering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAnswerer {
    /// The original scenario prompt (provides context for answers).
    pub scenario_context: String,

    /// Domain context from the scenario's Answerer Context section.
    pub answerer_context: String,

    /// LLM provider to use for answering.
    #[serde(default)]
    pub provider: LlmProvider,
}

impl LlmAnswerer {
    /// Create a new LLM answerer with the scenario context.
    pub fn new(scenario_context: &str, answerer_context: Option<&str>) -> Self {
        Self {
            scenario_context: scenario_context.to_string(),
            answerer_context: answerer_context.unwrap_or("").to_string(),
            provider: LlmProvider::default(),
        }
    }

    /// Create answerer with specific LLM provider.
    pub fn with_provider(mut self, provider: LlmProvider) -> Self {
        self.provider = provider;
        self
    }

    /// Generate answers for questions using an LLM.
    pub async fn answer_questions(&self, questions: &[String]) -> Result<String> {
        if questions.is_empty() {
            return Ok("Please proceed with reasonable defaults.".into());
        }

        let questions_text = questions
            .iter()
            .enumerate()
            .map(|(i, q)| format!("{}. {}", i + 1, q))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "You are a user being asked clarifying questions about this request:\n\n\
             SCENARIO: {}\n\n\
             YOUR CONTEXT (don't volunteer, only answer what's asked):\n{}\n\n\
             QUESTIONS:\n{}\n\n\
             Answer each question in EXACTLY 1 short sentence. Be ambiguous where possible.",
            self.scenario_context, self.answerer_context, questions_text
        );

        crate::llm::query(self.provider, &prompt, Some(ANSWERER_SYSTEM_PROMPT)).await
    }
}

/// Build env HashMap with dg binary in PATH.
fn eval_env(workspace: &Path) -> HashMap<String, String> {
    let mut env = HashMap::new();
    // Add workspace .local/bin (symlinked dg) + cargo target dir to PATH
    let mut path_parts = Vec::new();
    let ws_bin = workspace.join(".local/bin");
    if ws_bin.is_dir() {
        path_parts.push(ws_bin.display().to_string());
    }
    if let Some(dg_path) = setup::path_with_dg() {
        path_parts.push(dg_path);
    } else {
        path_parts.push(std::env::var("PATH").unwrap_or_default());
    }
    env.insert("PATH".into(), path_parts.join(":"));
    env.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
    env.insert("GIT_ASKPASS".into(), "/bin/false".into());
    env
}

/// Extract questions from assistant text (lines ending with ?).
fn extract_text_questions(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            // Look for question lines (ending with ?) that are substantial
            if trimmed.ends_with('?') && trimmed.len() > 10 {
                Some(trimmed.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Run an interactive scenario using ClaudeSDKClient for multi-turn conversations.
///
/// This function handles text questions by detecting them in assistant responses
/// and generating LLM-based answers.
pub async fn run_interactive_scenario(
    prompt: &str,
    workspace: &Path,
    answerer_context: Option<&str>,
    max_turns: u32,
    model: Option<&str>,
    cli_path: Option<&Path>,
    llm_provider: LlmProvider,
) -> Result<EvalStats> {
    // System prompt to reinforce dg workflow - Claude tends to ignore CLAUDE.md and build actual code
    let system_prompt = dg_schemas::EVAL_SYSTEM_PROMPT;

    // Default permission mode: all tools available, routed through can_use_tool callback.
    // AskUserQuestion is answered at the protocol level via updatedInput.
    // EnterPlanMode/ExitPlanMode allowed — Claude may plan before creating docs.
    let answerer_for_cb = Arc::new(LlmAnswerer::new(prompt, answerer_context).with_provider(llm_provider));
    let tool_handler = permission_callback(move |input| {
        let answerer = answerer_for_cb.clone();
        async move {
            match input.tool_name.as_str() {
                "EnterPlanMode" => PermissionResult::allow(),
                "ExitPlanMode" => PermissionResult::allow_with_input(input.input.clone()),
                "AskUserQuestion" => {
                    // Extract questions and generate LLM answers
                    let mut questions = Vec::new();
                    if let Some(qs_arr) = input.input.get("questions").and_then(|v| v.as_array()) {
                        for q in qs_arr {
                            if let Some(text) = q.get("question").and_then(|v| v.as_str()) {
                                questions.push(text.to_string());
                            }
                        }
                    }

                    let answer_text = answerer
                        .answer_questions(&questions)
                        .await
                        .unwrap_or_else(|e| format!("Please proceed with defaults. (Error: {e})"));

                    // Build answers map keyed by string index: {"0": "...", "1": "..."}
                    let mut answers = serde_json::Map::new();
                    for (i, _) in questions.iter().enumerate() {
                        answers.insert(i.to_string(), serde_json::Value::String(answer_text.clone()));
                    }

                    // Return updatedInput with original questions + answers
                    let mut updated = input.input.clone();
                    updated["answers"] = serde_json::Value::Object(answers);
                    PermissionResult::allow_with_input(updated)
                }
                _ => {
                    // Pass through all other tools with original input
                    PermissionResult::allow()
                }
            }
        }
    });

    let options = ClaudeAgentOptions {
        cwd: Some(workspace.to_path_buf()),
        permission_mode: PermissionMode::AcceptAll,
        allowed_tools: vec![],
        can_use_tool: Some(tool_handler),
        max_turns: Some(max_turns),
        model: model.map(|m| m.to_string()),
        env: eval_env(workspace),
        cli_path: cli_path.map(|p| p.to_path_buf()),
        append_system_prompt: Some(system_prompt.to_string()),
        use_websocket: true,
        ..Default::default()
    };

    // Build in-process MCP server with ask_user tool (fallback for mcp__eval__ask_user)
    let answerer_for_mcp = Arc::new(LlmAnswerer::new(prompt, answerer_context).with_provider(llm_provider));
    let answerer_for_text = answerer_for_mcp.clone();
    let ask_user_tool = new_tool(
        "ask_user",
        "Ask the user clarifying questions. Pass an array of question strings.",
        serde_json::json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of questions to ask the user"
                }
            },
            "required": ["questions"]
        }),
        move |input| {
            let answerer = answerer_for_mcp.clone();
            async move {
                let questions: Vec<String> = input
                    .get("questions")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();

                match answerer.answer_questions(&questions).await {
                    Ok(answer) => McpToolResult::text(answer),
                    Err(e) => McpToolResult::text(format!(
                        "Please proceed with reasonable defaults. (Error: {e})"
                    )),
                }
            }
        },
    );
    let eval_server = SdkMcpServer::new(vec![ask_user_tool]);

    let mut client = ClaudeSDKClient::new(options);
    client
        .add_mcp_server("eval", eval_server)
        .context("failed to register eval MCP server")?;

    let api_start = Instant::now();
    client.connect(Some(prompt)).await.context("connect failed")?;
    eprintln!("  [sdk] connected, waiting for responses...");

    let mut all_messages: Vec<Message> = Vec::new();
    let mut turn = 0u32;

    // Progress counters
    let mut questions_asked = 0u32;
    let mut questions_answered = 0u32;
    let mut files_written = 0u32;
    let mut in_plan_mode = false;

    loop {
        turn += 1;
        let response = client
            .receive_response()
            .await
            .context("receive_response failed")?;

        if response.is_empty() {
            eprintln!("  [sdk] turn {turn}: empty response (possible connection issue)");
        }

        // Extract text from assistant messages and track tool usage
        // AskUserQuestion is now answered at the protocol level via updatedInput,
        // so we only need to detect text questions as fallback.
        let mut assistant_text = String::new();
        let mut has_other_tools = false;
        let mut turn_tools: Vec<&str> = Vec::new();

        for msg in &response {
            if let Message::Assistant { message } = msg {
                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            assistant_text.push_str(text);
                            assistant_text.push('\n');
                        }
                        ContentBlock::ToolUse { name, input, .. } => {
                            turn_tools.push(name);
                            match name.as_str() {
                                "AskUserQuestion" => {
                                    let q_count = input
                                        .get("questions")
                                        .and_then(|v| v.as_array())
                                        .map(|a| a.len() as u32)
                                        .unwrap_or(1);
                                    questions_asked += q_count;
                                    questions_answered += q_count;
                                }
                                "mcp__eval__ask_user" => {
                                    let q_count = input
                                        .get("questions")
                                        .and_then(|v| v.as_array())
                                        .map(|a| a.len() as u32)
                                        .unwrap_or(1);
                                    questions_asked += q_count;
                                    questions_answered += q_count;
                                }
                                "EnterPlanMode" => { in_plan_mode = true; }
                                "ExitPlanMode" => { in_plan_mode = false; }
                                "Write" => { files_written += 1; }
                                "Bash" => {
                                    let cmd = input
                                        .get("command")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    if cmd.starts_with("dg new") || cmd.contains("dg new") {
                                        files_written += 1;
                                    }
                                }
                                _ => {}
                            }
                            match name.as_str() {
                                "AskUserQuestion" | "mcp__eval__ask_user"
                                | "EnterPlanMode" | "ExitPlanMode" => {}
                                _ => { has_other_tools = true; }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Store all messages
        all_messages.extend(response.clone());

        // Check if we hit a Result message (end of conversation)
        let has_result = response.iter().find_map(|m| {
            if let Message::Result { result } = m {
                Some(result)
            } else {
                None
            }
        });
        let is_done = has_result.is_some();

        // Log compact progress line
        let plan_tag = if in_plan_mode { " [plan]" } else { "" };
        let tools_str = if turn_tools.is_empty() {
            "text".to_string()
        } else {
            turn_tools.join(", ")
        };
        eprintln!(
            "  [sdk] turn {turn}: Q={questions_asked} A={questions_answered} W={files_written}{plan_tag} ({tools_str})"
        );

        // Log Result details if present (helps diagnose early exits)
        if let Some(result) = has_result {
            let cost = result.total_cost_usd.map(|c| format!("${c:.4}")).unwrap_or("-".into());
            let turns = result.num_turns.map(|t| t.to_string()).unwrap_or("-".into());
            eprintln!("  [sdk] done: turns={turns} cost={cost} is_error={}", result.is_error);
            if let Some(ref error) = result.error {
                eprintln!("  [sdk] error: {error}");
            }
        }

        // If Claude asked text questions (no tools), answer them as fallback
        let text_questions = extract_text_questions(&assistant_text);
        if !text_questions.is_empty() && !has_other_tools {
            questions_asked += text_questions.len() as u32;

            let answer = answerer_for_text
                .answer_questions(&text_questions)
                .await
                .unwrap_or_else(|e| format!("Please proceed with defaults. (Error: {e})"));

            questions_answered += text_questions.len() as u32;

            client
                .query(&answer, None)
                .await
                .context("failed to send answer")?;

            continue;
        }

        if is_done {
            break;
        }
    }

    if let Err(e) = client.disconnect().await {
        eprintln!("  [sdk] disconnect error: {e}");
    }
    let api_time_ms = api_start.elapsed().as_secs_f64() * 1000.0;

    // Detect auth/connection failures: 0 assistant messages means Claude never ran
    let assistant_count = all_messages.iter().filter(|m| matches!(m, Message::Assistant { .. })).count();
    if assistant_count == 0 {
        // Extract error from Result message if present
        let error_detail = all_messages.iter().find_map(|m| {
            if let Message::Result { result } = m {
                if result.is_error {
                    return result.error.clone().or_else(|| Some("unknown error".into()));
                }
            }
            None
        });

        let msg = match error_detail {
            Some(err) => format!(
                "Claude returned 0 messages (auth/connection failure).\n\
                 Error: {err}\n\n\
                 To fix, either:\n\
                 1. Set ANTHROPIC_API_KEY environment variable, or\n\
                 2. Run: claude login"
            ),
            None => format!(
                "Claude returned 0 messages after {turn} turn(s) ({} raw messages).\n\
                 This usually means authentication failed silently.\n\n\
                 To fix, either:\n\
                 1. Set ANTHROPIC_API_KEY environment variable, or\n\
                 2. Run: claude login",
                all_messages.len()
            ),
        };
        anyhow::bail!(msg);
    }

    let mut stats = analyze_messages(&all_messages);
    stats.timing.api_time_ms = api_time_ms;

    // Analyze workspace using DocGraph
    let analysis_start = Instant::now();
    crate::eval::analyze_workspace_pub(workspace, &mut stats);
    stats.timing.analysis_time_ms = analysis_start.elapsed().as_secs_f64() * 1000.0;

    // Extract cost/turns/duration from result message
    if let Some(Message::Result { result: res }) = all_messages.last() {
        stats.total_cost_usd = res.total_cost_usd;
        stats.num_turns = res.num_turns;
        stats.duration_ms = res.duration_ms;
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llm_answerer_new() {
        let answerer = LlmAnswerer::new("Build me a website to sell lama milk", None);
        assert!(answerer.scenario_context.contains("lama milk"));
        assert_eq!(answerer.provider, LlmProvider::Claude);
    }

    #[test]
    fn llm_answerer_with_context() {
        let answerer = LlmAnswerer::new(
            "Build me a website",
            Some("You are a farmer with $5000 budget"),
        );
        assert!(answerer.answerer_context.contains("farmer"));
        assert!(answerer.answerer_context.contains("$5000"));
    }

    #[test]
    fn llm_answerer_with_provider() {
        let answerer = LlmAnswerer::new("Test", None).with_provider(LlmProvider::Gemini);
        assert_eq!(answerer.provider, LlmProvider::Gemini);
    }
}

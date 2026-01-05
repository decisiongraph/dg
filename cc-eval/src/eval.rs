use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use claude_code_rs::types::content::ToolResultContent;
use claude_code_rs::{ContentBlock, Message};
use serde::{Deserialize, Serialize};

use crate::llm::LlmProvider;
use crate::scenario::ScenarioConfig;

/// Record of a single tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub name: String,
    pub input_summary: String,
    pub is_error: bool,
}

/// Timing breakdown for profiling eval runs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvalTiming {
    /// Time waiting for Claude API responses (ms)
    pub api_time_ms: f64,
    /// Time analyzing workspace with DocGraph (ms)
    pub analysis_time_ms: f64,
}

/// Rich statistics from running an eval scenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalStats {
    // Behavioral
    pub asked_questions_first: bool,
    pub question_count: usize,
    pub questions: Vec<String>,
    pub first_question_turn: Option<usize>,
    pub first_write_turn: Option<usize>,

    // AskUserQuestion tool usage (added later, needs default)
    #[serde(default)]
    pub ask_user_question_count: usize,
    #[serde(default)]
    pub ask_user_questions: Vec<String>,
    #[serde(default)]
    pub ask_user_answers: Vec<String>,

    // Tool usage
    pub tool_calls: Vec<ToolCallRecord>,
    pub tool_call_count: usize,
    pub tool_error_count: usize,
    pub tool_counts_by_name: HashMap<String, usize>,

    // Token usage
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,

    // Cost & timing
    pub total_cost_usd: Option<f64>,
    pub duration_ms: Option<f64>,
    pub num_turns: Option<u32>,

    // Artifacts
    pub opp_created: bool,
    pub pol_created: bool,
    pub adr_created: bool,
    #[serde(default)]
    pub inc_created: bool,
    #[serde(default)]
    pub cross_link_count: usize,
    #[serde(default)]
    pub fixme_count: usize,
    pub files_created: Vec<String>,
    pub doc_contents: Option<String>,
    #[serde(default)]
    pub assistant_text: String,

    // Timing breakdown
    #[serde(default)]
    pub timing: EvalTiming,
}

/// Run a scenario using its configuration.
pub async fn run_scenario(
    config: &ScenarioConfig,
    workspace: &Path,
    llm_provider: LlmProvider,
) -> Result<EvalStats> {
    run_scenario_with_cli(config, workspace, None, llm_provider).await
}

/// Run a scenario with an optional custom CLI path (for sandbox wrapper).
/// All scenarios use interactive mode with LLM-generated answers.
pub async fn run_scenario_with_cli(
    config: &ScenarioConfig,
    workspace: &Path,
    cli_path: Option<&Path>,
    llm_provider: LlmProvider,
) -> Result<EvalStats> {
    use crate::interactive::run_interactive_scenario;

    run_interactive_scenario(
        &config.prompt,
        workspace,
        config.answerer_context.as_deref(),
        config.max_turns,
        config.model.as_deref(),
        cli_path,
        llm_provider,
    )
    .await
}

/// Analyze collected messages for question-asking behavior, tool usage, and tokens.
pub fn analyze_messages(messages: &[Message]) -> EvalStats {
    let mut first_question_turn: Option<usize> = None;
    let mut first_write_turn: Option<usize> = None;
    let mut question_count = 0usize;
    let mut questions = Vec::new();
    let mut tool_calls = Vec::new();
    let mut tool_error_count = 0usize;
    let mut tool_counts_by_name: HashMap<String, usize> = HashMap::new();
    let mut total_input_tokens = 0u64;
    let mut total_output_tokens = 0u64;
    let mut total_cache_creation_tokens = 0u64;
    let mut total_cache_read_tokens = 0u64;
    let mut assistant_text = String::new();

    // AskUserQuestion tracking
    let mut ask_user_question_count = 0usize;
    let mut ask_user_questions: Vec<String> = Vec::new();
    let mut ask_user_answers: Vec<String> = Vec::new();
    let mut pending_ask_user_ids: HashMap<String, usize> = HashMap::new();

    // Track pending tool use IDs to match with ToolResult errors
    let mut pending_tool_ids: HashMap<String, usize> = HashMap::new();

    // Track pending write tool IDs - we only count writes that succeed
    let mut pending_write_ids: HashMap<String, usize> = HashMap::new();

    for (i, msg) in messages.iter().enumerate() {
        match msg {
            Message::Assistant { message } => {
                // Accumulate token usage
                if let Some(usage) = &message.usage {
                    total_input_tokens += usage.input_tokens.unwrap_or(0);
                    total_output_tokens += usage.output_tokens.unwrap_or(0);
                    total_cache_creation_tokens +=
                        usage.cache_creation_input_tokens.unwrap_or(0);
                    total_cache_read_tokens += usage.cache_read_input_tokens.unwrap_or(0);
                }

                for block in &message.content {
                    match block {
                        ContentBlock::Text { text } => {
                            let q_lines = extract_questions(text);
                            if !q_lines.is_empty() && first_question_turn.is_none() {
                                first_question_turn = Some(i);
                            }
                            question_count += q_lines.len();
                            questions.extend(q_lines);

                            if !assistant_text.is_empty() {
                                assistant_text.push('\n');
                            }
                            assistant_text.push_str(text);
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            // Track write tools - we'll only count them if they succeed
                            if is_write_tool(name, input) {
                                pending_write_ids.insert(id.clone(), i);
                            }

                            // Track question-asking tools (AskUserQuestion + MCP ask_user)
                            if name == "AskUserQuestion" || name == "mcp__eval__ask_user" {
                                if first_question_turn.is_none() {
                                    first_question_turn = Some(i);
                                }

                                let idx = ask_user_questions.len();
                                pending_ask_user_ids.insert(id.clone(), idx);
                                ask_user_question_count += 1;

                                // Extract question text — two formats:
                                // AskUserQuestion: questions[].question (object array)
                                // mcp__eval__ask_user: questions[] (string array)
                                if let Some(questions_arr) = input.get("questions") {
                                    if let Some(arr) = questions_arr.as_array() {
                                        for q in arr {
                                            let q_text = q
                                                .get("question")
                                                .and_then(|v| v.as_str())
                                                .or_else(|| q.as_str());
                                            if let Some(text) = q_text {
                                                ask_user_questions.push(text.to_string());
                                                question_count += 1;
                                                questions.push(text.to_string());
                                            }
                                        }
                                    }
                                }
                            }

                            let input_str = input.to_string();
                            let input_summary = if input_str.len() > 200 {
                                format!("{}...", &input_str[..200])
                            } else {
                                input_str
                            };

                            let idx = tool_calls.len();
                            tool_calls.push(ToolCallRecord {
                                name: name.clone(),
                                input_summary,
                                is_error: false,
                            });

                            *tool_counts_by_name.entry(name.clone()).or_insert(0) += 1;
                            pending_tool_ids.insert(id.clone(), idx);
                        }
                        ContentBlock::ToolResult {
                            tool_use_id,
                            is_error,
                            content,
                        } => {
                            if *is_error {
                                tool_error_count += 1;
                                if let Some(&idx) = pending_tool_ids.get(tool_use_id) {
                                    if let Some(rec) = tool_calls.get_mut(idx) {
                                        rec.is_error = true;
                                    }
                                }
                            } else {
                                // Successful write tool - record first write turn
                                if let Some(&turn) = pending_write_ids.get(tool_use_id) {
                                    if first_write_turn.is_none() || first_write_turn > Some(turn) {
                                        first_write_turn = Some(turn);
                                    }
                                }
                            }

                            // Capture AskUserQuestion answers
                            if pending_ask_user_ids.contains_key(tool_use_id) {
                                let answer_text = extract_tool_result_text(content);
                                if !answer_text.is_empty() {
                                    ask_user_answers.push(answer_text);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Message::User { message } => {
                if let claude_code_rs::types::messages::UserContent::Blocks(blocks) =
                    &message.content
                {
                    for block in blocks {
                        if let ContentBlock::ToolResult {
                            tool_use_id,
                            is_error,
                            content,
                        } = block
                        {
                            if *is_error {
                                tool_error_count += 1;
                                if let Some(&idx) = pending_tool_ids.get(tool_use_id) {
                                    if let Some(rec) = tool_calls.get_mut(idx) {
                                        rec.is_error = true;
                                    }
                                }
                            } else {
                                // Successful write tool - record first write turn
                                if let Some(&turn) = pending_write_ids.get(tool_use_id) {
                                    if first_write_turn.is_none() || first_write_turn > Some(turn) {
                                        first_write_turn = Some(turn);
                                    }
                                }
                            }

                            // Capture AskUserQuestion answers
                            if pending_ask_user_ids.contains_key(tool_use_id) {
                                let answer_text = extract_tool_result_text(content);
                                if !answer_text.is_empty() {
                                    ask_user_answers.push(answer_text);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let asked_questions_first = match (first_question_turn, first_write_turn) {
        (Some(q), Some(w)) => q < w,
        (Some(_), None) => true,
        _ => false,
    };

    let tool_call_count = tool_calls.len();

    EvalStats {
        asked_questions_first,
        question_count,
        questions,
        first_question_turn,
        first_write_turn,
        ask_user_question_count,
        ask_user_questions,
        ask_user_answers,
        tool_calls,
        tool_call_count,
        tool_error_count,
        tool_counts_by_name,
        total_input_tokens,
        total_output_tokens,
        total_cache_creation_tokens,
        total_cache_read_tokens,
        total_cost_usd: None,
        duration_ms: None,
        num_turns: None,
        opp_created: false,
        pol_created: false,
        adr_created: false,
        inc_created: false,
        cross_link_count: 0,
        fixme_count: 0,
        files_created: Vec::new(),
        doc_contents: None,
        assistant_text,
        timing: EvalTiming::default(),
    }
}

/// Extract text from ToolResultContent.
fn extract_tool_result_text(content: &ToolResultContent) -> String {
    match content {
        ToolResultContent::Text(s) => s.clone(),
        ToolResultContent::Blocks(blocks) => {
            blocks
                .iter()
                .filter_map(|b| match b {
                    claude_code_rs::types::content::ToolResultBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

/// Extract question lines from text (lines ending `?` with len > 5).
fn extract_questions(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.ends_with('?') && trimmed.len() > 5 {
                Some(trimmed.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Check if tool call is a file-writing action.
fn is_write_tool(name: &str, input: &serde_json::Value) -> bool {
    match name {
        "Write" | "Edit" | "NotebookEdit" => true,
        "Bash" => {
            // Only count Bash as write if it's not a read-only command
            let cmd = input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            !is_readonly_bash(cmd)
        }
        _ => false,
    }
}

/// Check if a bash command is read-only (doesn't modify files).
fn is_readonly_bash(cmd: &str) -> bool {
    let cmd = cmd.trim_start();
    let readonly_prefixes = [
        "dg list", "dg get", "dg graph", "dg refs", "dg guide", "dg describe",
        "ls", "cat", "head", "tail", "find", "which", "pwd", "env",
        "grep", "rg", "ag", "fd",
    ];
    readonly_prefixes.iter().any(|p| cmd.starts_with(p))
}

/// Analyze the workspace using DocGraph to populate artifact stats (public for interactive module).
pub fn analyze_workspace_pub(workspace: &Path, stats: &mut EvalStats) {
    analyze_workspace(workspace, stats);
}

/// Analyze the workspace using DocGraph to populate artifact stats.
///
/// Uses md-db's graph builder to detect created docs and count cross-links
/// between different document types (same logic as `dg refs`).
fn analyze_workspace(workspace: &Path, stats: &mut EvalStats) {
    let schema_path = workspace.join(".dg/schema.kdl");
    let schema = match md_db::schema::Schema::from_file(&schema_path) {
        Ok(s) => s,
        Err(_) => return,
    };

    let graph = match md_db::graph::DocGraph::build(workspace, &schema) {
        Ok(g) => g,
        Err(_) => return,
    };

    // Check which doc types exist by looking at graph nodes
    // Detect type from doc_type field OR from ID prefix (OPP-, POL-, ADR-)
    for node in graph.nodes.values() {
        let doc_type = node.doc_type.as_deref().or_else(|| {
            // Infer type from ID prefix if not in frontmatter
            let prefix = node.id.split('-').next()?;
            match prefix.to_uppercase().as_str() {
                "OPP" => Some("opp"),
                "POL" => Some("pol"),
                "ADR" => Some("adr"),
                "INC" => Some("inc"),
                _ => None,
            }
        });
        match doc_type {
            Some("opp") => stats.opp_created = true,
            Some("pol") => stats.pol_created = true,
            Some("adr") => stats.adr_created = true,
            Some("inc") => stats.inc_created = true,
            _ => {}
        }
    }

    // Count cross-links: edges where source and target have different doc type prefixes
    for edge in &graph.edges {
        let from_type = edge.from.split('-').next().unwrap_or("");
        let to_type = edge.to.split('-').next().unwrap_or("");
        if !from_type.is_empty() && !to_type.is_empty() && from_type != to_type {
            stats.cross_link_count += 1;
        }
    }

    // Collect created file paths from graph nodes
    for node in graph.nodes.values() {
        if let Ok(rel) = node.path.strip_prefix(workspace) {
            stats.files_created.push(rel.display().to_string());
        }
    }

    // Capture doc contents for judge
    stats.doc_contents = read_doc_contents(workspace);

    // Count FIXME/TBD markers in documents
    if let Some(ref contents) = stats.doc_contents {
        stats.fixme_count = count_fixme_markers(contents);
    }
}

/// Count FIXME, TBD, TODO, and similar incomplete markers in text.
fn count_fixme_markers(text: &str) -> usize {
    let text_upper = text.to_uppercase();
    let markers = ["FIXME", "TBD", "TODO", "XXX", "[TBD]", "[FIXME]"];
    markers.iter().map(|m| text_upper.matches(m).count()).sum()
}

/// Read contents of all markdown files under docs/ for judge evaluation.
pub fn read_doc_contents(workspace: &Path) -> Option<String> {
    let docs_dir = workspace.join("docs");
    let mut contents = String::new();

    fn walk(dir: &Path, contents: &mut String) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, contents);
                } else if path.extension().is_some_and(|e| e == "md") {
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        if !contents.is_empty() {
                            contents.push_str("\n---\n");
                        }
                        contents.push_str(&format!(
                            "# {}\n{}",
                            path.file_name().unwrap_or_default().to_string_lossy(),
                            text
                        ));
                    }
                }
            }
        }
    }

    walk(&docs_dir, &mut contents);

    if contents.is_empty() {
        None
    } else {
        Some(contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_questions_basic() {
        let text = "What problem are you trying to solve?\nThis is a statement.\nWho benefits?";
        assert_eq!(extract_questions(text).len(), 2);
    }

    #[test]
    fn extract_questions_ignores_short() {
        let text = "Why?\nOk?";
        assert_eq!(extract_questions(text).len(), 0);
    }

    #[test]
    fn extract_questions_collects_text() {
        let text = "What is the goal?\nSome statement.\nWho is the audience?";
        let qs = extract_questions(text);
        assert_eq!(qs.len(), 2);
        assert_eq!(qs[0], "What is the goal?");
        assert_eq!(qs[1], "Who is the audience?");
    }

    #[test]
    fn is_write_tool_matches() {
        let empty = serde_json::json!({});
        assert!(is_write_tool("Write", &empty));
        assert!(is_write_tool("Edit", &empty));
        assert!(is_write_tool("NotebookEdit", &empty));
        assert!(!is_write_tool("Read", &empty));
        assert!(!is_write_tool("Glob", &empty));
    }

    #[test]
    fn is_write_tool_bash_readonly() {
        // Read-only bash commands should NOT count as writes
        assert!(!is_write_tool("Bash", &serde_json::json!({"command": "dg list"})));
        assert!(!is_write_tool("Bash", &serde_json::json!({"command": "dg get opp-001"})));
        assert!(!is_write_tool("Bash", &serde_json::json!({"command": "dg graph"})));
        assert!(!is_write_tool("Bash", &serde_json::json!({"command": "dg refs opp-001"})));
        assert!(!is_write_tool("Bash", &serde_json::json!({"command": "ls -la"})));
        assert!(!is_write_tool("Bash", &serde_json::json!({"command": "cat README.md"})));
        assert!(!is_write_tool("Bash", &serde_json::json!({"command": "which dg"})));
        assert!(!is_write_tool("Bash", &serde_json::json!({"command": "grep foo bar.txt"})));
    }

    #[test]
    fn is_write_tool_bash_writes() {
        // Write bash commands SHOULD count as writes
        assert!(is_write_tool("Bash", &serde_json::json!({"command": "dg new opp --title test"})));
        assert!(is_write_tool("Bash", &serde_json::json!({"command": "touch foo.txt"})));
        assert!(is_write_tool("Bash", &serde_json::json!({"command": "rm file.md"})));
        assert!(is_write_tool("Bash", &serde_json::json!({"command": "mv a b"})));
        assert!(is_write_tool("Bash", &serde_json::json!({"command": "echo foo > bar"})));
        // Empty command counts as write (safer default)
        assert!(is_write_tool("Bash", &serde_json::json!({})));
    }

    #[test]
    fn is_readonly_bash_works() {
        assert!(is_readonly_bash("dg list"));
        assert!(is_readonly_bash("  dg list")); // leading whitespace
        assert!(is_readonly_bash("ls -la"));
        assert!(is_readonly_bash("cat file.txt"));
        assert!(!is_readonly_bash("dg new opp"));
        assert!(!is_readonly_bash("touch file"));
        assert!(!is_readonly_bash(""));
    }

    #[test]
    fn analyze_questions_first() {
        let messages = vec![
            Message::Assistant {
                message: claude_code_rs::AssistantMessage {
                    id: None,
                    model: None,
                    content: vec![ContentBlock::Text {
                        text: "What problem are you solving?\nWho is the audience?".into(),
                    }],
                    stop_reason: None,
                    usage: None,
                    extra: serde_json::Value::Object(Default::default()),
                },
            },
            Message::Assistant {
                message: claude_code_rs::AssistantMessage {
                    id: None,
                    model: None,
                    content: vec![ContentBlock::ToolUse {
                        id: "tu_1".into(),
                        name: "Write".into(),
                        input: serde_json::json!({}),
                    }],
                    stop_reason: None,
                    usage: None,
                    extra: serde_json::Value::Object(Default::default()),
                },
            },
            // ToolResult confirming write succeeded
            Message::User {
                message: claude_code_rs::UserMessage {
                    id: None,
                    content: claude_code_rs::types::messages::UserContent::Blocks(vec![
                        ContentBlock::ToolResult {
                            tool_use_id: "tu_1".into(),
                            is_error: false,
                            content: claude_code_rs::types::content::ToolResultContent::Text(
                                "File written".into(),
                            ),
                        },
                    ]),
                    extra: serde_json::Value::Object(Default::default()),
                },
            },
        ];

        let result = analyze_messages(&messages);
        assert!(result.asked_questions_first);
        assert_eq!(result.question_count, 2);
        assert_eq!(result.questions.len(), 2);
        assert_eq!(result.first_question_turn, Some(0));
        assert_eq!(result.first_write_turn, Some(1));
        assert_eq!(result.tool_call_count, 1);
        assert_eq!(result.tool_counts_by_name.get("Write"), Some(&1));
    }

    #[test]
    fn analyze_writes_first() {
        let messages = vec![
            Message::Assistant {
                message: claude_code_rs::AssistantMessage {
                    id: None,
                    model: None,
                    content: vec![ContentBlock::ToolUse {
                        id: "tu_1".into(),
                        name: "Write".into(),
                        input: serde_json::json!({}),
                    }],
                    stop_reason: None,
                    usage: None,
                    extra: serde_json::Value::Object(Default::default()),
                },
            },
            // ToolResult confirming write succeeded
            Message::User {
                message: claude_code_rs::UserMessage {
                    id: None,
                    content: claude_code_rs::types::messages::UserContent::Blocks(vec![
                        ContentBlock::ToolResult {
                            tool_use_id: "tu_1".into(),
                            is_error: false,
                            content: claude_code_rs::types::content::ToolResultContent::Text(
                                "File written".into(),
                            ),
                        },
                    ]),
                    extra: serde_json::Value::Object(Default::default()),
                },
            },
            Message::Assistant {
                message: claude_code_rs::AssistantMessage {
                    id: None,
                    model: None,
                    content: vec![ContentBlock::Text {
                        text: "What problem are you solving?".into(),
                    }],
                    stop_reason: None,
                    usage: None,
                    extra: serde_json::Value::Object(Default::default()),
                },
            },
        ];

        let result = analyze_messages(&messages);
        assert!(!result.asked_questions_first);
    }

    #[test]
    fn analyze_token_accumulation() {
        let messages = vec![
            Message::Assistant {
                message: claude_code_rs::AssistantMessage {
                    id: None,
                    model: None,
                    content: vec![ContentBlock::Text {
                        text: "Hello".into(),
                    }],
                    stop_reason: None,
                    usage: Some(claude_code_rs::Usage {
                        input_tokens: Some(100),
                        output_tokens: Some(50),
                        cache_creation_input_tokens: Some(10),
                        cache_read_input_tokens: Some(20),
                        extra: serde_json::Value::Object(Default::default()),
                    }),
                    extra: serde_json::Value::Object(Default::default()),
                },
            },
            Message::Assistant {
                message: claude_code_rs::AssistantMessage {
                    id: None,
                    model: None,
                    content: vec![ContentBlock::Text {
                        text: "World".into(),
                    }],
                    stop_reason: None,
                    usage: Some(claude_code_rs::Usage {
                        input_tokens: Some(200),
                        output_tokens: Some(80),
                        cache_creation_input_tokens: None,
                        cache_read_input_tokens: Some(30),
                        extra: serde_json::Value::Object(Default::default()),
                    }),
                    extra: serde_json::Value::Object(Default::default()),
                },
            },
        ];

        let stats = analyze_messages(&messages);
        assert_eq!(stats.total_input_tokens, 300);
        assert_eq!(stats.total_output_tokens, 130);
        assert_eq!(stats.total_cache_creation_tokens, 10);
        assert_eq!(stats.total_cache_read_tokens, 50);
        assert!(stats.assistant_text.contains("Hello"));
        assert!(stats.assistant_text.contains("World"));
    }
}

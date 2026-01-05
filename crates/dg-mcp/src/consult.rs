//! Multi-LLM consultation tool — call Gemini or OpenAI from within the MCP server.
//!
//! Provides `dg-consult` (single query) and `dg-debate` (two models debate) tools.

use std::env;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

// ── Available models ────────────────────────────────────────────────────────

struct ModelInfo {
    id: &'static str,
    provider: Provider,
}

#[derive(Clone, Copy)]
enum Provider {
    Gemini,
    OpenAi,
}

const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "gemini-3-pro-preview",
        provider: Provider::Gemini,
    },
    ModelInfo {
        id: "gemini-2.5-pro",
        provider: Provider::Gemini,
    },
    ModelInfo {
        id: "gemini-2.5-flash",
        provider: Provider::Gemini,
    },
    ModelInfo {
        id: "gpt-5.2",
        provider: Provider::OpenAi,
    },
    ModelInfo {
        id: "o3",
        provider: Provider::OpenAi,
    },
];

// ── Public helpers ──────────────────────────────────────────────────────────

/// Returns true if at least one LLM API key is set in the environment.
pub fn is_available() -> bool {
    env::var("GEMINI_API_KEY").is_ok() || env::var("OPENAI_API_KEY").is_ok()
}

/// Returns true if debate mode is possible (need at least 2 models from different providers,
/// OR at least 2 models from the same provider).
pub fn is_debate_available() -> bool {
    available_models().len() >= 2
}

/// Build the JSON tool descriptor for `dg-consult`.
pub fn consult_descriptor() -> Value {
    let models = available_models();
    let default = default_model();
    let model_enum: Vec<Value> = models.iter().map(|m| json!(m.id)).collect();

    json!({
        "name": "dg-consult",
        "description": format!(
            "Consult another LLM for a second opinion or help with hard problems. \
             Available models: {}. Default: {}",
            models.iter().map(|m| m.id).collect::<Vec<_>>().join(", "),
            default.map(|m| m.id).unwrap_or("none"),
        ),
        "inputSchema": {
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "Your question or request for the consultant LLM"
                },
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "File paths to include as context"
                },
                "model": {
                    "type": "string",
                    "enum": model_enum,
                    "description": format!(
                        "LLM model to use (default: {})",
                        default.map(|m| m.id).unwrap_or("auto"),
                    ),
                }
            },
            "required": ["prompt"]
        }
    })
}

/// Build the JSON tool descriptor for `dg-debate`.
pub fn debate_descriptor() -> Value {
    let models = available_models();
    let model_enum: Vec<Value> = models.iter().map(|m| json!(m.id)).collect();

    json!({
        "name": "dg-debate",
        "description": "Have two LLMs debate an approach, then synthesize the best answer. \
                         Each model proposes independently, critiques the other, then you get \
                         a synthesis of both perspectives.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The question or design problem to debate"
                },
                "files": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "File paths to include as context"
                },
                "model_a": {
                    "type": "string",
                    "enum": model_enum.clone(),
                    "description": "First debater model (default: auto-pick from available)"
                },
                "model_b": {
                    "type": "string",
                    "enum": model_enum,
                    "description": "Second debater model (default: auto-pick from available)"
                },
                "rounds": {
                    "type": "integer",
                    "description": "Number of debate rounds (default: 1, max: 3)"
                }
            },
            "required": ["prompt"]
        }
    })
}

/// Handle a `dg-consult` tool call.
pub fn tool_consult(args: &Value) -> Result<Value> {
    let prompt = args
        .get("prompt")
        .and_then(|v| v.as_str())
        .context("'prompt' is required")?;

    let files = extract_files(args);
    let model_id = args.get("model").and_then(|v| v.as_str()).unwrap_or("");

    let model = if model_id.is_empty() {
        default_model().context("no LLM API keys configured")?
    } else {
        find_model(model_id)?
    };

    let full_prompt = build_prompt(prompt, &files)?;
    let response = call_model(model, &full_prompt)?;

    Ok(json!({
        "model": model.id,
        "response": response,
    }))
}

/// Handle a `dg-debate` tool call — structured multi-round debate between two models.
pub fn tool_debate(args: &Value) -> Result<Value> {
    let prompt = args
        .get("prompt")
        .and_then(|v| v.as_str())
        .context("'prompt' is required")?;

    let files = extract_files(args);
    let rounds = args
        .get("rounds")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .min(3) as usize;

    // Pick two different models
    let (model_a, model_b) = pick_debate_models(args)?;

    let context = build_prompt(prompt, &files)?;

    // Phase 1: Opening arguments (independent)
    let opening_prompt_a = format!(
        "{context}\n\n\
         Propose your implementation approach:\n\
         1. **Approach**: Describe your recommended approach in 2-3 sentences\n\
         2. **Key decisions**: List the main architectural/design decisions\n\
         3. **Trade-offs**: What are the pros and cons?\n\
         4. **Steps**: High-level implementation steps\n\n\
         Be specific and opinionated. Defend your choices."
    );

    let opening_a = call_model(model_a, &opening_prompt_a)?;

    let opening_prompt_b = format!(
        "{context}\n\n\
         Propose your implementation approach:\n\
         1. **Approach**: Describe your recommended approach in 2-3 sentences\n\
         2. **Key decisions**: List the main architectural/design decisions\n\
         3. **Trade-offs**: What are the pros and cons?\n\
         4. **Steps**: High-level implementation steps\n\n\
         Be specific and opinionated. Defend your choices."
    );

    let opening_b = call_model(model_b, &opening_prompt_b)?;

    let mut debate_log = vec![
        json!({
            "phase": "opening",
            "model": model_a.id,
            "response": opening_a,
        }),
        json!({
            "phase": "opening",
            "model": model_b.id,
            "response": opening_b,
        }),
    ];

    // Phase 2: Rebuttal rounds
    let mut last_a = opening_a;
    let mut last_b = opening_b.clone();

    for round in 1..=rounds {
        // Model A critiques Model B
        let rebuttal_prompt_a = format!(
            "Original question:\n{context}\n\n\
             Your opponent ({}) proposed:\n{last_b}\n\n\
             Provide your counter-argument:\n\
             1. **Critique**: Weaknesses in opponent's approach\n\
             2. **Defense**: Address weaknesses in your approach\n\
             3. **Concessions**: Good ideas from opponent worth adopting\n\
             4. **Updated position**: Your refined recommendation\n\n\
             Be constructive but thorough.",
            model_b.id,
        );
        let rebuttal_a = call_model(model_a, &rebuttal_prompt_a)?;
        debate_log.push(json!({
            "phase": format!("rebuttal_round_{round}"),
            "model": model_a.id,
            "response": rebuttal_a,
        }));

        // Model B critiques Model A
        let rebuttal_prompt_b = format!(
            "Original question:\n{context}\n\n\
             Your opponent ({}) proposed:\n{rebuttal_a}\n\n\
             Provide your counter-argument:\n\
             1. **Critique**: Weaknesses in opponent's approach\n\
             2. **Defense**: Address weaknesses in your approach\n\
             3. **Concessions**: Good ideas from opponent worth adopting\n\
             4. **Updated position**: Your refined recommendation\n\n\
             Be constructive but thorough.",
            model_a.id,
        );
        let rebuttal_b = call_model(model_b, &rebuttal_prompt_b)?;
        debate_log.push(json!({
            "phase": format!("rebuttal_round_{round}"),
            "model": model_b.id,
            "response": rebuttal_b,
        }));

        last_a = rebuttal_a;
        last_b = rebuttal_b;
    }

    // Phase 3: Synthesis — ask the stronger model to synthesize
    let synthesis_prompt = format!(
        "Two AI models debated the following question:\n{context}\n\n\
         {}'s final position:\n{last_a}\n\n\
         {}'s final position:\n{last_b}\n\n\
         Synthesize the best approach from both perspectives:\n\
         1. **Points of agreement**: Where did both models agree?\n\
         2. **Resolved disagreements**: For each disagreement, which approach is better and why?\n\
         3. **Final recommendation**: The synthesized best approach\n\
         4. **Implementation steps**: Concrete steps to implement\n\n\
         Be honest about which model had the stronger argument on each point.",
        model_a.id, model_b.id,
    );

    let synthesis = call_model(default_model().unwrap_or(model_a), &synthesis_prompt)?;

    Ok(json!({
        "model_a": model_a.id,
        "model_b": model_b.id,
        "rounds": rounds,
        "debate": debate_log,
        "synthesis": synthesis,
    }))
}

// ── Internal helpers ────────────────────────────────────────────────────────

fn extract_files(args: &Value) -> Vec<String> {
    args.get("files")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn find_model(id: &str) -> Result<&'static ModelInfo> {
    MODELS
        .iter()
        .find(|m| m.id == id)
        .with_context(|| format!("unknown model: {id}"))
}

fn available_models() -> Vec<&'static ModelInfo> {
    let has_gemini = env::var("GEMINI_API_KEY").is_ok();
    let has_openai = env::var("OPENAI_API_KEY").is_ok();
    MODELS
        .iter()
        .filter(|m| match m.provider {
            Provider::Gemini => has_gemini,
            Provider::OpenAi => has_openai,
        })
        .collect()
}

fn default_model() -> Option<&'static ModelInfo> {
    let models = available_models();
    models
        .iter()
        .find(|m| m.id == "gemini-3-pro-preview")
        .or(models.first())
        .copied()
}

fn pick_debate_models(args: &Value) -> Result<(&'static ModelInfo, &'static ModelInfo)> {
    let models = available_models();
    if models.len() < 2 {
        bail!("debate requires at least 2 available models (set both GEMINI_API_KEY and OPENAI_API_KEY)");
    }

    let a = if let Some(id) = args.get("model_a").and_then(|v| v.as_str()) {
        find_model(id)?
    } else {
        models[0]
    };

    let b = if let Some(id) = args.get("model_b").and_then(|v| v.as_str()) {
        find_model(id)?
    } else {
        // Pick a different model than A, preferring a different provider
        models
            .iter()
            .find(|m| m.id != a.id)
            .copied()
            .context("need at least 2 different models for debate")?
    };

    Ok((a, b))
}

fn build_prompt(prompt: &str, files: &[String]) -> Result<String> {
    if files.is_empty() {
        return Ok(prompt.to_string());
    }

    let mut parts = vec!["## Relevant Files\n".to_string()];
    for file_path in files {
        let path = Path::new(file_path);
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read file: {file_path}"))?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        parts.push(format!(
            "### File: {file_path}\n```{ext}\n{content}\n```\n"
        ));
    }
    parts.push(format!("## Question\n\n{prompt}"));

    Ok(parts.join("\n"))
}

fn call_model(model: &ModelInfo, prompt: &str) -> Result<String> {
    match model.provider {
        Provider::Gemini => call_gemini(model.id, prompt),
        Provider::OpenAi => call_openai(model.id, prompt),
    }
}

fn call_gemini(model: &str, prompt: &str) -> Result<String> {
    let api_key = env::var("GEMINI_API_KEY").context("GEMINI_API_KEY not set")?;
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={api_key}"
    );

    let body = json!({
        "contents": [{ "parts": [{ "text": prompt }] }],
        "generationConfig": {
            "maxOutputTokens": 8192,
        }
    });

    let body_bytes = serde_json::to_vec(&body).context("failed to serialize request body")?;

    let response: Value = ureq::post(&url)
        .header("Content-Type", "application/json")
        .send(body_bytes.as_slice())
        .context("Gemini API request failed")?
        .body_mut()
        .read_json()
        .context("failed to parse Gemini response JSON")?;

    // Extract text: candidates[0].content.parts[0].text
    let text = response
        .get("candidates")
        .and_then(|c: &Value| c.get(0))
        .and_then(|c: &Value| c.get("content"))
        .and_then(|c: &Value| c.get("parts"))
        .and_then(|p: &Value| p.get(0))
        .and_then(|p: &Value| p.get("text"))
        .and_then(|t: &Value| t.as_str())
        .unwrap_or("");

    if text.is_empty() {
        if let Some(error) = response.get("error") {
            bail!(
                "Gemini API error: {}",
                error
                    .get("message")
                    .and_then(|m: &Value| m.as_str())
                    .unwrap_or("unknown")
            );
        }
        bail!("Gemini returned empty response: {response}");
    }

    Ok(text.to_string())
}

fn call_openai(model: &str, prompt: &str) -> Result<String> {
    let api_key = env::var("OPENAI_API_KEY").context("OPENAI_API_KEY not set")?;

    let body = json!({
        "model": model,
        "messages": [
            { "role": "user", "content": prompt }
        ],
        "max_completion_tokens": 8192,
    });

    let body_bytes = serde_json::to_vec(&body).context("failed to serialize request body")?;

    let response: Value = ureq::post("https://api.openai.com/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {api_key}"))
        .send(body_bytes.as_slice())
        .context("OpenAI API request failed")?
        .body_mut()
        .read_json()
        .context("failed to parse OpenAI response JSON")?;

    // Extract text: choices[0].message.content
    let text = response
        .get("choices")
        .and_then(|c: &Value| c.get(0))
        .and_then(|c: &Value| c.get("message"))
        .and_then(|m: &Value| m.get("content"))
        .and_then(|t: &Value| t.as_str())
        .unwrap_or("");

    if text.is_empty() {
        if let Some(error) = response.get("error") {
            bail!(
                "OpenAI API error: {}",
                error
                    .get("message")
                    .and_then(|m: &Value| m.as_str())
                    .unwrap_or("unknown")
            );
        }
        bail!("OpenAI returned empty response: {response}");
    }

    Ok(text.to_string())
}

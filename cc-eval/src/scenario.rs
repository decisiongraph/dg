//! Scenario configuration loaded from markdown files.
//!
//! Each scenario is a markdown file in `cc-eval/scenarios/` with:
//! - YAML frontmatter: name, prompt, max_turns, model, expect
//! - H2 sections: Answerer Context, Judge prompts
//!
//! All scenarios run in interactive mode with LLM-generated answers.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use md_db::document::Document;
use md_db::schema::Schema;

/// Expected outcomes for test assertions.
#[derive(Debug, Clone, Default)]
pub struct Expectations {
    /// Claude should ask questions before any write operations.
    pub questions_first: bool,
    /// Minimum number of questions Claude should ask.
    pub min_questions: u32,
    /// Minimum number of tool calls.
    pub min_tool_calls: u32,
    /// At least one doc (OPP/POL/ADR) should be created.
    pub any_doc_created: bool,
}

/// Configuration for a single eval scenario, loaded from markdown.
#[derive(Debug, Clone)]
pub struct ScenarioConfig {
    pub name: String,
    pub prompt: String,
    pub max_turns: u32,
    /// Model to use. None = use Claude Code's default.
    pub model: Option<String>,
    /// Expected outcomes for test assertions.
    pub expect: Expectations,
    /// Domain context for LlmAnswerer (from "Answerer Context" section).
    pub answerer_context: Option<String>,
    /// Judge prompt template for question quality (from "Judge: Question Quality" section).
    pub judge_question_prompt: Option<String>,
    /// Judge prompt template for document quality (from "Judge: Document Quality" section).
    pub judge_doc_prompt: Option<String>,
    /// Fixture directory name to copy into workspace (from scenarios/fixtures/).
    pub fixtures: Option<String>,
}

impl ScenarioConfig {
    /// Load a scenario from a markdown file.
    pub fn from_file(path: &Path) -> Result<Self> {
        let doc = Document::from_file(path)
            .with_context(|| format!("failed to read scenario: {}", path.display()))?;

        let fm = doc
            .frontmatter
            .as_ref()
            .context("scenario missing frontmatter")?;

        // Extract required frontmatter fields
        let name = fm
            .get_display("name")
            .context("scenario missing 'name' in frontmatter")?;

        let prompt = fm
            .get_display("prompt")
            .context("scenario missing 'prompt' in frontmatter")?;

        let max_turns = fm
            .get_display("max_turns")
            .and_then(|s| s.parse().ok())
            .unwrap_or(15);

        let model = fm.get_display("model");

        // Parse expectations from frontmatter
        let expect = Expectations {
            questions_first: fm
                .get_display("expect.questions_first")
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
            min_questions: fm
                .get_display("expect.min_questions")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            min_tool_calls: fm
                .get_display("expect.min_tool_calls")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            any_doc_created: fm
                .get_display("expect.any_doc_created")
                .and_then(|s| s.parse().ok())
                .unwrap_or(false),
        };

        // Parse body for h2 sections
        let parsed = doc.parse_body();

        let answerer_context = parsed
            .find_section("Answerer Context")
            .map(|s| s.content.trim().to_string());

        let judge_question_prompt = parsed
            .find_section("Judge: Question Quality")
            .map(|s| s.content.trim().to_string());

        let judge_doc_prompt = parsed
            .find_section("Judge: Document Quality")
            .map(|s| s.content.trim().to_string());

        let fixtures = fm.get_display("fixtures");

        Ok(Self {
            name,
            prompt,
            max_turns,
            model,
            expect,
            answerer_context,
            judge_question_prompt,
            judge_doc_prompt,
            fixtures,
        })
    }
}

/// Load all scenarios from the scenarios directory.
/// Validates each scenario against schema.kdl before returning.
pub fn load_scenarios() -> Result<Vec<ScenarioConfig>> {
    let scenarios_dir = scenarios_dir();

    if !scenarios_dir.exists() {
        anyhow::bail!("scenarios directory not found: {}", scenarios_dir.display());
    }

    // Load schema if present (for validation)
    let schema_path = scenarios_dir.join("schema.kdl");
    let schema = if schema_path.exists() {
        Some(Schema::from_file(&schema_path)
            .with_context(|| format!("failed to load schema: {}", schema_path.display()))?)
    } else {
        None
    };

    let mut scenarios = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&scenarios_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "md")
        })
        .collect();

    // Sort alphabetically by filename
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let config = ScenarioConfig::from_file(&path)
            .with_context(|| format!("failed to load scenario: {}", path.display()))?;

        // Validate against schema if present
        if let Some(ref schema) = schema {
            validate_scenario(&config, schema, &path)?;
        }

        scenarios.push(config);
    }

    Ok(scenarios)
}

/// Validate a scenario config against the schema.
fn validate_scenario(config: &ScenarioConfig, schema: &Schema, path: &Path) -> Result<()> {
    let type_def = schema.get_type("scenario");
    if type_def.is_none() {
        // No scenario type in schema, skip validation
        return Ok(());
    }
    let type_def = type_def.unwrap();

    let mut errors = Vec::new();

    // Validate required fields
    for field in &type_def.fields {
        if !field.required {
            continue;
        }
        let value_present = match field.name.as_str() {
            "name" => !config.name.is_empty(),
            "prompt" => !config.prompt.is_empty(),
            "max_turns" => config.max_turns > 0,
            "model" => config.model.as_ref().is_some_and(|m| !m.is_empty()),
            _ => true,
        };
        if !value_present {
            errors.push(format!("missing required field '{}'", field.name));
        }
    }

    // Validate name pattern (alphanumeric with dashes)
    if !config.name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        errors.push(format!(
            "name '{}' must match pattern ^[a-z0-9-]+$",
            config.name
        ));
    }

    // Validate required sections
    for section in &type_def.sections {
        if !section.required {
            continue;
        }
        let section_present = match section.name.as_str() {
            "Answerer Context" => config.answerer_context.is_some(),
            "Judge: Question Quality" => config.judge_question_prompt.is_some(),
            "Judge: Document Quality" => config.judge_doc_prompt.is_some(),
            _ => true,
        };
        if !section_present {
            errors.push(format!("missing required section '{}'", section.name));
        }
    }

    if !errors.is_empty() {
        anyhow::bail!(
            "scenario validation failed for {}: {}",
            path.display(),
            errors.join("; ")
        );
    }

    Ok(()
    )
}

/// Get the scenarios directory path.
/// Tries cc-eval/scenarios first (when run from project root),
/// then scenarios (when run from cc-eval directory).
pub fn scenarios_dir() -> PathBuf {
    let from_root = PathBuf::from("cc-eval/scenarios");
    if from_root.exists() {
        return from_root;
    }
    let from_cceval = PathBuf::from("scenarios");
    if from_cceval.exists() {
        return from_cceval;
    }
    // Default to the from-root path for error messages
    from_root
}


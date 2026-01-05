//! `dg questions` — manage open questions across decision documents.

use std::path::Path;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use markdown_tui::RenderOptions;
use md_db::document::Document;
use md_db::questions;
use md_db::schema::Schema;

#[derive(Args)]
pub struct QuestionsArgs {
    #[command(subcommand)]
    pub action: Option<QuestionsAction>,

    /// Filter by document type
    #[arg(long = "type", short = 't', global = true)]
    pub doc_type: Option<String>,

    /// Include resolved (checked) questions
    #[arg(long, global = true)]
    pub all: bool,

    /// Output as JSON
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Subcommand)]
pub enum QuestionsAction {
    /// List open questions (default)
    List {
        /// Specific document ID (e.g. OPP-001)
        doc_id: Option<String>,
    },
    /// Add a new question
    Add {
        /// Document ID (e.g. OPP-001)
        doc_id: String,
        /// Question text (use **Label:** prefix for categorization)
        text: String,
    },
    /// Mark a question as resolved
    Done {
        /// Document ID
        doc_id: String,
        /// Text to match (label or substring)
        match_text: String,
    },
    /// Remove a question
    Remove {
        /// Document ID
        doc_id: String,
        /// Text to match (label or substring)
        match_text: String,
    },
    /// Replace a question's text
    Replace {
        /// Document ID
        doc_id: String,
        /// Text to match (label or substring)
        match_text: String,
        /// New question text
        new_text: String,
    },
}

pub fn run(root: &Path, schema: &Schema, args: &QuestionsArgs) -> Result<()> {
    match &args.action {
        None | Some(QuestionsAction::List { doc_id: None }) => {
            run_list(root, schema, args)
        }
        Some(QuestionsAction::List { doc_id: Some(id) }) => {
            run_list_single(root, schema, id, args)
        }
        Some(QuestionsAction::Add { doc_id, text }) => {
            run_add(root, schema, doc_id, text)
        }
        Some(QuestionsAction::Done { doc_id, match_text }) => {
            run_done(root, schema, doc_id, match_text)
        }
        Some(QuestionsAction::Remove { doc_id, match_text }) => {
            run_remove(root, schema, doc_id, match_text)
        }
        Some(QuestionsAction::Replace {
            doc_id,
            match_text,
            new_text,
        }) => run_replace(root, schema, doc_id, match_text, new_text),
    }
}

fn run_list(root: &Path, schema: &Schema, args: &QuestionsArgs) -> Result<()> {
    let results = questions::scan_questions(root, schema, args.doc_type.as_deref())
        .context("failed to scan questions")?;

    if args.json {
        let arr: Vec<serde_json::Value> = results
            .iter()
            .flat_map(|dq| {
                dq.questions
                    .iter()
                    .filter(|q| args.all || !q.done)
                    .map(|q| {
                        serde_json::json!({
                            "doc_id": dq.doc_id,
                            "title": dq.title,
                            "index": q.index,
                            "done": q.done,
                            "label": q.label,
                            "text": q.text,
                        })
                    })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return Ok(());
    }

    let mut rows: Vec<Vec<String>> = Vec::new();
    for dq in &results {
        for q in &dq.questions {
            if !args.all && q.done {
                continue;
            }
            let doc = format!(
                "{}: {}",
                dq.doc_id,
                dq.title.as_deref().unwrap_or("-")
            );
            let status = if q.done { "[x]" } else { "[ ]" };
            let label = q.label.as_deref().unwrap_or("-");
            rows.push(vec![
                doc,
                status.to_string(),
                label.to_string(),
                q.text.clone(),
            ]);
        }
    }

    if rows.is_empty() {
        println!("No open questions found.");
        return Ok(());
    }

    let width = crossterm::terminal::size()
        .map(|(cols, _)| cols as usize)
        .unwrap_or(80);
    let options = RenderOptions {
        width,
        ..Default::default()
    };
    let headers = &["Document", "Status", "Label", "Question"];
    let rendered = markdown_tui::render_table(headers, &rows, &options);
    print!("{rendered}");

    Ok(())
}

fn run_list_single(root: &Path, schema: &Schema, doc_id: &str, args: &QuestionsArgs) -> Result<()> {
    let (_, doc) = find_doc(root, schema, doc_id)?;
    let qs = questions::extract_questions(&doc);

    if args.json {
        let arr: Vec<serde_json::Value> = qs
            .iter()
            .filter(|q| args.all || !q.done)
            .map(|q| {
                serde_json::json!({
                    "index": q.index,
                    "done": q.done,
                    "label": q.label,
                    "text": q.text,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return Ok(());
    }

    let filtered: Vec<&questions::Question> = qs
        .iter()
        .filter(|q| args.all || !q.done)
        .collect();

    if filtered.is_empty() {
        println!("No open questions in {doc_id}.");
        return Ok(());
    }

    for q in &filtered {
        let check = if q.done { "[x]" } else { "[ ]" };
        println!("  {check} {}", q.text);
    }

    Ok(())
}

fn run_add(root: &Path, schema: &Schema, doc_id: &str, text: &str) -> Result<()> {
    let (path, mut doc) = find_doc(root, schema, doc_id)?;
    questions::add_question(&mut doc, text)?;
    doc.save().with_context(|| format!("failed to save {}", path.display()))?;
    println!("Added question to {doc_id}");
    Ok(())
}

fn run_done(root: &Path, schema: &Schema, doc_id: &str, match_text: &str) -> Result<()> {
    let (path, mut doc) = find_doc(root, schema, doc_id)?;
    questions::resolve_question(&mut doc, match_text)?;
    doc.save().with_context(|| format!("failed to save {}", path.display()))?;
    println!("Resolved question in {doc_id}");
    Ok(())
}

fn run_remove(root: &Path, schema: &Schema, doc_id: &str, match_text: &str) -> Result<()> {
    let (path, mut doc) = find_doc(root, schema, doc_id)?;
    questions::remove_question(&mut doc, match_text)?;
    doc.save().with_context(|| format!("failed to save {}", path.display()))?;
    println!("Removed question from {doc_id}");
    Ok(())
}

fn run_replace(
    root: &Path,
    schema: &Schema,
    doc_id: &str,
    match_text: &str,
    new_text: &str,
) -> Result<()> {
    let (path, mut doc) = find_doc(root, schema, doc_id)?;
    questions::replace_question(&mut doc, match_text, new_text)?;
    doc.save().with_context(|| format!("failed to save {}", path.display()))?;
    println!("Replaced question in {doc_id}");
    Ok(())
}

/// Find a document by ID (e.g. OPP-001 → docs/opportunities/opp-001.md).
fn find_doc(root: &Path, schema: &Schema, doc_id: &str) -> Result<(std::path::PathBuf, Document)> {
    let dg = md_db::graph::DocGraph::build(root, schema)?;
    let normalized = doc_id.to_uppercase();
    let node = dg
        .nodes
        .get(&normalized)
        .with_context(|| format!("document not found: {doc_id}"))?;
    let doc = Document::from_file(&node.path)
        .with_context(|| format!("failed to read {}", node.path.display()))?;
    Ok((node.path.clone(), doc))
}

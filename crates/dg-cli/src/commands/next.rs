//! `dg next` command — analyze project state and suggest the next logical action.
//!
//! Walks through a priority chain and outputs the single most important action:
//! 1. No docs → interview user about project purpose
//! 2. Docs but no code in services/apps → import or start fresh
//! 3. Pending tasks (non-terminal docs) → work on highest-priority one
//! 4. All caught up → maintenance suggestions
//!
//! When running in a non-interactive terminal (piped to an LLM agent like Claude),
//! outputs detailed action prompts with step-by-step instructions.

use std::io::IsTerminal;
use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;
use md_db::discovery;
use md_db::frontmatter::Frontmatter;
use md_db::schema::Schema;
use serde_json::json;

#[derive(Args)]
pub struct NextArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// A recommended next action with priority and context.
struct Recommendation {
    priority: &'static str,
    action: &'static str,
    /// Short human-readable details (for interactive terminals).
    details: String,
    /// Detailed LLM prompt with step-by-step instructions (for non-interactive).
    llm_prompt: String,
    doc_id: Option<String>,
    doc_status: Option<String>,
}

/// Statuses that are considered "done" — documents with these are hidden.
const DONE_STATUSES: &[&str] = &[
    "completed",
    "declined",
    "resolved",
    "implemented",
    "deprecated",
    "superseded",
    "rejected",
];

pub fn run(root: &Path, schema: &Schema, args: &NextArgs) -> Result<()> {
    let rec = analyze(root, schema)?;
    let is_interactive = std::io::stdout().is_terminal();

    if args.json {
        print_json(&rec);
    } else if is_interactive {
        print_text(&rec);
    } else {
        // Non-interactive = LLM agent (e.g. Claude, Gemini, Codex)
        print_llm_prompt(&rec);
    }

    Ok(())
}

fn analyze(root: &Path, schema: &Schema) -> Result<Recommendation> {
    // ── Check 1: Any typed docs at all? ─────────────────────────────────
    let all_docs = discover_typed_docs(root, schema)?;
    if all_docs.is_empty() {
        return Ok(Recommendation {
            priority: "high",
            action: "Interview user about project purpose",
            details: "You don't have any docs yet!\n\
                      You can create your first opportunity document with:\n\n\
                      $ dg new opp \"Your project idea here\""
                .to_string(),
            llm_prompt: "The project doesn't have any docs. Interview the user about the \
                        opportunities, requirements, user stories, policies and architecture. \
                        Create OPP documents with `dg new opp` to capture what you learn. \
                        If there are existing code repositories ask if they should be imported \
                        with `dg import`. When done run `dg next` again."
                .to_string(),
            doc_id: None,
            doc_status: None,
        });
    }

    // ── Check 2: Code exists in services/ or apps/? ─────────────────────
    let has_services = has_subdirs(&root.join("services"));
    let has_apps = has_subdirs(&root.join("apps"));
    if !has_services && !has_apps {
        let doc_summary = summarize_docs(&all_docs);
        return Ok(Recommendation {
            priority: "high",
            action: "Import code or start fresh",
            details: "You have docs but no code yet!\n\
                      Import an existing repo or start from scratch:\n\n\
                      $ dg import https://github.com/your/repo\n\n\
                      Or create a new service/app directory and start coding."
                .to_string(),
            llm_prompt: format!("\
The project has decision documents but no code in services/ or apps/ yet.

Existing documents:
{doc_summary}

Ask the user if they want to import existing repositories with `dg import <repo-url>` \
or start building from scratch. Read the existing documents with `dg show <ID>` to understand \
what needs to be built. After importing or creating initial code run `dg next` again."),
            doc_id: None,
            doc_status: None,
        });
    }

    // ── Check 3: Pending tasks (non-terminal docs)? ─────────────────────
    let pending: Vec<&DocInfo> = all_docs
        .iter()
        .filter(|d| {
            let status = d.status.as_deref().unwrap_or("");
            !DONE_STATUSES.contains(&status)
        })
        .collect();

    if !pending.is_empty() {
        let best = pending
            .iter()
            .find(|d| d.status.as_deref() == Some("in-progress"))
            .or_else(|| pending.first())
            .unwrap();

        let total_pending = pending.len();
        let in_progress_count = pending
            .iter()
            .filter(|d| d.status.as_deref() == Some("in-progress"))
            .count();

        let pending_list = pending
            .iter()
            .take(10)
            .map(|d| {
                format!(
                    "- **{}**: {} (status: {})",
                    d.id,
                    d.title.as_deref().unwrap_or("(untitled)"),
                    d.status.as_deref().unwrap_or("draft"),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        return Ok(Recommendation {
            priority: "high",
            action: "Work on pending task",
            details: format!(
                "You have work to do!\n\n\
                 Next up: {} — {}\n\
                 Status: {}\n\n\
                 {} pending document(s) total ({} in-progress).\n\
                 Read it with:\n\n\
                 $ dg show {}",
                best.id,
                best.title.as_deref().unwrap_or("(untitled)"),
                best.status.as_deref().unwrap_or("draft"),
                total_pending,
                in_progress_count,
                best.id,
            ),
            llm_prompt: format!("\
There are {total_pending} pending document(s) ({in_progress_count} in-progress).

Pending documents:
{pending_list}

Work on {best_id}: {best_title}. Read it with `dg show {best_id}`, implement its requirements, \
and add inline comments like `// {best_id}: explanation` in related source files. When done \
mark it completed with `dg set {best_id} status completed` and run `dg next` again.",
                best_id = best.id,
                best_title = best.title.as_deref().unwrap_or("(untitled)"),
            ),
            doc_id: Some(best.id.clone()),
            doc_status: best.status.clone(),
        });
    }

    // ── Check 4: All caught up — maintenance mode ───────────────────────
    let total_docs = all_docs.len();
    Ok(Recommendation {
        priority: "low",
        action: "Maintenance and improvement",
        details: format!(
            "All caught up! All {} documents are done and code-linked. 🎉\n\n\
             Some ideas for what to do next:\n\
             • Check for dependency updates and Dependabot PRs\n\
             • Research competitors and new opportunities\n\
             • Refactor and clean up the codebase",
            total_docs,
        ),
        llm_prompt: format!("\
All {total_docs} documents are completed and code-linked. The project is in good shape.

Look for maintenance work: check for open Dependabot PRs, look for outdated dependencies, \
use the dg-consult MCP tool to get feedback on the codebase and create new OPP/ADR/SPEC docs \
from insights, research similar products and suggest new opportunities to the user (don't \
create OPP docs without user approval), or refactor and DRY the codebase using dg-debate \
for architectural discussions. When done run `dg next` again."),
        doc_id: None,
        doc_status: None,
    })
}

// ── Output formatters ───────────────────────────────────────────────────────

fn print_text(rec: &Recommendation) {
    println!("{}", rec.details);
}

fn print_llm_prompt(rec: &Recommendation) {
    println!("{}", rec.llm_prompt);
}

fn print_json(rec: &Recommendation) {
    let is_interactive = std::io::stdout().is_terminal();
    let val = json!({
        "priority": rec.priority,
        "action": rec.action,
        "document": rec.doc_id,
        "status": rec.doc_status,
        "details": rec.details,
        "llm_prompt": if is_interactive { None } else { Some(&rec.llm_prompt) },
    });
    println!("{}", serde_json::to_string_pretty(&val).unwrap_or_default());
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn summarize_docs(docs: &[DocInfo]) -> String {
    docs.iter()
        .take(15)
        .map(|d| {
            format!(
                "- **{}** ({}): {} [{}]",
                d.id,
                d.doc_type.as_deref().unwrap_or("?"),
                d.title.as_deref().unwrap_or("(untitled)"),
                d.status.as_deref().unwrap_or("draft"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

struct DocInfo {
    id: String,
    title: Option<String>,
    doc_type: Option<String>,
    status: Option<String>,
}

/// Discover all typed documents and extract their key metadata.
fn discover_typed_docs(root: &Path, _schema: &Schema) -> Result<Vec<DocInfo>> {
    let filters: Vec<discovery::Filter> = Vec::new();
    let files = discovery::discover_files(root, None, &filters, false)
        .context("failed to discover files")?;

    let mut docs = Vec::new();
    for path in &files {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let fm_json = match Frontmatter::try_parse(&content) {
            Ok((Some(fm), _)) => fm.to_json(),
            _ => continue,
        };

        let doc_type = fm_json.get("type").and_then(|v| v.as_str());
        if doc_type.is_none() || doc_type.unwrap().is_empty() {
            continue;
        }

        let id = md_db::graph::path_to_id(path);
        let title = fm_json
            .get("title")
            .and_then(|v| v.as_str())
            .map(String::from);
        let status = fm_json
            .get("status")
            .and_then(|v| v.as_str())
            .map(String::from);
        docs.push(DocInfo {
            id,
            title,
            doc_type: doc_type.map(String::from),
            status,
        });
    }

    // Sort: in-progress first, then by ID
    docs.sort_by(|a, b| {
        let a_ip = a.status.as_deref() == Some("in-progress");
        let b_ip = b.status.as_deref() == Some("in-progress");
        b_ip.cmp(&a_ip).then(a.id.cmp(&b.id))
    });

    Ok(docs)
}

/// Check whether a directory exists and has at least one subdirectory.
fn has_subdirs(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .any(|e| e.path().is_dir()),
        Err(_) => false,
    }
}

//! Build a dynamic project status section for AI system prompts.
//!
//! Reuses `dg next` analysis logic to inject project state into the system
//! prompt so the AI doesn't need to run discovery commands at startup.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use md_db::discovery;
use md_db::frontmatter::Frontmatter;
use md_db::schema::Schema;
use md_db::users::{EntityKind, OrgConfig, UserDef, UserStatus};

/// Detected user info from environment.
pub struct DetectedUser {
    pub handle: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub technical: bool,
}

/// Return PATH with the current binary's directory prepended, only if not already present.
/// Ensures the same `dg` that launched this process is visible to child processes.
pub fn path_with_self() -> OsString {
    let current_path = std::env::var_os("PATH").unwrap_or_default();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let exe_dir = match exe_dir {
        Some(d) => d,
        None => return current_path,
    };

    // Skip if already in PATH
    let existing: Vec<PathBuf> = std::env::split_paths(&current_path).collect();
    if existing.iter().any(|p| p == &exe_dir) {
        return current_path;
    }

    let mut dirs = vec![exe_dir];
    dirs.extend(existing);
    std::env::join_paths(dirs).unwrap_or(current_path)
}

/// Terminal doc statuses (same as next.rs).
const DONE_STATUSES: &[&str] = &[
    "completed",
    "declined",
    "resolved",
    "implemented",
    "deprecated",
    "superseded",
    "rejected",
];

/// Detect current user from environment (USER, git config, gh auth).
pub fn detect_current_user() -> Option<DetectedUser> {
    let handle = std::env::var("USER").ok().filter(|h| !h.is_empty())?;
    let handle = handle.to_lowercase();

    let name = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());

    let email = std::process::Command::new("git")
        .args(["config", "user.email"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());

    // Has git config → technical. No git config → non-technical.
    // FIXME: Use `gh` to detect which languages the user has been recently working with
    // to know what kind of questions would be good.
    let technical = name.is_some() || email.is_some();

    Some(DetectedUser {
        handle,
        name,
        email,
        technical,
    })
}

/// Ensure user exists in org.kdl; return final technical_level (org.kdl overrides detection).
fn ensure_user_in_org(root: &Path, detected: &mut DetectedUser) {
    let org_path = root.join(".dg/org.kdl");

    let mut config = if org_path.is_file() {
        match OrgConfig::from_file(&org_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("dg: warning: failed to parse org.kdl: {e}");
                return;
            }
        }
    } else if root.join(".dg").is_dir() {
        // .dg/ exists but no org.kdl yet — start empty
        OrgConfig {
            users: Default::default(),
            teams: Default::default(),
            orgs: Default::default(),
            jira: Default::default(),
        }
    } else {
        return; // no .dg/ at all
    };

    let mut dirty = false;

    if let Some(existing) = config.users.get(&detected.handle) {
        // User exists — use stored `technical` if present (allows manual override)
        if let Some(stored) = existing.extra.get("technical") {
            detected.technical = stored == "true";
        } else {
            // No stored value — add it
            let user = config.users.get_mut(&detected.handle).unwrap();
            user.extra
                .insert("technical".into(), detected.technical.to_string());
            dirty = true;
        }
    } else {
        // New user — create entry
        let mut extra = BTreeMap::new();
        extra.insert("technical".into(), detected.technical.to_string());
        config.add_user(UserDef {
            handle: detected.handle.clone(),
            name: detected.name.clone(),
            title: None,
            email: detected.email.clone(),
            teams: vec![],
            org: None,
            status: UserStatus::Active,
            kind: EntityKind::Internal,
            extra,
        });
        dirty = true;
    }

    // Ensure @claude AI user exists
    if !config.users.contains_key("claude") {
        let mut extra = BTreeMap::new();
        extra.insert("kind".into(), "ai".into());
        config.add_user(UserDef {
            handle: "claude".into(),
            name: Some("Claude AI".into()),
            title: None,
            email: None,
            teams: vec![],
            org: None,
            status: UserStatus::Active,
            kind: EntityKind::Internal,
            extra,
        });
        dirty = true;
    }

    if dirty {
        if let Err(e) = config.save(&org_path) {
            eprintln!("dg: warning: failed to save org.kdl: {e}");
        }
    }
}

/// Build `## Current User` prompt section with role-specific LLM instructions.
fn build_user_section(user: &DetectedUser) -> String {
    let display_name = user
        .name
        .as_deref()
        .map(|n| format!(" ({n})"))
        .unwrap_or_default();

    if user.technical {
        format!(
            "## Current User\n\
             You are talking to @{handle}{display_name}, a technical user.\n\
             Ask about architecture decisions, technology choices, and implementation details when creating ADRs.",
            handle = user.handle,
        )
    } else {
        format!(
            "## Current User\n\
             You are talking to @{handle}{display_name}, a non-technical stakeholder.\n\
             At conversation start, ask: \"Would you like me to handle all technical decisions \
             using best practices, or would you prefer to be more involved in technology choices?\"\n\
             - If best practices: Make ALL technical decisions yourself (language, framework, architecture). \
             Only ask business questions: budget, timeline, uptime needs, scale, compliance.\n\
             - If more involved: Explain options in business terms (cost, speed, reliability). \
             Never ask \"Should I use X or Y?\"\n\
             NEVER ask about programming languages, frameworks, or low-level implementation details \
             unless the user requests it.\n\
             You are responsible for creating ADR content based on industry best practices.",
            handle = user.handle,
        )
    }
}

/// Build a `## Project Status` section for the system prompt.
/// Returns empty string if project root can't be found.
pub fn build_status_section() -> String {
    let root = match find_project_root() {
        Some(r) => r,
        None => return String::new(),
    };

    // Detect user and inject role section
    let mut out = String::new();
    if let Some(mut user) = detect_current_user() {
        ensure_user_in_org(&root, &mut user);
        out.push_str(&build_user_section(&user));
        out.push_str("\n\n");
    }

    let schema = load_schema(&root);
    let docs = match discover_typed_docs(&root, &schema) {
        Ok(d) => d,
        Err(_) => return out,
    };

    let has_services = has_subdirs(&root.join("services"));
    let has_apps = has_subdirs(&root.join("apps"));

    out.push_str("## Project Status (auto-detected, no need to run discovery commands)\n");

    if docs.is_empty() {
        out.push_str("This is a FRESH PROJECT with no documents yet. ");
        out.push_str("Skip file listing and discovery. ");
        out.push_str("Go straight to the Startup Interview steps above.\n");
        return out;
    }

    // Count by status
    let total = docs.len();
    let pending: Vec<&DocInfo> = docs
        .iter()
        .filter(|d| !DONE_STATUSES.contains(&d.status.as_deref().unwrap_or("")))
        .collect();
    let done_count = total - pending.len();
    let in_progress = pending
        .iter()
        .filter(|d| d.status.as_deref() == Some("in-progress"))
        .count();

    out.push_str(&format!(
        "{total} documents total ({done_count} done, {} pending, {in_progress} in-progress).\n",
        pending.len()
    ));

    if !has_services && !has_apps {
        out.push_str("No code in services/ or apps/ yet.\n");
    } else {
        if has_services {
            out.push_str("Has services/ directory with code.\n");
        }
        if has_apps {
            out.push_str("Has apps/ directory with code.\n");
        }
    }

    // List all docs compactly
    out.push_str("\nDocuments:\n");
    for d in &docs {
        out.push_str(&format!(
            "- {} ({}): {} [{}]\n",
            d.id,
            d.doc_type.as_deref().unwrap_or("?"),
            d.title.as_deref().unwrap_or("(untitled)"),
            d.status.as_deref().unwrap_or("draft"),
        ));
    }

    // Direct instruction based on state
    if !pending.is_empty() {
        let best = pending
            .iter()
            .find(|d| d.status.as_deref() == Some("in-progress"))
            .or_else(|| pending.first())
            .unwrap();
        out.push_str(&format!(
            "\nSuggested next action: work on {} — {}. Read with `dg show {}`.\n",
            best.id,
            best.title.as_deref().unwrap_or("(untitled)"),
            best.id,
        ));
    }

    // Show available doc types from schema
    let type_names: Vec<String> = schema
        .types
        .iter()
        .filter(|t| !t.singleton)
        .map(|t| {
            if t.aliases.is_empty() {
                t.name.clone()
            } else {
                format!("{} ({})", t.name, t.aliases.join("/"))
            }
        })
        .collect();
    if !type_names.is_empty() {
        out.push_str(&format!(
            "\nAvailable types: {} (run `dg describe type=<name>` for sections/fields)\n",
            type_names.join(", ")
        ));
    }

    out.push_str("\nDo NOT run `dg list`, `ls docs/`, or other discovery commands — the status above is current.");

    out
}

// ── Helpers (adapted from next.rs) ──────────────────────────────────────

pub fn find_project_root() -> Option<PathBuf> {
    let start = std::env::current_dir().ok()?;
    let mut dir = start;
    loop {
        if dir.join(".dg").is_dir() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn load_schema(root: &Path) -> Schema {
    let schema_path = root.join(".dg/schema.kdl");
    let content = std::fs::read_to_string(&schema_path)
        .unwrap_or_else(|_| dg_schemas::SCHEMA.to_string());
    let mut schema = Schema::from_str(&content).unwrap_or_else(|_| {
        Schema::from_str(dg_schemas::SCHEMA).expect("built-in schema must parse")
    });

    // Merge extensions
    let ext_path = root.join(".dg/schema-ext.kdl");
    if ext_path.is_file() {
        if let Ok(ext) = std::fs::read_to_string(&ext_path) {
            let _ = schema.merge_ext(&ext);
        }
    }

    schema
}

struct DocInfo {
    id: String,
    title: Option<String>,
    doc_type: Option<String>,
    status: Option<String>,
}

fn discover_typed_docs(root: &Path, _schema: &Schema) -> anyhow::Result<Vec<DocInfo>> {
    let files = discovery::discover_files(root, None, &[], false)?;
    let mut docs = Vec::new();

    for path in &files {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let fm = match Frontmatter::try_parse(&content) {
            Ok((Some(fm), _)) => fm.to_json(),
            _ => continue,
        };

        let doc_type = fm.get("type").and_then(|v| v.as_str());
        if doc_type.is_none() || doc_type.unwrap().is_empty() {
            continue;
        }

        docs.push(DocInfo {
            id: md_db::graph::path_to_id(path),
            title: fm.get("title").and_then(|v| v.as_str()).map(String::from),
            doc_type: doc_type.map(String::from),
            status: fm.get("status").and_then(|v| v.as_str()).map(String::from),
        });
    }

    docs.sort_by(|a, b| {
        let a_ip = a.status.as_deref() == Some("in-progress");
        let b_ip = b.status.as_deref() == Some("in-progress");
        b_ip.cmp(&a_ip).then(a.id.cmp(&b.id))
    });

    Ok(docs)
}

fn has_subdirs(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    match std::fs::read_dir(dir) {
        Ok(entries) => entries.filter_map(|e| e.ok()).any(|e| e.path().is_dir()),
        Err(_) => false,
    }
}

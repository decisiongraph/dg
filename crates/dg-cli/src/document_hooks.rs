//! External document hooks: `.dg/hooks/on_{create,update,delete}`.
//!
//! Hooks are notifications that run after a CLI command has written its
//! changes. They never roll back or fail the mutation; failures are warnings.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::{bail, Context, Result};
use md_db::document::Document;
use md_db::graph;
use md_db::schema::Schema;
use serde_json::{json, Value};

use crate::commands::Command;

/// A snapshot of DG documents keyed by their paths.
///
/// The raw source is kept so updates can be compared without relying on file
/// mtimes or the document cache.
type DocumentSnapshot = BTreeMap<PathBuf, String>;

#[derive(Clone, Copy)]
enum DocumentEvent {
    Create,
    Update,
    Delete,
}

impl DocumentEvent {
    const ALL: [Self; 3] = [Self::Create, Self::Update, Self::Delete];

    fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

/// Return whether a command can write, rename, or remove DG documents.
///
/// Keeping this list explicit avoids scanning every document for read-only
/// commands. Commands that add a new document-writing path must be added here.
fn command_may_change_documents(command: &Command) -> bool {
    match command {
        Command::New(_) | Command::Delete(_) => true,
        Command::Set(args) => !args.dry_run,
        Command::Fmt(args) => !args.dry_run && !args.check,
        Command::Renumber(args) => !args.dry_run,
        Command::Team(_) | Command::Generate(_) | Command::Import(_) => true,
        _ => false,
    }
}

/// Snapshot documents before a command runs, if it may mutate documents and at
/// least one hook is installed. `None` means no hooks need to be dispatched.
pub(crate) fn before_command(root: &Path, command: &Command) -> Option<DocumentSnapshot> {
    if !command_may_change_documents(command) || !any_hook_installed(root) {
        return None;
    }
    capture(root)
        .inspect_err(|error| {
            eprintln!("warning: failed to snapshot documents for hooks: {error:#}")
        })
        .ok()
}

/// Snapshot documents after a command ran and dispatch hooks for every change.
pub(crate) fn after_command(root: &Path, schema: &Schema, before: &DocumentSnapshot) {
    match capture(root) {
        Ok(after) => dispatch(root, schema, before, &after),
        Err(error) => {
            eprintln!("warning: failed to snapshot documents for hooks: {error:#}");
        }
    }
}

fn hook_path(root: &Path, event: DocumentEvent) -> PathBuf {
    root.join(".dg")
        .join("hooks")
        .join(format!("on_{}", event.as_str()))
}

fn any_hook_installed(root: &Path) -> bool {
    DocumentEvent::ALL
        .iter()
        .any(|event| is_executable(&hook_path(root, *event)))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Capture all Markdown documents under the project root.
fn capture(root: &Path) -> Result<DocumentSnapshot> {
    let files = md_db::discovery::discover_files(root, None, &[], false)?;
    Ok(files
        .into_iter()
        .filter_map(|path| {
            let raw = std::fs::read_to_string(&path).ok()?;
            Some((path, raw))
        })
        .collect())
}

/// Dispatch hooks for all document changes observed between two snapshots.
fn dispatch(root: &Path, schema: &Schema, before: &DocumentSnapshot, after: &DocumentSnapshot) {
    let paths: BTreeSet<&PathBuf> = before.keys().chain(after.keys()).collect();

    for path in paths {
        let old = before.get(path).map(String::as_str);
        let new = after.get(path).map(String::as_str);
        let event = match (old, new) {
            (None, Some(_)) => DocumentEvent::Create,
            (Some(_), None) => DocumentEvent::Delete,
            (Some(old), Some(new)) if old != new => DocumentEvent::Update,
            _ => continue,
        };
        // Only DG documents (with frontmatter) fire hooks, not READMEs etc.
        if !old.into_iter().chain(new).any(has_frontmatter) {
            continue;
        }

        let id = graph::path_to_id_with_schema(path, schema);
        let payload = payload(event, path, &id, old, new);
        if let Err(error) = run_hook(root, event, &id, &payload) {
            eprintln!(
                "warning: dg on_{} hook failed for {id}: {error:#}",
                event.as_str()
            );
        }
    }
}

fn has_frontmatter(raw: &str) -> bool {
    Document::from_str(raw).is_ok_and(|doc| doc.frontmatter.is_some())
}

fn payload(
    event: DocumentEvent,
    path: &Path,
    id: &str,
    before: Option<&str>,
    after: Option<&str>,
) -> Value {
    match event {
        DocumentEvent::Create => document_json(path, after.unwrap_or_default()),
        DocumentEvent::Delete => document_json(path, before.unwrap_or_default()),
        DocumentEvent::Update => {
            let before = before.unwrap_or_default();
            let after = after.unwrap_or_default();
            json!({
                "before": document_json(path, before),
                "after": document_json(path, after),
                "diff": update_diff(path, id, before, after),
            })
        }
    }
}

fn document_json(path: &Path, raw: &str) -> Value {
    match Document::from_str(raw) {
        Ok(mut doc) => {
            doc.path = Some(path.to_path_buf());
            doc.to_json()
        }
        Err(_) => json!({
            "path": path.display().to_string(),
            "raw": raw,
        }),
    }
}

fn update_diff(path: &Path, id: &str, before: &str, after: &str) -> Value {
    let Ok(mut diff) = md_db::diff::diff_documents(before, after) else {
        return Value::Null;
    };
    diff.path = Some(path.display().to_string());
    diff.id = Some(id.to_string());
    serde_json::to_value(diff).unwrap_or(Value::Null)
}

/// Run one hook with `<id> <event>` argv and `payload` JSON on stdin.
///
/// Missing or non-executable hooks are skipped silently. The hook path is
/// executed directly (no shell). Its stdout is discarded so it cannot corrupt
/// dg's own output; stderr is reported when the hook exits non-zero.
fn run_hook(root: &Path, event: DocumentEvent, id: &str, payload: &Value) -> Result<()> {
    let hook = hook_path(root, event);
    if !is_executable(&hook) {
        return Ok(());
    }
    // Resolve before changing the child cwd, so a relative --root such as
    // `project` doesn't become `project/project/...`.
    let executable = hook
        .canonicalize()
        .with_context(|| format!("cannot resolve {}", hook.display()))?;
    let payload = serde_json::to_vec(payload).context("cannot serialize payload")?;

    let mut child = ProcessCommand::new(&executable)
        .arg(id)
        .arg(event.as_str())
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to run {}", hook.display()))?;

    // Write stdin on a separate thread so a hook that fills its stderr pipe
    // before reading stdin can't deadlock us.
    let stdin = child.stdin.take();
    let writer = std::thread::spawn(move || match stdin {
        Some(mut stdin) => stdin.write_all(&payload),
        None => Ok(()),
    });
    let output = child
        .wait_with_output()
        .with_context(|| format!("failed waiting for {}", hook.display()))?;

    match writer.join() {
        // Hooks that ignore stdin may exit before reading it; that's fine.
        Ok(Err(error)) if error.kind() != ErrorKind::BrokenPipe => {
            return Err(error).context("failed to write hook stdin");
        }
        Err(_) => bail!("hook stdin writer thread panicked"),
        _ => {}
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        if stderr.is_empty() {
            bail!("{} exited with {}", hook.display(), output.status);
        }
        bail!("{} exited with {}: {stderr}", hook.display(), output.status);
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn schema() -> Schema {
        Schema::from_str(dg_schemas::SCHEMA).expect("built-in schema must parse")
    }

    fn install_hook(root: &Path, event: &str, output: &Path) {
        let hooks = root.join(".dg/hooks");
        fs::create_dir_all(&hooks).unwrap();
        let script = hooks.join(format!("on_{event}"));
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s\\n%s\\n' \"$1\" \"$2\" > '{}'\nprintf '\\n--PAYLOAD--\\n' >> '{}'\ncat >> '{}'\n",
                output.display(),
                output.display(),
                output.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(script, permissions).unwrap();
    }

    fn doc(path: &Path, title: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!(
                "---\ntype: adr\ntitle: {title}\nstatus: proposed\nauthor: alice\ndate: 2026-01-01\n---\n\n# Context\n\n{title}\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn update_hook_receives_arguments_and_structured_diff() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let path = root.join("docs/architecture/adr-001-first.md");
        let output = root.join("hook-output");
        doc(&path, "Before");
        install_hook(root, "update", &output);

        let before = capture(root).unwrap();
        doc(&path, "After");
        let after = capture(root).unwrap();
        dispatch(root, &schema(), &before, &after);

        let output = fs::read_to_string(output).unwrap();
        let (args, payload) = output.split_once("\n--PAYLOAD--\n").unwrap();
        assert_eq!(args, "ADR-001\nupdate\n");
        let payload: Value = serde_json::from_str(payload).unwrap();
        assert_eq!(payload["before"]["frontmatter"]["title"], "Before");
        assert_eq!(payload["after"]["frontmatter"]["title"], "After");
        assert_eq!(payload["diff"]["id"], "ADR-001");
        assert!(!payload["diff"]["field_changes"]
            .as_array()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn create_and_delete_hooks_receive_document_json() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let path = root.join("docs/architecture/adr-001-first.md");
        let create_output = root.join("create-output");
        let delete_output = root.join("delete-output");
        install_hook(root, "create", &create_output);
        install_hook(root, "delete", &delete_output);

        let before = capture(root).unwrap();
        doc(&path, "Created");
        let after = capture(root).unwrap();
        dispatch(root, &schema(), &before, &after);

        let create = fs::read_to_string(&create_output).unwrap();
        let (args, payload) = create.split_once("\n--PAYLOAD--\n").unwrap();
        assert_eq!(args, "ADR-001\ncreate\n");
        let create_json: Value = serde_json::from_str(payload).unwrap();
        assert_eq!(create_json["frontmatter"]["title"], "Created");

        let before = after;
        fs::remove_file(&path).unwrap();
        let after = capture(root).unwrap();
        dispatch(root, &schema(), &before, &after);

        let delete = fs::read_to_string(&delete_output).unwrap();
        let (args, payload) = delete.split_once("\n--PAYLOAD--\n").unwrap();
        assert_eq!(args, "ADR-001\ndelete\n");
        let delete_json: Value = serde_json::from_str(payload).unwrap();
        assert_eq!(delete_json["frontmatter"]["title"], "Created");
    }

    #[test]
    fn hook_failures_do_not_fail_dispatch() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let hooks = root.join(".dg/hooks");
        fs::create_dir_all(&hooks).unwrap();
        let script = hooks.join("on_create");
        fs::write(&script, "#!/bin/sh\nexit 7\n").unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();

        let path = root.join("docs/architecture/adr-001-first.md");
        let before = capture(root).unwrap();
        doc(&path, "Created");
        let after = capture(root).unwrap();
        dispatch(root, &schema(), &before, &after);
    }
}

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

use anyhow::Result;
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
pub(crate) fn command_may_change_documents(command: &Command) -> bool {
    match command {
        Command::New(_) | Command::Delete(_) => true,
        Command::Set(args) => !args.dry_run,
        Command::Fmt(args) => !args.dry_run && !args.check,
        Command::Renumber(args) => !args.dry_run,
        Command::Team(_) | Command::Generate(_) | Command::Import(_) => true,
        _ => false,
    }
}

/// Capture all Markdown documents under the project root.
pub(crate) fn capture(root: &Path) -> Result<DocumentSnapshot> {
    let files = md_db::discovery::discover_files(root, None, &[], false)?;
    let mut snapshot = BTreeMap::new();

    for path in files {
        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        snapshot.insert(path, raw);
    }

    Ok(snapshot)
}

/// Dispatch hooks for all document changes observed between two snapshots.
///
/// Hook failures are warnings rather than command failures: the document write
/// already happened, and hooks are notifications rather than transaction
/// participants.
pub(crate) fn dispatch(
    root: &Path,
    schema: &Schema,
    before: &DocumentSnapshot,
    after: &DocumentSnapshot,
) {
    let mut paths = BTreeSet::new();
    paths.extend(before.keys().cloned());
    paths.extend(after.keys().cloned());

    for path in paths {
        let old = before.get(&path);
        let new = after.get(&path);
        let event = match (old, new) {
            (None, Some(_)) => DocumentEvent::Create,
            (Some(_), None) => DocumentEvent::Delete,
            (Some(old), Some(new)) if old != new => DocumentEvent::Update,
            _ => continue,
        };

        let id = graph::path_to_id_with_schema(&path, schema);
        let payload = payload(event, &path, &id, old, new);
        run_hook(root, event, &id, &payload);
    }
}

fn payload(
    event: DocumentEvent,
    path: &Path,
    id: &str,
    before: Option<&String>,
    after: Option<&String>,
) -> Value {
    match event {
        DocumentEvent::Create => after
            .map(|raw| document_json(path, raw))
            .unwrap_or(Value::Null),
        DocumentEvent::Delete => before
            .map(|raw| document_json(path, raw))
            .unwrap_or(Value::Null),
        DocumentEvent::Update => {
            let before_raw = before.map(String::as_str).unwrap_or("");
            let after_raw = after.map(String::as_str).unwrap_or("");
            let before_json = document_json(path, before_raw);
            let after_json = document_json(path, after_raw);
            let diff = update_diff(path, id, before_raw, after_raw);

            json!({
                "before": before_json,
                "after": after_json,
                "diff": diff,
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
    let mut diff = match md_db::diff::diff_documents(before, after) {
        Ok(diff) => diff,
        Err(_) => return Value::Null,
    };
    diff.path = Some(path.display().to_string());
    diff.id = Some(id.to_string());

    match serde_json::to_value(diff) {
        Ok(value) => value,
        Err(_) => Value::Null,
    }
}

fn run_hook(root: &Path, event: DocumentEvent, id: &str, payload: &Value) {
    let event_name = event.as_str();
    let hook_path = root
        .join(".dg")
        .join("hooks")
        .join(format!("on_{event_name}"));
    if !hook_path.is_file() {
        return;
    }

    // Resolve the executable before changing the child working directory. This
    // keeps --root values such as `project` from becoming `project/project/...`.
    let executable = match hook_path.canonicalize() {
        Ok(path) => path,
        Err(error) => {
            eprintln!(
                "warning: cannot resolve dg {event_name} hook {}: {error}",
                hook_path.display()
            );
            return;
        }
    };

    let payload = match serde_json::to_vec(payload) {
        Ok(payload) => payload,
        Err(error) => {
            eprintln!(
                "warning: cannot serialize payload for dg {event_name} hook {}: {error}",
                hook_path.display()
            );
            return;
        }
    };

    let mut child = match ProcessCommand::new(&executable)
        .arg(id)
        .arg(event_name)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!(
                "warning: failed to run dg {event_name} hook {}: {error}",
                hook_path.display()
            );
            return;
        }
    };

    let stdin = child.stdin.take();
    let writer = std::thread::spawn(move || stdin.map(|mut stdin| stdin.write_all(&payload)));
    let output = child.wait_with_output();
    let write_result = match writer.join() {
        Ok(result) => result,
        Err(_) => Some(Err(std::io::Error::other(
            "hook stdin writer thread panicked",
        ))),
    };

    if let Some(Err(error)) = write_result {
        eprintln!(
            "warning: failed to provide stdin to dg {event_name} hook {}: {error}",
            hook_path.display()
        );
    }

    match output {
        Ok(output) if !output.status.success() => {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if detail.is_empty() {
                eprintln!(
                    "warning: dg {event_name} hook {} exited with {}",
                    hook_path.display(),
                    output.status
                );
            } else {
                eprintln!(
                    "warning: dg {event_name} hook {} exited with {}: {detail}",
                    hook_path.display(),
                    output.status
                );
            }
        }
        Ok(_) => {}
        Err(error) => {
            eprintln!(
                "warning: failed waiting for dg {event_name} hook {}: {error}",
                hook_path.display()
            );
        }
    }
}

#[cfg(test)]
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

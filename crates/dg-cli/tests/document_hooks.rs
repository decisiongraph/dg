#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn setup_project() -> TempDir {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join(".dg/hooks")).unwrap();
    fs::write(
        temp.path().join(".dg/schema.kdl"),
        include_str!("../../dg-schemas/schema.kdl"),
    )
    .unwrap();
    temp
}

fn write_hook(root: &Path, event: &str, body: &str, mode: u32) {
    let script = root.join(".dg/hooks").join(format!("on_{event}"));
    fs::write(&script, format!("#!/bin/sh\n{body}")).unwrap();
    fs::set_permissions(script, fs::Permissions::from_mode(mode)).unwrap();
}

/// Hook that records argv (one per line) followed by stdin into `output`.
fn install_hook(root: &Path, event: &str, output: &Path) {
    let out = output.display();
    write_hook(
        root,
        event,
        &format!("printf '%s\\n%s\\n' \"$1\" \"$2\" > '{out}'\ncat >> '{out}'\n"),
        0o755,
    );
}

fn install_failing_hook(root: &Path, event: &str) {
    write_hook(root, event, "exit 7\n", 0o755);
}

fn run_dg(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dg"))
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

fn hook_record(path: &Path) -> (String, String, Value) {
    let record = fs::read_to_string(path).unwrap();
    let mut lines = record.splitn(3, '\n');
    let id = lines.next().unwrap().to_string();
    let event = lines.next().unwrap().to_string();
    let payload = serde_json::from_str(lines.next().unwrap()).unwrap();
    (id, event, payload)
}

#[test]
fn cli_dispatches_create_update_and_delete_hooks() {
    let project = setup_project();
    let create_record = project.path().join("create-record");
    let update_record = project.path().join("update-record");
    let delete_record = project.path().join("delete-record");
    install_hook(project.path(), "create", &create_record);
    install_hook(project.path(), "update", &update_record);
    install_hook(project.path(), "delete", &delete_record);

    let created = run_dg(project.path(), &["new", "adr", "A hook-enabled decision"]);
    assert!(
        created.status.success(),
        "dg new failed: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let (id, event, payload) = hook_record(&create_record);
    assert_eq!(id, "ADR-001");
    assert_eq!(event, "create");
    assert!(payload["body"]
        .as_str()
        .is_some_and(|body| body.contains("# A hook-enabled decision")));

    let updated = run_dg(project.path(), &["set", "ADR-001", "status=accepted"]);
    assert!(
        updated.status.success(),
        "dg set failed: {}",
        String::from_utf8_lossy(&updated.stderr)
    );
    let (id, event, payload) = hook_record(&update_record);
    assert_eq!(id, "ADR-001");
    assert_eq!(event, "update");
    assert_eq!(payload["before"]["frontmatter"]["status"], "proposed");
    assert_eq!(payload["after"]["frontmatter"]["status"], "accepted");
    assert_eq!(payload["diff"]["id"], "ADR-001");

    let deleted = run_dg(project.path(), &["delete", "ADR-001"]);
    assert!(
        deleted.status.success(),
        "dg delete failed: {}",
        String::from_utf8_lossy(&deleted.stderr)
    );
    let (id, event, payload) = hook_record(&delete_record);
    assert_eq!(id, "ADR-001");
    assert_eq!(event, "delete");
    assert_eq!(payload["frontmatter"]["status"], "accepted");
}

#[test]
fn failing_hook_does_not_fail_cli_mutation() {
    let project = setup_project();
    install_failing_hook(project.path(), "create");

    let created = run_dg(project.path(), &["new", "adr", "Hook failure is non-fatal"]);
    assert!(
        created.status.success(),
        "dg new failed after hook failure: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let stderr = String::from_utf8_lossy(&created.stderr);
    assert!(
        stderr.contains("warning: dg on_create hook failed for ADR-001"),
        "missing hook warning: {stderr}"
    );
    assert!(
        stderr.contains("exit status: 7"),
        "missing status: {stderr}"
    );
}

#[test]
fn non_executable_hook_is_skipped_silently() {
    let project = setup_project();
    let record = project.path().join("create-record");
    write_hook(
        project.path(),
        "create",
        &format!("touch '{}'\n", record.display()),
        0o644,
    );

    let created = run_dg(project.path(), &["new", "adr", "No hook runs"]);
    assert!(created.status.success());
    assert!(!record.exists(), "non-executable hook must not run");
    assert!(!String::from_utf8_lossy(&created.stderr).contains("warning"));
}

#[test]
fn hook_ignoring_stdin_does_not_warn() {
    let project = setup_project();
    write_hook(project.path(), "create", "exit 0\n", 0o755);

    let created = run_dg(project.path(), &["new", "adr", "Hook ignores stdin"]);
    assert!(created.status.success());
    let stderr = String::from_utf8_lossy(&created.stderr);
    assert!(!stderr.contains("warning"), "unexpected warning: {stderr}");
}

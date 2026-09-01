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

fn install_hook(root: &Path, event: &str, output: &Path) {
    let script = root.join(".dg/hooks").join(format!("on_{event}"));
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n%s\\n' \"$1\" \"$2\" > '{}'\ncat >> '{}'\n",
            output.display(),
            output.display()
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(script, permissions).unwrap();
}

fn install_failing_hook(root: &Path, event: &str) {
    let script = root.join(".dg/hooks").join(format!("on_{event}"));
    fs::write(&script, "#!/bin/sh\nexit 7\n").unwrap();
    let mut permissions = fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(script, permissions).unwrap();
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
    assert!(String::from_utf8_lossy(&created.stderr).contains("hook"));
}

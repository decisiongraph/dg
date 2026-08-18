//! Start devenv-managed services (postgres, redis, …) for the duration of
//! service checks, so test suites on a fresh clone find their backing
//! services running. Uses the devenv 2.x process manager: `devenv up -d`,
//! `devenv processes wait`, `devenv down`.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;

use crate::progress::ProgressSink;
use crate::toolchain::run_with_timeout;

const LIST_TIMEOUT: Duration = Duration::from_secs(60);
/// First `devenv up` may build the environment.
const UP_TIMEOUT: Duration = Duration::from_secs(600);
const WAIT_TIMEOUT: Duration = Duration::from_secs(300);
const DOWN_TIMEOUT: Duration = Duration::from_secs(60);

static SERVICE_NAME_RE: LazyLock<Option<regex::Regex>> =
    LazyLock::new(|| regex::Regex::new(r"services\.([A-Za-z0-9_-]+)").ok());
static SERVICE_ATTRSET_RE: LazyLock<Option<regex::Regex>> =
    LazyLock::new(|| regex::Regex::new(r"services\s*=\s*\{").ok());

/// Service names declared in `root/devenv.nix`, or `None` when the file is
/// missing or declares no services. Detects both the dotted form
/// (`services.postgres.enable = true;`) and the attribute-set form
/// (`services = { postgres = …; };` — names not extracted). Services pulled
/// in via `imports` or devenv.yaml profiles are not detected (v1 limitation).
pub fn declared_services(root: &Path) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(root.join("devenv.nix")).ok()?;
    let mut names: Vec<String> = Vec::new();
    if let Some(re) = SERVICE_NAME_RE.as_ref() {
        for cap in re.captures_iter(&text) {
            let name = cap[1].to_string();
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    let attrset = SERVICE_ATTRSET_RE
        .as_ref()
        .map(|re| re.is_match(&text))
        .unwrap_or(false);
    if names.is_empty() && !attrset {
        return None;
    }
    Some(names)
}

/// Stops services on drop when this run started them (never when they were
/// already running before dg).
pub struct DevenvGuard {
    root: PathBuf,
    path: OsString,
}

impl Drop for DevenvGuard {
    fn drop(&mut self) {
        let mut cmd = devenv_command(&self.root, &self.path);
        cmd.arg("down");
        let _ = run_with_timeout(&mut cmd, DOWN_TIMEOUT);
    }
}

/// Result of ensuring devenv services are up.
pub enum StartOutcome {
    /// dg started the services; drop the guard to stop them again.
    Started(DevenvGuard),
    /// Services were already running — leave them alone.
    AlreadyRunning,
    /// Starting failed; checks proceed but dependent tests may fail.
    Failed {
        /// Tail of the failing `devenv up -d` output, for the diagnostic hint.
        output_tail: String,
    },
}

fn devenv_command(root: &Path, path: &OsStr) -> Command {
    let mut cmd = Command::new("devenv");
    cmd.current_dir(root);
    // Explicit PATH also controls binary lookup for the child (documented
    // std::process behavior), which keeps this testable with stub binaries.
    cmd.env("PATH", path);
    cmd.env("NO_COLOR", "1");
    cmd
}

/// Ensure devenv-managed services are running under `root`. Emits a progress
/// notice before actually starting anything.
pub fn start_services(
    root: &Path,
    path: &OsStr,
    service_names: &[String],
    sink: Option<&dyn ProgressSink>,
) -> StartOutcome {
    // Already running? `devenv processes list` succeeds with process rows
    // when the process manager is up.
    let mut list = devenv_command(root, path);
    list.args(["processes", "list"]);
    if let Ok(out) = run_with_timeout(&mut list, LIST_TIMEOUT) {
        if out.success && !out.stdout.trim().is_empty() {
            return StartOutcome::AlreadyRunning;
        }
    }

    if let Some(sink) = sink {
        let names = if service_names.is_empty() {
            String::new()
        } else {
            format!(" ({})", service_names.join(", "))
        };
        sink.notice(&format!(
            "starting devenv services{names} in {} for the duration of the checks",
            root.display()
        ));
    }

    let mut up = devenv_command(root, path);
    up.args(["up", "-d"]);
    match run_with_timeout(&mut up, UP_TIMEOUT) {
        Ok(out) if out.success => {}
        Ok(out) => {
            // Belt and braces: a concurrent `devenv up` counts as running.
            let combined = format!("{}\n{}", out.stdout, out.stderr).to_lowercase();
            if combined.contains("already running") {
                return StartOutcome::AlreadyRunning;
            }
            let mut tail = crate::toolchain::output_preview(&out.stdout, &out.stderr, 8)
                .unwrap_or_else(|| "no output".to_string());
            if out.timed_out {
                tail = format!("timed out after {}s", UP_TIMEOUT.as_secs());
            }
            return StartOutcome::Failed { output_tail: tail };
        }
        Err(e) => {
            return StartOutcome::Failed {
                output_tail: format!("failed to run `devenv up -d`: {e}"),
            }
        }
    }

    let guard = DevenvGuard {
        root: root.to_path_buf(),
        path: path.to_os_string(),
    };

    // Best-effort readiness: services report ready via process-compose
    // probes; a failure here still lets the checks run.
    let mut wait = devenv_command(root, path);
    wait.args(["processes", "wait"]);
    let _ = run_with_timeout(&mut wait, WAIT_TIMEOUT);

    StartOutcome::Started(guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn declared_services_detects_dotted_and_attrset() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(declared_services(tmp.path()).is_none());

        fs::write(
            tmp.path().join("devenv.nix"),
            "{\n  services.postgres = {\n    enable = true;\n  };\n  services.redis.enable = true;\n}\n",
        )
        .unwrap();
        assert_eq!(
            declared_services(tmp.path()),
            Some(vec!["postgres".to_string(), "redis".to_string()])
        );

        fs::write(
            tmp.path().join("devenv.nix"),
            "{\n  services = {\n    postgres.enable = true;\n  };\n}\n",
        )
        .unwrap();
        assert_eq!(declared_services(tmp.path()), Some(vec![]));

        fs::write(tmp.path().join("devenv.nix"), "{ packages = [ ]; }\n").unwrap();
        assert!(declared_services(tmp.path()).is_none());
    }

    #[cfg(unix)]
    fn make_stub(bin_dir: &Path, name: &str, script: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(bin_dir).unwrap();
        let path = bin_dir.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn start_services_runs_up_wait_and_down_on_drop() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        let marker = tmp.path().join("marker");
        // `processes list` fails (nothing running); everything else succeeds.
        make_stub(
            &bin,
            "devenv",
            &format!(
                "#!/bin/sh\necho \"$@\" >> {m}\ncase \"$1\" in\n  processes) [ \"$2\" = list ] && exit 1 || exit 0;;\n  *) exit 0;;\nesac\n",
                m = marker.display()
            ),
        );
        let path = bin.into_os_string();

        let outcome = start_services(tmp.path(), &path, &[], None);
        let guard = match outcome {
            StartOutcome::Started(g) => g,
            StartOutcome::AlreadyRunning => panic!("expected Started, got AlreadyRunning"),
            StartOutcome::Failed { output_tail } => panic!("expected Started: {output_tail}"),
        };
        drop(guard);

        let calls = fs::read_to_string(&marker).unwrap();
        let calls: Vec<&str> = calls.lines().collect();
        assert_eq!(
            calls,
            vec!["processes list", "up -d", "processes wait", "down"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn start_services_leaves_running_services_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        let marker = tmp.path().join("marker");
        make_stub(
            &bin,
            "devenv",
            &format!(
                "#!/bin/sh\necho \"$@\" >> {m}\nif [ \"$1\" = processes ] && [ \"$2\" = list ]; then echo 'postgres  Running'; exit 0; fi\nexit 0\n",
                m = marker.display()
            ),
        );
        let path = bin.into_os_string();

        assert!(matches!(
            start_services(tmp.path(), &path, &[], None),
            StartOutcome::AlreadyRunning
        ));
        let calls = fs::read_to_string(&marker).unwrap();
        assert_eq!(calls.lines().collect::<Vec<_>>(), vec!["processes list"]);
    }

    #[cfg(unix)]
    #[test]
    fn start_services_reports_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        make_stub(
            &bin,
            "devenv",
            "#!/bin/sh\nif [ \"$1\" = up ]; then echo 'evaluation aborted' >&2; exit 1; fi\nexit 1\n",
        );
        let path = bin.into_os_string();

        match start_services(tmp.path(), &path, &[], None) {
            StartOutcome::Failed { output_tail } => {
                assert!(output_tail.contains("evaluation aborted"), "{output_tail}");
            }
            _ => panic!("expected Failed"),
        }
    }
}

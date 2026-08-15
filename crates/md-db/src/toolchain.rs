//! Toolchain detection for running service tests/linters: JS package-manager
//! resolution, dev-environment wrappers (devenv/mise/nix/direnv), and
//! dependency installation.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// JavaScript package manager used by a service or workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Bun,
    Pnpm,
    /// Yarn 1.x (classic)
    Yarn,
    /// Yarn 2+ (berry)
    YarnBerry,
    Npm,
}

impl PackageManager {
    /// Binary that must be on PATH for this package manager.
    pub fn binary(&self) -> &'static str {
        match self {
            PackageManager::Bun => "bun",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn | PackageManager::YarnBerry => "yarn",
            PackageManager::Npm => "npm",
        }
    }

    /// Human-readable name for diagnostics.
    pub fn name(&self) -> &'static str {
        match self {
            PackageManager::YarnBerry => "yarn",
            other => other.binary(),
        }
    }

    /// Program + prefix args for executing a locally-installed tool
    /// (e.g. `pnpm exec vitest run`, `bunx vitest run`).
    pub fn exec_prefix(&self) -> (&'static str, &'static [&'static str]) {
        match self {
            PackageManager::Bun => ("bunx", &[]),
            PackageManager::Pnpm => ("pnpm", &["exec"]),
            PackageManager::Yarn | PackageManager::YarnBerry => ("yarn", &[]),
            PackageManager::Npm => ("npx", &[]),
        }
    }

    /// Install command, preferring reproducible frozen-lockfile variants.
    pub fn install_command(&self, has_lockfile: bool) -> (&'static str, Vec<&'static str>) {
        match (self, has_lockfile) {
            (PackageManager::Bun, true) => ("bun", vec!["install", "--frozen-lockfile"]),
            (PackageManager::Bun, false) => ("bun", vec!["install"]),
            (PackageManager::Pnpm, true) => ("pnpm", vec!["install", "--frozen-lockfile"]),
            (PackageManager::Pnpm, false) => ("pnpm", vec!["install"]),
            (PackageManager::Yarn, true) => ("yarn", vec!["install", "--frozen-lockfile"]),
            (PackageManager::YarnBerry, true) => ("yarn", vec!["install", "--immutable"]),
            (PackageManager::Yarn | PackageManager::YarnBerry, false) => ("yarn", vec!["install"]),
            (PackageManager::Npm, true) => ("npm", vec!["ci"]),
            (PackageManager::Npm, false) => ("npm", vec!["install"]),
        }
    }

    /// Lockfile names owned by this package manager.
    fn lockfiles(&self) -> &'static [&'static str] {
        match self {
            PackageManager::Bun => &["bun.lock", "bun.lockb"],
            PackageManager::Pnpm => &["pnpm-lock.yaml"],
            PackageManager::Yarn | PackageManager::YarnBerry => &["yarn.lock"],
            PackageManager::Npm => &["package-lock.json", "npm-shrinkwrap.json"],
        }
    }
}

/// Resolved JS toolchain for a service: package manager + where installs run.
#[derive(Debug, Clone)]
pub struct JsToolchain {
    pub pm: PackageManager,
    /// Directory where the lockfile lives and installs run (monorepo root for
    /// workspaces). Falls back to the service directory.
    pub workspace_root: PathBuf,
    pub has_lockfile: bool,
}

/// Detect the JS package manager for a service, walking from `service_dir` up
/// to `stop_at` (inclusive). Within each directory the `packageManager` field
/// in package.json wins over lockfiles; across directories the nearest hit
/// wins (so a nested lockfile beats the monorepo root).
pub fn detect_js_toolchain(service_dir: &Path, stop_at: &Path) -> JsToolchain {
    let all_lockfiles: &[(&str, PackageManager)] = &[
        ("bun.lock", PackageManager::Bun),
        ("bun.lockb", PackageManager::Bun),
        ("pnpm-lock.yaml", PackageManager::Pnpm),
        ("yarn.lock", PackageManager::Yarn),
        ("package-lock.json", PackageManager::Npm),
        ("npm-shrinkwrap.json", PackageManager::Npm),
    ];

    let mut dir = service_dir;
    loop {
        if let Some(pm) = package_manager_field(dir) {
            // Field names the PM; the lockfile (and thus install root) may
            // live further up in a monorepo.
            let (workspace_root, has_lockfile) =
                find_lockfile_root(dir, stop_at, pm).unwrap_or((dir.to_path_buf(), false));
            let pm = refine_yarn(pm, dir, &workspace_root);
            return JsToolchain {
                pm,
                workspace_root,
                has_lockfile,
            };
        }
        for (name, pm) in all_lockfiles {
            if dir.join(name).is_file() {
                let pm = refine_yarn(*pm, dir, dir);
                return JsToolchain {
                    pm,
                    workspace_root: dir.to_path_buf(),
                    has_lockfile: true,
                };
            }
        }
        if dir == stop_at {
            break;
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    JsToolchain {
        pm: PackageManager::Npm,
        workspace_root: service_dir.to_path_buf(),
        has_lockfile: false,
    }
}

/// Parse the `packageManager` field ("pnpm@9.1.0+sha256...") from package.json.
fn package_manager_field(dir: &Path) -> Option<PackageManager> {
    let raw = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let field = json.get("packageManager")?.as_str()?;
    let (name, version) = field.split_once('@').unwrap_or((field, ""));
    match name {
        "bun" => Some(PackageManager::Bun),
        "pnpm" => Some(PackageManager::Pnpm),
        "npm" => Some(PackageManager::Npm),
        "yarn" => {
            let major: Option<u32> = version
                .split(['.', '+'])
                .next()
                .and_then(|v| v.parse().ok());
            match major {
                Some(m) if m >= 2 => Some(PackageManager::YarnBerry),
                _ => Some(PackageManager::Yarn),
            }
        }
        _ => None,
    }
}

/// Find the nearest directory (from `start` up to `stop_at`) holding one of
/// `pm`'s lockfiles.
fn find_lockfile_root(start: &Path, stop_at: &Path, pm: PackageManager) -> Option<(PathBuf, bool)> {
    let mut dir = start;
    loop {
        if pm.lockfiles().iter().any(|n| dir.join(n).is_file()) {
            return Some((dir.to_path_buf(), true));
        }
        if dir == stop_at {
            return None;
        }
        dir = dir.parent()?;
    }
}

/// Upgrade Yarn classic to berry when `.yarnrc.yml` is present.
fn refine_yarn(pm: PackageManager, dir: &Path, workspace_root: &Path) -> PackageManager {
    if pm == PackageManager::Yarn
        && (dir.join(".yarnrc.yml").is_file() || workspace_root.join(".yarnrc.yml").is_file())
    {
        return PackageManager::YarnBerry;
    }
    pm
}

/// True if dependencies look installed at `workspace_root`
/// (node_modules/ or Yarn PnP artifacts).
pub fn js_deps_present(workspace_root: &Path) -> bool {
    workspace_root.join("node_modules").is_dir()
        || workspace_root.join(".pnp.cjs").is_file()
        || workspace_root.join(".pnp.data.json").is_file()
}

/// Dev-environment wrapper that can provide missing binaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvWrapperKind {
    Devenv,
    Mise,
    NixFlake,
    Direnv,
}

/// A usable environment wrapper detected at the project root.
#[derive(Debug, Clone)]
pub struct EnvWrapper {
    pub kind: EnvWrapperKind,
    pub root: PathBuf,
}

impl EnvWrapper {
    /// Binary the wrapper itself needs on PATH.
    pub fn binary(&self) -> &'static str {
        match self.kind {
            EnvWrapperKind::Devenv => "devenv",
            EnvWrapperKind::Mise => "mise",
            EnvWrapperKind::NixFlake => "nix",
            EnvWrapperKind::Direnv => "direnv",
        }
    }

    /// Wrap `program args...` so it runs inside the dev environment.
    /// devenv and mise resolve their config by walking up from the cwd, so
    /// the command keeps running with cwd = service dir.
    pub fn wrap(&self, program: &str, args: &[String]) -> (String, Vec<String>) {
        let root = self.root.to_string_lossy().into_owned();
        let mut wrapped: Vec<String> = match self.kind {
            EnvWrapperKind::Devenv => vec!["shell".into(), "--".into()],
            EnvWrapperKind::Mise => vec!["exec".into(), "--".into()],
            EnvWrapperKind::NixFlake => vec!["develop".into(), root, "--command".into()],
            EnvWrapperKind::Direnv => vec!["exec".into(), root],
        };
        wrapped.push(program.to_string());
        wrapped.extend(args.iter().cloned());
        (self.binary().to_string(), wrapped)
    }
}

/// Environment config files checked at the project root, in wrapper
/// precedence order.
const ENV_WRAPPER_CANDIDATES: &[(&str, EnvWrapperKind)] = &[
    ("devenv.nix", EnvWrapperKind::Devenv),
    ("mise.toml", EnvWrapperKind::Mise),
    (".mise.toml", EnvWrapperKind::Mise),
    (".tool-versions", EnvWrapperKind::Mise),
    ("flake.nix", EnvWrapperKind::NixFlake),
    (".envrc", EnvWrapperKind::Direnv),
];

fn detect_env_wrapper_with_path(
    root: &Path,
    path: &OsStr,
) -> (Option<EnvWrapper>, Vec<&'static str>) {
    let mut found = Vec::new();
    let mut wrapper = None;
    for (file, kind) in ENV_WRAPPER_CANDIDATES {
        if !root.join(file).is_file() {
            continue;
        }
        found.push(*file);
        if wrapper.is_none() {
            let candidate = EnvWrapper {
                kind: *kind,
                root: root.to_path_buf(),
            };
            if binary_on_path(candidate.binary(), path) {
                wrapper = Some(candidate);
            }
        }
    }
    (wrapper, found)
}

/// Check if an executable named `name` exists on the given PATH value.
fn binary_on_path(name: &str, path: &OsStr) -> bool {
    std::env::split_paths(path).any(|dir| {
        let candidate = dir.join(name);
        is_executable(&candidate)
    })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.is_file()
        && std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Outcome of ensuring JS dependencies are installed for a workspace.
#[derive(Debug, Clone, PartialEq)]
pub enum InstallOutcome {
    AlreadyInstalled,
    Installed,
    /// `--no-install` was given and node_modules is missing.
    SkippedNoInstall,
    /// Package-manager binary not on PATH and no usable env wrapper.
    SkippedNoPm {
        pm_binary: &'static str,
    },
    Failed {
        exit_code: Option<i32>,
        output_tail: String,
    },
}

const INSTALL_TIMEOUT: Duration = Duration::from_secs(600);

/// Per-validate-run toolchain state: env wrapper (detected once) and install
/// outcomes cached per workspace root, so a monorepo installs only once.
pub struct ToolchainContext {
    wrapper: Option<EnvWrapper>,
    env_files_found: Vec<&'static str>,
    installs: HashMap<PathBuf, InstallOutcome>,
    no_install: bool,
    path: OsString,
}

impl ToolchainContext {
    pub fn new(root: &Path, no_install: bool) -> Self {
        let path = std::env::var_os("PATH").unwrap_or_default();
        Self::with_path(root, no_install, path)
    }

    /// Like [`ToolchainContext::new`] but with an explicit PATH (testable).
    pub fn with_path(root: &Path, no_install: bool, path: OsString) -> Self {
        let (wrapper, env_files_found) = detect_env_wrapper_with_path(root, &path);
        ToolchainContext {
            wrapper,
            env_files_found,
            installs: HashMap::new(),
            no_install,
            path,
        }
    }

    pub fn wrapper(&self) -> Option<&EnvWrapper> {
        self.wrapper.as_ref()
    }

    pub fn env_files_found(&self) -> &[&'static str] {
        &self.env_files_found
    }

    /// Hint suffix when env config files exist but no wrapper binary is
    /// usable (e.g. "devenv.nix found but `devenv` is not on PATH ...").
    pub fn env_hint(&self) -> Option<String> {
        if self.wrapper.is_some() || self.env_files_found.is_empty() {
            return None;
        }
        let files = self.env_files_found.join(", ");
        let binaries: Vec<&str> = self
            .env_files_found
            .iter()
            .filter_map(|f| {
                ENV_WRAPPER_CANDIDATES
                    .iter()
                    .find(|(name, _)| name == f)
                    .map(|(_, kind)| {
                        EnvWrapper {
                            kind: *kind,
                            root: PathBuf::new(),
                        }
                        .binary()
                    })
            })
            .collect();
        let mut uniq = Vec::new();
        for b in binaries {
            if !uniq.contains(&b) {
                uniq.push(b);
            }
        }
        Some(format!(
            "{files} found but `{}` is not on PATH — install it or run dg inside the dev shell",
            uniq.join("`/`")
        ))
    }

    /// Wrap a command in the env wrapper, but only when its program is
    /// missing from PATH (otherwise run it directly).
    pub fn finalize(&self, program: &str, args: Vec<String>) -> (String, Vec<String>) {
        if binary_on_path(program, &self.path) {
            return (program.to_string(), args);
        }
        match &self.wrapper {
            Some(w) => w.wrap(program, &args),
            None => (program.to_string(), args),
        }
    }

    /// Ensure JS dependencies are installed for the toolchain's workspace
    /// root, installing them if needed. Cached per workspace root.
    pub fn ensure_js_deps(&mut self, js: &JsToolchain) -> InstallOutcome {
        if let Some(outcome) = self.installs.get(&js.workspace_root) {
            return outcome.clone();
        }
        let outcome = self.install_js_deps(js);
        self.installs
            .insert(js.workspace_root.clone(), outcome.clone());
        outcome
    }

    fn install_js_deps(&self, js: &JsToolchain) -> InstallOutcome {
        if js_deps_present(&js.workspace_root) {
            return InstallOutcome::AlreadyInstalled;
        }
        if self.no_install {
            return InstallOutcome::SkippedNoInstall;
        }
        let (program, args) = js.pm.install_command(js.has_lockfile);
        if !binary_on_path(program, &self.path) && self.wrapper.is_none() {
            return InstallOutcome::SkippedNoPm {
                pm_binary: js.pm.binary(),
            };
        }
        let args: Vec<String> = args.into_iter().map(String::from).collect();
        let (program, args) = self.finalize(program, args);

        let mut command = Command::new(&program);
        command.args(&args);
        command.current_dir(&js.workspace_root);
        command.env("NO_COLOR", "1");
        command.env("CI", "1");
        command.env("PATH", &self.path);

        match run_with_timeout(&mut command, INSTALL_TIMEOUT) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => InstallOutcome::SkippedNoPm {
                pm_binary: js.pm.binary(),
            },
            Err(e) => InstallOutcome::Failed {
                exit_code: None,
                output_tail: format!("failed to run `{program}`: {e}"),
            },
            Ok(output) if output.success => InstallOutcome::Installed,
            Ok(output) => {
                let mut tail = output_preview(&output.stdout, &output.stderr, 5)
                    .unwrap_or_else(|| "no output".to_string());
                if output.timed_out {
                    tail = format!("timed out after {}s", INSTALL_TIMEOUT.as_secs());
                }
                if output.stderr.contains("is blocked") {
                    tail.push_str("\n        run `direnv allow` in the project root");
                }
                InstallOutcome::Failed {
                    exit_code: output.exit_code,
                    output_tail: tail,
                }
            }
        }
    }
}

/// Output of a command run with a timeout.
pub struct CommandOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Run a command, killing it after `timeout`. Stdout/stderr are drained on
/// background threads to avoid pipe-buffer deadlock while polling.
pub fn run_with_timeout(
    command: &mut Command,
    timeout: Duration,
) -> std::io::Result<CommandOutput> {
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = command.spawn()?;

    let stdout_handle = child.stdout.take().map(spawn_reader);
    let stderr_handle = child.stderr.take().map(spawn_reader);

    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait()? {
            Some(status) => break status,
            None => {
                if start.elapsed() >= timeout {
                    timed_out = true;
                    let _ = child.kill();
                    break child.wait()?;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };

    let stdout = join_reader(stdout_handle);
    let stderr = join_reader(stderr_handle);

    Ok(CommandOutput {
        success: status.success() && !timed_out,
        exit_code: status.code(),
        stdout,
        stderr,
        timed_out,
    })
}

fn spawn_reader<R: Read + Send + 'static>(mut reader: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        buf
    })
}

fn join_reader(handle: Option<std::thread::JoinHandle<Vec<u8>>>) -> String {
    handle
        .and_then(|h| h.join().ok())
        .map(|buf| String::from_utf8_lossy(&buf).into_owned())
        .unwrap_or_default()
}

/// Build a diagnostic hint preview from command output. Test runners like
/// vitest print failures at the *end* of stdout, so take the tail of stdout
/// first, then append a short stderr tail when both streams have content.
/// Lines are joined with the indentation used by diagnostic hints.
pub fn output_preview(stdout: &str, stderr: &str, max_lines: usize) -> Option<String> {
    let tail = |text: &str, n: usize| -> Vec<String> {
        let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
        let skip = lines.len().saturating_sub(n);
        lines[skip..].iter().map(|l| l.to_string()).collect()
    };

    let mut lines = tail(stdout, max_lines);
    if lines.is_empty() {
        lines = tail(stderr, max_lines);
    } else if !stderr.trim().is_empty() {
        lines.push("--- stderr ---".to_string());
        lines.extend(tail(stderr, 4));
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n        "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[cfg(unix)]
    fn make_stub(bin_dir: &Path, name: &str, script: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(bin_dir).unwrap();
        let path = bin_dir.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    // --- PM detection ---

    #[test]
    fn package_manager_field_beats_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            &tmp.path().join("package.json"),
            r#"{"packageManager": "pnpm@9.1.0"}"#,
        );
        write(&tmp.path().join("yarn.lock"), "");
        let js = detect_js_toolchain(tmp.path(), tmp.path());
        assert_eq!(js.pm, PackageManager::Pnpm);
    }

    #[test]
    fn lockfile_mappings() {
        for (lockfile, expected) in [
            ("bun.lock", PackageManager::Bun),
            ("bun.lockb", PackageManager::Bun),
            ("pnpm-lock.yaml", PackageManager::Pnpm),
            ("yarn.lock", PackageManager::Yarn),
            ("package-lock.json", PackageManager::Npm),
            ("npm-shrinkwrap.json", PackageManager::Npm),
        ] {
            let tmp = tempfile::tempdir().unwrap();
            write(&tmp.path().join(lockfile), "");
            let js = detect_js_toolchain(tmp.path(), tmp.path());
            assert_eq!(js.pm, expected, "lockfile {lockfile}");
            assert!(js.has_lockfile);
            assert_eq!(js.workspace_root, tmp.path());
        }
    }

    #[test]
    fn walkup_finds_root_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path().join("apps/board");
        fs::create_dir_all(&app).unwrap();
        write(&tmp.path().join("pnpm-lock.yaml"), "");
        let js = detect_js_toolchain(&app, tmp.path());
        assert_eq!(js.pm, PackageManager::Pnpm);
        assert_eq!(js.workspace_root, tmp.path());
        assert!(js.has_lockfile);
    }

    #[test]
    fn nested_lockfile_beats_root() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path().join("apps/board");
        fs::create_dir_all(&app).unwrap();
        write(&tmp.path().join("pnpm-lock.yaml"), "");
        write(&app.join("package-lock.json"), "");
        let js = detect_js_toolchain(&app, tmp.path());
        assert_eq!(js.pm, PackageManager::Npm);
        assert_eq!(js.workspace_root, app);
    }

    #[test]
    fn field_at_app_with_root_lockfile_installs_at_root() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path().join("apps/board");
        fs::create_dir_all(&app).unwrap();
        write(
            &app.join("package.json"),
            r#"{"packageManager": "bun@1.2.0"}"#,
        );
        write(&tmp.path().join("bun.lock"), "");
        let js = detect_js_toolchain(&app, tmp.path());
        assert_eq!(js.pm, PackageManager::Bun);
        assert_eq!(js.workspace_root, tmp.path());
        assert!(js.has_lockfile);
    }

    #[test]
    fn default_is_npm_without_lockfile() {
        let tmp = tempfile::tempdir().unwrap();
        let app = tmp.path().join("apps/board");
        fs::create_dir_all(&app).unwrap();
        let js = detect_js_toolchain(&app, tmp.path());
        assert_eq!(js.pm, PackageManager::Npm);
        assert_eq!(js.workspace_root, app);
        assert!(!js.has_lockfile);
    }

    #[test]
    fn yarn_berry_from_field_version() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            &tmp.path().join("package.json"),
            r#"{"packageManager": "yarn@4.1.0"}"#,
        );
        let js = detect_js_toolchain(tmp.path(), tmp.path());
        assert_eq!(js.pm, PackageManager::YarnBerry);
    }

    #[test]
    fn yarn_berry_from_yarnrc_yml() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("yarn.lock"), "");
        write(&tmp.path().join(".yarnrc.yml"), "nodeLinker: pnp\n");
        let js = detect_js_toolchain(tmp.path(), tmp.path());
        assert_eq!(js.pm, PackageManager::YarnBerry);
    }

    #[test]
    fn yarn_classic_from_v1_field() {
        let tmp = tempfile::tempdir().unwrap();
        write(
            &tmp.path().join("package.json"),
            r#"{"packageManager": "yarn@1.22.19"}"#,
        );
        let js = detect_js_toolchain(tmp.path(), tmp.path());
        assert_eq!(js.pm, PackageManager::Yarn);
    }

    // --- install commands ---

    #[test]
    fn install_command_variants() {
        assert_eq!(
            PackageManager::Npm.install_command(true),
            ("npm", vec!["ci"])
        );
        assert_eq!(
            PackageManager::Npm.install_command(false),
            ("npm", vec!["install"])
        );
        assert_eq!(
            PackageManager::Bun.install_command(true),
            ("bun", vec!["install", "--frozen-lockfile"])
        );
        assert_eq!(
            PackageManager::Yarn.install_command(true),
            ("yarn", vec!["install", "--frozen-lockfile"])
        );
        assert_eq!(
            PackageManager::YarnBerry.install_command(true),
            ("yarn", vec!["install", "--immutable"])
        );
        assert_eq!(
            PackageManager::Pnpm.install_command(true),
            ("pnpm", vec!["install", "--frozen-lockfile"])
        );
    }

    // --- deps presence ---

    #[test]
    fn deps_present_variants() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!js_deps_present(tmp.path()));
        fs::create_dir_all(tmp.path().join("node_modules")).unwrap();
        assert!(js_deps_present(tmp.path()));

        let pnp = tempfile::tempdir().unwrap();
        write(&pnp.path().join(".pnp.cjs"), "");
        assert!(js_deps_present(pnp.path()));
    }

    // --- wrapper detection ---

    #[cfg(unix)]
    #[test]
    fn wrapper_precedence_devenv_first() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        make_stub(&bin, "devenv", "#!/bin/sh\n");
        make_stub(&bin, "mise", "#!/bin/sh\n");
        write(&tmp.path().join("devenv.nix"), "");
        write(&tmp.path().join("mise.toml"), "");
        let (wrapper, found) = detect_env_wrapper_with_path(tmp.path(), bin.as_os_str());
        assert_eq!(wrapper.unwrap().kind, EnvWrapperKind::Devenv);
        assert_eq!(found, vec!["devenv.nix", "mise.toml"]);
    }

    #[cfg(unix)]
    #[test]
    fn wrapper_falls_back_when_binary_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        make_stub(&bin, "mise", "#!/bin/sh\n");
        write(&tmp.path().join("devenv.nix"), "");
        write(&tmp.path().join(".tool-versions"), "nodejs 22\n");
        let (wrapper, found) = detect_env_wrapper_with_path(tmp.path(), bin.as_os_str());
        assert_eq!(wrapper.unwrap().kind, EnvWrapperKind::Mise);
        assert_eq!(found, vec!["devenv.nix", ".tool-versions"]);
    }

    #[test]
    fn wrapper_none_without_binaries() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("devenv.nix"), "");
        write(&tmp.path().join(".envrc"), "use devenv\n");
        let empty = OsString::new();
        let (wrapper, found) = detect_env_wrapper_with_path(tmp.path(), &empty);
        assert!(wrapper.is_none());
        assert_eq!(found, vec!["devenv.nix", ".envrc"]);

        let ctx = ToolchainContext::with_path(tmp.path(), false, OsString::new());
        let hint = ctx.env_hint().unwrap();
        assert!(hint.contains("devenv.nix"), "{hint}");
        assert!(hint.contains("`devenv`"), "{hint}");
    }

    #[test]
    fn wrap_arg_construction() {
        let args = vec!["vitest".to_string(), "run".to_string()];
        let root = PathBuf::from("/proj");
        let cases = [
            (
                EnvWrapperKind::Devenv,
                ("devenv", vec!["shell", "--", "npx", "vitest", "run"]),
            ),
            (
                EnvWrapperKind::Mise,
                ("mise", vec!["exec", "--", "npx", "vitest", "run"]),
            ),
            (
                EnvWrapperKind::NixFlake,
                (
                    "nix",
                    vec!["develop", "/proj", "--command", "npx", "vitest", "run"],
                ),
            ),
            (
                EnvWrapperKind::Direnv,
                ("direnv", vec!["exec", "/proj", "npx", "vitest", "run"]),
            ),
        ];
        for (kind, (prog, expected)) in cases {
            let w = EnvWrapper {
                kind,
                root: root.clone(),
            };
            let (p, a) = w.wrap("npx", &args);
            assert_eq!(p, prog);
            assert_eq!(a, expected);
        }
    }

    // --- finalize ---

    #[cfg(unix)]
    #[test]
    fn finalize_wraps_only_missing_programs() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        make_stub(&bin, "devenv", "#!/bin/sh\n");
        make_stub(&bin, "npx", "#!/bin/sh\n");
        write(&tmp.path().join("devenv.nix"), "");
        let ctx = ToolchainContext::with_path(tmp.path(), false, bin.clone().into_os_string());

        // npx exists on PATH → unchanged
        let (p, a) = ctx.finalize("npx", vec!["vitest".into()]);
        assert_eq!(p, "npx");
        assert_eq!(a, vec!["vitest"]);

        // bunx missing → wrapped in devenv
        let (p, a) = ctx.finalize("bunx", vec!["vitest".into()]);
        assert_eq!(p, "devenv");
        assert_eq!(a, vec!["shell", "--", "bunx", "vitest"]);
    }

    // --- ensure_js_deps ---

    #[cfg(unix)]
    fn js_fixture(tmp: &Path) -> JsToolchain {
        write(&tmp.join("pnpm-lock.yaml"), "");
        detect_js_toolchain(tmp, tmp)
    }

    #[cfg(unix)]
    #[test]
    fn ensure_js_deps_installs_and_caches() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        let marker = tmp.path().join("marker");
        // Stub appends to a marker file so we can count invocations.
        make_stub(
            &bin,
            "pnpm",
            &format!("#!/bin/sh\necho ran >> {}\nexit 0\n", marker.display()),
        );
        let js = js_fixture(tmp.path());
        let mut ctx = ToolchainContext::with_path(tmp.path(), false, bin.into_os_string());

        assert_eq!(ctx.ensure_js_deps(&js), InstallOutcome::Installed);
        assert_eq!(ctx.ensure_js_deps(&js), InstallOutcome::Installed);
        let runs = fs::read_to_string(&marker).unwrap();
        assert_eq!(runs.lines().count(), 1, "install must run once (cached)");
    }

    #[cfg(unix)]
    #[test]
    fn ensure_js_deps_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        make_stub(&bin, "pnpm", "#!/bin/sh\necho boom >&2\nexit 1\n");
        let js = js_fixture(tmp.path());
        let mut ctx = ToolchainContext::with_path(tmp.path(), false, bin.into_os_string());
        match ctx.ensure_js_deps(&js) {
            InstallOutcome::Failed {
                exit_code,
                output_tail,
            } => {
                assert_eq!(exit_code, Some(1));
                assert!(output_tail.contains("boom"), "{output_tail}");
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn ensure_js_deps_skips() {
        let tmp = tempfile::tempdir().unwrap();
        write(&tmp.path().join("pnpm-lock.yaml"), "");
        let js = detect_js_toolchain(tmp.path(), tmp.path());

        let mut ctx = ToolchainContext::with_path(tmp.path(), true, OsString::new());
        assert_eq!(ctx.ensure_js_deps(&js), InstallOutcome::SkippedNoInstall);

        let mut ctx = ToolchainContext::with_path(tmp.path(), false, OsString::new());
        assert_eq!(
            ctx.ensure_js_deps(&js),
            InstallOutcome::SkippedNoPm { pm_binary: "pnpm" }
        );

        fs::create_dir_all(tmp.path().join("node_modules")).unwrap();
        let mut ctx = ToolchainContext::with_path(tmp.path(), false, OsString::new());
        assert_eq!(ctx.ensure_js_deps(&js), InstallOutcome::AlreadyInstalled);
    }

    // --- output preview ---

    #[test]
    fn output_preview_takes_stdout_tail() {
        let stdout = "line1\nline2\nline3\nline4\n";
        let preview = output_preview(stdout, "", 2).unwrap();
        assert_eq!(preview, "line3\n        line4");
    }

    #[test]
    fn output_preview_stderr_fallback_and_combined() {
        assert_eq!(
            output_preview("", "err1\nerr2", 5).unwrap(),
            "err1\n        err2"
        );
        let combined = output_preview("out", "err", 5).unwrap();
        assert!(combined.contains("out"));
        assert!(combined.contains("--- stderr ---"));
        assert!(combined.contains("err"));
        assert!(output_preview("", "", 5).is_none());
        assert!(output_preview("\n  \n", "\n", 5).is_none());
    }
}

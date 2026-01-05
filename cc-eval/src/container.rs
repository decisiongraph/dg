//! Linux container isolation for eval execution using Apple's `container` CLI.
//!
//! This module provides VM-level isolation using Apple's container platform.
//! Requires macOS 26+ on Apple Silicon with the container service running.
//!
//! ## Authentication
//!
//! Set `ANTHROPIC_API_KEY` environment variable. It is passed into containers automatically.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use anyhow::{bail, Context, Result};

/// Global state: whether we've already done container setup this session
static CONTAINER_READY: OnceLock<bool> = OnceLock::new();

/// Container image name for cc-eval
const IMAGE_NAME: &str = "cc-eval-claude:latest";

/// PATH inside container — includes Claude's default install location
const CONTAINER_PATH: &str = "/home/claude/.claude/local/bin:/usr/local/bin:/usr/bin:/bin:/sbin";


/// Check if container CLI is available and service is running.
pub fn is_available() -> bool {
    // Check if container binary exists
    let has_binary = Command::new("which")
        .arg("container")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_binary {
        return false;
    }

    // Check if service is running by listing containers
    Command::new("container")
        .args(["list"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Build env args for passing ANTHROPIC_API_KEY into containers.
fn api_key_env_args() -> Vec<String> {
    match std::env::var("ANTHROPIC_API_KEY") {
        Ok(key) => vec!["-e".into(), format!("ANTHROPIC_API_KEY={key}")],
        Err(_) => vec![],
    }
}

/// Try a non-interactive auth check. Returns true if Claude can authenticate.
fn try_container_auth() -> bool {
    let mut args = vec![
        "run".into(), "--rm".into(),
        "-e".into(), "HOME=/home/claude".into(),
        "-e".into(), format!("PATH={CONTAINER_PATH}"),
    ];
    args.extend(api_key_env_args());
    args.extend([IMAGE_NAME.into(), "claude".into(), "-p".into(), "respond with: ok".into()]);

    Command::new("container")
        .args(&args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Validate that Claude can authenticate inside the container.
fn validate_container_auth() -> Result<()> {
    eprintln!("cc-eval: validating Claude authentication in container...");

    if try_container_auth() {
        eprintln!("cc-eval: authentication OK");
        return Ok(());
    }

    bail!(
        "Claude authentication failed inside container.\n\n\
         Set ANTHROPIC_API_KEY environment variable and try again."
    );
}

/// Check if our claude image exists.
fn image_exists() -> bool {
    let output = Command::new("container")
        .args(["image", "list"])
        .output()
        .ok();

    output
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(IMAGE_NAME))
        .unwrap_or(false)
}

/// Find project root (directory with workspace Cargo.toml).
fn find_project_root() -> Result<PathBuf> {
    let mut dir = std::env::current_dir()?;
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return Ok(dir);
                }
            }
        }
        if !dir.pop() {
            bail!("Could not find project root with workspace Cargo.toml");
        }
    }
}

/// Build the dg binary for Linux inside a container.
/// Returns path to the built binary.
pub fn build_linux_dg() -> Result<PathBuf> {
    let project_root = find_project_root()?;
    let output_dir = project_root.join("target/aarch64-unknown-linux-musl/release");
    fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join("dg");

    eprintln!("cc-eval: building dg for Linux (first time only, may take a few minutes)...");

    let status = Command::new("container")
        .args([
            "run", "--rm",
            "-v", &format!("{}:/src:ro", project_root.display()),
            "-v", &format!("{}:/out", output_dir.display()),
            "-v", "cc-eval-cargo-cache:/cargo-cache",
            "-v", "cc-eval-build-target:/build-target",
            "-e", "CARGO_HOME=/cargo-cache",
            "-e", "CARGO_TARGET_DIR=/build-target",
            "-w", "/src",
            "rust:alpine",
            "sh", "-c",
            "apk add --no-cache musl-dev && cargo build --release -p dg-cli && cp /build-target/release/dg /out/dg && chmod +x /out/dg"
        ])
        .status()
        .context("failed to run container build")?;

    if !status.success() {
        bail!("Linux dg build failed");
    }

    if !output_path.exists() {
        bail!("Linux dg binary not found after build");
    }

    eprintln!("cc-eval: Linux dg binary built at {}", output_path.display());
    Ok(output_path)
}

/// Build cc-eval for Linux inside a container.
/// Returns path to the built binary.
pub fn build_linux_cc_eval() -> Result<PathBuf> {
    let project_root = find_project_root()?;
    let output_dir = project_root.join("cc-eval/target/aarch64-unknown-linux-musl/release");
    fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join("cc-eval");

    eprintln!("cc-eval: building cc-eval for Linux (first time only, may take a few minutes)...");

    // cc-eval is in a subdirectory with its own Cargo.toml
    let status = Command::new("container")
        .args([
            "run", "--rm",
            "-v", &format!("{}:/src:ro", project_root.display()),
            "-v", &format!("{}:/out", output_dir.display()),
            "-v", "cc-eval-cargo-cache:/cargo-cache",
            "-v", "cc-eval-build-target:/build-target",
            "-e", "CARGO_HOME=/cargo-cache",
            "-e", "CARGO_TARGET_DIR=/build-target",
            "-w", "/src/cc-eval",
            "rust:alpine",
            "sh", "-c",
            "apk add --no-cache musl-dev && cargo build --release && cp /build-target/release/cc-eval /out/cc-eval && chmod +x /out/cc-eval"
        ])
        .status()
        .context("failed to run container build")?;

    if !status.success() {
        bail!("Linux cc-eval build failed");
    }

    if !output_path.exists() {
        bail!("Linux cc-eval binary not found after build");
    }

    eprintln!("cc-eval: Linux cc-eval binary built at {}", output_path.display());
    Ok(output_path)
}

/// Build both dg and cc-eval for Linux.
pub fn build_linux_binaries() -> Result<(PathBuf, PathBuf)> {
    let project_root = find_project_root()?;
    let dg_output_dir = project_root.join("target/aarch64-unknown-linux-musl/release");
    let eval_output_dir = project_root.join("cc-eval/target/aarch64-unknown-linux-musl/release");

    fs::create_dir_all(&dg_output_dir)?;
    fs::create_dir_all(&eval_output_dir)?;

    let dg_path = dg_output_dir.join("dg");
    let eval_path = eval_output_dir.join("cc-eval");

    // cc-eval depends on claude-code-rs which is outside the project root
    // We need to mount it separately
    let claude_code_rs = project_root.parent()
        .context("no parent dir")?
        .join("claude-code-rs");

    if !claude_code_rs.exists() {
        bail!("claude-code-rs not found at {}", claude_code_rs.display());
    }

    eprintln!("cc-eval: building dg and cc-eval for Linux...");

    // Build both in one container run
    // Mount claude-code-rs at /claude-code-rs to match the path in Cargo.toml
    let status = Command::new("container")
        .args([
            "run", "--rm",
            "-v", &format!("{}:/src:ro", project_root.display()),
            "-v", &format!("{}:/claude-code-rs:ro", claude_code_rs.display()),
            "-v", &format!("{}:/out-dg", dg_output_dir.display()),
            "-v", &format!("{}:/out-eval", eval_output_dir.display()),
            "-v", "cc-eval-cargo-cache:/cargo-cache",
            "-v", "cc-eval-build-target:/build-target",
            "-e", "CARGO_HOME=/cargo-cache",
            "-e", "CARGO_TARGET_DIR=/build-target",
            "-w", "/src",
            "rust:alpine",
            "sh", "-c",
            "apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig perl && \
             cargo build --release -p dg-cli && \
             cp /build-target/release/dg /out-dg/dg && chmod +x /out-dg/dg && \
             cd /src/cc-eval && \
             OPENSSL_STATIC=1 cargo build --release && \
             cp /build-target/release/cc-eval /out-eval/cc-eval && chmod +x /out-eval/cc-eval"
        ])
        .status()
        .context("failed to run container build")?;

    if !status.success() {
        bail!("Linux build failed");
    }

    if !dg_path.exists() || !eval_path.exists() {
        bail!("Linux binaries not found after build");
    }

    eprintln!("cc-eval: Linux binaries built:");
    eprintln!("  dg: {}", dg_path.display());
    eprintln!("  cc-eval: {}", eval_path.display());
    Ok((dg_path, eval_path))
}

/// Run cc-eval entirely inside a container.
/// This runs the whole eval process in the container, including answerer/judge LLM calls.
pub fn run_eval_in_container(args: &[String]) -> Result<i32> {
    let project_root = find_project_root()?;

    // Find or build Linux binaries
    let dg_path = project_root.join("target/aarch64-unknown-linux-musl/release/dg");
    let eval_path = project_root.join("cc-eval/target/aarch64-unknown-linux-musl/release/cc-eval");

    if !dg_path.exists() || !eval_path.exists() {
        eprintln!("cc-eval: Linux binaries not found, building...");
        build_linux_binaries()?;
    }

    // Ensure image is ready
    ensure_container_ready()?;

    // evals directory for output (use cc-eval/evals to match expected structure)
    let evals_dir = project_root.join("cc-eval/evals");
    fs::create_dir_all(&evals_dir)?;

    // scenarios directory
    let scenarios_dir = project_root.join("cc-eval/scenarios");
    if !scenarios_dir.exists() {
        bail!("scenarios directory not found: {}", scenarios_dir.display());
    }

    eprintln!("cc-eval: running eval inside container...");

    // Build the args string - filter out flags that don't apply inside container
    // Note: --no-sandbox doesn't exist in Linux binary (it's cfg(target_os = "macos"))
    let container_args: Vec<String> = args.iter()
        .filter(|a| *a != "--in-container" && *a != "--no-sandbox")
        .cloned()
        .collect();

    let args_str = container_args.join(" ");

    let mut run_args: Vec<String> = vec![
        "run".into(), "--rm".into(), "-i".into(),
        // Mount binaries
        "-v".into(), format!("{}:/usr/local/bin/dg:ro", dg_path.display()),
        "-v".into(), format!("{}:/usr/local/bin/cc-eval:ro", eval_path.display()),
        // Mount scenarios and evals at expected locations
        "-v".into(), format!("{}:/workspace/scenarios:ro", scenarios_dir.display()),
        "-v".into(), format!("{}:/workspace/evals", evals_dir.display()),
        // Working directory
        "-w".into(), "/workspace".into(),
        // Environment
        "-e".into(), format!("PATH={CONTAINER_PATH}"),
        "-e".into(), "HOME=/home/claude".into(),
    ];
    // Pass ANTHROPIC_API_KEY if set on host
    run_args.extend(api_key_env_args());
    run_args.extend([
        IMAGE_NAME.into(),
        "sh".into(), "-c".into(),
        format!("cc-eval {}", args_str),
    ]);

    let status = Command::new("container")
        .args(&run_args)
        .status()
        .context("failed to run cc-eval in container")?;

    Ok(status.code().unwrap_or(1))
}

/// Generate Containerfile content for building the claude image.
/// Creates a non-root user to allow bypassPermissions mode.
fn containerfile_content() -> &'static str {
    // Note: dg binary is mounted at runtime, not baked into image
    // (Apple's container CLI has issues with COPY in build context)
    r#"FROM alpine

# Install bash (Claude Code's Bash tool requires it), git, and curl
RUN apk add --no-cache bash git curl

# Create non-root user (required for bypassPermissions mode)
RUN adduser -D -h /home/claude claude

# Install Claude Code as claude user (installs to ~/.claude/local/bin/)
USER claude
RUN curl -fsSL https://claude.ai/install.sh | bash

USER root

# Create workspace mount point with correct ownership
RUN mkdir -p /workspace && chown claude:claude /workspace

# Pre-seed theme to skip interactive theme picker on first run
RUN echo '{"theme":"dark"}' > /home/claude/.claude.json && chown claude:claude /home/claude/.claude.json

# Switch to non-root user
USER claude

WORKDIR /workspace
"#
}

/// Build the container image if it doesn't exist.
/// Note: dg binary is mounted at runtime (see create_wrapper_script).
pub fn ensure_image() -> Result<()> {
    if image_exists() {
        return Ok(());
    }

    eprintln!("cc-eval: building container image (first run, may take a minute)...");

    // Write Containerfile to temp dir
    let build_dir = std::env::temp_dir().join("cc-eval-container-build");
    fs::create_dir_all(&build_dir)?;

    let containerfile = build_dir.join("Containerfile");
    fs::write(&containerfile, containerfile_content())?;

    // Build the image
    let status = Command::new("container")
        .args(["build", "-t", IMAGE_NAME, "."])
        .current_dir(&build_dir)
        .status()
        .context("failed to run container build")?;

    if !status.success() {
        bail!("container build failed");
    }

    // Cleanup
    let _ = fs::remove_dir_all(&build_dir);

    eprintln!("cc-eval: container image built successfully");
    Ok(())
}

/// Create a wrapper script that runs claude inside a container.
///
/// Uses the `-i` (interactive) flag to properly connect stdin/stdout for
/// the stream-json bidirectional communication protocol.
///
/// Note: dg binary is mounted at runtime (Apple's container CLI has issues with COPY).
pub fn create_wrapper_script(
    workspace: &Path,
    dg_binary_path: Option<&Path>,
) -> Result<PathBuf> {
    use rand::Rng;
    let suffix: u64 = rand::thread_rng().gen();
    let wrapper_path = std::env::temp_dir().join(format!(
        "cc-eval-container-{:016x}.sh",
        suffix
    ));

    // Build mount arguments
    let mut mounts = vec![
        format!("-v {}:/workspace", workspace.display()),
    ];

    // Mount Linux dg binary if available
    if let Some(dg_path) = dg_binary_path {
        mounts.push(format!("-v {}:/usr/local/bin/dg:ro", dg_path.display()));
    }

    let mounts_str = mounts.join(" ");

    // Only pass ANTHROPIC_API_KEY if set
    let env_args = std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .map(|key| format!("-e ANTHROPIC_API_KEY={} ", key))
        .unwrap_or_default();

    // Find container binary path
    let container_bin = Command::new("which")
        .arg("container")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "container".to_string());

    // Use -i flag for interactive stdin (required for stream-json protocol)
    // No TTY needed - the container handles stdout properly with just -i
    // Use absolute path to container binary to avoid PATH issues
    let script = format!(
        r#"#!/bin/sh
# cc-eval container wrapper
# Runs claude inside a Linux container with -i for stdin connectivity

exec "{container_bin}" run --rm --progress none -i {mounts_str} -w /workspace -e PATH="{CONTAINER_PATH}" {env_args}{IMAGE_NAME} claude "$@"
"#,
        container_bin = container_bin
    );

    fs::write(&wrapper_path, &script).context("failed to write container wrapper")?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(&wrapper_path, perms)?;
    }

    Ok(wrapper_path)
}

/// Remove orphaned per-scenario volumes from previous runs.
/// These are named `cc-eval-{hex}` and accumulate if cleanup fails or is skipped.
pub fn cleanup_orphaned_volumes() {
    let output = match Command::new("container")
        .args(["volume", "list"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut removed = 0u32;
    for line in stdout.lines() {
        let name = line.split_whitespace().next().unwrap_or("");
        if name.starts_with("cc-eval-") && name.len() == "cc-eval-".len() + 16 {
            if Command::new("container")
                .args(["volume", "rm", name])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                removed += 1;
            }
        }
    }
    if removed > 0 {
        eprintln!("cc-eval: cleaned up {removed} orphaned scenario volumes");
    }
}

/// ContainerConfig holds paths needed for container-based execution.
pub struct ContainerConfig {
    pub wrapper_path: PathBuf,
}

/// Ensure container environment is ready (image built, credentials available).
/// This runs once per session, not per-scenario.
pub fn ensure_container_ready() -> Result<()> {
    // Only run setup once
    if CONTAINER_READY.get().is_some() {
        return Ok(());
    }

    if !is_available() {
        bail!(
            "container service not available. Run:\n\
             1. Install: https://github.com/apple/container\n\
             2. Start: container system start"
        );
    }

    // Build image if needed
    ensure_image()?;

    // Validate authentication by running a real prompt
    validate_container_auth()?;

    let _ = CONTAINER_READY.set(true);
    Ok(())
}

impl ContainerConfig {
    /// Create container configuration for an eval workspace.
    ///
    /// Call `ensure_container_ready()` once before creating configs.
    ///
    /// Pass Linux dg binary path if available (built with `cc-eval build-linux-dg`).
    pub fn new(workspace: &Path, linux_dg_path: Option<&Path>) -> Result<Self> {
        // Ensure setup is done (no-op if already done)
        ensure_container_ready()?;

        let wrapper_path = create_wrapper_script(workspace, linux_dg_path)?;

        Ok(Self { wrapper_path })
    }

    /// Get the path to use as cli_path in ClaudeAgentOptions.
    pub fn cli_path(&self) -> &Path {
        &self.wrapper_path
    }
}

impl Drop for ContainerConfig {
    fn drop(&mut self) {
        // Clean up wrapper script
        let _ = fs::remove_file(&self.wrapper_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_containerfile_content() {
        let content = containerfile_content();
        assert!(content.contains("alpine"));
        assert!(content.contains("claude.ai/install.sh"));
    }

    #[test]
    fn test_is_available_returns_bool() {
        // Just verify it doesn't panic
        let _ = is_available();
    }

}

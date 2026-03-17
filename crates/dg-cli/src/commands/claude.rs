//! Launch Claude Code with DecisionGraph system prompt.

use std::process::Command;

use anyhow::{Context, Result};
use clap::Args;

use dg_schemas::DG_SYSTEM_PROMPT;

#[derive(Args)]
pub struct ClaudeArgs {
    /// Additional arguments to pass to claude
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

pub fn run(args: &ClaudeArgs) -> Result<()> {
    let mut cmd = Command::new("claude");
    cmd.arg("--append-system-prompt").arg(DG_SYSTEM_PROMPT);

    // Prepend the directory of this dg binary to PATH so hooks resolve to the
    // same binary (and sibling binaries like dg-mcp) regardless of system PATH.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            let current_path = std::env::var("PATH").unwrap_or_default();
            let new_path = format!("{}:{}", bin_dir.display(), current_path);
            cmd.env("PATH", new_path);
        }
    }

    // Pass through any additional arguments
    for arg in &args.args {
        cmd.arg(arg);
    }

    // Replace current process with claude
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        // exec() only returns on error
        Err(err).context("failed to exec claude")
    }

    #[cfg(not(unix))]
    {
        let status = cmd.status().context("failed to run claude")?;
        if !status.success() {
            anyhow::bail!("claude exited with status: {}", status);
        }
        Ok(())
    }
}

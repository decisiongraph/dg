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

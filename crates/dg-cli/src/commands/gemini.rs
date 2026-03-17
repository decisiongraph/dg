//! Launch Gemini CLI with DecisionGraph system prompt.

use std::process::Command;

use anyhow::{Context, Result};
use clap::Args;

use dg_schemas::DG_SYSTEM_PROMPT;

#[derive(Args)]
pub struct GeminiArgs {
    /// Additional arguments to pass to gemini
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

pub fn run(args: &GeminiArgs) -> Result<()> {
    let mut cmd = Command::new("gemini");
    cmd.arg("--append-system-prompt").arg(DG_SYSTEM_PROMPT);

    // Prepend the directory of this dg binary to PATH so hooks resolve to the
    // same binary (and sibling binaries) regardless of system PATH.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            let current_path = std::env::var("PATH").unwrap_or_default();
            cmd.env("PATH", format!("{}:{}", bin_dir.display(), current_path));
        }
    }

    // Pass through any additional arguments
    for arg in &args.args {
        cmd.arg(arg);
    }

    // Replace current process with gemini
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        // exec() only returns on error
        Err(err).context("failed to exec gemini")
    }

    #[cfg(not(unix))]
    {
        let status = cmd.status().context("failed to run gemini")?;
        if !status.success() {
            anyhow::bail!("gemini exited with status: {}", status);
        }
        Ok(())
    }
}

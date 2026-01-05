use std::path::Path;
use std::process::Command as ProcessCommand;

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct ImportArgs {
    #[command(subcommand)]
    pub target: ImportTarget,
}

#[derive(Subcommand)]
pub enum ImportTarget {
    /// Import a service repository
    Service {
        /// Git repository URL
        url: String,
        /// Service name (defaults to repository name)
        #[arg(long)]
        name: Option<String>,
    },
    /// Import an app repository
    App {
        /// Git repository URL
        url: String,
        /// App name (defaults to repository name)
        #[arg(long)]
        name: Option<String>,
    },
    /// Merge all submodules in ./services/ and ./apps/ into monorepo
    MergeSubmodules,
}

pub fn run(args: &ImportArgs, root: &Path) -> Result<()> {
    match &args.target {
        ImportTarget::Service { url, name } => import_repo(root, url, name.as_deref(), "services"),
        ImportTarget::App { url, name } => import_repo(root, url, name.as_deref(), "apps"),
        ImportTarget::MergeSubmodules => merge_submodules(root),
    }
}

fn import_repo(root: &Path, url: &str, name: Option<&str>, target_dir: &str) -> Result<()> {
    // Validate we're in a git repository
    if !root.join(".git").exists() {
        bail!("not a git repository\nrun `git init` first");
    }

    // Extract repository name from URL
    let repo_name = name.unwrap_or_else(|| extract_repo_name(url));
    let target_path = root.join(target_dir).join(repo_name);

    // Check if target directory already exists
    if target_path.exists() {
        bail!("{} already exists at {}", repo_name, target_path.display());
    }

    // Ensure target directory exists
    let target_parent = root.join(target_dir);
    if !target_parent.exists() {
        std::fs::create_dir_all(&target_parent)
            .with_context(|| format!("failed to create {}", target_parent.display()))?;
    }

    // Ask user: submodule or monorepo?
    println!("Import strategy:");
    println!(
        "  1) Git submodule - keeps separate git history, requires `git submodule update` to sync"
    );
    println!("  2) Monorepo merge - merges commit history into main repo, fully integrated");
    println!();
    print!("Choose (1 or 2): ");
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let choice = input.trim();

    match choice {
        "1" => import_as_submodule(root, url, target_dir, repo_name),
        "2" => import_as_subtree(root, url, target_dir, repo_name),
        _ => bail!("invalid choice: {}", choice),
    }
}

fn import_as_submodule(root: &Path, url: &str, target_dir: &str, repo_name: &str) -> Result<()> {
    let submodule_path = format!("{}/{}", target_dir, repo_name);

    println!("Adding git submodule...");
    let status = ProcessCommand::new("git")
        .arg("submodule")
        .arg("add")
        .arg(url)
        .arg(&submodule_path)
        .current_dir(root)
        .status()
        .context("failed to execute git submodule add")?;

    if !status.success() {
        bail!("git submodule add failed");
    }

    println!();
    println!("✓ Added {} as submodule at {}/", repo_name, submodule_path);
    println!();
    println!("Next steps:");
    println!("  git commit -m \"Add {} submodule\"", repo_name);
    println!();
    println!("Team members will need to run:");
    println!("  git submodule update --init --recursive");

    Ok(())
}

fn import_as_subtree(root: &Path, url: &str, target_dir: &str, repo_name: &str) -> Result<()> {
    let prefix = format!("{}/{}", target_dir, repo_name);

    // Detect default branch by cloning with --bare to a temp location
    println!("Detecting default branch...");
    let temp_dir = std::env::temp_dir().join(format!("dg-import-{}", repo_name));
    if temp_dir.exists() {
        std::fs::remove_dir_all(&temp_dir)?;
    }

    let status = ProcessCommand::new("git")
        .arg("clone")
        .arg("--bare")
        .arg(url)
        .arg(&temp_dir)
        .status()
        .context("failed to clone repository to detect default branch")?;

    if !status.success() {
        bail!("failed to clone repository");
    }

    // Get the default branch
    let output = ProcessCommand::new("git")
        .arg("symbolic-ref")
        .arg("refs/remotes/origin/HEAD")
        .current_dir(&temp_dir)
        .output()
        .context("failed to detect default branch")?;

    let default_branch = if output.status.success() {
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .strip_prefix("refs/remotes/origin/")
            .unwrap_or("main")
            .to_string()
    } else {
        "main".to_string()
    };

    // Clean up temp directory
    std::fs::remove_dir_all(&temp_dir)?;

    println!(
        "Merging repository history into {}/ (branch: {})...",
        prefix, default_branch
    );
    println!("This preserves the full commit history.");
    let status = ProcessCommand::new("git")
        .arg("subtree")
        .arg("add")
        .arg("--prefix")
        .arg(&prefix)
        .arg(url)
        .arg(&default_branch)
        .current_dir(root)
        .status()
        .context("failed to execute git subtree add")?;

    if !status.success() {
        bail!("git subtree add failed");
    }

    println!();
    println!("✓ Merged {} into {}/", repo_name, prefix);
    println!();
    println!("The full commit history has been merged into your repository.");
    println!("Changes are already committed.");

    Ok(())
}

fn extract_repo_name(url: &str) -> &str {
    // Extract repository name from URL
    // Examples:
    //   https://github.com/user/repo.git -> repo
    //   git@github.com:user/repo.git -> repo
    //   https://github.com/user/repo -> repo
    let path = url
        .trim_end_matches(".git")
        .rsplit('/')
        .next()
        .unwrap_or("unknown");

    // Handle SSH URLs like git@github.com:user/repo
    if path.contains(':') {
        path.rsplit(':').next().unwrap_or("unknown")
    } else {
        path
    }
}

#[derive(Debug)]
struct SubmoduleInfo {
    path: String,
    url: String,
    branch: String,
}

fn merge_submodules(root: &Path) -> Result<()> {
    // Validate we're in a git repository
    if !root.join(".git").exists() {
        bail!("not a git repository");
    }

    // Check for uncommitted changes
    let status = ProcessCommand::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(root)
        .output()
        .context("failed to check git status")?;

    if !status.stdout.is_empty() {
        bail!("uncommitted changes detected\ncommit or stash changes before merging submodules");
    }

    println!("Scanning for git submodules in ./services/ and ./apps/...");

    // Get all submodules
    let output = ProcessCommand::new("git")
        .arg("submodule")
        .arg("status")
        .current_dir(root)
        .output()
        .context("failed to get submodule status")?;

    if !output.status.success() {
        bail!("failed to get submodule status");
    }

    let submodule_output = String::from_utf8_lossy(&output.stdout);
    let mut submodules = Vec::new();

    for line in submodule_output.lines() {
        // git submodule status output: " <commit> <path> (<branch>)" or "-<commit> <path>"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }

        let path = parts[1];

        // Only process submodules in ./services/ or ./apps/
        if !path.starts_with("services/") && !path.starts_with("apps/") {
            continue;
        }

        // Get submodule URL
        let url_output = ProcessCommand::new("git")
            .arg("config")
            .arg("--file")
            .arg(".gitmodules")
            .arg(format!("submodule.{}.url", path))
            .current_dir(root)
            .output()
            .context("failed to get submodule URL")?;

        if !url_output.status.success() {
            eprintln!("warning: failed to get URL for submodule {}", path);
            continue;
        }

        let url = String::from_utf8_lossy(&url_output.stdout)
            .trim()
            .to_string();

        // Get submodule branch (defaults to main/master)
        let branch_output = ProcessCommand::new("git")
            .arg("config")
            .arg("--file")
            .arg(".gitmodules")
            .arg(format!("submodule.{}.branch", path))
            .current_dir(root)
            .output();

        let branch = if let Ok(output) = branch_output {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            } else {
                // Detect default branch from submodule
                detect_default_branch(root, path).unwrap_or_else(|_| "main".to_string())
            }
        } else {
            detect_default_branch(root, path).unwrap_or_else(|_| "main".to_string())
        };

        submodules.push(SubmoduleInfo {
            path: path.to_string(),
            url,
            branch,
        });
    }

    if submodules.is_empty() {
        println!("No submodules found in ./services/ or ./apps/");
        return Ok(());
    }

    println!("\nFound {} submodule(s):", submodules.len());
    for sm in &submodules {
        println!("  {} ({})", sm.path, sm.url);
    }

    println!("\nThis will:");
    println!("  1. Remove submodule configurations");
    println!("  2. Merge full commit history into main repository");
    println!("  3. Commit all changes");
    println!();
    print!("Continue? (y/n): ");
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.trim().to_lowercase() != "y" {
        println!("Aborted");
        return Ok(());
    }

    // Process each submodule
    for sm in &submodules {
        println!("\nConverting {} to monorepo...", sm.path);

        // Step 1: Deinitialize submodule
        let status = ProcessCommand::new("git")
            .arg("submodule")
            .arg("deinit")
            .arg("-f")
            .arg(&sm.path)
            .current_dir(root)
            .status()
            .context("failed to deinitialize submodule")?;

        if !status.success() {
            bail!("failed to deinitialize submodule {}", sm.path);
        }

        // Step 2: Remove from git index (keep files)
        let status = ProcessCommand::new("git")
            .arg("rm")
            .arg("-f")
            .arg(&sm.path)
            .current_dir(root)
            .status()
            .context("failed to remove submodule from index")?;

        if !status.success() {
            bail!("failed to remove submodule {} from index", sm.path);
        }

        // Step 3: Remove .git/modules entry if exists
        let modules_path = root.join(".git").join("modules").join(&sm.path);
        if modules_path.exists() {
            std::fs::remove_dir_all(&modules_path)
                .context("failed to remove .git/modules entry")?;
        }

        // Step 4: Commit the removal
        let status = ProcessCommand::new("git")
            .arg("commit")
            .arg("-m")
            .arg(format!("Remove {} as submodule", sm.path))
            .current_dir(root)
            .status()
            .context("failed to commit submodule removal")?;

        if !status.success() {
            bail!("failed to commit removal of {}", sm.path);
        }

        // Step 5: Add as subtree with full history
        println!("Merging commit history from {}...", sm.url);
        let status = ProcessCommand::new("git")
            .arg("subtree")
            .arg("add")
            .arg("--prefix")
            .arg(&sm.path)
            .arg(&sm.url)
            .arg(&sm.branch)
            .current_dir(root)
            .status()
            .context("failed to add as subtree")?;

        if !status.success() {
            bail!("failed to merge {} as subtree", sm.path);
        }

        println!("✓ Converted {} to monorepo", sm.path);
    }

    println!();
    println!(
        "✓ Successfully merged {} submodule(s) into monorepo",
        submodules.len()
    );
    println!();
    println!("All changes have been committed.");
    println!("The full commit history from each submodule has been preserved.");

    Ok(())
}

fn detect_default_branch(root: &Path, submodule_path: &str) -> Result<String> {
    // Try to get the current branch from the submodule
    let submodule_git_dir = root.join(submodule_path).join(".git");

    if !submodule_git_dir.exists() {
        return Ok("main".to_string());
    }

    let output = ProcessCommand::new("git")
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .current_dir(root.join(submodule_path))
        .output()?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !branch.is_empty() && branch != "HEAD" {
            return Ok(branch);
        }
    }

    Ok("main".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_repo_name() {
        assert_eq!(
            extract_repo_name("https://github.com/user/my-repo.git"),
            "my-repo"
        );
        assert_eq!(
            extract_repo_name("https://github.com/user/my-repo"),
            "my-repo"
        );
        assert_eq!(
            extract_repo_name("git@github.com:user/my-repo.git"),
            "my-repo"
        );
        assert_eq!(extract_repo_name("git@github.com:user/my-repo"), "my-repo");
    }
}

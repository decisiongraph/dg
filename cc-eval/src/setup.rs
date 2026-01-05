use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tempfile::TempDir;

/// Create a temporary workspace with dg project structure + Claude files.
///
/// Initializes git (Claude Code requires a git repo) and writes:
/// - `.dg/schema.kdl` + `.dg/org.kdl`
/// - `docs/{architecture,policies,opportunities,incidents}/`
/// - `CLAUDE.md`
/// - `.claude/skills/opportunity/skill.md`
/// - `.claude/skills/adr/skill.md`
pub fn create_workspace() -> Result<TempDir> {
    let tmp = TempDir::new().context("failed to create temp dir")?;
    let root = tmp.path();

    // Init git repo (Claude Code requires it)
    let repo = git2::Repository::init(root).context("failed to git init")?;

    // Configure git to avoid SSH/signing prompts
    {
        let mut config = repo.config().context("failed to get git config")?;
        // Disable commit signing
        config.set_bool("commit.gpgsign", false).ok();
        config.set_bool("tag.gpgsign", false).ok();
        // Set dummy user for commits
        config.set_str("user.name", "cc-eval").ok();
        config.set_str("user.email", "eval@localhost").ok();
    }

    // .dg/ config
    let dg_dir = root.join(".dg");
    fs::create_dir_all(&dg_dir)?;
    fs::write(dg_dir.join("schema.kdl"), dg_schemas::SCHEMA)?;
    fs::write(dg_dir.join("org.kdl"), dg_schemas::ORG_TEMPLATE)?;

    // Doc directories
    for dir in [
        "docs/architecture",
        "docs/policies",
        "docs/opportunities",
        "docs/incidents",
    ] {
        fs::create_dir_all(root.join(dir))?;
    }

    // Claude files
    fs::write(root.join("CLAUDE.md"), dg_schemas::CLAUDE_MD)?;
    fs::write(root.join("AGENTS.md"), dg_schemas::AGENTS_MD)?;
    write_skill(root, "opportunity", dg_schemas::SKILL_OPPORTUNITY)?;
    write_skill(root, "adr", dg_schemas::SKILL_ADR)?;
    write_skill(root, "policy", dg_schemas::SKILL_POLICY)?;
    write_skill(root, "incident", dg_schemas::SKILL_INCIDENT)?;

    // Claude hooks
    write_hooks(root)?;

    // Create initial commit so Claude Code reads CLAUDE.md
    {
        let mut index = repo.index().context("get git index")?;
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .context("git add all")?;
        index.write().context("write index")?;

        let tree_id = index.write_tree().context("write tree")?;
        let tree = repo.find_tree(tree_id).context("find tree")?;
        let sig = repo.signature().context("get signature")?;

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "Initial DecisionGraph setup (dg init)",
            &tree,
            &[], // No parent - first commit
        )
        .context("create initial commit")?;
    }

    Ok(tmp)
}

/// Find the `dg` binary directory. Checks workspace target/debug first,
/// then falls back to looking in PATH.
pub fn find_dg_bin_dir() -> Option<PathBuf> {
    // cc-eval lives alongside crates/ — workspace root is the parent
    let candidates = [
        PathBuf::from("../target/debug"),
        PathBuf::from("target/debug"),
    ];
    for dir in &candidates {
        if dir.join("dg").is_file() || dir.join("dg.exe").is_file() {
            return dir.canonicalize().ok();
        }
    }
    None
}

/// Return PATH value with the dg binary dir prepended (if found).
pub fn path_with_dg() -> Option<String> {
    let dg_dir = find_dg_bin_dir()?;
    let current = std::env::var("PATH").unwrap_or_default();
    Some(format!("{}:{current}", dg_dir.display()))
}

fn write_skill(root: &Path, name: &str, content: &str) -> Result<()> {
    let dir = root.join(format!(".claude/skills/{name}"));
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("skill.md"), content)?;
    Ok(())
}

/// Copy fixture files from scenarios/fixtures/{name}/ into the workspace.
/// Commits fixtures so Claude sees them.
pub fn copy_fixtures(workspace: &Path, fixtures_name: &str) -> Result<()> {
    let fixtures_dir = crate::scenario::scenarios_dir().join("fixtures").join(fixtures_name);
    if !fixtures_dir.exists() {
        anyhow::bail!("fixtures not found: {}", fixtures_dir.display());
    }

    copy_dir_recursive(&fixtures_dir, workspace)?;

    // Commit fixtures so Claude sees them
    let repo = git2::Repository::open(workspace).context("open git repo")?;
    let mut index = repo.index().context("get git index")?;
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .context("git add fixtures")?;
    index.write().context("write index")?;

    let tree_id = index.write_tree().context("write tree")?;
    let tree = repo.find_tree(tree_id).context("find tree")?;
    let sig = repo.signature().context("get signature")?;
    let parent = repo.head()?.peel_to_commit().context("get HEAD commit")?;

    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "Add fixture files for eval scenario",
        &tree,
        &[&parent],
    )
    .context("create fixture commit")?;

    Ok(())
}

/// Recursively copy a directory's contents into destination.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src).with_context(|| format!("read dir {}", src.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let target = dst.join(&name);

        if path.is_dir() {
            fs::create_dir_all(&target)?;
            copy_dir_recursive(&path, &target)?;
        } else {
            // Create parent dirs if needed
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &target)
                .with_context(|| format!("copy {} -> {}", path.display(), target.display()))?;
        }
    }
    Ok(())
}

fn write_hooks(root: &Path) -> Result<()> {
    // No PostToolUse hooks in eval — `dg` is not in Claude Code's Bash sandbox PATH.
    // Write an empty hooks config to avoid inheriting user's global hooks.
    let settings = serde_json::json!({
        "hooks": {}
    });
    fs::create_dir_all(root.join(".claude"))?;
    fs::write(
        root.join(".claude/settings.local.json"),
        serde_json::to_string_pretty(&settings).unwrap_or_default(),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_has_expected_files() {
        let ws = create_workspace().unwrap();
        let root = ws.path();

        assert!(root.join(".dg/schema.kdl").is_file());
        assert!(root.join(".dg/org.kdl").is_file());
        assert!(root.join("CLAUDE.md").is_file());
        assert!(root.join(".claude/skills/opportunity/skill.md").is_file());
        assert!(root.join(".claude/skills/adr/skill.md").is_file());
        assert!(root.join(".claude/skills/policy/skill.md").is_file());
        assert!(root.join(".claude/skills/incident/skill.md").is_file());
        assert!(root.join(".claude/settings.local.json").is_file());
        assert!(root.join("docs/architecture").is_dir());
        assert!(root.join("docs/opportunities").is_dir());

        // Verify git init
        assert!(root.join(".git").is_dir());

        // Verify initial commit exists
        let repo = git2::Repository::open(root).unwrap();
        let head = repo.head().unwrap();
        let commit = head.peel_to_commit().unwrap();
        assert!(commit.message().unwrap().contains("dg init"));
    }

    #[test]
    fn copy_fixtures_copies_files() {
        // This test only runs if the incident fixtures exist
        let fixtures_dir = crate::scenario::scenarios_dir().join("fixtures/incident-production-500");
        if !fixtures_dir.exists() {
            eprintln!("Skipping test: fixtures not found at {}", fixtures_dir.display());
            return;
        }

        let ws = create_workspace().unwrap();
        copy_fixtures(ws.path(), "incident-production-500").unwrap();

        // Verify files were copied
        assert!(ws.path().join("src/api/users.py").is_file());
        assert!(ws.path().join("migrations/20260209_add_preferences_constraint.sql").is_file());
        assert!(ws.path().join("README.md").is_file());

        // Verify second commit was created
        let repo = git2::Repository::open(ws.path()).unwrap();
        let head = repo.head().unwrap();
        let commit = head.peel_to_commit().unwrap();
        assert!(commit.message().unwrap().contains("fixture"));
    }
}

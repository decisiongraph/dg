use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use md_db::users::ORG_CONFIG_FILENAME;
use regex::Regex;

/// Make a file executable on Unix (no-op on other platforms).
#[cfg(unix)]
fn make_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<()> {
    Ok(())
}

/// Check if a directory is inside a git work tree.
fn is_inside_git_repo(dir: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if a command exists in PATH
fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Resolve a template: check `.dg/templates/` for user override, fall back to embedded.
fn resolve(root: &Path, rel_path: &str, default: &str) -> String {
    let templates_dir = root.join(".dg").join("templates");
    dg_schemas::resolve_template(&templates_dir, rel_path, default)
}

/// Prompt user for y/N confirmation on stderr. Returns false in non-interactive mode.
fn confirm(prompt: &str) -> bool {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return false;
    }
    eprint!("{prompt} [y/N] ");
    std::io::stderr().flush().ok();
    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).unwrap_or(0) == 0 {
        return false;
    }
    matches!(input.trim(), "y" | "Y" | "yes" | "YES")
}

pub fn run(
    root: &Path,
    with_claude: bool,
    with_gemini: bool,
    with_opencode: bool,
    eject: bool,
) -> Result<()> {
    let dg_dir = root.join(".dg");
    let dg_exists = dg_dir.is_dir();

    // --eject works regardless of project state
    if eject {
        return eject_templates(root);
    }

    // Core: .dg/ + org.kdl
    if !dg_exists {
        fs::create_dir_all(&dg_dir).context("failed to create .dg/")?;

        let org_path = dg_dir.join(ORG_CONFIG_FILENAME);
        let org_content = if let Some((slug, name)) = detect_org(root) {
            eprintln!("  org: \"{slug}\" ({name})");
            format!(
                "org \"{slug}\" {{\n    name \"{name}\"\n}}\n\n\
                 // user \"example\" {{\n\
                 //     name \"Example User\"\n\
                 //     email \"user@example.com\"\n\
                 //     teams \"engineering\"\n\
                 //     org \"{slug}\"\n\
                 // }}\n\n\
                 // team \"engineering\" {{\n\
                 //     name \"Engineering\"\n\
                 //     org \"{slug}\"\n\
                 // }}\n"
            )
        } else {
            dg_schemas::ORG_TEMPLATE.to_string()
        };
        fs::write(&org_path, org_content)
            .with_context(|| format!("failed to write {ORG_CONFIG_FILENAME}"))?;
        eprintln!("  .dg/     created");
    } else {
        eprintln!("  .dg/     exists (skipped)");
    }

    // Doc dirs — create_dir_all is idempotent, always safe
    for dir in [
        "docs/architecture",
        "docs/policies",
        "docs/opportunities",
        "docs/incidents",
        "docs/specs",
        "docs/assets",
    ] {
        fs::create_dir_all(root.join(dir)).with_context(|| format!("failed to create {dir}"))?;
    }

    // Init git repo if not inside one
    if !is_inside_git_repo(root) {
        let status = Command::new("git")
            .args(["init"])
            .current_dir(root)
            .status()
            .context("failed to run git init")?;
        if status.success() {
            eprintln!("  git:     initialized");
        } else {
            eprintln!("  git:     init failed (skipped)");
        }
    }

    // Install git hooks if inside a git repo
    install_git_hooks(root);

    eprintln!("initialized dg project in {}", root.display());

    // Always create CLAUDE.md with dg instructions (useful even without claude CLI)
    let claude_md_path = root.join("CLAUDE.md");
    if !claude_md_path.exists() {
        fs::write(
            &claude_md_path,
            resolve(root, "claude/CLAUDE.md", dg_schemas::CLAUDE_MD),
        )
        .context("failed to write CLAUDE.md")?;
        eprintln!("  CLAUDE.md created");
    }

    // Auto-detect AI tools if no explicit flags provided
    let (enable_claude, enable_gemini, enable_opencode) =
        if !with_claude && !with_gemini && !with_opencode {
            let claude_found = command_exists("claude");
            let gemini_found = command_exists("gemini");
            let opencode_found = command_exists("opencode");
            (claude_found, gemini_found, opencode_found)
        } else {
            (with_claude, with_gemini, with_opencode)
        };

    // Track if AGENTS.md was written (for sharing between tools)
    let mut agents_md_written = false;

    // Multiple tools enabled = use shared AGENTS.md
    let multiple_tools =
        (enable_claude as u8) + (enable_gemini as u8) + (enable_opencode as u8) > 1;

    if enable_claude {
        let claude_exists = root.join(".claude").is_dir();
        if claude_exists && !confirm("overwrite .claude/?") {
            eprintln!("  claude: skipped (exists)");
        } else {
            write_claude_files(root, multiple_tools)?;
            if multiple_tools {
                agents_md_written = true;
                eprintln!("  claude: CLAUDE.md → @AGENTS.md + .claude/skills/");
            } else {
                eprintln!("  claude: CLAUDE.md + .claude/skills/");
            }
        }
    }

    if enable_gemini {
        let gemini_exists = root.join(".gemini").is_dir();
        if gemini_exists && !confirm("overwrite .gemini/?") {
            eprintln!("  gemini: skipped (exists)");
        } else {
            write_gemini_files(root, agents_md_written)?;
            agents_md_written = true;
            eprintln!("  gemini: AGENTS.md + .gemini/skills/");
        }
    }

    if enable_opencode {
        let opencode_exists = root.join(".opencode").is_dir();
        if opencode_exists && !confirm("overwrite .opencode/?") {
            eprintln!("  opencode: skipped (exists)");
        } else {
            write_opencode_files(root, agents_md_written)?;
            eprintln!("  opencode: AGENTS.md + .opencode/skills/");
        }
    }

    eprintln!("\nrun `dg init --eject` to customize schema and templates");

    Ok(())
}

/// Get today's date as YYYY-MM-DD.
fn today_date() -> String {
    let output = Command::new("date").arg("+%Y-%m-%d").output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "unknown".to_string(),
    }
}

/// Export all built-in templates to `.dg/templates/` for user customization.
/// Also writes `.dg/schema.kdl` with a version+date header.
fn eject_templates(root: &Path) -> Result<()> {
    let dg_dir = root.join(".dg");
    fs::create_dir_all(&dg_dir).context("failed to create .dg/")?;

    // Write schema.kdl with version stamp
    let schema_path = dg_dir.join("schema.kdl");
    let header = format!(
        "// Generated by dg v{} on {}\n// Customize this file — dg uses this when present, built-in defaults otherwise\n\n",
        env!("CARGO_PKG_VERSION"),
        today_date(),
    );
    let schema_content = format!("{header}{}", dg_schemas::SCHEMA);
    fs::write(&schema_path, schema_content).context("failed to write schema.kdl")?;
    eprintln!("  schema: {}", schema_path.display());

    let templates_dir = dg_dir.join("templates");
    let mut count = 0;

    for t in dg_schemas::ALL_TEMPLATES {
        let path = templates_dir.join(t.rel_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&path, t.content)
            .with_context(|| format!("failed to write {}", path.display()))?;
        count += 1;
    }

    eprintln!("ejected {count} templates to {}", templates_dir.display());
    eprintln!("edit templates there; `dg init` will use overrides automatically");
    Ok(())
}

/// Install git hooks (prepare-commit-msg, commit-msg) if inside a git repo.
/// Skips hooks that already exist to avoid overwriting user customizations.
fn install_git_hooks(root: &Path) {
    let git_dir = root.join(".git");
    if !git_dir.is_dir() {
        return;
    }

    let hooks_dir = git_dir.join("hooks");
    if fs::create_dir_all(&hooks_dir).is_err() {
        return;
    }

    let hooks: &[(&str, &str)] = &[
        (
            "prepare-commit-msg",
            dg_schemas::GIT_HOOK_PREPARE_COMMIT_MSG,
        ),
        ("commit-msg", dg_schemas::GIT_HOOK_COMMIT_MSG),
    ];

    for (name, content) in hooks {
        let path = hooks_dir.join(name);
        if path.exists() {
            eprintln!("  git hook: {name} exists (skipped)");
            continue;
        }
        if fs::write(&path, content).is_err() {
            eprintln!("  git hook: {name} failed to write");
            continue;
        }
        if make_executable(&path).is_err() {
            eprintln!("  git hook: {name} failed to set executable");
            continue;
        }
        eprintln!("  git hook: {name} installed");
    }
}

fn write_claude_files(root: &Path, use_agents_md: bool) -> Result<()> {
    // Write context file
    if use_agents_md {
        fs::write(
            root.join("AGENTS.md"),
            resolve(root, "shared/AGENTS.md", dg_schemas::AGENTS_MD),
        )
        .context("failed to write AGENTS.md")?;
        // Point CLAUDE.md at shared AGENTS.md
        fs::write(root.join("CLAUDE.md"), "@AGENTS.md\n").context("failed to write CLAUDE.md")?;
    } else {
        // CLAUDE.md may already exist from the always-create step; overwrite with
        // full template to ensure consistency when claude CLI is detected
        fs::write(
            root.join("CLAUDE.md"),
            resolve(root, "claude/CLAUDE.md", dg_schemas::CLAUDE_MD),
        )
        .context("failed to write CLAUDE.md")?;
    }

    let opp_dir = root.join(".claude/skills/opportunity");
    fs::create_dir_all(&opp_dir).context("failed to create .claude/skills/opportunity/")?;
    fs::write(
        opp_dir.join("skill.md"),
        resolve(
            root,
            "claude/skills/opportunity.md",
            dg_schemas::SKILL_OPPORTUNITY,
        ),
    )
    .context("failed to write opportunity skill")?;

    let adr_dir = root.join(".claude/skills/adr");
    fs::create_dir_all(&adr_dir).context("failed to create .claude/skills/adr/")?;
    fs::write(
        adr_dir.join("skill.md"),
        resolve(root, "claude/skills/adr.md", dg_schemas::SKILL_ADR),
    )
    .context("failed to write adr skill")?;

    let pol_dir = root.join(".claude/skills/policy");
    fs::create_dir_all(&pol_dir).context("failed to create .claude/skills/policy/")?;
    fs::write(
        pol_dir.join("skill.md"),
        resolve(root, "claude/skills/policy.md", dg_schemas::SKILL_POLICY),
    )
    .context("failed to write policy skill")?;

    let inc_dir = root.join(".claude/skills/incident");
    fs::create_dir_all(&inc_dir).context("failed to create .claude/skills/incident/")?;
    fs::write(
        inc_dir.join("skill.md"),
        resolve(
            root,
            "claude/skills/incident.md",
            dg_schemas::SKILL_INCIDENT,
        ),
    )
    .context("failed to write incident skill")?;

    let spec_dir = root.join(".claude/skills/spec");
    fs::create_dir_all(&spec_dir).context("failed to create .claude/skills/spec/")?;
    fs::write(
        spec_dir.join("skill.md"),
        resolve(root, "claude/skills/spec.md", dg_schemas::SKILL_SPEC),
    )
    .context("failed to write spec skill")?;

    let dia_dir = root.join(".claude/skills/diagram");
    fs::create_dir_all(&dia_dir).context("failed to create .claude/skills/diagram/")?;
    fs::write(
        dia_dir.join("skill.md"),
        resolve(root, "claude/skills/diagram.md", dg_schemas::SKILL_DIAGRAM),
    )
    .context("failed to write diagram skill")?;

    let team_dir = root.join(".claude/skills/team");
    fs::create_dir_all(&team_dir).context("failed to create .claude/skills/team/")?;
    fs::write(
        team_dir.join("skill.md"),
        resolve(root, "claude/skills/team.md", dg_schemas::SKILL_TEAM),
    )
    .context("failed to write team skill")?;

    let mermaid_dir = root.join(".claude/skills/mermaid-flowchart");
    fs::create_dir_all(&mermaid_dir)
        .context("failed to create .claude/skills/mermaid-flowchart/")?;
    fs::write(
        mermaid_dir.join("skill.md"),
        resolve(
            root,
            "claude/skills/mermaid-flowchart.md",
            dg_schemas::SKILL_MERMAID_FLOWCHART,
        ),
    )
    .context("failed to write mermaid-flowchart skill")?;

    let seq_dir = root.join(".claude/skills/mermaid-sequence");
    fs::create_dir_all(&seq_dir).context("failed to create .claude/skills/mermaid-sequence/")?;
    fs::write(
        seq_dir.join("skill.md"),
        resolve(
            root,
            "claude/skills/mermaid-sequence.md",
            dg_schemas::SKILL_MERMAID_SEQUENCE,
        ),
    )
    .context("failed to write mermaid-sequence skill")?;

    let img_dir = root.join(".claude/skills/image");
    fs::create_dir_all(&img_dir).context("failed to create .claude/skills/image/")?;
    fs::write(
        img_dir.join("skill.md"),
        resolve(root, "claude/skills/image.md", dg_schemas::SKILL_IMAGE),
    )
    .context("failed to write image skill")?;

    // Write settings.local.json (hooks config — commands call dg hooks directly)
    fs::write(
        root.join(".claude/settings.local.json"),
        resolve(
            root,
            "claude/hooks/settings.json",
            dg_schemas::HOOKS_SETTINGS,
        ),
    )
    .context("failed to write settings.local.json")?;

    Ok(())
}

fn write_gemini_files(root: &Path, agents_md_exists: bool) -> Result<()> {
    // Write AGENTS.md if not already written by Claude setup
    if !agents_md_exists {
        fs::write(
            root.join("AGENTS.md"),
            resolve(root, "shared/AGENTS.md", dg_schemas::AGENTS_MD),
        )
        .context("failed to write AGENTS.md")?;
    }

    // Write skills with YAML frontmatter (Gemini format)
    let opp_dir = root.join(".gemini/skills/opportunity");
    fs::create_dir_all(&opp_dir).context("failed to create .gemini/skills/opportunity/")?;
    fs::write(
        opp_dir.join("SKILL.md"),
        resolve(
            root,
            "gemini/skills/opportunity/SKILL.md",
            dg_schemas::GEMINI_SKILL_OPPORTUNITY,
        ),
    )
    .context("failed to write opportunity skill")?;

    let adr_dir = root.join(".gemini/skills/adr");
    fs::create_dir_all(&adr_dir).context("failed to create .gemini/skills/adr/")?;
    fs::write(
        adr_dir.join("SKILL.md"),
        resolve(
            root,
            "gemini/skills/adr/SKILL.md",
            dg_schemas::GEMINI_SKILL_ADR,
        ),
    )
    .context("failed to write adr skill")?;

    let pol_dir = root.join(".gemini/skills/policy");
    fs::create_dir_all(&pol_dir).context("failed to create .gemini/skills/policy/")?;
    fs::write(
        pol_dir.join("SKILL.md"),
        resolve(
            root,
            "gemini/skills/policy/SKILL.md",
            dg_schemas::GEMINI_SKILL_POLICY,
        ),
    )
    .context("failed to write policy skill")?;

    let inc_dir = root.join(".gemini/skills/incident");
    fs::create_dir_all(&inc_dir).context("failed to create .gemini/skills/incident/")?;
    fs::write(
        inc_dir.join("SKILL.md"),
        resolve(
            root,
            "gemini/skills/incident/SKILL.md",
            dg_schemas::GEMINI_SKILL_INCIDENT,
        ),
    )
    .context("failed to write incident skill")?;

    let spec_dir = root.join(".gemini/skills/spec");
    fs::create_dir_all(&spec_dir).context("failed to create .gemini/skills/spec/")?;
    fs::write(
        spec_dir.join("SKILL.md"),
        resolve(
            root,
            "gemini/skills/spec/SKILL.md",
            dg_schemas::GEMINI_SKILL_SPEC,
        ),
    )
    .context("failed to write spec skill")?;

    let dia_dir = root.join(".gemini/skills/diagram");
    fs::create_dir_all(&dia_dir).context("failed to create .gemini/skills/diagram/")?;
    fs::write(
        dia_dir.join("SKILL.md"),
        resolve(
            root,
            "gemini/skills/diagram/SKILL.md",
            dg_schemas::GEMINI_SKILL_DIAGRAM,
        ),
    )
    .context("failed to write diagram skill")?;

    let mermaid_dir = root.join(".gemini/skills/mermaid-flowchart");
    fs::create_dir_all(&mermaid_dir)
        .context("failed to create .gemini/skills/mermaid-flowchart/")?;
    fs::write(
        mermaid_dir.join("SKILL.md"),
        resolve(
            root,
            "gemini/skills/mermaid-flowchart/SKILL.md",
            dg_schemas::GEMINI_SKILL_MERMAID_FLOWCHART,
        ),
    )
    .context("failed to write mermaid-flowchart skill")?;

    let seq_dir = root.join(".gemini/skills/mermaid-sequence");
    fs::create_dir_all(&seq_dir).context("failed to create .gemini/skills/mermaid-sequence/")?;
    fs::write(
        seq_dir.join("SKILL.md"),
        resolve(
            root,
            "gemini/skills/mermaid-sequence/SKILL.md",
            dg_schemas::GEMINI_SKILL_MERMAID_SEQUENCE,
        ),
    )
    .context("failed to write mermaid-sequence skill")?;

    let img_dir = root.join(".gemini/skills/image");
    fs::create_dir_all(&img_dir).context("failed to create .gemini/skills/image/")?;
    fs::write(
        img_dir.join("SKILL.md"),
        resolve(
            root,
            "gemini/skills/image/SKILL.md",
            dg_schemas::GEMINI_SKILL_IMAGE,
        ),
    )
    .context("failed to write image skill")?;

    // Write settings.json (context + hooks config)
    fs::write(
        root.join(".gemini/settings.json"),
        resolve(root, "gemini/settings.json", dg_schemas::GEMINI_SETTINGS),
    )
    .context("failed to write .gemini/settings.json")?;

    // Write hooks
    let hooks_dir = root.join(".gemini/hooks");
    fs::create_dir_all(&hooks_dir).context("failed to create .gemini/hooks/")?;

    let script_path = hooks_dir.join("check-fixme.sh");
    fs::write(
        &script_path,
        resolve(
            root,
            "gemini/hooks/check-fixme.sh",
            dg_schemas::GEMINI_HOOK_CHECK_FIXME,
        ),
    )
    .context("failed to write check-fixme.sh")?;
    make_executable(&script_path)?;

    let script_path = hooks_dir.join("check-code.sh");
    fs::write(
        &script_path,
        resolve(
            root,
            "gemini/hooks/check-code.sh",
            dg_schemas::GEMINI_HOOK_CHECK_CODE,
        ),
    )
    .context("failed to write check-code.sh")?;
    make_executable(&script_path)?;

    Ok(())
}

fn write_opencode_files(root: &Path, agents_md_exists: bool) -> Result<()> {
    // Write AGENTS.md if not already written
    if !agents_md_exists {
        fs::write(
            root.join("AGENTS.md"),
            resolve(root, "shared/AGENTS.md", dg_schemas::AGENTS_MD),
        )
        .context("failed to write AGENTS.md")?;
    }

    // Write skills with YAML frontmatter (same format as Gemini)
    let opp_dir = root.join(".opencode/skills/opportunity");
    fs::create_dir_all(&opp_dir).context("failed to create .opencode/skills/opportunity/")?;
    fs::write(
        opp_dir.join("SKILL.md"),
        resolve(
            root,
            "opencode/skills/opportunity/SKILL.md",
            dg_schemas::OPENCODE_SKILL_OPPORTUNITY,
        ),
    )
    .context("failed to write opportunity skill")?;

    let adr_dir = root.join(".opencode/skills/adr");
    fs::create_dir_all(&adr_dir).context("failed to create .opencode/skills/adr/")?;
    fs::write(
        adr_dir.join("SKILL.md"),
        resolve(
            root,
            "opencode/skills/adr/SKILL.md",
            dg_schemas::OPENCODE_SKILL_ADR,
        ),
    )
    .context("failed to write adr skill")?;

    let pol_dir = root.join(".opencode/skills/policy");
    fs::create_dir_all(&pol_dir).context("failed to create .opencode/skills/policy/")?;
    fs::write(
        pol_dir.join("SKILL.md"),
        resolve(
            root,
            "opencode/skills/policy/SKILL.md",
            dg_schemas::OPENCODE_SKILL_POLICY,
        ),
    )
    .context("failed to write policy skill")?;

    let inc_dir = root.join(".opencode/skills/incident");
    fs::create_dir_all(&inc_dir).context("failed to create .opencode/skills/incident/")?;
    fs::write(
        inc_dir.join("SKILL.md"),
        resolve(
            root,
            "opencode/skills/incident/SKILL.md",
            dg_schemas::OPENCODE_SKILL_INCIDENT,
        ),
    )
    .context("failed to write incident skill")?;

    let spec_dir = root.join(".opencode/skills/spec");
    fs::create_dir_all(&spec_dir).context("failed to create .opencode/skills/spec/")?;
    fs::write(
        spec_dir.join("SKILL.md"),
        resolve(
            root,
            "opencode/skills/spec/SKILL.md",
            dg_schemas::OPENCODE_SKILL_SPEC,
        ),
    )
    .context("failed to write spec skill")?;

    let dia_dir = root.join(".opencode/skills/diagram");
    fs::create_dir_all(&dia_dir).context("failed to create .opencode/skills/diagram/")?;
    fs::write(
        dia_dir.join("SKILL.md"),
        resolve(
            root,
            "opencode/skills/diagram/SKILL.md",
            dg_schemas::OPENCODE_SKILL_DIAGRAM,
        ),
    )
    .context("failed to write diagram skill")?;

    let mermaid_dir = root.join(".opencode/skills/mermaid-flowchart");
    fs::create_dir_all(&mermaid_dir)
        .context("failed to create .opencode/skills/mermaid-flowchart/")?;
    fs::write(
        mermaid_dir.join("SKILL.md"),
        resolve(
            root,
            "opencode/skills/mermaid-flowchart/SKILL.md",
            dg_schemas::OPENCODE_SKILL_MERMAID_FLOWCHART,
        ),
    )
    .context("failed to write mermaid-flowchart skill")?;

    let seq_dir = root.join(".opencode/skills/mermaid-sequence");
    fs::create_dir_all(&seq_dir).context("failed to create .opencode/skills/mermaid-sequence/")?;
    fs::write(
        seq_dir.join("SKILL.md"),
        resolve(
            root,
            "opencode/skills/mermaid-sequence/SKILL.md",
            dg_schemas::OPENCODE_SKILL_MERMAID_SEQUENCE,
        ),
    )
    .context("failed to write mermaid-sequence skill")?;

    let img_dir = root.join(".opencode/skills/image");
    fs::create_dir_all(&img_dir).context("failed to create .opencode/skills/image/")?;
    fs::write(
        img_dir.join("SKILL.md"),
        resolve(
            root,
            "opencode/skills/image/SKILL.md",
            dg_schemas::OPENCODE_SKILL_IMAGE,
        ),
    )
    .context("failed to write image skill")?;

    // Write opencode.json (hooks config) in project root
    fs::write(
        root.join("opencode.json"),
        resolve(
            root,
            "opencode/opencode.json",
            dg_schemas::OPENCODE_SETTINGS,
        ),
    )
    .context("failed to write opencode.json")?;

    // Write hooks
    let hooks_dir = root.join(".opencode/hooks");
    fs::create_dir_all(&hooks_dir).context("failed to create .opencode/hooks/")?;

    let script_path = hooks_dir.join("check-fixme.sh");
    fs::write(
        &script_path,
        resolve(
            root,
            "opencode/hooks/check-fixme.sh",
            dg_schemas::OPENCODE_HOOK_CHECK_FIXME,
        ),
    )
    .context("failed to write check-fixme.sh")?;
    make_executable(&script_path)?;

    let script_path = hooks_dir.join("check-code.sh");
    fs::write(
        &script_path,
        resolve(
            root,
            "opencode/hooks/check-code.sh",
            dg_schemas::OPENCODE_HOOK_CHECK_CODE,
        ),
    )
    .context("failed to write check-code.sh")?;
    make_executable(&script_path)?;

    Ok(())
}

/// Detect an organization from the project context.
/// Returns (slug, display_name) if found.
fn detect_org(root: &Path) -> Option<(String, String)> {
    let git_user = git_config_user_name(root);
    // 1. Try LICENSE file
    if let Some(name) = detect_from_license(root, git_user.as_deref()) {
        return Some((slugify(&name), name));
    }
    // 2. Try GitHub org
    if let Some(name) = detect_from_github(root) {
        return Some((slugify(&name), name));
    }
    None
}

/// Get `git config user.name` for the repo.
fn git_config_user_name(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["config", "user.name"])
        .current_dir(root)
        .output()
        .ok()?;
    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
}

/// Extract copyright holder from LICENSE/LICENCE file.
/// Returns None if the holder matches `git_user_name` (personal project).
fn detect_from_license(root: &Path, git_user_name: Option<&str>) -> Option<String> {
    let candidates = [
        "LICENSE",
        "LICENCE",
        "LICENSE.md",
        "LICENCE.md",
        "LICENSE.txt",
    ];
    for name in candidates {
        let path = root.join(name);
        if let Ok(content) = fs::read_to_string(&path) {
            if let Some(holder) = extract_copyright_holder(&content) {
                // Skip if this matches the git user — likely a personal project
                if is_personal_holder(&holder, git_user_name) {
                    return None;
                }
                return Some(holder);
            }
        }
    }
    None
}

/// Parse copyright holder name from license text content.
fn extract_copyright_holder(content: &str) -> Option<String> {
    // Handles: "Copyright (c) 2024", "Copyright © 2020–2024", "Copyright (C) 2007"
    let re = Regex::new(r"(?i)(?:copyright\s+(?:\(c\)\s+)?|©\s*)(?:\d{4}[-–,\s]*)+(.+)").ok()?;
    if let Some(caps) = re.captures(content) {
        let holder = caps[1]
            .trim()
            .trim_end_matches('.')
            .trim_start_matches("by ")
            .trim()
            .to_string();
        if holder.len() > 1 {
            return Some(holder);
        }
    }
    None
}

/// Check if a copyright holder looks like a personal name matching the git user.
fn is_personal_holder(holder: &str, git_user_name: Option<&str>) -> bool {
    let Some(git_name) = git_user_name else {
        return false;
    };
    // Case-insensitive comparison
    holder.eq_ignore_ascii_case(git_name)
}

/// Detect org name from GitHub remote using gh CLI.
fn detect_from_github(root: &Path) -> Option<String> {
    // Get remote URL
    let url_output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(root)
        .output()
        .ok()?;
    if !url_output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&url_output.stdout)
        .trim()
        .to_string();

    // Parse owner from URL (github.com/OWNER/repo or git@github.com:OWNER/repo)
    let owner = if let Some(rest) = url.strip_prefix("https://github.com/") {
        rest.split('/').next()
    } else if let Some(rest) = url.strip_prefix("git@github.com:") {
        rest.split('/').next()
    } else {
        return None;
    }?;

    // Check if owner is an org (not a personal account)
    let gh_output = Command::new("gh")
        .args(["api", &format!("/orgs/{owner}"), "--jq", ".name"])
        .output()
        .ok()?;
    if gh_output.status.success() {
        let name = String::from_utf8_lossy(&gh_output.stdout)
            .trim()
            .to_string();
        if !name.is_empty() {
            return Some(name);
        }
        // Org exists but has no display name — use the login
        return Some(owner.to_string());
    }
    None
}

/// Convert a name to a URL-friendly slug.
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- extract_copyright_holder ---

    #[test]
    fn mit_license() {
        let content = r#"MIT License

Copyright (c) 2024 Example Corp

Permission is hereby granted, free of charge, to any person obtaining a copy"#;
        assert_eq!(
            extract_copyright_holder(content).as_deref(),
            Some("Example Corp")
        );
    }

    #[test]
    fn apache_license() {
        let content = r#"
                                 Apache License
                           Version 2.0, January 2004

   Copyright 2019 Google LLC

   Licensed under the Apache License, Version 2.0"#;
        assert_eq!(
            extract_copyright_holder(content).as_deref(),
            Some("Google LLC")
        );
    }

    #[test]
    fn bsd_3_clause() {
        let content = r#"BSD 3-Clause License

Copyright (c) 2023, Meta Platforms, Inc. and affiliates.

Redistribution and use in source and binary forms"#;
        assert_eq!(
            extract_copyright_holder(content).as_deref(),
            Some("Meta Platforms, Inc. and affiliates")
        );
    }

    #[test]
    fn copyright_year_range() {
        let content = "Copyright 2018-2024 Mozilla Foundation\n\nLicensed under...";
        assert_eq!(
            extract_copyright_holder(content).as_deref(),
            Some("Mozilla Foundation")
        );
    }

    #[test]
    fn copyright_multiple_years() {
        let content = "Copyright (c) 2020, 2021, 2023 Microsoft Corporation\n";
        assert_eq!(
            extract_copyright_holder(content).as_deref(),
            Some("Microsoft Corporation")
        );
    }

    #[test]
    fn copyright_with_trailing_period() {
        let content = "Copyright (c) 2024 Acme Inc.\n";
        assert_eq!(
            extract_copyright_holder(content).as_deref(),
            Some("Acme Inc")
        );
    }

    #[test]
    fn isc_license() {
        let content = r#"ISC License

Copyright (c) 2024, Vercel, Inc.

Permission to use, copy, modify"#;
        assert_eq!(
            extract_copyright_holder(content).as_deref(),
            Some("Vercel, Inc")
        );
    }

    #[test]
    fn gpl_style_copyright() {
        let content = r#"Copyright (C) 2007 Free Software Foundation, Inc.
Everyone is permitted to copy"#;
        assert_eq!(
            extract_copyright_holder(content).as_deref(),
            Some("Free Software Foundation, Inc")
        );
    }

    #[test]
    fn copyright_with_by_prefix() {
        // Some licenses use "Copyright 2024 by The Rust Project"
        let content = "Copyright 2024 by The Rust Project Developers\n";
        assert_eq!(
            extract_copyright_holder(content).as_deref(),
            Some("The Rust Project Developers")
        );
    }

    #[test]
    fn no_copyright_line() {
        let content = "MIT License\n\nPermission is hereby granted...";
        assert_eq!(extract_copyright_holder(content), None);
    }

    #[test]
    fn copyright_en_dash_year_range() {
        let content = "Copyright © 2020–2024 Stripe, Inc.\n";
        assert_eq!(
            extract_copyright_holder(content).as_deref(),
            Some("Stripe, Inc")
        );
    }

    // --- is_personal_holder ---

    #[test]
    fn personal_project_exact_match() {
        assert!(is_personal_holder("Jane Doe", Some("Jane Doe")));
    }

    #[test]
    fn personal_project_case_insensitive() {
        assert!(is_personal_holder("jane doe", Some("Jane Doe")));
    }

    #[test]
    fn not_personal_when_different_name() {
        assert!(!is_personal_holder("Example Corp", Some("Jane Doe")));
    }

    #[test]
    fn not_personal_when_no_git_user() {
        assert!(!is_personal_holder("Jane Doe", None));
    }

    // --- slugify ---

    #[test]
    fn slugify_company_name() {
        assert_eq!(slugify("Example Corp Oy"), "example-corp-oy");
    }

    #[test]
    fn slugify_with_punctuation() {
        assert_eq!(slugify("Meta Platforms, Inc."), "meta-platforms-inc");
    }

    #[test]
    fn slugify_already_slug() {
        assert_eq!(slugify("acme-corp"), "acme-corp");
    }

    #[test]
    fn slugify_llc() {
        assert_eq!(slugify("Google LLC"), "google-llc");
    }
}

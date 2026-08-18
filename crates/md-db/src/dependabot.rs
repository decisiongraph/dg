//! Dependabot coverage detection.
//!
//! Scans a repository for package manifests, maps them to Dependabot
//! `package-ecosystem` values, and compares against `.github/dependabot.yml`
//! to find ecosystems Dependabot is not keeping updated.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use serde::Deserialize;

/// A detected package manifest expressed as a Dependabot update entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EcosystemHit {
    /// Dependabot `directory` value relative to repo root (e.g. "/", "/services/api").
    pub directory: String,
    /// Dependabot `package-ecosystem` value (e.g. "cargo", "npm", "mix").
    pub ecosystem: String,
}

impl EcosystemHit {
    /// Human-readable "ecosystem in directory" label.
    pub fn label(&self) -> String {
        format!("{} ({})", self.ecosystem, self.directory)
    }
}

/// Detected ecosystems plus npm workspace layout, so coverage checks can
/// treat a root npm entry as covering workspace member manifests (Dependabot
/// handles npm/yarn/pnpm workspaces from the workspace root).
#[derive(Debug, Default)]
pub struct EcosystemScan {
    /// All detected update entries, including npm workspace members.
    pub hits: Vec<EcosystemHit>,
    /// Positive npm workspace member globs in Dependabot dir form ("/apps/*"),
    /// from root pnpm-workspace.yaml `packages:` and package.json `workspaces`.
    pub npm_workspace_globs: Vec<String>,
    /// Negated ("!...") member globs excluded from membership.
    pub npm_workspace_negations: Vec<String>,
}

impl EcosystemScan {
    /// True if `dir` is an npm workspace member directory (not the root).
    fn is_npm_workspace_member(&self, dir: &str) -> bool {
        dir != "/"
            && self.npm_workspace_globs.iter().any(|g| dir_matches(g, dir))
            && !self
                .npm_workspace_negations
                .iter()
                .any(|g| dir_matches(g, dir))
    }

    /// Hits suitable for generating a dependabot.yml: npm workspace member
    /// directories are dropped because the root entry covers them.
    pub fn config_hits(&self) -> Vec<EcosystemHit> {
        self.hits
            .iter()
            .filter(|h| !(h.ecosystem == "npm" && self.is_npm_workspace_member(&h.directory)))
            .cloned()
            .collect()
    }
}

/// Map a manifest filename to its Dependabot ecosystem.
fn ecosystem_for_file(name: &str) -> Option<&'static str> {
    match name {
        "Cargo.toml" => Some("cargo"),
        "package.json" => Some("npm"),
        "bun.lock" | "bun.lockb" => Some("bun"),
        "go.mod" => Some("gomod"),
        "Gemfile" => Some("bundler"),
        "mix.exs" => Some("mix"),
        "composer.json" => Some("composer"),
        "pom.xml" => Some("maven"),
        "build.gradle" | "build.gradle.kts" => Some("gradle"),
        "packages.config" => Some("nuget"),
        "pyproject.toml" | "requirements.txt" => Some("pip"),
        "uv.lock" => Some("uv"),
        "Dockerfile" | "Containerfile" => Some("docker"),
        "docker-compose.yml" | "docker-compose.yaml" | "compose.yml" | "compose.yaml" => {
            Some("docker-compose")
        }
        "flake.nix" | "flake.lock" => Some("nix"),
        "devcontainer.json" => Some("devcontainers"),
        "Package.swift" => Some("swift"),
        "pubspec.yaml" => Some("pub"),
        ".gitmodules" => Some("gitsubmodule"),
        _ => {
            if name.ends_with(".csproj") {
                Some("nuget")
            } else if name.ends_with(".tf") {
                Some("terraform")
            } else if name.ends_with(".dockerfile") {
                Some("docker")
            } else {
                None
            }
        }
    }
}

/// Convert a path relative to repo root into Dependabot directory form ("/", "/services/api").
fn to_dependabot_dir(rel: &Path) -> String {
    let parent = rel.parent().unwrap_or(Path::new(""));
    let s = parent.to_string_lossy().replace('\\', "/");
    if s.is_empty() {
        "/".to_string()
    } else {
        format!("/{s}")
    }
}

/// Scan `root` for package manifests and return deduplicated Dependabot update
/// entries (sorted by directory then ecosystem) plus npm workspace layout.
/// Respects `.gitignore`.
pub fn detect_ecosystems(root: &Path) -> EcosystemScan {
    let mut hits: BTreeSet<EcosystemHit> = BTreeSet::new();
    // Terraform/OpenTofu share file layout (.tf, .terraform.lock.hcl) but are
    // separate Dependabot ecosystems — collect dirs first, decide after the walk.
    let mut terraform_family_dirs: BTreeSet<String> = BTreeSet::new();
    let mut terraform_lock_dirs: BTreeSet<String> = BTreeSet::new();
    let mut opentofu_dirs: BTreeSet<String> = BTreeSet::new();

    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .follow_links(false)
        .max_depth(Some(8))
        .build();

    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let rel = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if crate::discovery::is_ignored_dir(rel) {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        // GitHub Actions workflows: any yml/yaml under .github/workflows → single "/" entry
        if rel.starts_with(".github/workflows")
            && (name.ends_with(".yml") || name.ends_with(".yaml"))
        {
            hits.insert(EcosystemHit {
                ecosystem: "github-actions".into(),
                directory: "/".into(),
            });
            continue;
        }

        if name == ".terraform.lock.hcl" {
            let dir = to_dependabot_dir(rel);
            // OpenTofu pins providers from its own registry in the lockfile
            if std::fs::read_to_string(path)
                .map(|s| s.contains("registry.opentofu.org"))
                .unwrap_or(false)
            {
                opentofu_dirs.insert(dir.clone());
            }
            terraform_lock_dirs.insert(dir.clone());
            terraform_family_dirs.insert(dir);
            continue;
        }
        if name == ".opentofu-version" || name == ".tofu-version" {
            opentofu_dirs.insert(to_dependabot_dir(rel));
            continue;
        }
        if name.ends_with(".tofu") {
            let dir = to_dependabot_dir(rel);
            opentofu_dirs.insert(dir.clone());
            terraform_family_dirs.insert(dir);
            continue;
        }

        let Some(ecosystem) = ecosystem_for_file(name) else {
            continue;
        };

        let mut directory = to_dependabot_dir(rel);
        if ecosystem == "terraform" {
            terraform_family_dirs.insert(directory);
            continue;
        }
        // Dependabot scans .devcontainer/ from the parent directory
        if ecosystem == "devcontainers" {
            directory = directory
                .strip_suffix("/.devcontainer")
                .map(|s| if s.is_empty() { "/" } else { s })
                .unwrap_or(&directory)
                .to_string();
        }

        hits.insert(EcosystemHit {
            ecosystem: ecosystem.into(),
            directory,
        });
    }

    // Terraform/OpenTofu: local modules without lockfiles are managed by their
    // root module — when lockfiles exist, only report the directories that have one
    if !terraform_lock_dirs.is_empty() {
        terraform_family_dirs.retain(|d| terraform_lock_dirs.contains(d));
    }
    for dir in terraform_family_dirs {
        // Directory naming ("infra/opentofu") counts as tofu evidence too
        let is_tofu = opentofu_dirs.contains(&dir)
            || dir
                .split('/')
                .any(|part| part == "opentofu" || part == "tofu");
        hits.insert(EcosystemHit {
            ecosystem: if is_tofu { "opentofu" } else { "terraform" }.into(),
            directory: dir,
        });
    }

    let mut hits: Vec<EcosystemHit> = hits.into_iter().collect();

    // Prefer uv over pip when uv.lock lives in the same directory
    let uv_dirs: BTreeSet<String> = hits
        .iter()
        .filter(|h| h.ecosystem == "uv")
        .map(|h| h.directory.clone())
        .collect();
    hits.retain(|h| !(h.ecosystem == "pip" && uv_dirs.contains(&h.directory)));

    // Cargo workspace roots cover nested members: one entry at "/" is enough.
    // npm workspace members stay in `hits` (so missing coverage can name
    // them); config generation collapses them via `config_hits()`.
    if cargo_workspace_at_root(root) {
        hits.retain(|h| h.ecosystem != "cargo" || h.directory == "/");
    }
    let (npm_workspace_globs, npm_workspace_negations) = npm_workspace_globs(root);

    EcosystemScan {
        hits,
        npm_workspace_globs,
        npm_workspace_negations,
    }
}

/// True if root `Cargo.toml` declares a `[workspace]`.
fn cargo_workspace_at_root(root: &Path) -> bool {
    std::fs::read_to_string(root.join("Cargo.toml"))
        .map(|s| {
            s.lines()
                .any(|l| l.trim() == "[workspace]" || l.trim().starts_with("[workspace."))
        })
        .unwrap_or(false)
}

/// npm workspace member globs at `root`, as (positive, negated) lists in
/// Dependabot dir form. Sources: pnpm-workspace.yaml `packages:` and root
/// package.json `workspaces` (both the array and `{"packages": [...]}` forms).
fn npm_workspace_globs(root: &Path) -> (Vec<String>, Vec<String>) {
    let mut positive = Vec::new();
    let mut negated = Vec::new();
    let mut push = |glob: &str| {
        if let Some(rest) = glob.strip_prefix('!') {
            negated.push(normalize_dir(rest));
        } else {
            positive.push(normalize_dir(glob));
        }
    };

    if let Ok(text) = std::fs::read_to_string(root.join("pnpm-workspace.yaml")) {
        if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(&text) {
            for glob in yaml
                .get("packages")
                .and_then(|v| v.as_sequence())
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
            {
                push(glob);
            }
        }
    }

    if let Ok(text) = std::fs::read_to_string(root.join("package.json")) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            let ws = json.get("workspaces");
            let globs = ws.and_then(|w| w.as_array()).or_else(|| {
                ws.and_then(|w| w.get("packages"))
                    .and_then(|p| p.as_array())
            });
            for glob in globs.into_iter().flatten().filter_map(|v| v.as_str()) {
                push(glob);
            }
        }
    }

    (positive, negated)
}

/// Locate `.github/dependabot.yml` (or `.yaml`) under `root`.
pub fn find_config(root: &Path) -> Option<PathBuf> {
    for name in ["dependabot.yml", "dependabot.yaml"] {
        let p = root.join(".github").join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// True if the project uses Renovate instead of Dependabot.
pub fn has_renovate(root: &Path) -> bool {
    const RENOVATE_FILES: &[&str] = &[
        "renovate.json",
        "renovate.json5",
        ".renovaterc",
        ".renovaterc.json",
        ".renovaterc.json5",
        ".github/renovate.json",
        ".github/renovate.json5",
    ];
    RENOVATE_FILES.iter().any(|f| root.join(f).is_file())
}

#[derive(Deserialize)]
struct DependabotConfig {
    #[serde(default)]
    updates: Vec<DependabotUpdate>,
}

#[derive(Deserialize)]
struct DependabotUpdate {
    #[serde(rename = "package-ecosystem")]
    package_ecosystem: Option<String>,
    directory: Option<String>,
    #[serde(default)]
    directories: Vec<String>,
}

/// Normalize a Dependabot directory value: leading "/", no trailing "/" (except root).
fn normalize_dir(dir: &str) -> String {
    let d = dir.trim();
    let d = if d.starts_with('/') {
        d.to_string()
    } else {
        format!("/{d}")
    };
    let trimmed = d.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Match a Dependabot directory pattern (supports `*` and `**` globs) against a directory.
fn dir_matches(pattern: &str, dir: &str) -> bool {
    let pattern = normalize_dir(pattern);
    let dir = normalize_dir(dir);
    if !pattern.contains('*') {
        return pattern == dir;
    }
    let mut re = String::from("^");
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    re.push_str(".*");
                } else {
                    re.push_str("[^/]*");
                }
            }
            _ => re.push_str(&regex::escape(&c.to_string())),
        }
    }
    re.push('$');
    match regex::Regex::new(&re) {
        Ok(r) => r.is_match(&dir),
        Err(_) => pattern == dir,
    }
}

/// Return detected hits that have no matching `updates:` entry in the config.
/// An npm entry for the workspace root ("/") also covers workspace member
/// directories. An unparseable config is treated as covering everything —
/// GitHub itself reports invalid dependabot.yml files, so no extra warning is
/// needed.
pub fn uncovered_hits(config_text: &str, scan: &EcosystemScan) -> Vec<EcosystemHit> {
    let config: DependabotConfig = match serde_yaml::from_str(config_text) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    scan.hits
        .iter()
        .filter(|hit| {
            let covers = |pattern: &str| {
                dir_matches(pattern, &hit.directory)
                    || (hit.ecosystem == "npm"
                        && scan.is_npm_workspace_member(&hit.directory)
                        && dir_matches(pattern, "/"))
            };
            !config.updates.iter().any(|u| {
                if u.package_ecosystem.as_deref() != Some(hit.ecosystem.as_str()) {
                    return false;
                }
                u.directory.as_deref().map(covers).unwrap_or(false)
                    || u.directories.iter().any(|d| covers(d))
            })
        })
        .cloned()
        .collect()
}

/// Generate a `.github/dependabot.yml` covering the given hits (daily schedule).
pub fn generate_config(hits: &[EcosystemHit]) -> String {
    let mut out = String::from("version: 2\nupdates:\n");
    for hit in hits {
        out.push_str(&format!(
            "  - package-ecosystem: \"{}\"\n    directory: \"{}\"\n    schedule:\n      interval: \"daily\"\n",
            hit.ecosystem, hit.directory
        ));
    }
    out
}

/// Detect root-level devenv usage. Dependabot's `nix` ecosystem only covers
/// `flake.lock` inputs, so `devenv.lock` needs a scheduled workflow instead.
pub fn detect_devenv(root: &Path) -> bool {
    ["devenv.nix", "devenv.lock", "devenv.yaml"]
        .iter()
        .any(|f| root.join(f).is_file())
}

/// True if some workflow in `.github/workflows/` already runs `devenv update`.
pub fn has_devenv_update_workflow(root: &Path) -> bool {
    let dir = root.join(".github").join("workflows");
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return false,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_yaml = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e == "yml" || e == "yaml")
            .unwrap_or(false);
        if !is_yaml {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.contains("devenv update") {
                return true;
            }
        }
    }
    false
}

/// GitHub Actions workflow that periodically updates `devenv.lock` and opens a PR.
/// Dependabot's `nix` ecosystem only covers flake.lock inputs, not devenv.
pub const DEVENV_UPDATE_WORKFLOW: &str = r#"# Dependabot's nix ecosystem only covers flake.lock inputs, so this workflow
# keeps devenv.lock updated and opens a PR with the changes.
name: Update devenv lockfile

on:
  schedule:
    - cron: "0 6 * * 1" # Mondays 06:00 UTC
  workflow_dispatch:

permissions:
  contents: write
  pull-requests: write

jobs:
  update:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: DeterminateSystems/nix-installer-action@main
      - name: Update devenv.lock
        run: |
          nix profile install nixpkgs#devenv
          devenv update
      - uses: peter-evans/create-pull-request@v7
        with:
          commit-message: "chore: update devenv.lock"
          title: "chore: update devenv.lock"
          body: "Automated devenv lockfile update (Dependabot does not cover devenv.lock)."
          branch: chore/update-devenv-lock
          delete-branch: true
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn test_detect_ecosystems_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "Cargo.toml", "[package]\nname = \"x\"\n");
        write(root, "services/api/mix.exs", "");
        write(root, "services/api/Dockerfile", "FROM alpine\n");
        write(root, "services/web/package.json", "{}");
        write(root, ".github/workflows/ci.yml", "name: ci\n");

        let hits = detect_ecosystems(root).hits;
        assert!(hits.contains(&EcosystemHit {
            ecosystem: "cargo".into(),
            directory: "/".into()
        }));
        assert!(hits.contains(&EcosystemHit {
            ecosystem: "mix".into(),
            directory: "/services/api".into()
        }));
        assert!(hits.contains(&EcosystemHit {
            ecosystem: "docker".into(),
            directory: "/services/api".into()
        }));
        assert!(hits.contains(&EcosystemHit {
            ecosystem: "npm".into(),
            directory: "/services/web".into()
        }));
        assert!(hits.contains(&EcosystemHit {
            ecosystem: "github-actions".into(),
            directory: "/".into()
        }));
    }

    #[test]
    fn test_cargo_workspace_dedupes_members() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(
            root,
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/a\"]\n",
        );
        write(root, "crates/a/Cargo.toml", "[package]\nname = \"a\"\n");

        let hits = detect_ecosystems(root).hits;
        let cargo: Vec<_> = hits.iter().filter(|h| h.ecosystem == "cargo").collect();
        assert_eq!(cargo.len(), 1);
        assert_eq!(cargo[0].directory, "/");
    }

    #[test]
    fn test_terraform_lockfile_dirs_only() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "infra/envs/prod/main.tf", "");
        write(root, "infra/envs/prod/.terraform.lock.hcl", "");
        write(root, "infra/modules/net/main.tf", "");

        let hits = detect_ecosystems(root).hits;
        let tf: Vec<_> = hits.iter().filter(|h| h.ecosystem == "terraform").collect();
        assert_eq!(tf.len(), 1);
        assert_eq!(tf[0].directory, "/infra/envs/prod");
    }

    #[test]
    fn test_terraform_no_lockfiles_falls_back_to_tf_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "infra/main.tf", "");

        let hits = detect_ecosystems(root).hits;
        assert!(hits.iter().any(|h| h.ecosystem == "terraform"));
    }

    #[test]
    fn test_opentofu_lockfile_registry_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "infra/prod/main.tf", "");
        write(
            root,
            "infra/prod/.terraform.lock.hcl",
            "provider \"registry.opentofu.org/hashicorp/aws\" {\n  version = \"5.0.0\"\n}\n",
        );

        let hits = detect_ecosystems(root).hits;
        assert!(hits.contains(&EcosystemHit {
            ecosystem: "opentofu".into(),
            directory: "/infra/prod".into()
        }));
        assert!(!hits.iter().any(|h| h.ecosystem == "terraform"));
    }

    #[test]
    fn test_opentofu_dir_name_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "infra/opentofu/main.tf", "");

        let hits = detect_ecosystems(root).hits;
        assert!(hits.contains(&EcosystemHit {
            ecosystem: "opentofu".into(),
            directory: "/infra/opentofu".into()
        }));
        assert!(!hits.iter().any(|h| h.ecosystem == "terraform"));
    }

    #[test]
    fn test_tofu_extension_and_version_file_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "infra/main.tofu", "");

        let hits = detect_ecosystems(root).hits;
        assert!(hits.contains(&EcosystemHit {
            ecosystem: "opentofu".into(),
            directory: "/infra".into()
        }));

        let tmp2 = tempfile::tempdir().unwrap();
        let root2 = tmp2.path();
        write(root2, "envs/dev/main.tf", "");
        write(root2, "envs/dev/.opentofu-version", "1.8.0\n");

        let hits2 = detect_ecosystems(root2).hits;
        assert!(hits2.contains(&EcosystemHit {
            ecosystem: "opentofu".into(),
            directory: "/envs/dev".into()
        }));
    }

    #[test]
    fn test_terraform_registry_stays_terraform() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "infra/prod/main.tf", "");
        write(
            root,
            "infra/prod/.terraform.lock.hcl",
            "provider \"registry.terraform.io/hashicorp/aws\" {\n  version = \"5.0.0\"\n}\n",
        );

        let hits = detect_ecosystems(root).hits;
        assert!(hits.contains(&EcosystemHit {
            ecosystem: "terraform".into(),
            directory: "/infra/prod".into()
        }));
        assert!(!hits.iter().any(|h| h.ecosystem == "opentofu"));
    }

    #[test]
    fn test_uv_lock_replaces_pip() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "tool/pyproject.toml", "");
        write(root, "tool/uv.lock", "");

        let hits = detect_ecosystems(root).hits;
        assert!(hits.iter().any(|h| h.ecosystem == "uv"));
        assert!(!hits.iter().any(|h| h.ecosystem == "pip"));
    }

    #[test]
    fn test_gitignored_manifests_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, ".gitignore", "vendor/\n");
        write(root, "vendor/pkg/package.json", "{}");
        write(root, "go.mod", "module x\n");

        let hits = detect_ecosystems(root).hits;
        assert!(!hits.iter().any(|h| h.ecosystem == "npm"));
        assert!(hits.iter().any(|h| h.ecosystem == "gomod"));
    }

    fn scan_of(hits: Vec<EcosystemHit>) -> EcosystemScan {
        EcosystemScan {
            hits,
            ..Default::default()
        }
    }

    #[test]
    fn test_uncovered_hits() {
        let scan = scan_of(vec![
            EcosystemHit {
                ecosystem: "cargo".into(),
                directory: "/".into(),
            },
            EcosystemHit {
                ecosystem: "mix".into(),
                directory: "/services/api".into(),
            },
        ]);
        let config = "version: 2\nupdates:\n  - package-ecosystem: cargo\n    directory: /\n    schedule:\n      interval: weekly\n";
        let missing = uncovered_hits(config, &scan);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].ecosystem, "mix");
    }

    #[test]
    fn test_uncovered_hits_directories_glob() {
        let scan = scan_of(vec![
            EcosystemHit {
                ecosystem: "mix".into(),
                directory: "/services/api".into(),
            },
            EcosystemHit {
                ecosystem: "npm".into(),
                directory: "/services/web".into(),
            },
        ]);
        let config = "version: 2\nupdates:\n  - package-ecosystem: mix\n    directories:\n      - \"/services/*\"\n  - package-ecosystem: npm\n    directory: \"/services/*\"\n";
        assert!(uncovered_hits(config, &scan).is_empty());
    }

    #[test]
    fn test_uncovered_hits_invalid_yaml_covers_all() {
        let scan = scan_of(vec![EcosystemHit {
            ecosystem: "cargo".into(),
            directory: "/".into(),
        }]);
        assert!(uncovered_hits(": not yaml [", &scan).is_empty());
    }

    #[test]
    fn test_generate_config() {
        let scan = scan_of(vec![EcosystemHit {
            ecosystem: "cargo".into(),
            directory: "/".into(),
        }]);
        let yaml = generate_config(&scan.hits);
        assert!(yaml.starts_with("version: 2\n"));
        // Round-trip: generated config covers all hits
        assert!(uncovered_hits(&yaml, &scan).is_empty());
    }

    /// A pnpm monorepo: workspace with member apps, plus a standalone npm
    /// service that is NOT a workspace member.
    #[test]
    fn test_npm_workspace_members_reported_and_covered_by_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "package.json", "{}");
        write(root, "pnpm-lock.yaml", "");
        write(
            root,
            "pnpm-workspace.yaml",
            "packages:\n  - apps/*\n  - packages/*\n",
        );
        write(root, "apps/board/package.json", "{}");
        write(root, "apps/web/package.json", "{}");
        write(root, "services/marketing/package.json", "{}");
        write(root, "services/marketing/package-lock.json", "{}");

        let scan = detect_ecosystems(root);
        for dir in ["/", "/apps/board", "/apps/web", "/services/marketing"] {
            assert!(
                scan.hits
                    .iter()
                    .any(|h| h.ecosystem == "npm" && h.directory == dir),
                "expected npm hit for {dir}: {:?}",
                scan.hits
            );
        }

        // No npm entry at all → every npm dir is uncovered.
        let missing = uncovered_hits("version: 2\nupdates: []\n", &scan);
        let npm_dirs: Vec<&str> = missing
            .iter()
            .filter(|h| h.ecosystem == "npm")
            .map(|h| h.directory.as_str())
            .collect();
        assert_eq!(
            npm_dirs,
            vec!["/", "/apps/board", "/apps/web", "/services/marketing"]
        );

        // Root npm entry covers the workspace members but not the
        // standalone service.
        let config = "version: 2\nupdates:\n  - package-ecosystem: npm\n    directory: /\n";
        let missing = uncovered_hits(config, &scan);
        let npm_dirs: Vec<&str> = missing
            .iter()
            .filter(|h| h.ecosystem == "npm")
            .map(|h| h.directory.as_str())
            .collect();
        assert_eq!(npm_dirs, vec!["/services/marketing"]);

        // Config generation collapses members but keeps the standalone dir.
        let config_dirs: Vec<String> = scan
            .config_hits()
            .into_iter()
            .filter(|h| h.ecosystem == "npm")
            .map(|h| h.directory)
            .collect();
        assert_eq!(config_dirs, vec!["/", "/services/marketing"]);
        assert!(uncovered_hits(&generate_config(&scan.config_hits()), &scan).is_empty());
    }

    #[test]
    fn test_npm_workspace_negated_glob_excludes_member() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(
            root,
            "package.json",
            r#"{"workspaces": {"packages": ["apps/*", "!apps/legacy"]}}"#,
        );
        write(root, "apps/board/package.json", "{}");
        write(root, "apps/legacy/package.json", "{}");

        let scan = detect_ecosystems(root);
        let config = "version: 2\nupdates:\n  - package-ecosystem: npm\n    directory: /\n";
        let missing = uncovered_hits(config, &scan);
        let npm_dirs: Vec<&str> = missing.iter().map(|h| h.directory.as_str()).collect();
        // Excluded member is not covered by the root entry.
        assert_eq!(npm_dirs, vec!["/apps/legacy"]);
    }

    #[test]
    fn test_detect_devenv() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        assert!(!detect_devenv(root));
        write(root, "devenv.nix", "{ }\n");
        assert!(detect_devenv(root));
    }

    #[test]
    fn test_flake_maps_to_nix_ecosystem() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "flake.nix", "{ }\n");
        let hits = detect_ecosystems(root).hits;
        assert!(hits.contains(&EcosystemHit {
            ecosystem: "nix".into(),
            directory: "/".into()
        }));
    }

    #[test]
    fn test_has_devenv_update_workflow() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        assert!(!has_devenv_update_workflow(root));
        write(root, ".github/workflows/devenv.yml", DEVENV_UPDATE_WORKFLOW);
        assert!(has_devenv_update_workflow(root));
    }

    #[test]
    fn test_dir_matches() {
        assert!(dir_matches("/", "/"));
        assert!(dir_matches("/services/*", "/services/api"));
        assert!(!dir_matches("/services/*", "/services/api/nested"));
        assert!(dir_matches("/**", "/services/api/nested"));
        assert!(dir_matches("services/api", "/services/api"));
    }

    #[test]
    fn test_has_renovate() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        assert!(!has_renovate(root));
        write(root, "renovate.json", "{}");
        assert!(has_renovate(root));
    }
}

use std::path::{Path, PathBuf};

use crate::document::Document;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;

/// Language breakdown with percentage and line count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub name: String,
    pub percentage: f64,
    pub lines: Option<u64>,
}

/// Deployment platform information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentInfo {
    pub platform: String,
    pub detected_from: String, // e.g., "Procfile", "heroku.yml"
}

/// Rich technology stack information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechStack {
    pub primary_language: String,
    pub languages: Vec<LanguageInfo>,
    pub lines_of_code: Option<u64>,
    pub dependencies_count: Option<u64>,
    pub license: Option<String>,
    pub repo_size: Option<String>,
    pub frameworks: Vec<String>,
    pub deployment: Option<DeploymentInfo>,
    pub database: Option<String>,
    pub language_version: Option<String>,
    pub framework_versions: Vec<(String, String)>, // (framework, version)
}

impl TechStack {
    /// Validate that TechStack has meaningful data.
    pub fn is_valid(&self) -> bool {
        !self.languages.is_empty() && self.primary_language != "Unknown"
    }

    /// Render compact version for table cells: "Rust" or "Rust (+3)" for multi-language.
    pub fn render_table_cell(&self) -> String {
        if self.languages.len() <= 1 {
            self.primary_language.clone()
        } else {
            format!("{} (+{})", self.primary_language, self.languages.len() - 1)
        }
    }

    /// Create a simple TechStack from a single language name (for fallback).
    pub fn from_simple_string(s: &str) -> Self {
        let lang_info = LanguageInfo {
            name: s.to_string(),
            percentage: 100.0,
            lines: None,
        };
        TechStack {
            primary_language: s.to_string(),
            languages: vec![lang_info],
            lines_of_code: None,
            dependencies_count: None,
            license: None,
            repo_size: None,
            frameworks: Vec::new(),
            deployment: None,
            database: None,
            language_version: None,
            framework_versions: Vec::new(),
        }
    }
}

/// Onefetch v2.26+ JSON output structures.
/// The new format uses `infoFields` array with tagged enum variants.
#[derive(Debug, Deserialize)]
struct OnefetchOutput {
    #[serde(rename = "infoFields")]
    info_fields: Option<Vec<OnefetchInfoField>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OnefetchInfoField {
    Languages(OnefetchLanguagesWrapper),
    Loc(OnefetchLocWrapper),
    Dependencies(OnefetchDependenciesWrapper),
    License(OnefetchLicenseWrapper),
    Size(OnefetchSizeWrapper),
    #[allow(dead_code)]
    Other(serde_json::Value),
}

#[derive(Debug, Deserialize)]
struct OnefetchLanguagesWrapper {
    #[serde(rename = "LanguagesInfo")]
    languages_info: OnefetchLanguagesInfo,
}

#[derive(Debug, Deserialize)]
struct OnefetchLanguagesInfo {
    #[serde(rename = "languagesWithPercentage")]
    languages_with_percentage: Vec<OnefetchLanguage>,
}

#[derive(Debug, Deserialize)]
struct OnefetchLocWrapper {
    #[serde(rename = "LocInfo")]
    loc_info: OnefetchLocInfo,
}

#[derive(Debug, Deserialize)]
struct OnefetchLocInfo {
    #[serde(rename = "linesOfCode")]
    lines_of_code: u64,
}

#[derive(Debug, Deserialize)]
struct OnefetchDependenciesWrapper {
    #[serde(rename = "DependenciesInfo")]
    dependencies_info: OnefetchDependenciesInfo,
}

#[derive(Debug, Deserialize)]
struct OnefetchDependenciesInfo {
    dependencies: String,
}

#[derive(Debug, Deserialize)]
struct OnefetchLicenseWrapper {
    #[serde(rename = "LicenseInfo")]
    license_info: OnefetchLicenseInfo,
}

#[derive(Debug, Deserialize)]
struct OnefetchLicenseInfo {
    license: String,
}

#[derive(Debug, Deserialize)]
struct OnefetchSizeWrapper {
    #[serde(rename = "SizeInfo")]
    size_info: OnefetchSizeInfo,
}

#[derive(Debug, Deserialize)]
struct OnefetchSizeInfo {
    #[serde(rename = "repoSize")]
    repo_size: String,
}

#[derive(Debug, Deserialize)]
struct OnefetchLanguage {
    language: String,
    percentage: f64,
}

/// Service metadata extracted from a service README.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMetadata {
    /// Service name (extracted from first H1 heading)
    pub name: String,
    /// Service status (Live, Beta, Sunset, Deprecated, Planned)
    pub status: String,
    /// Service owner (@handle)
    pub owner: String,
    /// Technology stack (inferred from service directory)
    pub tech_stack: TechStack,
    /// Service description (from preamble)
    pub description: String,
    /// Relative path to service README
    pub readme_path: String,
    /// First commit date (ISO 8601) for age calculation
    pub created_at: Option<String>,
    /// Number of commits touching this service directory
    pub commit_count: Option<u64>,
    /// Most recent commit date (ISO 8601) for this service directory
    pub last_commit_at: Option<String>,
    /// Engineering practices (linter, tests)
    pub practices: EngineeringPractices,
}

/// Detect the creation date of a service directory from git history.
/// Returns the ISO 8601 date of the first commit touching this directory.
pub fn detect_created_at(service_dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args([
            "log",
            "--reverse",
            "--format=%aI",
            "--diff-filter=A",
            "--",
            ".",
        ])
        .current_dir(service_dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout
        .lines()
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Count commits touching a service directory.
/// For submodules, counts commits inside the submodule.
/// For monorepo folders, counts only commits that modified files in that folder.
pub fn count_commits(service_dir: &Path) -> Option<u64> {
    // Check if this is a git submodule (has its own .git)
    let is_submodule = service_dir.join(".git").exists();

    let output = if is_submodule {
        // Submodule: count all commits in its own repo
        Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .current_dir(service_dir)
            .output()
            .ok()?
    } else {
        // Monorepo: count commits that touched files in this folder only
        Command::new("git")
            .args(["rev-list", "--count", "HEAD", "--", "."])
            .current_dir(service_dir)
            .output()
            .ok()?
    };

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .parse()
        .ok()
        .filter(|&n: &u64| n > 0)
}

/// Get the most recent commit date for a service directory.
pub fn detect_last_commit_at(service_dir: &Path) -> Option<String> {
    let is_submodule = service_dir.join(".git").exists();

    let output = if is_submodule {
        Command::new("git")
            .args(["log", "-1", "--format=%aI"])
            .current_dir(service_dir)
            .output()
            .ok()?
    } else {
        Command::new("git")
            .args(["log", "-1", "--format=%aI", "--", "."])
            .current_dir(service_dir)
            .output()
            .ok()?
    };

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout)
        .ok()?
        .lines()
        .next()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Discover all service README files in the services/ directory.
pub fn discover_service_readmes(dir: &Path) -> Result<Vec<PathBuf>> {
    discover_readmes_in_dir(&dir.join("services"))
}

/// Discover all app README files in the apps/ directory.
pub fn discover_app_readmes(dir: &Path) -> Result<Vec<PathBuf>> {
    discover_readmes_in_dir(&dir.join("apps"))
}

/// Discover all infra README files in the infra/ directory.
pub fn discover_infra_readmes(dir: &Path) -> Result<Vec<PathBuf>> {
    discover_readmes_in_dir(&dir.join("infra"))
}

/// Helper to discover README files in a directory (services/ or apps/).
fn discover_readmes_in_dir(target_dir: &Path) -> Result<Vec<PathBuf>> {
    if !target_dir.exists() {
        return Ok(Vec::new());
    }

    let mut readmes = Vec::new();

    // Look for */README.md
    if let Ok(entries) = std::fs::read_dir(target_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let readme = path.join("README.md");
                if readme.exists() {
                    readmes.push(readme);
                }
            }
        }
    }

    Ok(readmes)
}

/// Extract service metadata from a service README file.
pub fn extract_service_metadata(path: &Path, root: &Path) -> Result<ServiceMetadata> {
    let doc = Document::from_file(path)?;
    let parsed = doc.parse_body();

    // Prefer frontmatter for name, fallback to H1 heading, then folder name
    let fm = doc.frontmatter.as_ref();
    let name = fm
        .and_then(|f| f.get_display("name"))
        .or_else(|| extract_service_name(&parsed).ok())
        .unwrap_or_else(|| {
            // Fallback: capitalize folder name (e.g. "core" → "Core")
            let folder = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            titlecase_slug(folder)
        });

    let status = fm
        .and_then(|f| f.get_display("status"))
        .or_else(|| {
            parsed
                .find_section("Status")
                .map(|s| extract_non_comment_content(&s.content))
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "Unknown".to_string());

    let owner = fm
        .and_then(|f| f.get_display("owner"))
        .or_else(|| {
            parsed
                .find_section("Owner")
                .map(|s| extract_non_comment_content(&s.content))
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "Unknown".to_string());

    // Extract description: text between H1 and first H2
    let description = doc
        .body
        .lines()
        .skip_while(|l| !l.starts_with("# "))
        .skip(1) // skip the H1 line itself
        .take_while(|l| !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    // Infer tech stack from service directory
    let service_dir = path.parent().unwrap_or(path);
    let tech_stack = extract_tech_stack(service_dir);

    // Detect engineering practices
    let practices = detect_engineering_practices(service_dir, &tech_stack.primary_language, fm);

    // Detect git history
    let created_at = detect_created_at(service_dir);
    let commit_count = count_commits(service_dir);
    let last_commit_at = detect_last_commit_at(service_dir);

    // Get relative path
    let readme_path = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();

    Ok(ServiceMetadata {
        name,
        status,
        owner,
        tech_stack,
        description,
        readme_path,
        created_at,
        commit_count,
        last_commit_at,
        practices,
    })
}

/// Extract service name from the first H1 heading in the README.
/// Convert a slug like "my-service" to "My Service".
fn titlecase_slug(slug: &str) -> String {
    slug.split('-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_service_name(parsed: &crate::document::ParsedBody) -> Result<String> {
    // Only use H1 headings — H2+ are section headers (Status, Owner, etc.)
    for section in &parsed.sections {
        if section.level == 1 {
            return Ok(section.heading.clone());
        }
    }

    Err(crate::error::Error::InvalidFieldValue(
        "No heading found in README".to_string(),
    ))
}

/// Extract content from a section, filtering out HTML comments.
fn extract_non_comment_content(content: &str) -> String {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("<!--") && !trimmed.is_empty()
        })
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Extract rich technology stack information from service directory.
/// Tries onefetch JSON first, falls back to file detection.
pub fn extract_tech_stack(service_dir: &Path) -> TechStack {
    // Try onefetch JSON parsing first
    if let Some(tech_stack) = try_onefetch_json(service_dir) {
        return tech_stack;
    }

    // Fall back to file detection
    fallback_tech_stack_detection(service_dir)
}

/// Try to extract tech stack from onefetch JSON output (v2.26+ format).
fn try_onefetch_json(service_dir: &Path) -> Option<TechStack> {
    let output = std::process::Command::new("onefetch")
        .arg("--output")
        .arg("json")
        .arg("--no-color-palette")
        .current_dir(service_dir)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json_str = String::from_utf8(output.stdout).ok()?;
    let onefetch: OnefetchOutput = serde_json::from_str(&json_str).ok()?;

    let fields = onefetch.info_fields?;

    // Extract data from info fields
    let mut languages = Vec::new();
    let mut lines_of_code = None;
    let mut dependencies_count = None;
    let mut license = None;
    let mut repo_size = None;

    for field in &fields {
        match field {
            OnefetchInfoField::Languages(w) => {
                languages = w
                    .languages_info
                    .languages_with_percentage
                    .iter()
                    .filter(|lang| !is_non_language(&lang.language))
                    .map(|lang| LanguageInfo {
                        name: lang.language.clone(),
                        percentage: lang.percentage,
                        lines: None,
                    })
                    .collect();
            }
            OnefetchInfoField::Loc(w) => {
                lines_of_code = Some(w.loc_info.lines_of_code);
            }
            OnefetchInfoField::Dependencies(w) => {
                // Format: "6 (Npm)" — extract the number
                if let Some(num_str) = w.dependencies_info.dependencies.split_whitespace().next() {
                    dependencies_count = num_str.parse().ok();
                }
            }
            OnefetchInfoField::License(w) => {
                let l = &w.license_info.license;
                if !l.is_empty() {
                    license = Some(l.clone());
                }
            }
            OnefetchInfoField::Size(w) => {
                repo_size = Some(w.size_info.repo_size.clone());
            }
            OnefetchInfoField::Other(_) => {}
        }
    }

    if languages.is_empty() {
        return None;
    }

    let primary_language = languages[0].name.clone();

    // Detect frameworks based on primary language
    let frameworks = detect_frameworks(service_dir, &primary_language);

    // Detect deployment platform
    let deployment = detect_deployment_platform(service_dir);

    // Detect database
    let database = detect_database(service_dir, &primary_language);

    // Detect versions
    let (language_version, framework_versions) =
        detect_versions(service_dir, &primary_language, &frameworks);

    // If onefetch didn't report deps, count from manifest files
    if dependencies_count.is_none() {
        dependencies_count = count_dependencies(service_dir, &primary_language);
    }

    Some(TechStack {
        primary_language,
        languages,
        lines_of_code,
        dependencies_count,
        license,
        repo_size,
        frameworks,
        deployment,
        database,
        language_version,
        framework_versions,
    })
}

/// Names reported by onefetch that are not programming languages.
fn is_non_language(name: &str) -> bool {
    matches!(
        name,
        "Dockerfile"
            | "Makefile"
            | "CMake"
            | "Markdown"
            | "JSON"
            | "YAML"
            | "TOML"
            | "XML"
            | "Plain Text"
            | "Text"
            | "INI"
            | "CSV"
            | "Nix"
            | "Meson"
    )
}

/// Count dependencies from manifest files when onefetch doesn't report them.
fn count_dependencies(service_dir: &Path, primary_lang: &str) -> Option<u64> {
    match primary_lang {
        "Go" => count_go_deps(service_dir),
        "Python" => count_python_deps(service_dir),
        "Ruby" => count_ruby_deps(service_dir),
        "Elixir" => count_elixir_deps(service_dir),
        "PHP" => count_php_deps(service_dir),
        "Rust" => count_rust_deps(service_dir),
        "JavaScript" | "TypeScript" => count_node_deps(service_dir),
        _ => None,
    }
}

fn count_go_deps(dir: &Path) -> Option<u64> {
    let content = std::fs::read_to_string(dir.join("go.mod")).ok()?;
    let count = content
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty()
                && !t.starts_with("module")
                && !t.starts_with("go ")
                && !t.starts_with("require")
                && !t.starts_with("//")
                && !t.starts_with(')')
                && !t.starts_with("replace")
                && !t.starts_with("exclude")
                && !t.starts_with("retract")
                && !t.starts_with("toolchain")
                && t.contains(' ')
        })
        .count() as u64;
    if count > 0 {
        Some(count)
    } else {
        None
    }
}

fn count_python_deps(dir: &Path) -> Option<u64> {
    // Try pyproject.toml first
    if let Ok(content) = std::fs::read_to_string(dir.join("pyproject.toml")) {
        let mut in_deps = false;
        let mut count = 0u64;
        for line in content.lines() {
            let t = line.trim();
            if t == "[project]" || t.starts_with("dependencies") {
                in_deps = true;
                continue;
            }
            if t.starts_with('[') && in_deps {
                break;
            }
            if in_deps && t.starts_with('"') {
                count += 1;
            }
        }
        if count > 0 {
            return Some(count);
        }
    }
    // Fall back to requirements.txt
    for req_file in &["requirements.txt", "requirements/base.txt"] {
        if let Ok(content) = std::fs::read_to_string(dir.join(req_file)) {
            let count = content
                .lines()
                .filter(|l| {
                    let t = l.trim();
                    !t.is_empty() && !t.starts_with('#') && !t.starts_with('-')
                })
                .count() as u64;
            if count > 0 {
                return Some(count);
            }
        }
    }
    None
}

fn count_ruby_deps(dir: &Path) -> Option<u64> {
    let content = std::fs::read_to_string(dir.join("Gemfile")).ok()?;
    let count = content
        .lines()
        .filter(|l| l.trim().starts_with("gem "))
        .count() as u64;
    if count > 0 {
        Some(count)
    } else {
        None
    }
}

fn count_elixir_deps(dir: &Path) -> Option<u64> {
    let content = std::fs::read_to_string(dir.join("mix.exs")).ok()?;
    // Count {:dep_name, ...} patterns in defp deps section
    let count = content
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.starts_with("{:") && t.contains(',')
        })
        .count() as u64;
    if count > 0 {
        Some(count)
    } else {
        None
    }
}

fn count_php_deps(dir: &Path) -> Option<u64> {
    let content = std::fs::read_to_string(dir.join("composer.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let require = json.get("require")?.as_object()?;
    // Exclude "php" itself
    let count = require.keys().filter(|k| *k != "php").count() as u64;
    if count > 0 {
        Some(count)
    } else {
        None
    }
}

fn count_rust_deps(dir: &Path) -> Option<u64> {
    let content = std::fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    let mut in_deps = false;
    let mut count = 0u64;
    for line in content.lines() {
        let t = line.trim();
        if t == "[dependencies]" || t.starts_with("[dependencies.") {
            in_deps = true;
            continue;
        }
        if t.starts_with('[') {
            in_deps = false;
            continue;
        }
        if in_deps && t.contains('=') && !t.starts_with('#') {
            count += 1;
        }
    }
    if count > 0 {
        Some(count)
    } else {
        None
    }
}

fn count_node_deps(dir: &Path) -> Option<u64> {
    let content = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let deps = json.get("dependencies").and_then(|d| d.as_object());
    let count = deps.map(|d| d.len() as u64).unwrap_or(0);
    if count > 0 {
        Some(count)
    } else {
        None
    }
}

/// Detect frameworks based on language and project files.
fn detect_frameworks(service_dir: &Path, primary_lang: &str) -> Vec<String> {
    let mut frameworks = Vec::new();

    match primary_lang {
        "JavaScript" | "TypeScript" => {
            // Check package.json for frameworks
            if let Ok(content) = std::fs::read_to_string(service_dir.join("package.json")) {
                if let Ok(package) = serde_json::from_str::<serde_json::Value>(&content) {
                    let deps = package.get("dependencies").and_then(|d| d.as_object());
                    let dev_deps = package.get("devDependencies").and_then(|d| d.as_object());

                    let all_deps: Vec<String> = deps
                        .iter()
                        .chain(dev_deps.iter())
                        .flat_map(|d| d.keys())
                        .map(|k| k.to_string())
                        .collect();

                    // Check for common frameworks
                    if all_deps.contains(&"react".to_string()) {
                        frameworks.push("React".to_string());
                    }
                    if all_deps.contains(&"next".to_string()) {
                        frameworks.push("Next.js".to_string());
                    }
                    if all_deps.contains(&"vue".to_string()) {
                        frameworks.push("Vue".to_string());
                    }
                    if all_deps.contains(&"express".to_string()) {
                        frameworks.push("Express".to_string());
                    }
                    if all_deps.contains(&"@nestjs/core".to_string()) {
                        frameworks.push("NestJS".to_string());
                    }
                    if all_deps.contains(&"nuxt".to_string()) {
                        frameworks.push("Nuxt".to_string());
                    }
                    if all_deps.contains(&"svelte".to_string()) {
                        frameworks.push("Svelte".to_string());
                    }
                    if all_deps.contains(&"react-native".to_string()) {
                        frameworks.push("React Native".to_string());
                    }
                    if all_deps.contains(&"expo".to_string()) {
                        frameworks.push("Expo".to_string());
                    }
                }
            }
        }
        "Python" => {
            // Check for Django
            if service_dir.join("manage.py").exists() {
                frameworks.push("Django".to_string());
            }
            // Check requirements files
            for req_file in &[
                "requirements.txt",
                "requirements/base.txt",
                "pyproject.toml",
            ] {
                if let Ok(content) = std::fs::read_to_string(service_dir.join(req_file)) {
                    if content.contains("flask") {
                        frameworks.push("Flask".to_string());
                    }
                    if content.contains("fastapi") {
                        frameworks.push("FastAPI".to_string());
                    }
                }
            }
        }
        "Ruby" => {
            if let Ok(content) = std::fs::read_to_string(service_dir.join("Gemfile")) {
                if content.contains("rails") {
                    frameworks.push("Rails".to_string());
                }
                if content.contains("sinatra") {
                    frameworks.push("Sinatra".to_string());
                }
            }
        }
        "Rust" => {
            if let Ok(content) = std::fs::read_to_string(service_dir.join("Cargo.toml")) {
                if content.contains("axum") {
                    frameworks.push("Axum".to_string());
                }
                if content.contains("actix-web") {
                    frameworks.push("Actix".to_string());
                }
                if content.contains("rocket") {
                    frameworks.push("Rocket".to_string());
                }
            }
        }
        "Go" => {
            if let Ok(content) = std::fs::read_to_string(service_dir.join("go.mod")) {
                if content.contains("gin-gonic/gin") {
                    frameworks.push("Gin".to_string());
                }
                if content.contains("gofiber/fiber") {
                    frameworks.push("Fiber".to_string());
                }
                if content.contains("echo") {
                    frameworks.push("Echo".to_string());
                }
            }
        }
        _ => {}
    }

    frameworks.dedup();
    frameworks
}

/// Detect deployment platform from service directory.
fn detect_deployment_platform(service_dir: &Path) -> Option<DeploymentInfo> {
    // Check for Heroku
    if service_dir.join("Procfile").exists() {
        return Some(DeploymentInfo {
            platform: "Heroku".to_string(),
            detected_from: "Procfile".to_string(),
        });
    }

    if service_dir.join("heroku.yml").exists() {
        return Some(DeploymentInfo {
            platform: "Heroku".to_string(),
            detected_from: "heroku.yml".to_string(),
        });
    }

    if service_dir.join("app.json").exists() {
        // app.json is commonly used for Heroku
        if let Ok(content) = std::fs::read_to_string(service_dir.join("app.json")) {
            if content.contains("heroku") || content.contains("buildpacks") {
                return Some(DeploymentInfo {
                    platform: "Heroku".to_string(),
                    detected_from: "app.json".to_string(),
                });
            }
        }
    }

    // Check for other platforms
    if service_dir.join("vercel.json").exists() || service_dir.join(".vercel").exists() {
        return Some(DeploymentInfo {
            platform: "Vercel".to_string(),
            detected_from: "vercel.json".to_string(),
        });
    }

    if service_dir.join("netlify.toml").exists() {
        return Some(DeploymentInfo {
            platform: "Netlify".to_string(),
            detected_from: "netlify.toml".to_string(),
        });
    }

    if service_dir.join("Dockerfile").exists() {
        return Some(DeploymentInfo {
            platform: "Docker".to_string(),
            detected_from: "Dockerfile".to_string(),
        });
    }

    if service_dir.join(".platform").exists() {
        return Some(DeploymentInfo {
            platform: "AWS Elastic Beanstalk".to_string(),
            detected_from: ".platform".to_string(),
        });
    }

    None
}

/// Detect database from service directory.
fn detect_database(service_dir: &Path, primary_lang: &str) -> Option<String> {
    // Check database.yml for Rails
    if primary_lang == "Ruby" {
        if let Ok(content) = std::fs::read_to_string(service_dir.join("config/database.yml")) {
            if content.contains("postgresql") || content.contains("adapter: postgresql") {
                return Some("PostgreSQL".to_string());
            }
            if content.contains("mysql") {
                return Some("MySQL".to_string());
            }
            if content.contains("sqlite") {
                return Some("SQLite".to_string());
            }
        }
    }

    // Check package.json dependencies
    if let Ok(content) = std::fs::read_to_string(service_dir.join("package.json")) {
        if let Ok(package) = serde_json::from_str::<serde_json::Value>(&content) {
            let deps = package.get("dependencies").and_then(|d| d.as_object());
            let dev_deps = package.get("devDependencies").and_then(|d| d.as_object());

            let all_deps: Vec<String> = deps
                .iter()
                .chain(dev_deps.iter())
                .flat_map(|d| d.keys())
                .map(|k| k.to_string())
                .collect();

            if all_deps
                .iter()
                .any(|d| d.contains("pg") || d.contains("postgres"))
            {
                return Some("PostgreSQL".to_string());
            }
            if all_deps.iter().any(|d| d.contains("mysql")) {
                return Some("MySQL".to_string());
            }
            if all_deps.iter().any(|d| d.contains("mongodb")) {
                return Some("MongoDB".to_string());
            }
            if all_deps.iter().any(|d| d.contains("redis")) {
                return Some("Redis".to_string());
            }
        }
    }

    // Check requirements.txt for Python
    if let Ok(content) = std::fs::read_to_string(service_dir.join("requirements.txt")) {
        if content.contains("psycopg") || content.contains("postgresql") {
            return Some("PostgreSQL".to_string());
        }
        if content.contains("mysql") {
            return Some("MySQL".to_string());
        }
        if content.contains("pymongo") {
            return Some("MongoDB".to_string());
        }
    }

    None
}

/// Detect language and framework versions.
fn detect_versions(
    service_dir: &Path,
    primary_lang: &str,
    frameworks: &[String],
) -> (Option<String>, Vec<(String, String)>) {
    let mut language_version = None;
    let mut framework_versions = Vec::new();

    match primary_lang {
        "Ruby" => {
            // Check .ruby-version
            if let Ok(content) = std::fs::read_to_string(service_dir.join(".ruby-version")) {
                language_version = Some(content.trim().to_string());
            }

            // Check Gemfile.lock for Rails version
            if frameworks.contains(&"Rails".to_string()) {
                if let Ok(content) = std::fs::read_to_string(service_dir.join("Gemfile.lock")) {
                    // Look for "    rails (X.Y.Z)"
                    if let Some(line) = content.lines().find(|l| l.trim().starts_with("rails (")) {
                        if let Some(version) =
                            line.split('(').nth(1).and_then(|s| s.split(')').next())
                        {
                            framework_versions.push(("Rails".to_string(), version.to_string()));
                        }
                    }
                }
            }
        }
        "JavaScript" | "TypeScript" | "Node.js" => {
            // Check package.json for versions
            if let Ok(content) = std::fs::read_to_string(service_dir.join("package.json")) {
                if let Ok(package) = serde_json::from_str::<serde_json::Value>(&content) {
                    // Node version from engines
                    if let Some(engines) = package.get("engines") {
                        if let Some(node) = engines.get("node").and_then(|v| v.as_str()) {
                            language_version = Some(node.to_string());
                        }
                    }

                    // Framework versions from dependencies
                    if let Some(deps) = package.get("dependencies").and_then(|d| d.as_object()) {
                        for framework in frameworks {
                            let key = framework.to_lowercase().replace(".js", "");
                            if let Some(version) = deps.get(&key).and_then(|v| v.as_str()) {
                                framework_versions.push((
                                    framework.clone(),
                                    version
                                        .trim_start_matches('^')
                                        .trim_start_matches('~')
                                        .to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }
        "Python" => {
            // Check .python-version or runtime.txt
            if let Ok(content) = std::fs::read_to_string(service_dir.join(".python-version")) {
                language_version = Some(content.trim().to_string());
            } else if let Ok(content) = std::fs::read_to_string(service_dir.join("runtime.txt")) {
                if let Some(version) = content.trim().strip_prefix("python-") {
                    language_version = Some(version.to_string());
                }
            }
        }
        "Go" => {
            // Check go.mod for Go version
            if let Ok(content) = std::fs::read_to_string(service_dir.join("go.mod")) {
                if let Some(line) = content.lines().find(|l| l.starts_with("go ")) {
                    if let Some(version) = line.split_whitespace().nth(1) {
                        language_version = Some(version.to_string());
                    }
                }
            }
        }
        _ => {}
    }

    (language_version, framework_versions)
}

/// Returns true if the Gemfile contains a Ruby app framework gem (rails, sinatra, etc.)
/// as opposed to tooling-only gems like fastlane or cocoapods.
fn is_ruby_app_gemfile(dir: &Path) -> bool {
    const APP_GEMS: &[&str] = &["rails", "sinatra", "grape", "hanami", "roda", "padrino"];
    if let Ok(content) = std::fs::read_to_string(dir.join("Gemfile")) {
        APP_GEMS.iter().any(|gem| content.contains(gem))
    } else {
        false
    }
}

/// Fallback tech stack detection using file-based heuristics.
fn fallback_tech_stack_detection(service_dir: &Path) -> TechStack {
    let has_gemfile = service_dir.join("Gemfile").exists();
    let has_package_json = service_dir.join("package.json").exists();

    // If Gemfile exists but contains only tooling gems (fastlane, cocoapods)
    // and package.json exists, skip Ruby and fall through to JavaScript/TypeScript.
    let checks: Vec<(&str, &str)> =
        if has_gemfile && has_package_json && !is_ruby_app_gemfile(service_dir) {
            // Skip Gemfile, prioritise package.json
            vec![
                ("package.json", "JavaScript"),
                ("requirements.txt", "Python"),
                ("Cargo.toml", "Rust"),
                ("go.mod", "Go"),
                ("pom.xml", "Java"),
                ("build.gradle", "Java"),
                ("composer.json", "PHP"),
                (".csproj", ".NET"),
            ]
        } else {
            vec![
                ("Gemfile", "Ruby"),
                ("package.json", "JavaScript"),
                ("requirements.txt", "Python"),
                ("Cargo.toml", "Rust"),
                ("go.mod", "Go"),
                ("pom.xml", "Java"),
                ("build.gradle", "Java"),
                ("composer.json", "PHP"),
                (".csproj", ".NET"),
            ]
        };

    for (file, lang) in &checks {
        if service_dir.join(file).exists() {
            let mut primary = lang.to_string();

            // Upgrade JavaScript → TypeScript if tsconfig.json exists
            if primary == "JavaScript" && service_dir.join("tsconfig.json").exists() {
                primary = "TypeScript".to_string();
            }

            let frameworks = detect_frameworks(service_dir, &primary);

            let deployment = detect_deployment_platform(service_dir);
            let database = detect_database(service_dir, &primary);
            let (language_version, framework_versions) =
                detect_versions(service_dir, &primary, &frameworks);

            return TechStack {
                primary_language: primary.clone(),
                languages: vec![LanguageInfo {
                    name: primary,
                    percentage: 100.0,
                    lines: None,
                }],
                lines_of_code: None,
                dependencies_count: None,
                license: None,
                repo_size: None,
                frameworks,
                deployment,
                database,
                language_version,
                framework_versions,
            };
        }
    }

    let deployment = detect_deployment_platform(service_dir);
    let database = detect_database(service_dir, "Unknown");
    let mut stack = TechStack::from_simple_string("Unknown");
    stack.deployment = deployment;
    stack.database = database;
    stack
}

// ─── Completion check functions ─────────────────────────────────────────────

/// An item found at the project root that should be in a subdirectory.
#[derive(Debug, Clone)]
pub struct MisplacedItem {
    /// File or directory name.
    pub name: String,
    /// Where it should go.
    pub suggested_location: &'static str,
}

/// Source code indicators that should NOT be at the project root.
const ROOT_SOURCE_FILES: &[(&str, &str)] = &[
    ("package.json", "services/<name>/"),
    ("Cargo.toml", "services/<name>/"),
    ("go.mod", "services/<name>/"),
    ("Gemfile", "services/<name>/"),
    ("requirements.txt", "services/<name>/"),
    ("pyproject.toml", "services/<name>/"),
    ("pom.xml", "services/<name>/"),
    ("build.gradle", "services/<name>/"),
    ("composer.json", "services/<name>/"),
    ("mix.exs", "services/<name>/"),
    ("wrangler.toml", "services/<name>/"),
    ("wrangler.jsonc", "services/<name>/"),
    ("vercel.json", "services/<name>/"),
    ("fly.toml", "services/<name>/"),
    ("Dockerfile", "services/<name>/"),
];

const ROOT_SOURCE_DIRS: &[(&str, &str)] = &[
    ("src", "services/<name>/"),
    ("lib", "services/<name>/"),
    ("app", "services/<name>/"),
    ("migrations", "services/<name>/"),
    ("prisma", "services/<name>/"),
];

const ROOT_INFRA_FILES: &[(&str, &str)] = &[("main.tf", "infra/"), ("Pulumi.yaml", "infra/")];

/// Detect source code files/dirs at project root that should be in services/ or infra/.
/// Skips detection if root has a Cargo workspace or npm workspaces.
pub fn detect_misplaced_source_files(root: &Path) -> Vec<MisplacedItem> {
    // Skip if root is a Cargo workspace
    let cargo_toml = root.join("Cargo.toml");
    if cargo_toml.is_file() {
        if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
            if content.contains("[workspace]") {
                return Vec::new();
            }
        }
    }

    // Skip if root has npm workspaces
    let package_json = root.join("package.json");
    if package_json.is_file() {
        if let Ok(content) = std::fs::read_to_string(&package_json) {
            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                if pkg.get("workspaces").is_some() {
                    return Vec::new();
                }
            }
        }
    }

    let mut items = Vec::new();

    for (file, location) in ROOT_SOURCE_FILES {
        if root.join(file).is_file() {
            items.push(MisplacedItem {
                name: file.to_string(),
                suggested_location: location,
            });
        }
    }

    for (dir, location) in ROOT_SOURCE_DIRS {
        if root.join(dir).is_dir() {
            items.push(MisplacedItem {
                name: format!("{dir}/"),
                suggested_location: location,
            });
        }
    }

    for (file, location) in ROOT_INFRA_FILES {
        if root.join(file).is_file() {
            items.push(MisplacedItem {
                name: file.to_string(),
                suggested_location: location,
            });
        }
    }

    // Check for *.tf files at root
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".tf") && name_str != "main.tf" {
                items.push(MisplacedItem {
                    name: name_str.to_string(),
                    suggested_location: "infra/",
                });
            }
        }
    }

    items
}

/// Check if a service/app directory has actual source code.
/// Tries `onefetch --output json` first, falls back to file detection.
pub fn has_code(service_dir: &Path) -> bool {
    // Try onefetch first
    if let Ok(output) = Command::new("onefetch")
        .args(["--output", "json", "--no-color-palette"])
        .current_dir(service_dir)
        .output()
    {
        if output.status.success() {
            if let Ok(json_str) = String::from_utf8(output.stdout) {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                    if let Some(fields) = parsed.get("infoFields").and_then(|v| v.as_array()) {
                        for field in fields {
                            if field.get("LanguagesInfo").is_some() {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback: check for known source files/dirs
    let source_indicators: &[&str] = &[
        "package.json",
        "Cargo.toml",
        "go.mod",
        "Gemfile",
        "requirements.txt",
        "pyproject.toml",
        "pom.xml",
        "build.gradle",
        "composer.json",
        "mix.exs",
        "wrangler.toml",
    ];

    for indicator in source_indicators {
        if service_dir.join(indicator).is_file() {
            return true;
        }
    }

    // Check for src/ or lib/ directories
    service_dir.join("src").is_dir() || service_dir.join("lib").is_dir()
}

/// Engineering practices detected from filesystem + frontmatter overrides.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EngineeringPractices {
    pub has_linter: bool,
    pub linter_tool: Option<String>,
    pub has_tests: bool,
    pub test_framework: Option<String>,
}

/// Detect linter config in service_dir or its parent.
pub fn detect_linter(service_dir: &Path, primary_lang: &str) -> (bool, Option<String>) {
    let dirs_to_check: Vec<&Path> = {
        let mut v = vec![service_dir];
        if let Some(parent) = service_dir.parent() {
            v.push(parent);
        }
        v
    };

    let checks: &[(&[&str], &str)] = match primary_lang {
        "JavaScript" | "TypeScript" => &[(
            &[
                ".eslintrc",
                ".eslintrc.js",
                ".eslintrc.cjs",
                ".eslintrc.json",
                ".eslintrc.yml",
                ".eslintrc.yaml",
                "eslint.config.js",
                "eslint.config.mjs",
                "eslint.config.cjs",
                "eslint.config.ts",
            ],
            "ESLint",
        )],
        "Python" => &[(&["ruff.toml", ".ruff.toml", ".flake8"] as &[&str], "Ruff")],
        "Ruby" => &[(&[".rubocop.yml"] as &[&str], "RuboCop")],
        "Rust" => &[(&["clippy.toml", ".clippy.toml"] as &[&str], "Clippy")],
        "Go" => &[(
            &[".golangci.yml", ".golangci.yaml"] as &[&str],
            "golangci-lint",
        )],
        "Elixir" => &[(&[".credo.exs"] as &[&str], "Credo")],
        _ => return (false, None),
    };

    for (files, tool) in checks {
        for dir in &dirs_to_check {
            for file in *files {
                if dir.join(file).exists() {
                    return (true, Some(tool.to_string()));
                }
            }
        }
    }

    // Check biome.json for JS/TS
    if matches!(primary_lang, "JavaScript" | "TypeScript") {
        for dir in &dirs_to_check {
            if dir.join("biome.json").exists() || dir.join("biome.jsonc").exists() {
                return (true, Some("Biome".to_string()));
            }
        }
    }

    // Check pyproject.toml for [tool.ruff] or [tool.flake8]
    if primary_lang == "Python" {
        for dir in &dirs_to_check {
            let pyproject = dir.join("pyproject.toml");
            if let Ok(content) = std::fs::read_to_string(&pyproject) {
                if content.contains("[tool.ruff]") {
                    return (true, Some("Ruff".to_string()));
                }
                if content.contains("[tool.flake8]") {
                    return (true, Some("Flake8".to_string()));
                }
            }
        }
    }

    // Check Cargo.toml for [lints] section (Rust)
    if primary_lang == "Rust" {
        for dir in &dirs_to_check {
            let cargo = dir.join("Cargo.toml");
            if let Ok(content) = std::fs::read_to_string(&cargo) {
                if content.contains("[lints") {
                    return (true, Some("Clippy".to_string()));
                }
            }
        }
    }

    (false, None)
}

/// Detect test files/directories in service_dir.
pub fn detect_tests(service_dir: &Path, primary_lang: &str) -> (bool, Option<String>) {
    match primary_lang {
        "JavaScript" | "TypeScript" => {
            // Check for test directories
            for dir_name in &["__tests__", "test", "tests"] {
                if service_dir.join(dir_name).is_dir() {
                    let framework = detect_js_test_framework(service_dir);
                    return (true, framework);
                }
            }
            // Check for *.test.* or *.spec.* in src/
            if let Some(found) = find_file_pattern(service_dir, &["test.", "spec."]) {
                if found {
                    let framework = detect_js_test_framework(service_dir);
                    return (true, framework);
                }
            }
            (false, None)
        }
        "Python" => {
            for dir_name in &["tests", "test"] {
                if service_dir.join(dir_name).is_dir() {
                    return (true, Some("pytest".to_string()));
                }
            }
            // Check for test_*.py files
            if let Some(found) = find_file_pattern(service_dir, &["test_"]) {
                if found {
                    return (true, Some("pytest".to_string()));
                }
            }
            (false, None)
        }
        "Ruby" => {
            if service_dir.join("spec").is_dir() {
                return (true, Some("RSpec".to_string()));
            }
            if service_dir.join("test").is_dir() {
                return (true, Some("Minitest".to_string()));
            }
            (false, None)
        }
        "Rust" => {
            if service_dir.join("tests").is_dir() {
                return (true, Some("cargo test".to_string()));
            }
            (false, None)
        }
        "Go" => {
            if let Some(found) = find_file_pattern(service_dir, &["_test.go"]) {
                if found {
                    return (true, Some("go test".to_string()));
                }
            }
            (false, None)
        }
        "Elixir" => {
            if service_dir.join("test").is_dir() {
                return (true, Some("ExUnit".to_string()));
            }
            (false, None)
        }
        _ => (false, None),
    }
}

/// Detect JS test framework from package.json devDependencies.
fn detect_js_test_framework(service_dir: &Path) -> Option<String> {
    let pkg = service_dir.join("package.json");
    let content = std::fs::read_to_string(&pkg).ok()?;
    let lower = content.to_lowercase();
    if lower.contains("vitest") {
        Some("Vitest".to_string())
    } else if lower.contains("jest") {
        Some("Jest".to_string())
    } else if lower.contains("mocha") {
        Some("Mocha".to_string())
    } else {
        None
    }
}

/// Search for files matching any of the given patterns in src/ (bounded walk, stop at first).
fn find_file_pattern(service_dir: &Path, patterns: &[&str]) -> Option<bool> {
    let src = service_dir.join("src");
    let search_dir = if src.is_dir() { &src } else { service_dir };

    let entries = match std::fs::read_dir(search_dir) {
        Ok(e) => e,
        Err(_) => return Some(false),
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        for pattern in patterns {
            if name_str.contains(pattern) {
                return Some(true);
            }
        }
    }
    Some(false)
}

/// Detect engineering practices with frontmatter overrides.
pub fn detect_engineering_practices(
    service_dir: &Path,
    primary_lang: &str,
    frontmatter: Option<&crate::frontmatter::Frontmatter>,
) -> EngineeringPractices {
    let (detected_linter, linter_tool) = detect_linter(service_dir, primary_lang);
    let (detected_tests, test_framework) = detect_tests(service_dir, primary_lang);

    let has_linter = frontmatter
        .and_then(|fm| fm.get_display("has_linter"))
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(detected_linter);

    let has_tests = frontmatter
        .and_then(|fm| fm.get_display("has_tests"))
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(detected_tests);

    EngineeringPractices {
        has_linter,
        linter_tool,
        has_tests,
        test_framework,
    }
}

/// Command specification for running a linter.
#[derive(Debug, Clone)]
pub struct LintCommand {
    pub program: String,
    pub args: Vec<String>,
    pub json_args: Vec<String>,
    pub tool_name: String,
}

/// Result of running a linter.
#[derive(Debug)]
pub struct LintResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub command_not_found: bool,
    pub issues: Vec<LintIssue>,
}

/// A single issue reported by a linter.
#[derive(Debug, Clone)]
pub struct LintIssue {
    pub file: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub severity: String,
    pub rule: Option<String>,
}

impl LintIssue {
    /// Format as a compact one-liner for hints.
    pub fn to_hint_line(&self) -> String {
        let loc = match (self.line, self.column) {
            (Some(l), Some(c)) => format!("{}:{}:{}", self.file, l, c),
            (Some(l), None) => format!("{}:{}", self.file, l),
            _ => self.file.clone(),
        };
        match &self.rule {
            Some(r) => format!("{loc} [{r}] {}", self.message),
            None => format!("{loc} {}", self.message),
        }
    }
}

/// Map a detected linter tool name to the command needed to run it.
pub fn resolve_lint_command(linter_tool: &str) -> Option<LintCommand> {
    let (program, args, json_args) = match linter_tool {
        "ESLint" => ("npx", vec!["eslint", "."], vec!["--format", "json"]),
        "Biome" => (
            "npx",
            vec!["biome", "check", "."],
            vec!["--reporter", "json"],
        ),
        "RuboCop" => ("bundle", vec!["exec", "rubocop"], vec!["--format", "json"]),
        "Clippy" => (
            "cargo",
            vec!["clippy", "--all-targets"],
            vec!["--message-format=json"],
        ),
        "Ruff" => ("ruff", vec!["check", "."], vec!["--output-format", "json"]),
        "golangci-lint" => ("golangci-lint", vec!["run"], vec!["--out-format", "json"]),
        "Credo" => ("mix", vec!["credo"], vec!["--format", "json"]),
        "Flake8" => ("flake8", vec!["."], vec!["--format", "json"]),
        _ => return None,
    };
    Some(LintCommand {
        program: program.to_string(),
        args: args.into_iter().map(String::from).collect(),
        json_args: json_args.into_iter().map(String::from).collect(),
        tool_name: linter_tool.to_string(),
    })
}

/// Run a linter in the given service directory.
pub fn run_linter(service_dir: &Path, cmd: &LintCommand) -> LintResult {
    let mut command = Command::new(&cmd.program);
    command.args(&cmd.args);
    command.args(&cmd.json_args);
    command.current_dir(service_dir);
    command.env("NO_COLOR", "1");

    match command.output() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => LintResult {
            success: true,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            command_not_found: true,
            issues: Vec::new(),
        },
        Err(_) => LintResult {
            success: true,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            command_not_found: false,
            issues: Vec::new(),
        },
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code();
            // Exit code 127 = command not found (shell convention, also used by
            // wrappers like `bundle exec` when the gem is missing)
            if exit_code == Some(127) {
                return LintResult {
                    success: true,
                    exit_code,
                    stdout,
                    stderr,
                    command_not_found: true,
                    issues: Vec::new(),
                };
            }
            let success = output.status.success();
            let issues = parse_linter_output(&cmd.tool_name, &stdout);
            LintResult {
                success,
                exit_code,
                stdout,
                stderr,
                command_not_found: false,
                issues,
            }
        }
    }
}

/// Best-effort parse of linter JSON output into issues.
pub fn parse_linter_output(tool: &str, stdout: &str) -> Vec<LintIssue> {
    match tool {
        "ESLint" => parse_eslint_output(stdout),
        "RuboCop" => parse_rubocop_output(stdout),
        "Clippy" => parse_clippy_output(stdout),
        "Ruff" => parse_ruff_output(stdout),
        _ => Vec::new(),
    }
}

fn parse_eslint_output(stdout: &str) -> Vec<LintIssue> {
    // ESLint JSON: [{filePath, messages: [{line, column, message, severity, ruleId}]}]
    let arr: Vec<serde_json::Value> = match serde_json::from_str(stdout) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut issues = Vec::new();
    for file_obj in &arr {
        let file_path = file_obj["filePath"].as_str().unwrap_or("");
        let messages = match file_obj["messages"].as_array() {
            Some(m) => m,
            None => continue,
        };
        for msg in messages {
            issues.push(LintIssue {
                file: file_path.to_string(),
                line: msg["line"].as_u64().map(|n| n as usize),
                column: msg["column"].as_u64().map(|n| n as usize),
                message: msg["message"].as_str().unwrap_or("").to_string(),
                severity: match msg["severity"].as_u64() {
                    Some(2) => "error".to_string(),
                    _ => "warning".to_string(),
                },
                rule: msg["ruleId"].as_str().map(String::from),
            });
        }
    }
    issues
}

fn parse_rubocop_output(stdout: &str) -> Vec<LintIssue> {
    // RuboCop JSON: {files: [{path, offenses: [{message, severity, location: {line, column}, cop_name}]}]}
    let obj: serde_json::Value = match serde_json::from_str(stdout) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut issues = Vec::new();
    let files = match obj["files"].as_array() {
        Some(f) => f,
        None => return issues,
    };
    for file_obj in files {
        let path = file_obj["path"].as_str().unwrap_or("");
        let offenses = match file_obj["offenses"].as_array() {
            Some(o) => o,
            None => continue,
        };
        for offense in offenses {
            issues.push(LintIssue {
                file: path.to_string(),
                line: offense["location"]["line"].as_u64().map(|n| n as usize),
                column: offense["location"]["column"].as_u64().map(|n| n as usize),
                message: offense["message"].as_str().unwrap_or("").to_string(),
                severity: offense["severity"]
                    .as_str()
                    .unwrap_or("warning")
                    .to_string(),
                rule: offense["cop_name"].as_str().map(String::from),
            });
        }
    }
    issues
}

fn parse_clippy_output(stdout: &str) -> Vec<LintIssue> {
    // Clippy: one JSON object per line, {reason: "compiler-message", message: {message, level, spans: [{file_name, line_start}]}}
    let mut issues = Vec::new();
    for line in stdout.lines() {
        let obj: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if obj["reason"].as_str() != Some("compiler-message") {
            continue;
        }
        let msg = &obj["message"];
        let level = msg["level"].as_str().unwrap_or("");
        if level == "note" || level == "help" {
            continue;
        }
        let message_text = msg["message"].as_str().unwrap_or("").to_string();
        let span = msg["spans"].as_array().and_then(|s| s.first());
        let (file, line_num) = match span {
            Some(s) => (
                s["file_name"].as_str().unwrap_or("").to_string(),
                s["line_start"].as_u64().map(|n| n as usize),
            ),
            None => (String::new(), None),
        };
        issues.push(LintIssue {
            file,
            line: line_num,
            column: None,
            message: message_text,
            severity: level.to_string(),
            rule: msg["code"]["code"].as_str().map(String::from),
        });
    }
    issues
}

fn parse_ruff_output(stdout: &str) -> Vec<LintIssue> {
    // Ruff JSON: [{filename, message, location: {row, column}, code}]
    let arr: Vec<serde_json::Value> = match serde_json::from_str(stdout) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut issues = Vec::new();
    for obj in &arr {
        issues.push(LintIssue {
            file: obj["filename"].as_str().unwrap_or("").to_string(),
            line: obj["location"]["row"].as_u64().map(|n| n as usize),
            column: obj["location"]["column"].as_u64().map(|n| n as usize),
            message: obj["message"].as_str().unwrap_or("").to_string(),
            severity: "error".to_string(),
            rule: obj["code"].as_str().map(String::from),
        });
    }
    issues
}

// ── Test runner ──────────────────────────────────────────────────────────

/// Command specification for running tests.
#[derive(Debug, Clone)]
pub struct TestCommand {
    pub program: String,
    pub args: Vec<String>,
    pub tool_name: String,
}

/// Result of running tests.
#[derive(Debug)]
pub struct TestResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub command_not_found: bool,
}

/// Map a detected test framework to the command needed to run it.
pub fn resolve_test_command(test_framework: &str) -> Option<TestCommand> {
    let (program, args) = match test_framework {
        "Vitest" => ("npx", vec!["vitest", "run"]),
        "Jest" => ("npx", vec!["jest", "--ci"]),
        "Mocha" => ("npx", vec!["mocha"]),
        "pytest" => ("pytest", vec![]),
        "RSpec" => ("bundle", vec!["exec", "rspec"]),
        "Minitest" => ("bundle", vec!["exec", "rake", "test"]),
        "cargo test" => ("cargo", vec!["test"]),
        "go test" => ("go", vec!["test", "./..."]),
        "ExUnit" => ("mix", vec!["test"]),
        _ => return None,
    };
    Some(TestCommand {
        program: program.to_string(),
        args: args.into_iter().map(String::from).collect(),
        tool_name: test_framework.to_string(),
    })
}

/// Run tests in the given service directory.
pub fn run_tests(service_dir: &Path, cmd: &TestCommand) -> TestResult {
    let mut command = Command::new(&cmd.program);
    command.args(&cmd.args);
    command.current_dir(service_dir);
    command.env("NO_COLOR", "1");
    command.env("CI", "1");

    match command.output() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => TestResult {
            success: true,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            command_not_found: true,
        },
        Err(_) => TestResult {
            success: true,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            command_not_found: false,
        },
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code();
            if exit_code == Some(127) {
                return TestResult {
                    success: true,
                    exit_code,
                    stdout,
                    stderr,
                    command_not_found: true,
                };
            }
            TestResult {
                success: output.status.success(),
                exit_code,
                stdout,
                stderr,
                command_not_found: false,
            }
        }
    }
}

/// Dev commands extracted from a service README.
#[derive(Debug, Default)]
pub struct DevCommands {
    pub setup: Option<String>,
    pub build: Option<String>,
    /// Test command.
    pub test: Option<String>,
    pub run: Option<String>,
    /// Lint command.
    pub lint: Option<String>,
}

impl DevCommands {
    pub fn has_any(&self) -> bool {
        self.setup.is_some()
            || self.build.is_some()
            || self.test.is_some()
            || self.run.is_some()
            || self.lint.is_some()
    }
}

/// Extract dev commands from a service README's Development/Local Development section.
/// Matches H3 subsections (### Setup, ### Build, ### Test, ### Run, ### Lint) first,
/// then falls back to keyword-proximity matching.
pub fn extract_dev_commands(readme_path: &Path) -> DevCommands {
    let mut cmds = DevCommands::default();

    let content = match std::fs::read_to_string(readme_path) {
        Ok(c) => c,
        Err(_) => return cmds,
    };

    // Find the Development or Local Development section
    let mut in_development = false;
    let mut in_code_block = false;
    let mut current_block = String::new();
    let mut last_text_lower = String::new();
    let mut current_h3: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        // Track H2 headings
        if trimmed.starts_with("## ") && !trimmed.starts_with("### ") {
            if trimmed.contains("Development") {
                in_development = true;
                current_h3 = None;
                continue;
            } else if in_development {
                break;
            }
            continue;
        }

        if !in_development {
            continue;
        }

        // Track H3 subsections
        if trimmed.starts_with("### ") {
            let h3_name = trimmed.trim_start_matches("### ").trim().to_lowercase();
            current_h3 = Some(h3_name);
            continue;
        }

        // Track code blocks
        if trimmed.starts_with("```") {
            if in_code_block {
                let block = current_block.trim().to_string();
                if !block.is_empty() {
                    // First try H3-based assignment
                    if let Some(ref h3) = current_h3 {
                        match h3.as_str() {
                            "setup" if cmds.setup.is_none() => cmds.setup = Some(block.clone()),
                            "build" if cmds.build.is_none() => cmds.build = Some(block.clone()),
                            "test" | "testing" if cmds.test.is_none() => {
                                cmds.test = Some(block.clone())
                            }
                            "run" | "running" if cmds.run.is_none() => {
                                cmds.run = Some(block.clone())
                            }
                            "lint" | "linting" if cmds.lint.is_none() => {
                                cmds.lint = Some(block.clone())
                            }
                            _ => {
                                // H3 didn't match — fall through to keyword matching
                                assign_by_keyword(&mut cmds, &block, &last_text_lower);
                            }
                        }
                    } else {
                        // No H3 context — use keyword proximity
                        assign_by_keyword(&mut cmds, &block, &last_text_lower);
                    }
                }
                current_block.clear();
                in_code_block = false;
            } else {
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            if !current_block.is_empty() {
                current_block.push('\n');
            }
            current_block.push_str(line);
        } else {
            let lower = trimmed.to_lowercase();
            if !lower.is_empty() {
                last_text_lower = lower;
            }
        }
    }

    cmds
}

/// Assign a code block to a DevCommands field based on keyword proximity.
fn assign_by_keyword(cmds: &mut DevCommands, block: &str, context: &str) {
    if context.contains("setup") && cmds.setup.is_none() {
        cmds.setup = Some(block.to_string());
    }
    if context.contains("build") && cmds.build.is_none() {
        cmds.build = Some(block.to_string());
    }
    if context.contains("test") && cmds.test.is_none() {
        cmds.test = Some(block.to_string());
    }
    if context.contains("run") && cmds.run.is_none() {
        cmds.run = Some(block.to_string());
    }
    if context.contains("lint") && cmds.lint.is_none() {
        cmds.lint = Some(block.to_string());
    }
}

/// Check if source files in a directory contain code reference comments for given doc IDs.
/// Returns doc IDs that have zero references.
pub fn check_code_refs_coverage(service_dir: &Path, doc_ids: &[String]) -> Vec<String> {
    use ignore::WalkBuilder;

    if doc_ids.is_empty() {
        return Vec::new();
    }

    let mut found: std::collections::HashSet<String> = std::collections::HashSet::new();

    let walker = WalkBuilder::new(service_dir)
        .hidden(true)
        .git_ignore(true)
        .build();

    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        let path = entry.path();
        // Skip non-source files
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !is_source_ext(ext) {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(path) {
            for id in doc_ids {
                if !found.contains(id) && content.contains(id.as_str()) {
                    found.insert(id.clone());
                }
            }
        }

        // Early exit if all found
        if found.len() == doc_ids.len() {
            return Vec::new();
        }
    }

    doc_ids
        .iter()
        .filter(|id| !found.contains(id.as_str()))
        .cloned()
        .collect()
}

/// Check if a file extension is a source code file.
fn is_source_ext(ext: &str) -> bool {
    matches!(
        ext,
        "rs" | "js"
            | "ts"
            | "jsx"
            | "tsx"
            | "py"
            | "rb"
            | "go"
            | "java"
            | "kt"
            | "swift"
            | "c"
            | "cpp"
            | "h"
            | "cs"
            | "php"
            | "ex"
            | "exs"
            | "erl"
            | "hs"
            | "ml"
            | "scala"
            | "clj"
            | "lua"
            | "sh"
            | "bash"
            | "zsh"
            | "sql"
    )
}

/// Check if a dev_url returns HTTP 200.
#[cfg(feature = "avatars")]
pub fn check_dev_url(url: &str) -> bool {
    let agent: ureq::Agent = ureq::config::Config::builder()
        .timeout_global(Some(std::time::Duration::from_secs(2)))
        .build()
        .into();
    match agent.get(url).call() {
        Ok(resp) => resp.status().as_u16() == 200,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_discover_service_readmes() {
        let fixtures = PathBuf::from("../../tests/fixtures/singleton");
        let readmes = discover_service_readmes(&fixtures).unwrap();

        // Should find at least the test services we created
        assert!(!readmes.is_empty(), "Should find service READMEs");

        // All results should be README.md files
        for readme in &readmes {
            assert_eq!(readme.file_name().unwrap(), "README.md");
            assert!(readme
                .parent()
                .unwrap()
                .starts_with(fixtures.join("services")));
        }
    }

    #[test]
    fn test_extract_service_metadata() {
        let fixtures = PathBuf::from("../../tests/fixtures/singleton");
        let valid_readme = fixtures.join("services/valid-service/README.md");

        let metadata = extract_service_metadata(&valid_readme, &fixtures).unwrap();

        assert_eq!(metadata.name, "Valid Service");
        assert_eq!(metadata.status, "Live");
        assert_eq!(metadata.owner, "@alice");
        assert!(metadata
            .readme_path
            .contains("services/valid-service/README.md"));
    }

    #[test]
    fn test_extract_non_comment_content() {
        let content = "<!-- This is a comment -->\nActual content\n<!-- Another comment -->";
        let result = extract_non_comment_content(content);
        assert_eq!(result, "Actual content");
    }

    #[test]
    fn test_extract_service_name() {
        use crate::document::Document;

        let doc = Document::from_str("# My Service\n\nSome content").unwrap();
        let parsed = doc.parse_body();

        let name = extract_service_name(&parsed).unwrap();
        assert_eq!(name, "My Service");
    }

    #[test]
    fn test_extract_tech_stack_from_files() {
        let tmp = std::env::temp_dir().join("dg_service_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Test JavaScript detection
        std::fs::write(tmp.join("package.json"), "{}").unwrap();
        let stack = extract_tech_stack(&tmp);
        assert_eq!(stack.primary_language, "JavaScript");

        std::fs::remove_file(tmp.join("package.json")).unwrap();

        // Test Rust detection
        std::fs::write(tmp.join("Cargo.toml"), "").unwrap();
        let stack = extract_tech_stack(&tmp);
        assert_eq!(stack.primary_language, "Rust");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_extract_tech_stack_unknown() {
        let tmp = std::env::temp_dir().join("dg_service_unknown");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let stack = extract_tech_stack(&tmp);
        assert_eq!(stack.primary_language, "Unknown");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ── Gemfile vs package.json priority ────────────────────────────

    #[test]
    fn test_is_ruby_app_gemfile() {
        let tmp = std::env::temp_dir().join("dg_is_ruby_app");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Tooling-only Gemfile → not a Ruby app
        std::fs::write(
            tmp.join("Gemfile"),
            "source 'https://rubygems.org'\ngem 'fastlane'\ngem 'cocoapods'\n",
        )
        .unwrap();
        assert!(!is_ruby_app_gemfile(&tmp));

        // Rails Gemfile → Ruby app
        std::fs::write(
            tmp.join("Gemfile"),
            "source 'https://rubygems.org'\ngem 'rails', '~> 7.0'\ngem 'pg'\n",
        )
        .unwrap();
        assert!(is_ruby_app_gemfile(&tmp));

        // Sinatra Gemfile → Ruby app
        std::fs::write(tmp.join("Gemfile"), "gem 'sinatra'\n").unwrap();
        assert!(is_ruby_app_gemfile(&tmp));

        // No Gemfile → not a Ruby app
        std::fs::remove_file(tmp.join("Gemfile")).unwrap();
        assert!(!is_ruby_app_gemfile(&tmp));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Simulates a React Native / Expo app with a Gemfile containing only
    /// fastlane/cocoapods (not a Ruby app). Should detect as JavaScript with
    /// React Native + Expo frameworks — not Ruby.
    #[test]
    fn test_rn_expo_app_with_fastlane_gemfile() {
        let tmp = std::env::temp_dir().join("dg_service_rn_expo");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        std::fs::write(
            tmp.join("Gemfile"),
            "source 'https://rubygems.org'\ngem 'fastlane'\ngem 'cocoapods'\n",
        )
        .unwrap();
        std::fs::write(
            tmp.join("package.json"),
            r#"{
                "dependencies": {
                    "react": "18.2.0",
                    "react-native": "0.73.4",
                    "expo": "~50.0.0",
                    "@react-navigation/native": "^6.1.9"
                },
                "devDependencies": {
                    "typescript": "~5.3.3"
                }
            }"#,
        )
        .unwrap();

        // Use the public API (extract_tech_stack) — same path dg serve uses
        let stack = extract_tech_stack(&tmp);
        assert_eq!(stack.primary_language, "JavaScript");
        assert!(
            stack.frameworks.contains(&"React Native".to_string()),
            "Expected React Native in {:?}",
            stack.frameworks
        );
        assert!(
            stack.frameworks.contains(&"Expo".to_string()),
            "Expected Expo in {:?}",
            stack.frameworks
        );
        assert!(
            stack.frameworks.contains(&"React".to_string()),
            "Expected React in {:?}",
            stack.frameworks
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Same as above but with tsconfig.json → should upgrade to TypeScript.
    #[test]
    fn test_rn_expo_app_with_tsconfig_detected_as_typescript() {
        let tmp = std::env::temp_dir().join("dg_service_rn_ts");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        std::fs::write(tmp.join("Gemfile"), "gem 'fastlane'\n").unwrap();
        std::fs::write(
            tmp.join("package.json"),
            r#"{"dependencies":{"react-native":"0.73","expo":"~50.0"}}"#,
        )
        .unwrap();
        std::fs::write(tmp.join("tsconfig.json"), r#"{"compilerOptions":{}}"#).unwrap();

        let stack = extract_tech_stack(&tmp);
        assert_eq!(stack.primary_language, "TypeScript");
        assert!(stack.frameworks.contains(&"React Native".to_string()));
        assert!(stack.frameworks.contains(&"Expo".to_string()));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Rails app that also has package.json (webpacker/esbuild assets) should
    /// stay Ruby with Rails framework, not be reclassified as JavaScript.
    #[test]
    fn test_rails_app_with_package_json_stays_ruby() {
        let tmp = std::env::temp_dir().join("dg_service_rails");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        std::fs::write(
            tmp.join("Gemfile"),
            "source 'https://rubygems.org'\ngem 'rails', '~> 7.0'\ngem 'pg'\ngem 'puma'\n",
        )
        .unwrap();
        std::fs::write(
            tmp.join("package.json"),
            r#"{"dependencies":{"esbuild":"^0.19"}}"#,
        )
        .unwrap();

        let stack = extract_tech_stack(&tmp);
        assert_eq!(stack.primary_language, "Ruby");
        assert!(
            stack.frameworks.contains(&"Rails".to_string()),
            "Expected Rails in {:?}",
            stack.frameworks
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Sinatra app (no package.json) — plain Ruby detection.
    #[test]
    fn test_sinatra_app_without_package_json() {
        let tmp = std::env::temp_dir().join("dg_service_sinatra");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        std::fs::write(tmp.join("Gemfile"), "gem 'sinatra'\ngem 'puma'\n").unwrap();

        let stack = extract_tech_stack(&tmp);
        assert_eq!(stack.primary_language, "Ruby");
        assert!(stack.frameworks.contains(&"Sinatra".to_string()));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Plain package.json + tsconfig.json → TypeScript (no frameworks).
    #[test]
    fn test_package_json_with_tsconfig_detected_as_typescript() {
        let tmp = std::env::temp_dir().join("dg_service_ts");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        std::fs::write(tmp.join("package.json"), "{}").unwrap();
        std::fs::write(tmp.join("tsconfig.json"), "{}").unwrap();

        let stack = extract_tech_stack(&tmp);
        assert_eq!(stack.primary_language, "TypeScript");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// detect_frameworks picks up React Native + Expo from package.json
    /// when called with either "JavaScript" or "TypeScript" primary language.
    #[test]
    fn test_detect_frameworks_rn_expo_for_both_js_and_ts() {
        let tmp = std::env::temp_dir().join("dg_detect_fw_rn");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        std::fs::write(
            tmp.join("package.json"),
            r#"{"dependencies":{"react":"18.2","react-native":"0.73","expo":"~50.0","next":"14.0"}}"#,
        )
        .unwrap();

        for lang in &["JavaScript", "TypeScript"] {
            let fw = detect_frameworks(&tmp, lang);
            assert!(fw.contains(&"React".to_string()), "{lang}: missing React");
            assert!(
                fw.contains(&"React Native".to_string()),
                "{lang}: missing React Native"
            );
            assert!(fw.contains(&"Expo".to_string()), "{lang}: missing Expo");
            assert!(
                fw.contains(&"Next.js".to_string()),
                "{lang}: missing Next.js"
            );
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Gemfile with only cocoapods + package.json with no RN deps →
    /// JavaScript (not Ruby), no RN/Expo frameworks.
    #[test]
    fn test_tooling_gemfile_with_plain_js_package() {
        let tmp = std::env::temp_dir().join("dg_service_tooling_js");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        std::fs::write(tmp.join("Gemfile"), "gem 'cocoapods'\n").unwrap();
        std::fs::write(
            tmp.join("package.json"),
            r#"{"dependencies":{"express":"^4.18"}}"#,
        )
        .unwrap();

        let stack = extract_tech_stack(&tmp);
        assert_eq!(stack.primary_language, "JavaScript");
        assert!(stack.frameworks.contains(&"Express".to_string()));
        assert!(!stack.frameworks.contains(&"React Native".to_string()));
        assert!(!stack.frameworks.contains(&"Rails".to_string()));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_linter_eslint() {
        let tmp = std::env::temp_dir().join("dg_detect_linter_eslint");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join(".eslintrc.json"), "{}").unwrap();

        let (has, tool) = detect_linter(&tmp, "TypeScript");
        assert!(has);
        assert_eq!(tool.as_deref(), Some("ESLint"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_linter_empty_dir() {
        let tmp = std::env::temp_dir().join("dg_detect_linter_empty");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let (has, tool) = detect_linter(&tmp, "TypeScript");
        assert!(!has);
        assert!(tool.is_none());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_linter_parent_dir() {
        let tmp = std::env::temp_dir().join("dg_detect_linter_parent");
        let _ = std::fs::remove_dir_all(&tmp);
        let child = tmp.join("sub");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::write(tmp.join(".eslintrc.json"), "{}").unwrap();

        let (has, tool) = detect_linter(&child, "JavaScript");
        assert!(has);
        assert_eq!(tool.as_deref(), Some("ESLint"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_tests_with_tests_dir() {
        let tmp = std::env::temp_dir().join("dg_detect_tests_dir");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("tests")).unwrap();

        let (has, fw) = detect_tests(&tmp, "Rust");
        assert!(has);
        assert_eq!(fw.as_deref(), Some("cargo test"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_detect_tests_empty_dir() {
        let tmp = std::env::temp_dir().join("dg_detect_tests_empty");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let (has, fw) = detect_tests(&tmp, "TypeScript");
        assert!(!has);
        assert!(fw.is_none());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_frontmatter_override_linter() {
        let tmp = std::env::temp_dir().join("dg_fm_override_linter");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        // Config exists but frontmatter says false
        std::fs::write(tmp.join(".eslintrc.json"), "{}").unwrap();

        let doc =
            crate::document::Document::from_str("---\nhas_linter: false\n---\n# Test\n").unwrap();
        let practices = detect_engineering_practices(&tmp, "TypeScript", doc.frontmatter.as_ref());
        assert!(!practices.has_linter);
        // Tool name still detected
        assert_eq!(practices.linter_tool.as_deref(), Some("ESLint"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_extract_dev_commands_h3_subsections() {
        let tmp = std::env::temp_dir().join("dg_dev_cmds_h3");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let readme = tmp.join("README.md");
        std::fs::write(
            &readme,
            "# Service\n\n## Local Development\n\n### Setup\n\n```sh\nnpm install\n```\n\n### Test\n\n```sh\nnpm test\n```\n\n### Run\n\n```sh\nnpm start\n```\n",
        )
        .unwrap();

        let cmds = extract_dev_commands(&readme);
        assert_eq!(cmds.setup.as_deref(), Some("npm install"));
        assert_eq!(cmds.test.as_deref(), Some("npm test"));
        assert_eq!(cmds.run.as_deref(), Some("npm start"));
        assert!(cmds.has_any());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_resolve_lint_command_known() {
        let cmd = resolve_lint_command("RuboCop").unwrap();
        assert_eq!(cmd.program, "bundle");
        assert_eq!(cmd.args, vec!["exec", "rubocop"]);
        assert_eq!(cmd.json_args, vec!["--format", "json"]);
        assert_eq!(cmd.tool_name, "RuboCop");

        let cmd = resolve_lint_command("ESLint").unwrap();
        assert_eq!(cmd.program, "npx");
        assert_eq!(cmd.args, vec!["eslint", "."]);

        let cmd = resolve_lint_command("Clippy").unwrap();
        assert_eq!(cmd.program, "cargo");

        let cmd = resolve_lint_command("Ruff").unwrap();
        assert_eq!(cmd.program, "ruff");
    }

    #[test]
    fn test_resolve_lint_command_unknown() {
        assert!(resolve_lint_command("UnknownLinter").is_none());
        assert!(resolve_lint_command("").is_none());
    }

    #[test]
    fn test_parse_eslint_output() {
        let json = r#"[{"filePath":"/app/src/index.js","messages":[{"line":10,"column":5,"message":"Unexpected var","severity":2,"ruleId":"no-var"},{"line":20,"column":1,"message":"Missing semicolon","severity":1,"ruleId":"semi"}]}]"#;
        let issues = parse_linter_output("ESLint", json);
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].file, "/app/src/index.js");
        assert_eq!(issues[0].line, Some(10));
        assert_eq!(issues[0].column, Some(5));
        assert_eq!(issues[0].message, "Unexpected var");
        assert_eq!(issues[0].severity, "error");
        assert_eq!(issues[0].rule.as_deref(), Some("no-var"));
        assert_eq!(issues[1].severity, "warning");
    }

    #[test]
    fn test_parse_rubocop_output() {
        let json = r#"{"files":[{"path":"app/models/user.rb","offenses":[{"message":"Prefer single-quoted strings","severity":"convention","location":{"line":42,"column":5},"cop_name":"Style/StringLiterals"}]}]}"#;
        let issues = parse_linter_output("RuboCop", json);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].file, "app/models/user.rb");
        assert_eq!(issues[0].line, Some(42));
        assert_eq!(issues[0].rule.as_deref(), Some("Style/StringLiterals"));
    }

    #[test]
    fn test_parse_ruff_output() {
        let json = r#"[{"filename":"main.py","message":"Unused import","location":{"row":1,"column":1},"code":"F401"}]"#;
        let issues = parse_linter_output("Ruff", json);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].file, "main.py");
        assert_eq!(issues[0].line, Some(1));
        assert_eq!(issues[0].rule.as_deref(), Some("F401"));
    }

    #[test]
    fn test_parse_clippy_output() {
        let json = r#"{"reason":"compiler-message","message":{"message":"unused variable: `x`","level":"warning","spans":[{"file_name":"src/main.rs","line_start":5}],"code":{"code":"unused_variables"}}}"#;
        let issues = parse_linter_output("Clippy", json);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].file, "src/main.rs");
        assert_eq!(issues[0].line, Some(5));
        assert_eq!(issues[0].rule.as_deref(), Some("unused_variables"));
    }

    #[test]
    fn test_parse_invalid_json() {
        let issues = parse_linter_output("ESLint", "not json");
        assert!(issues.is_empty());
        let issues = parse_linter_output("RuboCop", "");
        assert!(issues.is_empty());
    }

    #[test]
    fn test_lint_issue_hint_line() {
        let issue = LintIssue {
            file: "src/main.rs".to_string(),
            line: Some(42),
            column: Some(5),
            message: "unused variable".to_string(),
            severity: "warning".to_string(),
            rule: Some("W001".to_string()),
        };
        assert_eq!(
            issue.to_hint_line(),
            "src/main.rs:42:5 [W001] unused variable"
        );

        let issue_no_rule = LintIssue {
            file: "main.py".to_string(),
            line: Some(10),
            column: None,
            message: "syntax error".to_string(),
            severity: "error".to_string(),
            rule: None,
        };
        assert_eq!(issue_no_rule.to_hint_line(), "main.py:10 syntax error");
    }
}

//! `dg site` command — generate static documentation site.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use md_db::schema::Schema;
use md_db::users::OrgConfig;

#[derive(Args)]
pub struct SiteArgs {
    /// Output directory
    #[arg(short, long, default_value = "site")]
    pub output: PathBuf,
    /// Site title (default: org name or directory name)
    #[arg(long)]
    pub title: Option<String>,
    /// Skip individual user pages
    #[arg(long)]
    pub no_users: bool,
    /// Skip roadmap page
    #[arg(long)]
    pub no_roadmap: bool,
    /// Skip git history (for roadmap)
    #[arg(long)]
    pub no_git: bool,
}

pub fn run(
    root: &Path,
    schema: &Schema,
    users: Option<&OrgConfig>,
    args: &SiteArgs,
    cache: &mut md_db::cache::DocCache,
) -> Result<()> {
    use md_db::site::{self, SiteConfig};

    let title = resolve_title(args.title.as_deref(), users, root);

    let (roadmap_html, roadmap_generated_at) = if args.no_roadmap {
        (None, None)
    } else {
        match build_roadmap_html(root, schema, users, args.no_git, cache) {
            Ok((html, date)) => (Some(html), Some(date)),
            Err(_) => (None, None),
        }
    };

    let edit_url_prefix = detect_edit_url_prefix(root);

    let config = SiteConfig {
        title,
        roadmap: !args.no_roadmap,
        users: !args.no_users,
        roadmap_html,
        roadmap_generated_at,
        readme_html: render_readme_html(root),
        logo_path: None,
        edit_url_prefix,
        is_local_dev: false,
    };

    let count = site::generate_site(root, schema, users, &config, &args.output)
        .context("failed to generate site")?;

    eprintln!("Generated {count} pages in {}", args.output.display());
    Ok(())
}

/// Resolve site title: explicit > org name > directory name.
pub(crate) fn resolve_title(
    explicit: Option<&str>,
    users: Option<&OrgConfig>,
    root: &Path,
) -> String {
    if let Some(t) = explicit {
        return t.to_string();
    }
    if let Some(org) = users {
        if let Some(name) = org
            .orgs
            .values()
            .find(|o| o.primary)
            .and_then(|o| o.name.clone())
        {
            return name;
        }
        if let Some(name) = org
            .orgs
            .values()
            .find(|o| o.parent.is_none())
            .and_then(|o| o.name.clone())
        {
            return name;
        }
    }
    dir_name_fallback(root)
}

pub(crate) fn dir_name_fallback(root: &Path) -> String {
    root.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Documentation")
        .to_string()
}

/// Build roadmap HTML body (without the standalone page header).
/// Returns `(html_body, generated_at_date)`.
pub(crate) fn build_roadmap_html(
    root: &Path,
    schema: &Schema,
    users: Option<&OrgConfig>,
    no_git: bool,
    cache: &mut md_db::cache::DocCache,
) -> Result<(String, String)> {
    use md_db::roadmap;

    let today = today_date();

    let config_path = root.join(".dg").join("roadmap.yaml");
    let config = if config_path.is_file() {
        Some(roadmap::RoadmapConfig::from_file(&config_path)?)
    } else {
        None
    };

    let past = config
        .as_ref()
        .map(|c| c.display.past_quarters)
        .unwrap_or(4);
    let future = config
        .as_ref()
        .map(|c| c.display.future_quarters)
        .unwrap_or(4);

    let status_history = if no_git {
        None
    } else {
        collect_git_history(root, schema, cache).ok()
    };

    let data = roadmap::build_roadmap(
        root,
        schema,
        config.as_ref(),
        users,
        &today,
        past,
        future,
        status_history.as_ref(),
    )?;

    let current_q = roadmap::Quarter::from_date(&today).unwrap_or(roadmap::Quarter::new(2026, 1));
    let full_html = roadmap::render_roadmap_html(&data, &current_q);

    let style = extract_style_tags(&full_html);
    let start = full_html.find("<body>").map(|i| i + 6).unwrap_or(0);
    let end = full_html.rfind("</body>").unwrap_or(full_html.len());
    let mut body = full_html[start..end].to_string();

    // Strip the <header>...</header> — the SPA renders its own title
    if let Some(h_start) = body.find("<header") {
        if let Some(h_end) = body[h_start..].find("</header>") {
            body = format!(
                "{}{}",
                &body[..h_start],
                &body[h_start + h_end + "</header>".len()..]
            );
        }
    }

    Ok((format!("{}{}", style, body), today))
}

fn today_date() -> String {
    let output = std::process::Command::new("date").arg("+%Y-%m-%d").output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "2026-01-01".to_string(),
    }
}

fn collect_git_history(
    root: &Path,
    schema: &Schema,
    cache: &mut md_db::cache::DocCache,
) -> Result<std::collections::BTreeMap<String, Vec<md_db::history::StatusTransition>>> {
    use md_db::graph::DocGraph;

    let graph = DocGraph::build_cached(root, schema, cache)?;
    let opp_paths: Vec<(String, PathBuf)> = graph
        .nodes
        .values()
        .filter(|n| {
            n.doc_type
                .as_deref()
                .map(|t| t.eq_ignore_ascii_case("opp"))
                .unwrap_or(false)
        })
        .map(|n| (n.id.clone(), n.path.clone()))
        .collect();

    Ok(md_db::history::collect_status_history(root, &opp_paths)?)
}

/// Extract all `<style>...</style>` tags from an HTML string.
/// Strips global reset rules (`*`, `body`) that would conflict with the site layout.
fn extract_style_tags(html: &str) -> String {
    let mut styles = String::new();
    let mut search_from = 0;
    while let Some(start) = html[search_from..].find("<style>") {
        let abs_start = search_from + start;
        if let Some(end) = html[abs_start..].find("</style>") {
            let abs_end = abs_start + end + 8;
            let css = &html[abs_start + 7..abs_start + end];
            let scoped = strip_global_css_rules(css);
            if !scoped.trim().is_empty() {
                styles.push_str("<style>");
                styles.push_str(&scoped);
                styles.push_str("</style>\n");
            }
            search_from = abs_end;
        } else {
            break;
        }
    }
    styles
}

/// Render root README.md to HTML, or None if not found.
pub(crate) fn render_readme_html(root: &Path) -> Option<String> {
    let readme = root.join("README.md");
    if readme.is_file() {
        let content = std::fs::read_to_string(&readme).ok()?;
        let html = md_db::export::render_markdown_to_html(&content);
        Some(md_db::export::rewrite_internal_links(&html))
    } else {
        None
    }
}

/// Detect logo file in .dg/ directory.
pub(crate) fn detect_logo(root: &Path) -> Option<PathBuf> {
    for ext in &["svg", "png", "jpg", "jpeg"] {
        let path = root.join(format!(".dg/logo.{ext}"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// Detect GitHub/GitLab edit URL prefix from git remote.
pub(crate) fn detect_edit_url_prefix(root: &Path) -> Option<String> {
    let (base_url, branch) = md_db::code_refs::detect_repo_web_url(root)?;
    let is_gitlab = base_url.contains("gitlab");
    if is_gitlab {
        Some(format!("{base_url}/-/edit/{branch}/"))
    } else {
        Some(format!("{base_url}/edit/{branch}/"))
    }
}

/// Remove CSS rules targeting `*` or `body` from a CSS string.
fn strip_global_css_rules(css: &str) -> String {
    let mut result = String::new();
    let mut current_rule = String::new();
    let mut depth = 0;

    for c in css.chars() {
        current_rule.push(c);
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let selector = current_rule.split('{').next().unwrap_or("").trim();
                    let is_global =
                        selector == "*" || selector == "body" || selector.starts_with("* ");
                    if !is_global {
                        result.push_str(&current_rule);
                        result.push('\n');
                    }
                    current_rule.clear();
                }
            }
            _ => {}
        }
    }
    if !current_rule.trim().is_empty() {
        result.push_str(&current_rule);
    }
    result
}

pub(crate) mod data;
mod embed;
pub(crate) mod nav;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::document::Document;
use crate::graph::{path_to_id, DocGraph};
use crate::schema::Schema;
use crate::users::OrgConfig;

/// Configuration for site generation.
pub struct SiteConfig {
    /// Site title (shown in top bar and browser title).
    pub title: String,
    /// Whether to generate the roadmap page.
    pub roadmap: bool,
    /// Whether to generate individual user pages.
    pub users: bool,
    /// Pre-rendered roadmap HTML body (set by CLI if roadmap=true).
    pub roadmap_html: Option<String>,
    /// Pre-rendered README body HTML (Introduction section of the site).
    pub readme_html: Option<String>,
    /// Path to a custom logo file (.dg/logo.svg or .dg/logo.png).
    pub logo_path: Option<PathBuf>,
    /// Date the roadmap was generated (YYYY-MM-DD).
    pub roadmap_generated_at: Option<String>,
    /// GitHub/GitLab edit URL prefix (e.g. "https://github.com/org/repo/edit/main/").
    pub edit_url_prefix: Option<String>,
    /// True when served via `dg serve` (enables local file opening).
    pub is_local_dev: bool,
}

impl Default for SiteConfig {
    fn default() -> Self {
        Self {
            title: "Documentation".to_string(),
            roadmap: true,
            users: true,
            roadmap_html: None,
            roadmap_generated_at: None,
            readme_html: None,
            logo_path: None,
            edit_url_prefix: None,
            is_local_dev: false,
        }
    }
}

/// Generate the full documentation site.
///
/// Writes the embedded SvelteKit SPA + project-specific JSON data files.
/// Returns the number of files written.
pub fn generate_site(
    dir: impl AsRef<Path>,
    schema: &Schema,
    org: Option<&OrgConfig>,
    config: &SiteConfig,
    output_dir: impl AsRef<Path>,
) -> crate::error::Result<usize> {
    let dir = dir.as_ref();
    let output_dir = output_dir.as_ref();

    std::fs::create_dir_all(output_dir)
        .map_err(|_| crate::error::Error::WriteFailed(output_dir.to_path_buf()))?;

    // 1. Write embedded SPA files
    let spa_count = embed::write_spa_files(output_dir)?;

    // 2. Discover + load documents (only schema-typed docs)
    let known_prefixes: std::collections::HashSet<String> = schema
        .types
        .iter()
        .flat_map(|t| {
            let mut prefixes = vec![t.name.to_uppercase()];
            for a in &t.aliases {
                prefixes.push(a.to_uppercase());
            }
            prefixes
        })
        .collect();

    // Collect singleton type names to skip (e.g. readme, service-readme)
    let singleton_prefixes: std::collections::HashSet<String> = schema
        .types
        .iter()
        .filter(|t| t.singleton)
        .flat_map(|t| {
            let mut prefixes = vec![t.name.to_uppercase()];
            for a in &t.aliases {
                prefixes.push(a.to_uppercase());
            }
            prefixes
        })
        .collect();

    let files = crate::discovery::discover_files(dir, None, &[], false)?;
    let mut docs: Vec<(String, Document)> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    for path in &files {
        let doc = match Document::from_file(path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let id = path_to_id(path);

        // Skip files that aren't schema-typed documents
        let prefix = id.split('-').next().unwrap_or("");
        if !known_prefixes.contains(prefix) {
            continue;
        }

        // Skip singleton types (readme, service-readme — handled by services data)
        if singleton_prefixes.contains(prefix) {
            continue;
        }

        // Deduplicate by ID
        if !seen_ids.insert(id.clone()) {
            continue;
        }

        docs.push((id, doc));
    }

    // 3. Build graph
    let graph = DocGraph::build(dir, schema).unwrap_or_else(|_| DocGraph {
        nodes: BTreeMap::new(),
        edges: Vec::new(),
    });

    // 4. Group docs by type (for nav)
    let mut by_type: BTreeMap<String, Vec<(String, &Document)>> = BTreeMap::new();
    for (id, doc) in &docs {
        let doc_type = doc
            .frontmatter
            .as_ref()
            .and_then(|fm| fm.get_display("type"))
            .or_else(|| schema.type_name_for_doc_id(id))
            .or_else(|| id.split('-').next().map(|s| s.to_lowercase()))
            .unwrap_or_else(|| "other".to_string())
            .to_lowercase();
        by_type.entry(doc_type).or_default().push((id.clone(), doc));
    }

    // 5. Fetch + copy avatars
    let avatar_map = if let Some(org_cfg) = org {
        #[cfg(feature = "avatars")]
        {
            // Cache under .dg/cache like the other caches — not a stray
            // `cache/` dir in the project root.
            let _ = crate::avatars::sync_avatars(&dir.join(".dg"), org_cfg);
            crate::avatars::copy_avatars_to_output(&dir.join(".dg"), output_dir, org_cfg)
                .unwrap_or_default()
        }
        #[cfg(not(feature = "avatars"))]
        {
            let _ = org_cfg;
            std::collections::HashMap::new()
        }
    } else {
        std::collections::HashMap::new()
    };

    // 6. Ensure D2 browser bundle is available for diagram rendering
    #[cfg(feature = "avatars")] // reuse ureq dep from avatars feature
    {
        let d2_dest = output_dir.join("data/d2/d2-browser.js");
        if !d2_dest.exists() {
            let cache_dir = dir.join(".dg/cache/d2");
            let cache_file = cache_dir.join("d2-browser.js");

            // Use cached copy if available
            if cache_file.is_file() {
                if let Some(parent) = d2_dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::copy(&cache_file, &d2_dest);
            } else {
                // Download from CDN
                let url =
                    "https://cdn.jsdelivr.net/npm/@terrastruct/d2@0.1.33/dist/browser/index.js";
                match ureq::get(url).call() {
                    Ok(resp) => {
                        if let Ok(body) = resp.into_body().read_to_vec() {
                            let _ = std::fs::create_dir_all(&cache_dir);
                            let _ = std::fs::write(&cache_file, &body);
                            if let Some(parent) = d2_dest.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            let _ = std::fs::write(&d2_dest, &body);
                            eprintln!(
                                "Downloaded D2 browser bundle ({:.1} MB)",
                                body.len() as f64 / 1_048_576.0
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: could not download D2 bundle: {e}");
                    }
                }
            }
        }
    }

    // 7. Generate JSON data files
    let roadmap_html = config.roadmap_html.as_deref();
    let data_count = data::generate_data_files(
        output_dir,
        &docs,
        &graph,
        org,
        schema,
        &by_type,
        config,
        roadmap_html,
        &avatar_map,
        dir,
    )?;

    // 8. Copy static assets (images, etc.) from doc folders to output
    let asset_count = copy_doc_assets(dir, output_dir, schema)?;

    // 9. Write per-route index.html for static server compatibility
    let fallback_count = write_fallback_pages(dir, output_dir, &docs, &by_type, org, schema)?;

    Ok(spa_count + data_count + asset_count + fallback_count)
}

/// Copy static assets (images, PDFs, etc.) from the `docs/` tree to the site output.
///
/// Walks each schema type folder and copies non-markdown files, preserving
/// the directory structure relative to the project root. For example,
/// `docs/assets/proc-001/screenshot.png` → `<output>/docs/assets/proc-001/screenshot.png`.
fn copy_doc_assets(
    project_dir: &Path,
    output_dir: &Path,
    schema: &crate::schema::Schema,
) -> crate::error::Result<usize> {
    use std::collections::HashSet;

    let asset_exts: HashSet<&str> = [
        "png", "jpg", "jpeg", "gif", "svg", "webp", "ico", "pdf", "webm", "mp4",
    ]
    .into_iter()
    .collect();

    let mut count = 0;

    // Collect unique parent directories from schema type folders (e.g. "docs")
    let mut roots: HashSet<&str> = HashSet::new();
    for td in &schema.types {
        if let Some(folder) = &td.folder {
            // "docs/architecture" → "docs"
            if let Some(root) = folder.split('/').next() {
                roots.insert(root);
            }
        }
    }

    for root in roots {
        let src_root = project_dir.join(root);
        if !src_root.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&src_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !asset_exts.contains(ext.as_str()) {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(project_dir)
                .unwrap_or(entry.path());
            let dest = output_dir.join(rel);
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(entry.path(), &dest);
            count += 1;
        }
    }

    Ok(count)
}

/// Write a copy of index.html into each SPA route directory so plain static
/// servers (e.g. `python3 -m http.server`) can serve deep links without 404.
fn write_fallback_pages(
    project_dir: &Path,
    output_dir: &Path,
    docs: &[(String, Document)],
    by_type: &BTreeMap<String, Vec<(String, &Document)>>,
    org: Option<&OrgConfig>,
    schema: &crate::schema::Schema,
) -> crate::error::Result<usize> {
    let index_html = std::fs::read(output_dir.join("index.html"))
        .map_err(|_| crate::error::Error::WriteFailed(output_dir.join("index.html")))?;

    let mut routes: Vec<String> = vec![
        "roadmap".into(),
        "graph".into(),
        "onboarding".into(),
        "kanban".into(),
        "org".into(),
        "org/teams".into(),
        "org/users".into(),
        "org/entities".into(),
    ];

    // Per-doc-type list + per-doc routes — use schema nav route names, not schema folder
    let nav_types = schema.nav_types();
    for (type_key, docs) in by_type {
        let spa_route = nav_types
            .iter()
            .find(|(key, _, _)| *key == type_key.as_str())
            .map(|(_, folder, _)| folder.as_str())
            .unwrap_or(type_key.as_str());
        routes.push(spa_route.to_string());
        for (id, _) in docs {
            routes.push(format!("{}/{}", spa_route, id.to_lowercase()));
        }
    }

    // Org entity routes
    if let Some(org_cfg) = org {
        for id in org_cfg.teams.keys() {
            routes.push(format!("org/teams/{}", id.to_lowercase()));
        }
        for user in org_cfg.users.values() {
            routes.push(format!("org/users/{}", user.handle.to_lowercase()));
        }
        for id in org_cfg.orgs.keys() {
            routes.push(format!("org/{}", id.to_lowercase()));
        }
    }

    // Service + software routes (kind slugs mirror the SPA's /software/[kind] map)
    let mut software: Vec<(&str, Vec<PathBuf>)> = Vec::new();
    if let Ok(svc) = crate::service::discover_service_readmes(project_dir) {
        software.push(("services", svc));
    }
    if let Ok(apps) = crate::service::discover_app_readmes(project_dir) {
        software.push(("apps", apps));
    }
    if let Ok(infra) = crate::service::discover_infra_readmes(project_dir) {
        software.push(("infra", infra));
    }
    for (kind_slug, readmes) in &software {
        if readmes.is_empty() {
            continue;
        }
        routes.push(format!("software/{}", kind_slug));
        for readme_path in readmes {
            let Some(slug) = readme_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
            else {
                continue;
            };
            routes.push(format!("services/{}", slug.to_lowercase()));
            routes.push(format!("software/{}/{}", kind_slug, slug.to_lowercase()));
        }
    }

    // Tag routes from doc frontmatter
    let mut tags: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (_, doc) in docs {
        if let Some(fm) = &doc.frontmatter {
            if let Some(value) = fm.get("tags") {
                for tag in data::yaml_to_string_list(value) {
                    tags.insert(tag.to_lowercase());
                }
            }
        }
    }
    for tag in &tags {
        routes.push(format!("tags/{}", tag));
    }

    let mut count = 0;
    for route in &routes {
        let dir = output_dir.join(route);
        std::fs::create_dir_all(&dir).map_err(|_| crate::error::Error::WriteFailed(dir.clone()))?;
        write_atomic(&dir.join("index.html"), &index_html)?;
        count += 1;
    }
    Ok(count)
}

/// Write a file via temp-file + rename so `dg serve` never serves a
/// half-written file during a live rebuild.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> crate::error::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp-write");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).map_err(|_| crate::error::Error::WriteFailed(tmp.clone()))?;
    std::fs::rename(&tmp, path).map_err(|_| crate::error::Error::WriteFailed(path.to_path_buf()))
}

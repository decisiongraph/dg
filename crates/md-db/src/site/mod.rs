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
            let _ = crate::avatars::sync_avatars(dir, org_cfg);
            crate::avatars::copy_avatars_to_output(dir, output_dir, org_cfg).unwrap_or_default()
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

    Ok(spa_count + data_count)
}

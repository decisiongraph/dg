//! Devicon icon management - downloads and caches SVG icons locally.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::LazyLock;

/// Mapping of technology names to devicon slugs and variants.
static DEVICON_MAP: LazyLock<HashMap<&'static str, (&'static str, &'static str)>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();

        // Languages
        m.insert("rust", ("rust", "plain"));
        m.insert("javascript", ("javascript", "original"));
        m.insert("node.js", ("javascript", "original"));
        m.insert("typescript", ("typescript", "original"));
        m.insert("python", ("python", "original"));
        m.insert("ruby", ("ruby", "original"));
        m.insert("go", ("go", "original"));
        m.insert("java", ("java", "original"));
        m.insert("php", ("php", "original"));
        m.insert("c#", ("csharp", "original"));
        m.insert(".net", ("csharp", "original"));
        m.insert("html", ("html5", "original"));
        m.insert("css", ("css3", "original"));

        // Frameworks
        m.insert("rails", ("rails", "plain"));
        m.insert("react", ("react", "original"));
        m.insert("vue", ("vuejs", "original"));
        m.insert("angular", ("angularjs", "original"));
        m.insert("django", ("django", "plain"));
        m.insert("flask", ("flask", "original"));
        m.insert("express", ("express", "original"));
        m.insert("next.js", ("nextjs", "original"));
        m.insert("nuxt", ("nuxtjs", "original"));
        m.insert("svelte", ("svelte", "original"));
        m.insert("fastapi", ("fastapi", "original"));
        m.insert("nestjs", ("nestjs", "plain"));

        // Deployment platforms
        m.insert("docker", ("docker", "plain"));
        m.insert("heroku", ("heroku", "original"));
        m.insert("vercel", ("vercel", "original"));
        m.insert("netlify", ("netlify", "original"));
        m.insert("aws", ("amazonwebservices", "plain-wordmark"));
        m.insert(
            "aws elastic beanstalk",
            ("amazonwebservices", "plain-wordmark"),
        );

        // Databases
        m.insert("postgresql", ("postgresql", "plain"));
        m.insert("mysql", ("mysql", "plain"));
        m.insert("mongodb", ("mongodb", "plain"));
        m.insert("redis", ("redis", "plain"));
        m.insert("sqlite", ("sqlite", "plain"));

        m
    });

/// Build a map of tech name → CDN URL for all known devicons.
pub fn build_cdn_url_map() -> BTreeMap<String, String> {
    DEVICON_MAP
        .iter()
        .map(|(&name, &(slug, variant))| {
            (
                name.to_string(),
                format!(
                    "https://cdn.jsdelivr.net/gh/devicons/devicon/icons/{}/{}-{}.svg",
                    slug, slug, variant
                ),
            )
        })
        .collect()
}

/// Get local path for a devicon SVG.
pub fn devicon_path(name: &str) -> Option<String> {
    let key = name.to_lowercase();
    DEVICON_MAP
        .get(key.as_str())
        .map(|(slug, variant)| format!("devicons/{}-{}.svg", slug, variant))
}

/// Download devicon SVG content from CDN.
fn fetch_devicon_svg(slug: &str, variant: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!(
        "https://cdn.jsdelivr.net/gh/devicons/devicon/icons/{}/{}-{}.svg",
        slug, slug, variant
    );

    // Use system curl to fetch the SVG
    let output = std::process::Command::new("curl")
        .arg("-sL")
        .arg(&url)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(format!("Failed to fetch devicon: {}", url).into())
    }
}

/// Ensure devicon SVGs are downloaded to the site directory.
pub fn ensure_devicons(site_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let devicons_dir = site_dir.join("devicons");
    std::fs::create_dir_all(&devicons_dir)?;

    // Download all devicons we might need
    for (slug, variant) in DEVICON_MAP.values() {
        let filename = format!("{}-{}.svg", slug, variant);
        let filepath = devicons_dir.join(&filename);

        // Skip if already exists
        if filepath.exists() {
            continue;
        }

        // Download the SVG
        match fetch_devicon_svg(slug, variant) {
            Ok(svg_content) => {
                std::fs::write(&filepath, svg_content)?;
                eprintln!("Downloaded devicon: {}", filename);
            }
            Err(e) => {
                eprintln!("Warning: Failed to download devicon {}: {}", filename, e);
            }
        }
    }

    Ok(())
}

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
        m.insert("elixir", ("elixir", "original"));
        m.insert("terraform", ("terraform", "original"));
        // Devicon has no OpenTofu icon yet; reuse the Terraform one.
        m.insert("opentofu", ("terraform", "original"));

        // Frameworks
        m.insert("rails", ("rails", "plain"));
        m.insert("react", ("react", "original"));
        m.insert("react native", ("reactnative", "original"));
        m.insert("expo", ("expo", "original"));
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
        m.insert("phoenix", ("phoenix", "original"));

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
        m.insert("cloudflare", ("cloudflare", "original"));
        m.insert("cloudflare pages", ("cloudflare", "original"));
        m.insert("cloudflare workers", ("cloudflareworkers", "original"));

        // Cloud platforms (Terraform/OpenTofu provider pills)
        m.insert("google cloud", ("googlecloud", "original"));
        m.insert("gcp", ("googlecloud", "original"));
        m.insert("azure", ("azure", "original"));
        m.insert("kubernetes", ("kubernetes", "plain"));
        m.insert("digitalocean", ("digitalocean", "original"));

        // Databases
        m.insert("postgresql", ("postgresql", "plain"));
        m.insert("mysql", ("mysql", "plain"));
        m.insert("mongodb", ("mongodb", "plain"));
        m.insert("redis", ("redis", "plain"));
        m.insert("sqlite", ("sqlite", "plain"));

        m
    });

/// Logos that devicon doesn't ship, served from their official repos.
/// Maps tech name → (local filename, full CDN URL). URLs are pinned to a
/// release tag so upstream renames can't break the site.
static CUSTOM_ICON_MAP: LazyLock<HashMap<&'static str, (&'static str, &'static str)>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();
        m.insert(
            "ash",
            (
                "ash-logo-orange.svg",
                "https://cdn.jsdelivr.net/gh/ash-project/ash@v3.31.3/logos/ash_logo_orange.svg",
            ),
        );
        m
    });

/// Short blurb + homepage for a technology, shown as a logo tooltip.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TechInfo {
    pub description: &'static str,
    pub url: &'static str,
}

/// Tech name → blurb + homepage (GitHub repo when a project has no
/// dedicated site). Keys are lowercase; look up with `name.to_lowercase()`.
static TECH_INFO_MAP: LazyLock<HashMap<&'static str, TechInfo>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    let mut add = |name, description, url| m.insert(name, TechInfo { description, url });

    // Languages
    add(
        "rust",
        "Systems programming language focused on memory safety and performance",
        "https://www.rust-lang.org",
    );
    add(
        "javascript",
        "The scripting language of the web",
        "https://developer.mozilla.org/docs/Web/JavaScript",
    );
    add(
        "node.js",
        "JavaScript runtime for servers and tooling",
        "https://nodejs.org",
    );
    add(
        "typescript",
        "JavaScript with static types, compiled by tsc",
        "https://www.typescriptlang.org",
    );
    add(
        "python",
        "General-purpose language known for readability and data tooling",
        "https://www.python.org",
    );
    add(
        "ruby",
        "Dynamic language optimized for programmer happiness",
        "https://www.ruby-lang.org",
    );
    add(
        "go",
        "Google's statically typed language for simple, concurrent services",
        "https://go.dev",
    );
    add(
        "java",
        "Long-standing JVM language for enterprise software",
        "https://www.java.com",
    );
    add(
        "php",
        "Server-side scripting language powering much of the web",
        "https://www.php.net",
    );
    add(
        "c#",
        "Microsoft's object-oriented language on the .NET platform",
        "https://learn.microsoft.com/dotnet/csharp/",
    );
    add(
        ".net",
        "Microsoft's cross-platform application framework",
        "https://dotnet.microsoft.com",
    );
    add(
        "html",
        "Markup language that structures web pages",
        "https://developer.mozilla.org/docs/Web/HTML",
    );
    add(
        "css",
        "Stylesheet language for web page presentation",
        "https://developer.mozilla.org/docs/Web/CSS",
    );
    add(
        "elixir",
        "Functional language on the Erlang VM for scalable, fault-tolerant apps",
        "https://elixir-lang.org",
    );
    add(
        "terraform",
        "Infrastructure-as-code tool by HashiCorp",
        "https://developer.hashicorp.com/terraform",
    );
    add(
        "opentofu",
        "Open-source Terraform fork under the Linux Foundation",
        "https://opentofu.org",
    );

    // Frameworks
    add(
        "rails",
        "Full-stack Ruby web framework, convention over configuration",
        "https://rubyonrails.org",
    );
    add(
        "react",
        "UI library for building component-based interfaces",
        "https://react.dev",
    );
    add(
        "react native",
        "Build native iOS/Android apps with React",
        "https://reactnative.dev",
    );
    add(
        "expo",
        "Toolchain and platform for building React Native apps",
        "https://expo.dev",
    );
    add(
        "vue",
        "Progressive JavaScript framework for building UIs",
        "https://vuejs.org",
    );
    add(
        "angular",
        "Google's TypeScript web application framework",
        "https://angular.dev",
    );
    add(
        "django",
        "Batteries-included Python web framework",
        "https://www.djangoproject.com",
    );
    add(
        "flask",
        "Minimal Python web framework",
        "https://flask.palletsprojects.com",
    );
    add(
        "express",
        "Minimal Node.js web framework",
        "https://expressjs.com",
    );
    add(
        "next.js",
        "React framework with SSR, routing and bundling by Vercel",
        "https://nextjs.org",
    );
    add(
        "nuxt",
        "Vue framework with SSR and file-based routing",
        "https://nuxt.com",
    );
    add(
        "svelte",
        "Compiler-based UI framework with no virtual DOM",
        "https://svelte.dev",
    );
    add(
        "fastapi",
        "Fast Python API framework with automatic OpenAPI docs",
        "https://fastapi.tiangolo.com",
    );
    add(
        "nestjs",
        "TypeScript Node.js framework with modular architecture",
        "https://nestjs.com",
    );
    add(
        "phoenix",
        "Productive Elixir web framework with real-time LiveView",
        "https://www.phoenixframework.org",
    );
    add(
        "ash",
        "Declarative, resource-oriented application framework for Elixir",
        "https://ash-hq.org",
    );

    // Deployment platforms
    add(
        "docker",
        "Container platform for packaging and running applications",
        "https://www.docker.com",
    );
    add(
        "heroku",
        "Platform-as-a-service for deploying apps from git",
        "https://www.heroku.com",
    );
    add(
        "vercel",
        "Frontend cloud for deploying web apps and serverless functions",
        "https://vercel.com",
    );
    add(
        "netlify",
        "Platform for deploying static sites and serverless functions",
        "https://www.netlify.com",
    );
    add(
        "aws",
        "Amazon Web Services cloud platform",
        "https://aws.amazon.com",
    );
    add(
        "aws elastic beanstalk",
        "AWS service for deploying apps without managing servers",
        "https://aws.amazon.com/elasticbeanstalk/",
    );
    add(
        "cloudflare",
        "CDN, DNS and edge security platform",
        "https://www.cloudflare.com",
    );
    add(
        "cloudflare pages",
        "Cloudflare's platform for deploying static sites and full-stack apps",
        "https://pages.cloudflare.com",
    );
    add(
        "cloudflare workers",
        "Serverless functions running on Cloudflare's edge network",
        "https://workers.cloudflare.com",
    );
    add(
        "google cloud",
        "Google's cloud computing platform",
        "https://cloud.google.com",
    );
    add(
        "gcp",
        "Google's cloud computing platform",
        "https://cloud.google.com",
    );
    add(
        "azure",
        "Microsoft's cloud computing platform",
        "https://azure.microsoft.com",
    );
    add(
        "kubernetes",
        "Container orchestration system for automating deployment and scaling",
        "https://kubernetes.io",
    );
    add(
        "digitalocean",
        "Developer-friendly cloud hosting",
        "https://www.digitalocean.com",
    );

    // Databases
    add(
        "postgresql",
        "Advanced open-source relational database",
        "https://www.postgresql.org",
    );
    add(
        "mysql",
        "Popular open-source relational database",
        "https://www.mysql.com",
    );
    add(
        "mongodb",
        "Document-oriented NoSQL database",
        "https://www.mongodb.com",
    );
    add(
        "redis",
        "In-memory key-value store for caching and queues",
        "https://redis.io",
    );
    add(
        "sqlite",
        "Embedded zero-configuration SQL database",
        "https://www.sqlite.org",
    );

    m
});

/// Build a map of tech name → description + homepage for all known techs.
pub fn build_tech_info_map() -> BTreeMap<String, TechInfo> {
    TECH_INFO_MAP
        .iter()
        .map(|(&name, info)| (name.to_string(), info.clone()))
        .collect()
}

/// Build a map of tech name → CDN URL for all known icons.
pub fn build_cdn_url_map() -> BTreeMap<String, String> {
    let mut map: BTreeMap<String, String> = DEVICON_MAP
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
        .collect();
    for (&name, &(_, url)) in CUSTOM_ICON_MAP.iter() {
        map.insert(name.to_string(), url.to_string());
    }
    map
}

/// Get local path for an icon SVG.
pub fn devicon_path(name: &str) -> Option<String> {
    let key = name.to_lowercase();
    if let Some((filename, _)) = CUSTOM_ICON_MAP.get(key.as_str()) {
        return Some(format!("devicons/{filename}"));
    }
    DEVICON_MAP
        .get(key.as_str())
        .map(|(slug, variant)| format!("devicons/{}-{}.svg", slug, variant))
}

/// Download SVG content from a CDN URL.
fn fetch_svg(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Use system curl to fetch the SVG
    let output = std::process::Command::new("curl")
        .arg("-sL")
        .arg(url)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8(output.stdout)?)
    } else {
        Err(format!("Failed to fetch icon: {}", url).into())
    }
}

/// Ensure icon SVGs are downloaded to the site directory.
pub fn ensure_devicons(site_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let devicons_dir = site_dir.join("devicons");
    std::fs::create_dir_all(&devicons_dir)?;

    let devicon_downloads = DEVICON_MAP.values().map(|(slug, variant)| {
        (
            format!("{slug}-{variant}.svg"),
            format!(
                "https://cdn.jsdelivr.net/gh/devicons/devicon/icons/{slug}/{slug}-{variant}.svg"
            ),
        )
    });
    let custom_downloads = CUSTOM_ICON_MAP
        .values()
        .map(|(filename, url)| (filename.to_string(), url.to_string()));

    // Download all icons we might need
    for (filename, url) in devicon_downloads.chain(custom_downloads) {
        let filepath = devicons_dir.join(&filename);

        // Skip if already exists
        if filepath.exists() {
            continue;
        }

        match fetch_svg(&url) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framework_logos_resolve() {
        let urls = build_cdn_url_map();
        assert!(urls["react native"].contains("reactnative-original.svg"));
        assert!(urls["expo"].contains("expo-original.svg"));
        assert!(urls["ash"].contains("ash-project"), "{}", urls["ash"]);

        assert_eq!(
            devicon_path("React Native").as_deref(),
            Some("devicons/reactnative-original.svg")
        );
        assert_eq!(
            devicon_path("Ash").as_deref(),
            Some("devicons/ash-logo-orange.svg")
        );
        assert_eq!(
            devicon_path("Expo").as_deref(),
            Some("devicons/expo-original.svg")
        );
    }
}

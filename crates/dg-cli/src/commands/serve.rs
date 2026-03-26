//! `dg serve` command — development server with live reload.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use clap::Args;
use md_db::schema::Schema;
use md_db::users::OrgConfig;
use notify::{RecursiveMode, Watcher};

#[derive(Args)]
pub struct ServeArgs {
    /// Port to serve on
    #[arg(short, long, default_value = "8080")]
    pub port: u16,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Output directory (default: .dg/site)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Skip initial build
    #[arg(long)]
    pub no_build: bool,

    /// Open browser automatically
    #[arg(long)]
    pub open: bool,
}

pub fn run(
    root: &Path,
    schema: &Schema,
    users: Option<&OrgConfig>,
    args: &ServeArgs,
    cache: &mut md_db::cache::DocCache,
) -> Result<()> {
    let output = args.output.clone().unwrap_or_else(|| root.join(".dg/site"));

    // Initial build
    if !args.no_build {
        println!("Building site...");
        build_site(root, schema, users, &output, cache)?;
        println!("✓ Site built to {}", output.display());
    }

    // Start HTTP server — try requested port, auto-increment if busy
    let (server, actual_port) = bind_server(&args.host, args.port)?;

    if actual_port != args.port {
        println!("Port {} in use, using {} instead", args.port, actual_port);
    }

    let addr = format!("{}:{}", args.host, actual_port);
    println!("Serving at http://{}", addr);
    println!("Press Ctrl+C to stop\n");

    // Open browser if requested
    if args.open {
        let url = format!("http://{}", addr);
        let _ = opener::open(&url);
    }

    // Start file watcher in background thread
    let rebuild_flag = Arc::new(Mutex::new(false));
    start_watcher(root, rebuild_flag.clone())?;

    // Serve HTTP requests
    serve_with_server(server, &output, root, schema, users, cache, rebuild_flag)?;

    Ok(())
}

fn build_site(
    root: &Path,
    schema: &Schema,
    users: Option<&OrgConfig>,
    output: &Path,
    cache: &mut md_db::cache::DocCache,
) -> Result<()> {
    use md_db::site::{self, SiteConfig};

    let title = super::site::resolve_title(None, users, root);
    let (roadmap_html, roadmap_generated_at) =
        match super::site::build_roadmap_html(root, schema, users, false, cache) {
            Ok((html, date)) => (Some(html), Some(date)),
            Err(_) => (None, None),
        };

    let readme_html = super::site::render_readme_html(root);

    let logo_path = super::site::detect_logo(root);

    let edit_url_prefix = super::site::detect_edit_url_prefix(root);

    let config = SiteConfig {
        title,
        roadmap: roadmap_html.is_some(),
        users: true,
        roadmap_html,
        roadmap_generated_at,
        readme_html,
        logo_path,
        edit_url_prefix,
        is_local_dev: true,
    };

    site::generate_site(root, schema, users, &config, output)?;
    Ok(())
}

fn start_watcher(root: &Path, rebuild_flag: Arc<Mutex<bool>>) -> Result<()> {
    let root = root.to_path_buf();

    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = notify::recommended_watcher(tx).unwrap();

        // Watch docs/ and services/ directories
        let _ = watcher.watch(root.join("docs").as_path(), RecursiveMode::Recursive);
        let _ = watcher.watch(root.join("services").as_path(), RecursiveMode::Recursive);

        // Watch git refs for new commits (triggers code-refs rescan)
        let git_refs = root.join(".git/refs/heads");
        if git_refs.is_dir() {
            let _ = watcher.watch(git_refs.as_path(), RecursiveMode::Recursive);
        }

        // In debug builds, watch ui/build/ for SPA changes (useful with bun build --watch)
        #[cfg(debug_assertions)]
        {
            for rel in &["../ui/build", "../../ui/build"] {
                let ui_build = root.join(rel);
                if ui_build.is_dir() {
                    let _ = watcher.watch(ui_build.as_path(), RecursiveMode::Recursive);
                    break;
                }
            }
        }

        let _ = watcher.watch(
            root.join("README.md").as_path(),
            RecursiveMode::NonRecursive,
        );
        let _ = watcher.watch(
            root.join(".dg/org.kdl").as_path(),
            RecursiveMode::NonRecursive,
        );
        let _ = watcher.watch(
            root.join(".dg/schema.kdl").as_path(),
            RecursiveMode::NonRecursive,
        );
        let _ = watcher.watch(
            root.join(".dg/schema-ext.kdl").as_path(),
            RecursiveMode::NonRecursive,
        );

        // Debounce events (collect for 300ms before signaling rebuild)
        let mut last_event = std::time::Instant::now();

        for event in rx.into_iter().flatten() {
            if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                let now = std::time::Instant::now();
                if now.duration_since(last_event) > Duration::from_millis(300) {
                    *rebuild_flag.lock().unwrap() = true;
                    last_event = now;
                }
            }
        }
    });

    Ok(())
}

fn serve_with_server(
    server: Arc<tiny_http::Server>,
    output: &Path,
    root: &Path,
    _schema: &Schema,
    _users: Option<&OrgConfig>,
    _cache: &mut md_db::cache::DocCache,
    rebuild_flag: Arc<Mutex<bool>>,
) -> Result<()> {
    let output = output.to_path_buf();
    let root = root.to_path_buf();

    println!("Watching for changes...");

    // Spawn rebuild thread
    let rebuild_output = output.clone();
    let rebuild_root = root.clone();
    std::thread::spawn(move || {
        let mut cache_local = md_db::cache::DocCache::default();

        loop {
            std::thread::sleep(Duration::from_millis(300));

            if *rebuild_flag.lock().unwrap() {
                println!("\n🔄 Change detected, rebuilding...");

                // Reload schema and users (they might have changed)
                let schema_content = std::fs::read_to_string(rebuild_root.join(".dg/schema.kdl"))
                    .unwrap_or_else(|_| dg_schemas::SCHEMA.to_string());
                let schema = match md_db::schema::Schema::from_str(&schema_content) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("✗ Schema parse error: {}", e);
                        *rebuild_flag.lock().unwrap() = false;
                        continue;
                    }
                };

                let users =
                    md_db::users::OrgConfig::from_file(rebuild_root.join(".dg/org.kdl")).ok();

                match build_site(
                    &rebuild_root,
                    &schema,
                    users.as_ref(),
                    &rebuild_output,
                    &mut cache_local,
                ) {
                    Ok(_) => println!("✓ Rebuilt"),
                    Err(e) => eprintln!("✗ Build failed: {}", e),
                }

                *rebuild_flag.lock().unwrap() = false;
            }
        }
    });

    // Serve HTTP requests concurrently — browsers load SPAs with 6+ parallel
    // requests for JS modules. Sequential handling causes white-screen race.
    let output = Arc::new(output);
    let root = Arc::new(root);
    loop {
        let request = match server.recv() {
            Ok(req) => req,
            Err(_) => break,
        };
        let output = Arc::clone(&output);
        let root = Arc::clone(&root);
        std::thread::spawn(move || {
            let response = handle_request(&request, &output, &root);
            let _ = request.respond(response);
        });
    }

    Ok(())
}

fn handle_request(
    request: &tiny_http::Request,
    output: &Path,
    root: &Path,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let raw_url: &str = request.url();
    // Split path and query string
    let (url_path, query_string) = raw_url.split_once('?').unwrap_or((raw_url, ""));

    // Handle POST /__dg/open?path=... — open file in editor
    if url_path == "/__dg/open" && request.method() == &tiny_http::Method::Post {
        return handle_open_file(query_string, root);
    }

    let url_path = url_path.trim_start_matches('/');
    let file_path = if url_path.is_empty() {
        output.join("index.html")
    } else {
        output.join(url_path)
    };

    if file_path.is_file() {
        let content = std::fs::read(&file_path).unwrap_or_default();
        let mime = mime_type(&file_path);
        let mut resp = tiny_http::Response::from_data(content).with_header(
            tiny_http::Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap(),
        );
        // Immutable assets (hashed filenames) get long cache; everything else no-cache
        let cache_val: &[u8] = if url_path.contains("/_app/immutable/") {
            b"public, max-age=31536000, immutable"
        } else {
            b"no-cache"
        };
        resp = resp
            .with_header(tiny_http::Header::from_bytes(&b"Cache-Control"[..], cache_val).unwrap());
        resp
    } else if file_path.is_dir() && file_path.join("index.html").is_file() {
        let content = std::fs::read(file_path.join("index.html")).unwrap_or_default();
        tiny_http::Response::from_data(content)
            .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], b"text/html").unwrap())
            .with_header(tiny_http::Header::from_bytes(&b"Cache-Control"[..], b"no-cache").unwrap())
    } else if has_file_extension(url_path) {
        // Try serving static assets from the project root (images, etc.)
        let root_path = root.join(url_path);
        if root_path.is_file() {
            let content = std::fs::read(&root_path).unwrap_or_default();
            let mime = mime_type(&root_path);
            tiny_http::Response::from_data(content)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], mime.as_bytes()).unwrap(),
                )
                .with_header(
                    tiny_http::Header::from_bytes(&b"Cache-Control"[..], b"no-cache").unwrap(),
                )
        } else {
            tiny_http::Response::from_data(b"Not Found".to_vec())
                .with_status_code(404)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], b"text/plain").unwrap(),
                )
        }
    } else {
        // SPA fallback: serve index.html for non-asset routes
        let fallback = output.join("index.html");
        let content = std::fs::read(&fallback).unwrap_or_default();
        tiny_http::Response::from_data(content)
            .with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], b"text/html; charset=utf-8")
                    .unwrap(),
            )
            .with_header(tiny_http::Header::from_bytes(&b"Cache-Control"[..], b"no-cache").unwrap())
    }
}

fn handle_open_file(
    query_string: &str,
    root: &Path,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    // Parse path= from query string
    let rel_path: Option<String> = query_string.split('&').find_map(|param| {
        let (key, value) = param.split_once('=')?;
        if key == "path" {
            // Simple percent-decode for file paths (spaces, etc.)
            Some(percent_decode(value))
        } else {
            None
        }
    });

    let Some(rel_path) = rel_path else {
        return tiny_http::Response::from_data(b"{\"error\":\"missing path\"}".to_vec())
            .with_status_code(400)
            .with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], b"application/json").unwrap(),
            );
    };

    // Prevent path traversal
    if rel_path.contains("..") {
        return tiny_http::Response::from_data(b"{\"error\":\"invalid path\"}".to_vec())
            .with_status_code(400)
            .with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], b"application/json").unwrap(),
            );
    }

    let abs_path = root.join(&rel_path);
    if !abs_path.is_file() {
        return tiny_http::Response::from_data(b"{\"error\":\"file not found\"}".to_vec())
            .with_status_code(404)
            .with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], b"application/json").unwrap(),
            );
    }

    let _ = opener::open(&abs_path);

    tiny_http::Response::from_data(b"{\"ok\":true}".to_vec()).with_header(
        tiny_http::Header::from_bytes(&b"Content-Type"[..], b"application/json").unwrap(),
    )
}

fn mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    }
}

/// Check if a URL path looks like a file request (has an extension).
fn has_file_extension(url_path: &str) -> bool {
    url_path
        .rsplit('/')
        .next()
        .is_some_and(|last| last.contains('.'))
}

/// Simple percent-decode for URL query values (handles %20, %2F, etc.)
fn percent_decode(input: &str) -> String {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(byte);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

fn bind_server(host: &str, start_port: u16) -> Result<(Arc<tiny_http::Server>, u16)> {
    for port in start_port..start_port + 10 {
        let addr = format!("{}:{}", host, port);
        match tiny_http::Server::http(&addr) {
            Ok(server) => return Ok((Arc::new(server), port)),
            Err(_) => continue,
        }
    }
    anyhow::bail!(
        "No available ports in range {}-{}",
        start_port,
        start_port + 9
    )
}

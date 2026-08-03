//! `dg serve` command — development server with live reload.
//!
//! The HTTP layer is a deliberately simple thread-per-connection server on
//! `std::net::TcpListener` with `Connection: close` semantics. It replaced
//! tiny_http, whose task pool could lose an accepted connection under
//! concurrent load (all pooled threads blocked reading idle keep-alive
//! connections + a lost condvar wakeup) — the browser then waited forever on
//! a request the server never read, showing a blank page.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
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
    let (listener, actual_port) = bind_server(&args.host, args.port)?;

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
    serve_with_listener(listener, &output, root, schema, users, cache, rebuild_flag)?;

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

        // Signal on every relevant event; the rebuild loop's 300ms poll coalesces
        // bursts. A trailing event during a rebuild re-sets the flag, so no save
        // is ever silently dropped.
        for event in rx.into_iter().flatten() {
            if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                *rebuild_flag.lock().unwrap() = true;
            }
        }
    });

    Ok(())
}

fn serve_with_listener(
    listener: TcpListener,
    output: &Path,
    root: &Path,
    _schema: &Schema,
    _users: Option<&OrgConfig>,
    cache: &mut md_db::cache::DocCache,
    rebuild_flag: Arc<Mutex<bool>>,
) -> Result<()> {
    let output = output.to_path_buf();
    let root = root.to_path_buf();

    println!("Watching for changes...");

    // Spawn rebuild thread, seeded with the warm cache from the initial build
    let rebuild_output = output.clone();
    let rebuild_root = root.clone();
    let mut cache_local = cache.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(300));

            if *rebuild_flag.lock().unwrap() {
                // Clear before rebuilding: events arriving mid-rebuild re-set the
                // flag and trigger a follow-up rebuild on the next poll.
                *rebuild_flag.lock().unwrap() = false;
                println!("\n🔄 Change detected, rebuilding...");

                // Reload schema and users (they might have changed)
                let schema_content = std::fs::read_to_string(rebuild_root.join(".dg/schema.kdl"))
                    .unwrap_or_else(|_| dg_schemas::SCHEMA.to_string());
                let schema = match md_db::schema::Schema::from_str(&schema_content) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("✗ Schema parse error: {}", e);
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
            }
        }
    });

    // Serve HTTP requests concurrently — browsers load SPAs with 6+ parallel
    // requests for JS modules, each on its own connection (we always answer
    // with Connection: close). Thread-per-connection is plenty for localhost.
    let output = Arc::new(output);
    let root = Arc::new(root);
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let output = Arc::clone(&output);
        let root = Arc::clone(&root);
        std::thread::spawn(move || {
            handle_connection(stream, &output, &root);
        });
    }

    Ok(())
}

/// A minimal HTTP/1.1 response.
struct HttpResponse {
    status: u16,
    content_type: &'static str,
    cache_control: Option<&'static str>,
    body: Vec<u8>,
}

impl HttpResponse {
    fn new(status: u16, content_type: &'static str, body: Vec<u8>) -> Self {
        Self {
            status,
            content_type,
            cache_control: None,
            body,
        }
    }

    fn with_cache(mut self, value: &'static str) -> Self {
        self.cache_control = Some(value);
        self
    }
}

/// Read one request from the connection, answer it, close the connection.
fn handle_connection(stream: TcpStream, output: &Path, root: &Path) {
    // Bound reads so an abandoned connection can't pin this thread forever
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));

    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });

    // Request line: "GET /path?query HTTP/1.1"
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let (Some(method), Some(raw_url)) = (parts.next(), parts.next()) else {
        return;
    };
    let method = method.to_string();
    let raw_url = raw_url.to_string();

    // Headers: we only need Content-Length to drain a request body
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let line = line.trim_end();
                if line.is_empty() {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("content-length") {
                        content_length = value.trim().parse().unwrap_or(0);
                    }
                }
            }
            Err(_) => return,
        }
    }

    // Drain a bounded request body so the client can read our response cleanly
    if content_length > 0 {
        let mut body = vec![0u8; content_length.min(1_048_576)];
        let _ = reader.read_exact(&mut body);
    }

    let response = handle_request(&method, &raw_url, output, root);
    let _ = write_response(stream, &response);
}

fn write_response(mut stream: TcpStream, response: &HttpResponse) -> std::io::Result<()> {
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "OK",
    };
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        reason,
        response.content_type,
        response.body.len()
    );
    if let Some(cache) = response.cache_control {
        head.push_str(&format!("Cache-Control: {}\r\n", cache));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()?;
    let _ = stream.shutdown(std::net::Shutdown::Both);
    Ok(())
}

fn handle_request(method: &str, raw_url: &str, output: &Path, root: &Path) -> HttpResponse {
    // Split path and query string
    let (url_path, query_string) = raw_url.split_once('?').unwrap_or((raw_url, ""));

    // Handle POST /__dg/open?path=... — open file in editor
    if url_path == "/__dg/open" && method.eq_ignore_ascii_case("POST") {
        return handle_open_file(query_string, root);
    }

    if !method.eq_ignore_ascii_case("GET") && !method.eq_ignore_ascii_case("HEAD") {
        return HttpResponse::new(405, "text/plain", b"Method Not Allowed".to_vec());
    }

    // Decode %XX escapes so assets with spaces/UTF-8 names resolve on disk
    let url_path = percent_decode_path(url_path.trim_start_matches('/'));

    // Reject traversal attempts before touching the filesystem
    if url_path.split('/').any(|seg| seg == "..") {
        return HttpResponse::new(404, "text/plain", b"Not Found".to_vec());
    }

    let file_path = if url_path.is_empty() {
        output.join("index.html")
    } else {
        output.join(&url_path)
    };

    if file_path.is_file() {
        let content = std::fs::read(&file_path).unwrap_or_default();
        // Immutable assets (hashed filenames) get long cache; everything else no-cache
        // (url_path has the leading slash trimmed, so match without it)
        let cache_val = if url_path.contains("_app/immutable/") {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        HttpResponse::new(200, mime_type(&file_path), content).with_cache(cache_val)
    } else if file_path.is_dir() && file_path.join("index.html").is_file() {
        let content = std::fs::read(file_path.join("index.html")).unwrap_or_default();
        HttpResponse::new(200, "text/html; charset=utf-8", content).with_cache("no-cache")
    } else if is_asset_request(&url_path) {
        // Try serving static assets from the project root (images, etc.)
        let root_path = root.join(&url_path);
        if root_path.is_file() {
            let content = std::fs::read(&root_path).unwrap_or_default();
            HttpResponse::new(200, mime_type(&root_path), content).with_cache("no-cache")
        } else {
            HttpResponse::new(404, "text/plain", b"Not Found".to_vec())
        }
    } else {
        // SPA fallback: serve index.html for non-asset routes
        let content = std::fs::read(output.join("index.html")).unwrap_or_default();
        HttpResponse::new(200, "text/html; charset=utf-8", content).with_cache("no-cache")
    }
}

fn handle_open_file(query_string: &str, root: &Path) -> HttpResponse {
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
        return HttpResponse::new(
            400,
            "application/json",
            b"{\"error\":\"missing path\"}".to_vec(),
        );
    };

    // Prevent path traversal
    if rel_path.contains("..") {
        return HttpResponse::new(
            400,
            "application/json",
            b"{\"error\":\"invalid path\"}".to_vec(),
        );
    }

    let abs_path = root.join(&rel_path);
    if !abs_path.is_file() {
        return HttpResponse::new(
            404,
            "application/json",
            b"{\"error\":\"file not found\"}".to_vec(),
        );
    }

    let _ = opener::open(&abs_path);

    HttpResponse::new(200, "application/json", b"{\"ok\":true}".to_vec())
}

fn mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") | Some("map") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("pdf") => "application/pdf",
        Some("webm") => "video/webm",
        Some("mp4") => "video/mp4",
        Some("txt") => "text/plain; charset=utf-8",
        Some("xml") => "application/xml",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Known static-asset extensions. Only these get 404-on-miss; anything else
/// falls through to the SPA (so routes with dots, e.g. /org/users/john.doe, work).
const ASSET_EXTENSIONS: &[&str] = &[
    "html", "css", "js", "mjs", "map", "json", "svg", "png", "jpg", "jpeg", "gif", "webp", "ico",
    "woff", "woff2", "ttf", "otf", "pdf", "webm", "mp4", "txt", "xml", "wasm",
];

/// Check if a URL path looks like a static-asset request.
fn is_asset_request(url_path: &str) -> bool {
    url_path
        .rsplit('/')
        .next()
        .and_then(|last| last.rsplit_once('.'))
        .is_some_and(|(_, ext)| ASSET_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Simple percent-decode for URL query values (handles %20, %2F; `+` = space).
fn percent_decode(input: &str) -> String {
    percent_decode_impl(input, true)
}

/// Percent-decode for URL paths — `+` stays literal (only queries encode space as `+`).
fn percent_decode_path(input: &str) -> String {
    percent_decode_impl(input, false)
}

fn percent_decode_impl(input: &str, plus_as_space: bool) -> String {
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
        } else if bytes[i] == b'+' && plus_as_space {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

fn bind_server(host: &str, start_port: u16) -> Result<(TcpListener, u16)> {
    for port in start_port..start_port + 10 {
        let addr = format!("{}:{}", host, port);
        match TcpListener::bind(&addr) {
            Ok(listener) => return Ok((listener, port)),
            Err(_) => continue,
        }
    }
    anyhow::bail!(
        "No available ports in range {}-{}",
        start_port,
        start_port + 9
    )
}

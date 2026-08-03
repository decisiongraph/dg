//! Integration test: `dg serve` must handle concurrent requests without hanging.
//!
//! Browsers load SPAs by fetching 10+ JS modules in parallel — historically this
//! surfaced two real bugs: sequential request handling (white screen) and
//! tiny_http's task pool losing an accepted connection under concurrent load
//! (page hung forever). This test drives the REAL `dg` binary.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

/// Find a free port by binding to port 0.
fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// Create a minimal SPA site structure that mimics what a SvelteKit build produces.
/// Returns the temp dir path (must stay alive for the test duration).
fn create_test_site() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    // Create .dg directory so dg recognizes it as a project
    std::fs::create_dir_all(root.join(".dg/site/_app/immutable/chunks")).unwrap();
    std::fs::create_dir_all(root.join(".dg/site/data")).unwrap();

    // Write index.html referencing multiple JS modules (like SvelteKit does)
    let mut modules = Vec::new();
    for i in 0..12 {
        let name = format!("chunk_{i}.js");
        let js_path = root.join(format!(".dg/site/_app/immutable/chunks/{name}"));
        let content = format!("// chunk {i}\nexport const id = {i};\n");
        std::fs::write(&js_path, content).unwrap();
        modules.push(format!(
            r#"<link href="/_app/immutable/chunks/{name}" rel="modulepreload">"#
        ));
    }

    let index_html = format!(
        r#"<!doctype html>
<html><head>{}</head>
<body><div id="app">loaded</div></body></html>"#,
        modules.join("\n")
    );
    std::fs::write(root.join(".dg/site/index.html"), index_html).unwrap();

    std::fs::write(
        root.join(".dg/site/data/docs.json"),
        r#"[{"id":"ADR-001","title":"Test"}]"#,
    )
    .unwrap();

    std::fs::write(root.join(".dg/schema.kdl"), dg_schemas::SCHEMA).unwrap();

    dir
}

/// Spawn the real `dg serve` binary against the prebuilt test site and parse
/// the address it prints (serve auto-increments if the port is busy).
fn spawn_dg_serve(root: &std::path::Path, port: u16) -> (Child, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dg"))
        .args(["serve", "--no-build", "--port", &port.to_string()])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("failed to spawn dg serve");

    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut addr = None;
    let mut line = String::new();
    while Instant::now() < deadline {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if let Some(rest) = line.trim().strip_prefix("Serving at http://") {
            addr = Some(rest.to_string());
            break;
        }
    }
    let addr = addr.expect("dg serve did not print its address");

    // Keep draining stdout — dropping the pipe would make the child's next
    // println! fail with a broken pipe and kill the server.
    std::thread::spawn(move || {
        let mut sink = String::new();
        loop {
            sink.clear();
            if reader.read_line(&mut sink).unwrap_or(0) == 0 {
                break;
            }
        }
    });

    (child, addr)
}

fn http_get(addr: &str, path: &str, timeout: Duration) -> String {
    let mut stream = std::net::TcpStream::connect(addr).expect("failed to connect");
    stream.set_read_timeout(Some(timeout)).unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new();
    let _ = std::io::Read::read_to_end(&mut stream, &mut response);
    String::from_utf8_lossy(&response).into_owned()
}

#[test]
fn serve_handles_concurrent_requests_without_hanging() {
    let site_dir = create_test_site();
    let (mut child, addr) = spawn_dg_serve(site_dir.path(), free_port());

    // Simulate browser: 12 concurrent requests (like SvelteKit module preloads)
    let num_requests = 12;
    let barrier = Arc::new(Barrier::new(num_requests));
    let start = Instant::now();

    let handles: Vec<_> = (0..num_requests)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            let addr = addr.clone();
            std::thread::spawn(move || {
                // All threads wait here, then fire simultaneously
                barrier.wait();
                let response = http_get(
                    &addr,
                    &format!("/_app/immutable/chunks/chunk_{i}.js"),
                    Duration::from_secs(5),
                );
                assert!(
                    response.starts_with("HTTP/1.1 200"),
                    "request {i} got non-200 response: {}",
                    response.lines().next().unwrap_or("(empty)")
                );
                assert!(
                    response.contains(&format!("export const id = {i};")),
                    "request {i} got wrong content"
                );
            })
        })
        .collect();

    for (i, h) in handles.into_iter().enumerate() {
        h.join()
            .unwrap_or_else(|_| panic!("request thread {i} panicked"));
    }

    let elapsed = start.elapsed();

    // All 12 concurrent requests should complete quickly; sequential handling
    // or a lost connection would blow well past this.
    assert!(
        elapsed < Duration::from_secs(3),
        "concurrent requests took {elapsed:?} — likely sequential handling"
    );

    // SPA fallback + routing behaviors
    let resp = http_get(&addr, "/architecture/adr-001", Duration::from_secs(5));
    assert!(resp.starts_with("HTTP/1.1 200"), "deep link should 200");
    assert!(
        resp.contains("<div id=\"app\">"),
        "deep link serves SPA shell"
    );

    let resp = http_get(&addr, "/org/users/john.doe", Duration::from_secs(5));
    assert!(
        resp.contains("<div id=\"app\">"),
        "dotted SPA route serves SPA shell, got: {}",
        resp.lines().next().unwrap_or("")
    );

    let resp = http_get(&addr, "/..%2F..%2FCargo.toml", Duration::from_secs(5));
    assert!(
        resp.starts_with("HTTP/1.1 404"),
        "traversal must 404, got: {}",
        resp.lines().next().unwrap_or("")
    );

    let resp = http_get(&addr, "/missing.png", Duration::from_secs(5));
    assert!(
        resp.starts_with("HTTP/1.1 404"),
        "missing asset must 404, got: {}",
        resp.lines().next().unwrap_or("")
    );

    let _ = child.kill();
    let _ = child.wait();
}

//! Integration test: `dg serve` must handle concurrent requests without hanging.
//!
//! Browsers load SPAs by fetching 10+ JS modules in parallel. If the HTTP server
//! handles requests sequentially, some get blocked causing a white screen.
//! This test verifies concurrent requests all complete within a reasonable timeout.

use std::io::Write;
use std::net::TcpListener;
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
        // Each chunk is non-trivial size to ensure the server actually reads the file
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

    // Write a data JSON file
    std::fs::write(
        root.join(".dg/site/data/docs.json"),
        r#"[{"id":"ADR-001","title":"Test"}]"#,
    )
    .unwrap();

    // Write minimal schema
    std::fs::write(root.join(".dg/schema.kdl"), dg_schemas::SCHEMA).unwrap();

    dir
}

#[test]
fn serve_handles_concurrent_requests_without_hanging() {
    let site_dir = create_test_site();
    let site_path = site_dir.path().join(".dg/site");
    let port = free_port();
    let addr = format!("127.0.0.1:{port}");

    // Start tiny_http server directly (same pattern as serve.rs)
    let server = Arc::new(tiny_http::Server::http(&addr).unwrap());
    let site_path_arc = Arc::new(site_path);

    // Spawn server thread handling requests concurrently (the fix we're testing)
    let server_clone = Arc::clone(&server);
    let site_clone = Arc::clone(&site_path_arc);
    let server_handle = std::thread::spawn(move || loop {
        let request = match server_clone.recv_timeout(Duration::from_secs(5)) {
            Ok(Some(req)) => req,
            Ok(None) | Err(_) => break,
        };
        let output = Arc::clone(&site_clone);
        std::thread::spawn(move || {
            let url_path = request.url().trim_start_matches('/');
            let file_path = if url_path.is_empty() {
                output.join("index.html")
            } else {
                output.join(url_path)
            };

            let response = if file_path.is_file() {
                let content = std::fs::read(&file_path).unwrap_or_default();
                tiny_http::Response::from_data(content)
            } else {
                let content = std::fs::read(output.join("index.html")).unwrap_or_default();
                tiny_http::Response::from_data(content)
            };
            let _ = request.respond(response);
        });
    });

    // Give server a moment to start
    std::thread::sleep(Duration::from_millis(50));

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

                let mut stream = std::net::TcpStream::connect(&addr).expect("failed to connect");
                stream
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();

                let request = format!(
                    "GET /_app/immutable/chunks/chunk_{i}.js HTTP/1.1\r\n\
                     Host: {addr}\r\n\
                     Connection: close\r\n\r\n"
                );
                stream.write_all(request.as_bytes()).unwrap();

                let mut response = Vec::new();
                loop {
                    let mut buf = [0u8; 4096];
                    match std::io::Read::read(&mut stream, &mut buf) {
                        Ok(0) => break,
                        Ok(n) => response.extend_from_slice(&buf[..n]),
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            panic!("request {i} timed out — server likely blocking");
                        }
                        Err(e) => panic!("request {i} read error: {e}"),
                    }
                }

                let response_str = String::from_utf8_lossy(&response);
                assert!(
                    response_str.starts_with("HTTP/1.1 200"),
                    "request {i} got non-200 response: {}",
                    response_str.lines().next().unwrap_or("(empty)")
                );
                assert!(
                    response_str.contains(&format!("export const id = {i};")),
                    "request {i} got wrong content"
                );
            })
        })
        .collect();

    // Wait for all requests to complete
    for (i, h) in handles.into_iter().enumerate() {
        h.join()
            .unwrap_or_else(|_| panic!("request thread {i} panicked"));
    }

    let elapsed = start.elapsed();

    // All 12 concurrent requests should complete in under 2 seconds.
    // Sequential handling would take much longer due to connection queuing.
    assert!(
        elapsed < Duration::from_secs(2),
        "concurrent requests took {elapsed:?} — likely sequential handling"
    );

    // Shut down
    server.unblock();
    let _ = server_handle.join();
}

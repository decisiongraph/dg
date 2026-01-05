fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let ui_dir = std::path::Path::new(&manifest_dir).join("../../ui");
    let ui_dir = ui_dir.canonicalize().unwrap_or(ui_dir);

    println!("cargo:rerun-if-changed={}/src", ui_dir.display());
    println!("cargo:rerun-if-changed={}/static", ui_dir.display());
    println!(
        "cargo:rerun-if-changed={}/svelte.config.js",
        ui_dir.display()
    );
    println!("cargo:rerun-if-changed={}/vite.config.js", ui_dir.display());
    println!("cargo:rerun-if-changed={}/package.json", ui_dir.display());
    println!("cargo:rerun-if-changed={}/bun.lock", ui_dir.display());

    // CI escape hatch: DG_SKIP_UI_BUILD=1 cargo build
    if std::env::var("DG_SKIP_UI_BUILD").is_ok() {
        return;
    }

    let build_dir = ui_dir.join("build");

    // Check if bun is available
    let bun = std::process::Command::new("bun").arg("--version").output();

    match bun {
        Ok(output) if output.status.success() => {
            // Install deps if needed
            if !ui_dir.join("node_modules").exists() {
                let install = std::process::Command::new("bun")
                    .current_dir(&ui_dir)
                    .arg("install")
                    .status()
                    .expect("Failed to run bun install");
                if !install.success() {
                    panic!("bun install failed");
                }
            }

            // Clean stale build output to prevent hash-mismatched chunks
            let _ = std::fs::remove_dir_all(build_dir.join("_app"));

            // Build SvelteKit
            let status = std::process::Command::new("bun")
                .current_dir(&ui_dir)
                .args(["run", "build"])
                .status()
                .expect("Failed to run bun run build");
            if !status.success() {
                panic!("SvelteKit build failed");
            }
        }
        _ => {
            // bun not available — create a minimal fallback
            eprintln!("cargo:warning=bun not found, creating minimal SPA fallback");
            std::fs::create_dir_all(&build_dir).expect("Failed to create ui/build/");
            std::fs::write(
                build_dir.join("index.html"),
                r#"<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>DecisionGraph</title></head>
<body>
<h1>DecisionGraph</h1>
<p>SPA not built. Install <a href="https://bun.sh">bun</a> and run <code>cd ui && bun install && bun run build</code>.</p>
<script>
fetch('./data/docs.json').then(r=>r.json()).then(data=>{
  const ul=document.createElement('ul');
  data.docs.forEach(d=>{
    const li=document.createElement('li');
    li.innerHTML='<strong>'+d.id+'</strong>: '+d.title+' <em>('+d.status+')</em>';
    ul.appendChild(li);
  });
  document.body.appendChild(ul);
}).catch(()=>{});
</script>
</body>
</html>"#,
            )
            .expect("Failed to write fallback index.html");
        }
    }
}

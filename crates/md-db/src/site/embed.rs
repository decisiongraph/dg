//! Embeds the pre-built SvelteKit SPA into the binary via rust-embed.
//!
//!
//! The SPA is built at compile time by `build.rs` (runs `bun run build` in `ui/`).
//! At runtime, `write_spa_files()` copies the embedded assets to the output directory.

use std::path::Path;

use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../../ui/build/"]
#[include = "*.html"]
#[include = "*.js"]
#[include = "*.css"]
#[include = "*.json"]
#[include = "*.svg"]
#[include = "*.png"]
#[include = "*.ico"]
#[include = "*.woff"]
#[include = "*.woff2"]
struct SpaAssets;

/// Write all embedded SPA files to `output_dir`.
///
/// Skips the write if the on-disk `_app/version.json` matches the embedded
/// version (same binary, no SPA changes). This avoids nuking `_app/` during
/// live-reload rebuilds in `dg serve`, which would cause the browser to fail
/// loading JS chunks while files are being rewritten.
///
/// Returns the number of files written (0 if skipped).
pub fn write_spa_files(output_dir: &Path) -> crate::error::Result<usize> {
    // Check if on-disk SPA matches embedded version — skip _app/ rewrite if so.
    // Always rewrite root HTML files (index.html, 200.html) since they can change
    // independently of the JS bundle version (e.g. app.html template changes).
    let version_path = output_dir.join("_app/version.json");
    let version_matches = version_path.is_file()
        && <SpaAssets as Embed>::get("_app/version.json")
            .and_then(|embedded| {
                std::fs::read(&version_path)
                    .ok()
                    .map(|on_disk| on_disk == embedded.data.as_ref())
            })
            .unwrap_or(false);

    if version_matches {
        // JS bundles unchanged — only refresh root HTML files
        let mut count = 0;
        for file_path in <SpaAssets as Embed>::iter() {
            let path_str = file_path.as_ref();
            if !path_str.ends_with(".html") || path_str.contains('/') {
                continue;
            }
            let content = match <SpaAssets as Embed>::get(path_str) {
                Some(c) => c,
                None => continue,
            };
            let dest = output_dir.join(path_str);
            let needs_update = std::fs::read(&dest)
                .map(|on_disk| on_disk != content.data.as_ref())
                .unwrap_or(true);
            if needs_update {
                std::fs::write(&dest, content.data.as_ref())
                    .map_err(|_| crate::error::Error::WriteFailed(dest))?;
                count += 1;
            }
        }
        return Ok(count);
    }

    // Remove stale SPA assets before writing current ones
    let app_dir = output_dir.join("_app");
    if app_dir.is_dir() {
        let _ = std::fs::remove_dir_all(&app_dir);
    }

    let mut count = 0;
    for file_path in <SpaAssets as Embed>::iter() {
        let content = match <SpaAssets as Embed>::get(&file_path) {
            Some(c) => c,
            None => continue,
        };
        let dest = output_dir.join(file_path.as_ref());
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|_| crate::error::Error::WriteFailed(parent.to_path_buf()))?;
        }
        std::fs::write(&dest, content.data.as_ref())
            .map_err(|_| crate::error::Error::WriteFailed(dest))?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same-version writes are skipped (no _app/ nuke during live reload).
    #[test]
    fn write_spa_files_skips_when_version_matches() {
        let tmp = std::env::temp_dir().join("dg-test-spa-skip");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // First write — populates _app/
        let count1 = write_spa_files(&tmp).unwrap();
        assert!(count1 > 0, "should write at least one SPA file");

        // Second write — same binary, should skip
        let count2 = write_spa_files(&tmp).unwrap();
        assert_eq!(count2, 0, "should skip when version matches");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Stale _app/ assets from a different SPA version are cleaned on rewrite.
    #[test]
    fn write_spa_files_removes_stale_assets() {
        let tmp = std::env::temp_dir().join("dg-test-spa-stale");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // First write — populates _app/
        let count1 = write_spa_files(&tmp).unwrap();
        assert!(count1 > 0, "should write at least one SPA file");

        // Corrupt version.json to simulate a different SPA version
        let version_path = tmp.join("_app/version.json");
        std::fs::write(&version_path, b"{}").unwrap();

        // Plant a stale file
        let stale = tmp.join("_app/immutable/chunks/STALE_OLD_HASH.js");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(&stale, b"// stale").unwrap();
        assert!(stale.exists());

        // Second write — version mismatch, should clean _app/ and rewrite
        let count2 = write_spa_files(&tmp).unwrap();
        assert_eq!(count1, count2, "same files should be written");
        assert!(
            !stale.exists(),
            "stale file must be removed by write_spa_files"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}

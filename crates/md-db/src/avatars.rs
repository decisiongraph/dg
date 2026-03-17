//! GitHub avatar fetching and caching.
//!
//! Looks up GitHub profile pictures by email via the commit search API,
//! caches them locally in `.dg/cache/avatars/`, and copies to site output.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use crate::users::OrgConfig;

const CACHE_DIR: &str = "cache/avatars";
const AVATAR_SIZE: u16 = 128;

/// Fetch missing GitHub avatars for all users with emails.
///
/// Avatars are cached in `{dg_root}/cache/avatars/{handle}.jpg`.
/// Only fetches avatars that aren't already cached.
/// Returns the number of newly fetched avatars.
pub fn sync_avatars(dg_root: &Path, org: &OrgConfig) -> anyhow::Result<usize> {
    let cache_dir = dg_root.join(CACHE_DIR);
    std::fs::create_dir_all(&cache_dir)?;

    let mut fetched = 0usize;

    for (handle, user) in &org.users {
        let avatar_path = cache_dir.join(format!("{handle}.jpg"));
        if avatar_path.exists() {
            continue;
        }

        let email = match &user.email {
            Some(e) => e,
            None => continue,
        };

        match fetch_avatar_by_email(email) {
            Ok(Some(bytes)) => {
                std::fs::write(&avatar_path, &bytes)?;
                eprintln!("Fetched avatar for @{handle}");
                fetched += 1;
            }
            Ok(None) => {
                // No GitHub account found for this email — skip silently
            }
            Err(e) => {
                eprintln!("Warning: failed to fetch avatar for @{handle}: {e}");
            }
        }
    }

    Ok(fetched)
}

/// Copy cached avatars to site output directory.
///
/// Returns a map of `handle → "data/avatars/{handle}.jpg"` for users with cached avatars.
pub fn copy_avatars_to_output(
    dg_root: &Path,
    output_dir: &Path,
    org: &OrgConfig,
) -> anyhow::Result<HashMap<String, String>> {
    let cache_dir = dg_root.join(CACHE_DIR);
    let out_avatars = output_dir.join("data/avatars");
    std::fs::create_dir_all(&out_avatars)?;

    let mut map = HashMap::new();

    for handle in org.users.keys() {
        let cached = cache_dir.join(format!("{handle}.jpg"));
        if cached.exists() {
            let dest = out_avatars.join(format!("{handle}.jpg"));
            std::fs::copy(&cached, &dest)?;
            map.insert(handle.clone(), format!("/data/avatars/{handle}.jpg"));
        }
    }

    Ok(map)
}

/// Look up a GitHub user's avatar URL by email via the commit search API,
/// then download the image bytes.
fn fetch_avatar_by_email(email: &str) -> anyhow::Result<Option<Vec<u8>>> {
    let search_url =
        format!("https://api.github.com/search/commits?q=author-email:{email}&per_page=1");

    let resp = ureq::get(&search_url)
        .header("Accept", "application/vnd.github.cloak-preview+json")
        .header("User-Agent", "dg-cli")
        .call()?;

    let body = resp.into_body().read_to_string()?;
    let json: serde_json::Value = serde_json::from_str(&body)?;

    let avatar_url = json
        .pointer("/items/0/author/avatar_url")
        .and_then(|v| v.as_str());

    let url = match avatar_url {
        Some(u) => format!("{u}&s={AVATAR_SIZE}"),
        None => return Ok(None),
    };

    let img_resp = ureq::get(&url).header("User-Agent", "dg-cli").call()?;

    let mut buf = Vec::new();
    img_resp.into_body().as_reader().read_to_end(&mut buf)?;

    Ok(Some(buf))
}

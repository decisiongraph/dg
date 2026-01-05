//! End-of-Life version detection via the endoflife.date API.
//!
//! Checks language and framework versions against the community-maintained
//! EOL database. Results are cached in `.dg/cache/eol/` with a 7-day TTL.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::service::TechStack;

/// Warning emitted when a detected version is end-of-life.
#[derive(Debug, Clone, Serialize)]
pub struct EolWarning {
    /// Human-readable product name ("Ruby", "Node.js")
    pub product: String,
    /// Matched cycle version ("3.1")
    pub version: String,
    /// EOL date if known ("2025-03-31"), None if boolean true
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eol_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedProduct {
    fetched_at: String,
    cycles: Vec<serde_json::Value>,
}

// ── Product slug mapping ────────────────────────────────────────────────

/// Map a detected language/framework name to an endoflife.date product slug.
fn product_slug(name: &str) -> Option<&'static str> {
    match name.to_lowercase().as_str() {
        "ruby" => Some("ruby"),
        "javascript" | "typescript" => Some("nodejs"),
        "python" => Some("python"),
        "go" | "golang" => Some("go"),
        "elixir" => Some("elixir"),
        "rust" => Some("rust"),
        "rails" | "ruby on rails" => Some("rails"),
        "react" => Some("react"),
        "django" => Some("django"),
        "laravel" => Some("laravel"),
        "vue" | "vue.js" => Some("vue"),
        "angular" => Some("angular"),
        "next.js" | "nextjs" => Some("nextjs"),
        "node" | "node.js" | "nodejs" => Some("nodejs"),
        _ => None,
    }
}

/// Human-readable display name for a product slug.
fn product_display_name(slug: &str) -> &str {
    match slug {
        "ruby" => "Ruby",
        "nodejs" => "Node.js",
        "python" => "Python",
        "go" => "Go",
        "elixir" => "Elixir",
        "rust" => "Rust",
        "rails" => "Rails",
        "react" => "React",
        "django" => "Django",
        "laravel" => "Laravel",
        "vue" => "Vue",
        "angular" => "Angular",
        "nextjs" => "Next.js",
        other => other,
    }
}

// ── Date math (same epoch-day approach as suggest.rs) ───────────────────

fn parse_date(s: &str) -> Option<(i64, i64, i64)> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() < 3 {
        return None;
    }
    let y = parts[0].parse::<i64>().ok()?;
    let m = parts[1].parse::<i64>().ok()?;
    let d = parts[2].parse::<i64>().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some((y, m, d))
}

fn epoch_days(y: i64, m: i64, d: i64) -> i64 {
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    365 * y + y / 4 - y / 100 + y / 400 + (m * 306 + 5) / 10 + d - 1
}

fn days_between(a: &str, b: &str) -> Option<i64> {
    let (ay, am, ad) = parse_date(a)?;
    let (by, bm, bd) = parse_date(b)?;
    Some(epoch_days(ay, am, ad) - epoch_days(by, bm, bd))
}

// ── Version normalization ───────────────────────────────────────────────

/// Normalize a version string to major.minor for cycle matching.
/// "3.1.5" → "3.1", ">=18" → "18", "~20.0.1" → "20.0", "^3.2" → "3.2"
fn normalize_version(version: &str) -> String {
    // Strip leading constraint chars
    let v = version.trim_start_matches(|c: char| "^~>=<".contains(c));
    // Strip trailing .x or .*
    let v = v.trim_end_matches(".x").trim_end_matches(".*");
    // Split by dots, take first two segments
    let parts: Vec<&str> = v.split('.').collect();
    match parts.len() {
        0 => String::new(),
        1 => parts[0].to_string(),
        _ => format!("{}.{}", parts[0], parts[1]),
    }
}

// ── Cache ───────────────────────────────────────────────────────────────

fn cache_path(cache_dir: &Path, product: &str) -> std::path::PathBuf {
    cache_dir.join("eol").join(format!("{product}.json"))
}

fn read_cache(cache_dir: &Path, product: &str, today: &str) -> Option<Vec<serde_json::Value>> {
    let path = cache_path(cache_dir, product);
    let data = std::fs::read_to_string(&path).ok()?;
    let cached: CachedProduct = serde_json::from_str(&data).ok()?;
    // 7-day TTL
    let age = days_between(today, &cached.fetched_at)?;
    if age <= 7 {
        Some(cached.cycles)
    } else {
        None
    }
}

fn write_cache(
    cache_dir: &Path,
    product: &str,
    today: &str,
    cycles: &[serde_json::Value],
) -> Option<()> {
    let path = cache_path(cache_dir, product);
    std::fs::create_dir_all(path.parent()?).ok()?;
    let cached = CachedProduct {
        fetched_at: today.to_string(),
        cycles: cycles.to_vec(),
    };
    let json = serde_json::to_string(&cached).ok()?;
    std::fs::write(&path, json).ok()
}

// ── API fetch ───────────────────────────────────────────────────────────

fn fetch_product_cycles(product: &str, cache_dir: &Path, today: &str) -> Vec<serde_json::Value> {
    // Try fresh cache first
    if let Some(cycles) = read_cache(cache_dir, product, today) {
        return cycles;
    }

    // Fetch from API
    let url = format!("https://endoflife.date/api/{product}.json");
    let cycles: Vec<serde_json::Value> = match ureq::get(&url).call() {
        Ok(resp) => {
            let body = match resp.into_body().read_to_string() {
                Ok(b) => b,
                Err(_) => return Vec::new(),
            };
            serde_json::from_str(&body).unwrap_or_default()
        }
        Err(_) => {
            // Network failure — try stale cache
            let path = cache_path(cache_dir, product);
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Ok(cached) = serde_json::from_str::<CachedProduct>(&data) {
                    return cached.cycles;
                }
            }
            return Vec::new();
        }
    };

    write_cache(cache_dir, product, today, &cycles);
    cycles
}

// ── EOL check logic ────────────────────────────────────────────────────

/// Check if an `eol` field value indicates the product is end-of-life.
fn is_eol(eol_field: &serde_json::Value, today: &str) -> bool {
    match eol_field {
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::String(date) => {
            // EOL if today >= eol_date (i.e., days_between(today, eol_date) >= 0)
            days_between(today, date).map(|d| d >= 0).unwrap_or(false)
        }
        _ => false,
    }
}

/// Extract the EOL date string from the field, if it's a date.
fn eol_date_str(eol_field: &serde_json::Value) -> Option<String> {
    match eol_field {
        serde_json::Value::String(date) => Some(date.clone()),
        _ => None,
    }
}

/// Find the matching cycle for a version string.
fn match_cycle<'a>(
    version: &str,
    cycles: &'a [serde_json::Value],
) -> Option<&'a serde_json::Value> {
    let normalized = normalize_version(version);
    if normalized.is_empty() {
        return None;
    }

    // Try exact match first, then prefix match on major version
    let major = normalized.split('.').next().unwrap_or(&normalized);

    // Helper: extract cycle field as string (handles both String and Number)
    let cycle_value = |c: &serde_json::Value| -> Option<String> {
        match c.get("cycle")? {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    };

    // Exact match on normalized version
    for cycle in cycles {
        if let Some(cv) = cycle_value(cycle) {
            if cv == normalized {
                return Some(cycle);
            }
        }
    }

    // Fallback: match on major only
    for cycle in cycles {
        if let Some(cv) = cycle_value(cycle) {
            if cv == major {
                return Some(cycle);
            }
        }
    }

    None
}

/// Check a single product+version against the EOL database.
fn check_product_eol(
    name: &str,
    version: &str,
    cache_dir: &Path,
    today: &str,
) -> Option<EolWarning> {
    let slug = product_slug(name)?;
    let cycles = fetch_product_cycles(slug, cache_dir, today);
    if cycles.is_empty() {
        return None;
    }

    let cycle = match_cycle(version, &cycles)?;
    let eol_field = cycle.get("eol")?;

    if is_eol(eol_field, today) {
        let display = product_display_name(slug);
        let matched_version = cycle
            .get("cycle")
            .and_then(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .unwrap_or_else(|| normalize_version(version));

        Some(EolWarning {
            product: display.to_string(),
            version: matched_version,
            eol_date: eol_date_str(eol_field),
        })
    } else {
        None
    }
}

/// Check all detected language + framework versions for EOL status.
pub fn check_service_eol(tech: &TechStack, cache_dir: &Path, today: &str) -> Vec<EolWarning> {
    let mut warnings = Vec::new();

    // Check primary language version
    if let Some(lv) = &tech.language_version {
        // language_version is like "Ruby 3.1.5" or "3.1.5"
        let (lang, ver) = if let Some(idx) = lv.find(char::is_whitespace) {
            (lv[..idx].to_string(), lv[idx..].trim().to_string())
        } else {
            (tech.primary_language.clone(), lv.clone())
        };

        if let Some(w) = check_product_eol(&lang, &ver, cache_dir, today) {
            warnings.push(w);
        }
    }

    // Check framework versions
    for (framework, version) in &tech.framework_versions {
        if let Some(w) = check_product_eol(framework, version, cache_dir, today) {
            warnings.push(w);
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_version() {
        assert_eq!(normalize_version("3.1.5"), "3.1");
        assert_eq!(normalize_version(">=18"), "18");
        assert_eq!(normalize_version("~20.0.1"), "20.0");
        assert_eq!(normalize_version("^3.2"), "3.2");
        assert_eq!(normalize_version("3.x"), "3");
        assert_eq!(normalize_version("18"), "18");
    }

    #[test]
    fn test_is_eol_bool() {
        assert!(is_eol(&serde_json::Value::Bool(true), "2026-01-01"));
        assert!(!is_eol(&serde_json::Value::Bool(false), "2026-01-01"));
    }

    #[test]
    fn test_is_eol_date() {
        let past = serde_json::Value::String("2025-01-01".into());
        let future = serde_json::Value::String("2099-01-01".into());
        assert!(is_eol(&past, "2026-03-11"));
        assert!(!is_eol(&future, "2026-03-11"));
    }

    #[test]
    fn test_match_cycle() {
        let cycles: Vec<serde_json::Value> = serde_json::from_str(
            r#"[
                {"cycle": "3.3", "eol": "2027-03-31"},
                {"cycle": "3.2", "eol": "2026-03-31"},
                {"cycle": "3.1", "eol": "2025-03-31"},
                {"cycle": "3.0", "eol": "2024-03-31"}
            ]"#,
        )
        .unwrap();

        let matched = match_cycle("3.1.5", &cycles).unwrap();
        assert_eq!(matched.get("cycle").unwrap().as_str().unwrap(), "3.1");

        let matched = match_cycle("3.3", &cycles).unwrap();
        assert_eq!(matched.get("cycle").unwrap().as_str().unwrap(), "3.3");

        assert!(match_cycle("4.0", &cycles).is_none());
    }

    #[test]
    fn test_product_slug_mapping() {
        assert_eq!(product_slug("Ruby"), Some("ruby"));
        assert_eq!(product_slug("TypeScript"), Some("nodejs"));
        assert_eq!(product_slug("Next.js"), Some("nextjs"));
        assert_eq!(product_slug("Unknown"), None);
    }
}

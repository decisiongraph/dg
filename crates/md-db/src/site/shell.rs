//! Make the SPA HTML shell work when the site is hosted under any subpath.
//!
//! SvelteKit's `fallback: 'index.html'` page hard-codes `base: ""` and
//! root-absolute `/_app/...` asset URLs, so it only works at the domain root.
//! Since the hosting prefix isn't known at build time, each written shell
//! detects its base at runtime from `location.pathname`:
//!
//! - a per-route copy (e.g. `records/prc-001/index.html`) strips its own route
//!   suffix, which is exact regardless of trailing slash;
//! - otherwise (root shell, or served as SPA fallback for an unknown path) the
//!   path is cut before the first known top-level SPA route segment.
//!
//! The detected base is exposed as `globalThis.__dg_base`, fed into SvelteKit
//! (`$app/paths` `base`) and used to load `/_app/` assets.

use std::sync::LazyLock;

use regex::Regex;

/// Global JS variable holding the detected base path (`""` or `/some/prefix`).
const BASE_VAR: &str = "__dg_base";

static APP_LINK: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r#"[ \t]*<link href="(/_app/[^"]+)" rel="([a-z]+)">[ \t]*\r?\n?"#).ok()
});
static KIT_BASE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r#"(__sveltekit_\w+\s*=\s*\{\s*base:\s*)"""#).ok());

/// Rewrite the SvelteKit shell `template` for the page written at `route`
/// (`""` for the root `index.html`, e.g. `"records/prc-001"` otherwise).
/// `top_segments` are the first path segments of all SPA routes.
pub fn localize_shell(template: &str, route: &str, top_segments: &[String]) -> String {
    let (Some(app_link), Some(kit_base)) = (APP_LINK.as_ref(), KIT_BASE.as_ref()) else {
        return template.to_string();
    };
    // Insert where the first `/_app/` link was (after <meta charset>), else before </head>.
    let Some(at) = app_link
        .find(template)
        .map(|m| m.start())
        .or_else(|| template.find("</head>"))
    else {
        return template.to_string();
    };
    let (head, rest) = template.split_at(at);

    // Collect + drop static `/_app/` <link> tags; re-emit them under the base.
    let mut links: Vec<(String, String)> = Vec::new();
    let body = app_link.replace_all(rest, |caps: &regex::Captures| {
        links.push((caps[2].to_string(), caps[1].to_string()));
        String::new()
    });
    let body = kit_base.replace(&body, format!("${{1}}{BASE_VAR}"));
    let body = body.replace("import(\"/_app/", &format!("import({BASE_VAR} + \"/_app/"));

    let mut script = format!(
        "\t\t<script>var {BASE_VAR}=(function(r,k){{\
var p=location.pathname,d=/(\\/|\\/index\\.html)$/.test(p);\
p=p.replace(/\\/index\\.html$/,'').replace(/\\/+$/,'');\
var l=p.toLowerCase(),s='/'+r;\
if(r&&(l===s||l.slice(-s.length)===s))return p.slice(0,p.length-s.length);\
if(!r&&d)return p;\
var g=p.split('/');\
for(var i=1;i<g.length;i++){{var x=g[i].toLowerCase();try{{x=decodeURIComponent(x)}}catch(e){{}}if(k.indexOf(x)>=0)return g.slice(0,i).join('/')}}\
return p}})({route},{segments});",
        route = js(&route.to_lowercase()),
        segments = js(top_segments),
    );
    if !links.is_empty() {
        script.push_str("\n(function(b){var h='';");
        for (rel, href) in &links {
            script.push_str(&format!(
                "h+='<link href=\"'+b+{}+'\" rel={}>';",
                js(href),
                js(rel)
            ));
        }
        script.push_str(&format!("document.write(h)}})({BASE_VAR});"));
    }
    script.push_str("</script>\n");

    format!("{head}{script}{body}")
}

/// JSON-encode `v` as a JS literal that is safe inside an inline `<script>`.
fn js<T: serde::Serialize + ?Sized>(v: &T) -> String {
    serde_json::to_string(v)
        .unwrap_or_else(|_| "null".into())
        .replace('<', "\\u003c")
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = r#"<!doctype html>
<html lang="en">
	<head>
		<meta charset="utf-8" />
		<link href="/_app/immutable/entry/start.abc.js" rel="modulepreload">
		<link href="/_app/immutable/assets/0.def.css" rel="stylesheet">
	</head>
	<body>
			<script>
				{
					__sveltekit_1le073x = {
						base: ""
					};
					Promise.all([
						import("/_app/immutable/entry/start.abc.js"),
						import("/_app/immutable/entry/app.xyz.js")
					]);
				}
			</script>
	</body>
</html>"#;

    #[test]
    fn rewrites_assets_and_base() {
        let out = localize_shell(TEMPLATE, "records/prc-001", &["records".into()]);
        assert!(!out.contains(r#"<link href="/_app/"#), "{out}");
        assert!(!out.contains(r#"import("/_app/"#), "{out}");
        assert!(!out.contains(r#"base: """#), "{out}");
        assert!(out.contains("base: __dg_base"));
        assert!(out.contains(r#"import(__dg_base + "/_app/immutable/entry/app.xyz.js")"#));
        assert!(out.contains(r#""records/prc-001""#));
        assert!(out.contains(r#"["records"]"#));
        assert!(out.contains(r#""/_app/immutable/assets/0.def.css""#));
        // Base detection replaces the asset links: after <meta charset>, inside <head>
        let script = out.find("<script>var __dg_base=").unwrap();
        assert!(out.find("<meta charset").unwrap() < script);
        assert!(script < out.find("</head>").unwrap());
    }

    #[test]
    fn leaves_unknown_template_alone() {
        let html = "<html><body>no head</body></html>";
        assert_eq!(localize_shell(html, "", &[]), html);
    }
}

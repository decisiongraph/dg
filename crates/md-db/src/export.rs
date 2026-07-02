use std::collections::BTreeMap;
use std::path::Path;

use comrak::{Arena, Options};
use regex::Regex;

use crate::document::Document;
use crate::graph::{path_to_id, DocGraph};
use crate::schema::Schema;

/// Build a JSON object mapping forward relation names to their inverses.
/// e.g. `{"enables":"enabled_by","supersedes":"superseded_by","triggers":"triggered_by"}`
fn build_inverses_json(schema: &Schema) -> String {
    let mut out = String::from("{");
    let mut first = true;
    for r in &schema.relations {
        if let Some(ref inv) = r.inverse {
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str(&format!(
                "\"{}\":\"{}\"",
                encode_json_str(&r.name),
                encode_json_str(inv),
            ));
        }
    }
    out.push('}');
    out
}

/// Encode a string for safe use in HTML double-quoted attributes (href, class, etc.).
/// Uses encode_minimal which escapes &, <, >, ", and ' — sufficient for attribute values
/// inside double quotes. We don't use encode_attribute because it hex-encodes `-` and `.`
/// which breaks URLs.
pub(crate) fn encode_attr(s: &str) -> String {
    htmlescape::encode_minimal(s)
}

/// Encode a string for safe use in HTML text content.
pub(crate) fn encode_text(s: &str) -> String {
    htmlescape::encode_minimal(s)
}

/// Encode a string for safe use in JSON string values (embedded in HTML).
fn encode_json_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
}

/// Render a Document's markdown body to HTML using comrak.
/// Raw HTML blocks in markdown are stripped (r#unsafe = false) to prevent XSS.
pub fn render_markdown_to_html(body: &str) -> String {
    // Insert hard line break (backslash) before └► so it renders on its own line
    let body = body.replace("\n└►", "  \n└►");
    let arena = Arena::new();
    let mut opts = Options::default();
    opts.extension.table = true;
    opts.extension.strikethrough = true;
    opts.extension.autolink = true;
    opts.extension.footnotes = true;
    opts.extension.alerts = true;
    opts.render.r#unsafe = false;
    let root = comrak::parse_document(&arena, &body, &opts);
    let mut html = String::new();
    let _ = comrak::format_html(root, &opts, &mut html);
    html
}

/// Build a frontmatter metadata grid (key/value pills, not a table).
/// Skips title/status/type since they're shown in the header.
/// Lists are rendered as individual pills; brackets are stripped.
pub(crate) fn frontmatter_meta(doc: &Document, known_ids: &[String]) -> String {
    let fm = match &doc.frontmatter {
        Some(fm) => fm,
        None => return String::new(),
    };

    let mut html = String::from("<div class=\"meta-grid\">\n");
    for (key, val) in fm.data() {
        if matches!(key.as_str(), "title" | "status" | "type") {
            continue;
        }
        let display = crate::frontmatter::yaml_value_to_string(val);
        // Strip outer brackets/quotes for cleaner display
        let clean = display
            .trim_matches(|c| c == '[' || c == ']' || c == '"' || c == '\'')
            .trim();

        // Split by comma to detect lists
        let parts: Vec<&str> = clean
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let value_html = if parts.len() > 1 {
            // Render each part as a pill
            let mut pills = String::new();
            for part in &parts {
                let encoded = htmlescape::encode_minimal(part);
                let linked = linkify_refs(&encoded, known_ids);
                pills.push_str(&format!("<span class=\"meta-pill\">{}</span>", linked));
            }
            pills
        } else {
            // Single value — linkify directly
            linkify_refs(&htmlescape::encode_minimal(clean), known_ids)
        };

        html.push_str(&format!(
            "<div class=\"meta-item\"><span class=\"meta-label\">{}</span><span class=\"meta-value\">{}</span></div>\n",
            htmlescape::encode_minimal(key),
            value_html,
        ));
    }
    html.push_str("</div>\n");
    html
}

/// Convert cross-document refs (e.g. ADR-001) in HTML to clickable links.
pub(crate) fn linkify_refs(html: &str, known_ids: &[String]) -> String {
    if known_ids.is_empty() {
        return html.to_string();
    }
    // Build pattern like (ADR-001|OPP-002|...)
    let escaped: Vec<String> = known_ids.iter().map(|id| regex::escape(id)).collect();
    let pattern = format!(r"\b({})\b", escaped.join("|"));
    let re = match Regex::new(&pattern) {
        Ok(re) => re,
        Err(_) => return html.to_string(), // pattern too large or invalid
    };

    re.replace_all(html, |caps: &regex::Captures| {
        let id = &caps[0];
        let lower = id.to_lowercase();
        format!(
            "<a href=\"{}\">{}</a>",
            encode_attr(&format!("{lower}.html")),
            encode_text(id),
        )
    })
    .to_string()
}

/// CSS for document pages — clean, Linear/Vercel-inspired design.
pub(crate) const CSS: &str = r#"
:root {
  --font-sans: "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  --c-bg: #ffffff; --c-bg-subtle: #f9fafb;
  --c-text: #1e293b; --c-text-secondary: #64748b; --c-text-tertiary: #94a3b8;
  --c-border: #e2e8f0; --c-border-hover: #cbd5e1;
  --c-primary: #2563eb;
  --shadow-sm: 0 1px 2px 0 rgb(0 0 0 / 0.05);
  --shadow-md: 0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1);
}
*, *:before, *:after { box-sizing: border-box; }
body { font-family: var(--font-sans); color: var(--c-text); background: var(--c-bg); line-height: 1.6; margin: 0; -webkit-font-smoothing: antialiased; }

/* Sticky top bar */
.site-header { position: sticky; top: 0; background: rgba(255,255,255,0.9); backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px); border-bottom: 1px solid var(--c-border); padding: 1rem 1.5rem; z-index: 100; }
.breadcrumbs { display: flex; align-items: center; gap: 0.5rem; font-size: 0.875rem; color: var(--c-text-tertiary); max-width: 48rem; margin: 0 auto; }
.breadcrumbs a { color: inherit; text-decoration: none; transition: color 0.15s; }
.breadcrumbs a:hover { color: var(--c-text); }
.breadcrumbs .sep { color: var(--c-border); font-weight: 300; }
.breadcrumbs .current { color: var(--c-text); font-weight: 600; }

/* Layout */
.container { max-width: 48rem; margin: 0 auto; padding: 3rem 1.5rem 6rem; }

/* Doc header */
.doc-header { margin-bottom: 3rem; }
h1 { font-size: 2.25rem; font-weight: 700; letter-spacing: -0.03em; margin: 0 0 1.5rem 0; line-height: 1.1; color: var(--c-text); }

/* Status badges with dot indicator */
.status-badge { display: inline-flex; align-items: center; gap: 0.375rem; padding: 0.25rem 0.625rem 0.25rem 0.5rem; border-radius: 9999px; font-size: 0.75rem; font-weight: 500; text-transform: capitalize; line-height: 1; margin-bottom: 1rem; border: 1px solid transparent; }
.status-badge::before { content: ""; display: block; width: 0.375rem; height: 0.375rem; border-radius: 50%; }
.status-proposed, .status-draft, .status-identified { background: #f1f5f9; color: #475569; border-color: #e2e8f0; }
.status-proposed::before, .status-draft::before, .status-identified::before { background: #94a3b8; }
.status-validating, .status-pursuing, .status-investigating { background: #fffbeb; color: #b45309; border-color: #fcd34d; }
.status-validating::before, .status-pursuing::before, .status-investigating::before { background: #f59e0b; }
.status-accepted, .status-active, .status-completed, .status-resolved, .status-mitigated { background: #f0fdf4; color: #15803d; border-color: #bbf7d0; }
.status-accepted::before, .status-active::before, .status-completed::before, .status-resolved::before, .status-mitigated::before { background: #22c55e; }
.status-open, .status-rejected, .status-declined { background: #fef2f2; color: #b91c1c; border-color: #fecaca; }
.status-open::before, .status-rejected::before, .status-declined::before { background: #ef4444; }
.status-deprecated, .status-superseded { background: #f8fafc; color: #64748b; border-color: #e2e8f0; text-decoration: line-through; opacity: 0.8; }
.status-deprecated::before, .status-superseded::before { background: #cbd5e1; }

/* Metadata grid */
.meta-grid { display: flex; flex-wrap: wrap; gap: 2rem 3rem; padding-top: 1.5rem; border-top: 1px solid var(--c-border); }
.meta-item { display: flex; flex-direction: column; gap: 0.25rem; }
.meta-label { font-size: 0.75rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em; color: var(--c-text-tertiary); }
.meta-value { font-size: 0.9375rem; font-weight: 500; color: var(--c-text); display: flex; flex-wrap: wrap; gap: 0.375rem; align-items: center; }
.meta-value a { color: var(--c-text); font-weight: 600; text-decoration: underline; text-decoration-color: var(--c-border); text-underline-offset: 3px; text-decoration-thickness: 1px; transition: text-decoration-color 0.15s; }
.meta-value a:hover { text-decoration-color: var(--c-text); }
.meta-pill { display: inline-flex; padding: 0.125rem 0.5rem; background: var(--c-bg-subtle); border: 1px solid var(--c-border); border-radius: 4px; font-size: 0.8125rem; color: var(--c-text-secondary); text-decoration: none !important; transition: all 0.1s; }
.meta-pill:hover { border-color: var(--c-text-tertiary); color: var(--c-text); }

/* Prose body */
.prose { max-width: 100%; margin-bottom: 4rem; }
.prose h2 { font-size: 1.5rem; margin: 2.5rem 0 1rem; font-weight: 600; letter-spacing: -0.02em; color: var(--c-text); }
.prose h3 { font-size: 1.25rem; margin: 2rem 0 0.75rem; font-weight: 600; color: var(--c-text); }
.prose p { margin: 0 0 1.25rem; color: #334155; font-size: 1.0625rem; line-height: 1.75; }
.prose ul, .prose ol { padding-left: 1.5rem; margin: 0 0 1.5rem; color: #334155; }
.prose li { margin-bottom: 0.5rem; }
.prose strong { font-weight: 600; color: var(--c-text); }
.prose code { font-family: var(--font-mono); font-size: 0.875em; background: var(--c-bg-subtle); padding: 0.125rem 0.375rem; border-radius: 4px; border: 1px solid var(--c-border); }
.prose pre { background: #0f172a; padding: 1.25rem; border-radius: 8px; overflow-x: auto; margin: 1.5rem 0; box-shadow: var(--shadow-sm); }
.prose pre code { background: transparent; border: none; color: #f8fafc; padding: 0; font-size: 0.85rem; line-height: 1.7; }
.prose blockquote { border-left: 3px solid var(--c-border); padding-left: 1rem; font-style: italic; color: var(--c-text-secondary); margin: 1.5rem 0; }
.prose .markdown-alert { border-left: 4px solid; padding: 0.75rem 1rem; margin: 1.5rem 0; border-radius: 0 6px 6px 0; font-style: normal; color: var(--c-text); }
.prose .markdown-alert > :first-child { margin-top: 0; }
.prose .markdown-alert > :last-child { margin-bottom: 0; }
.prose .markdown-alert-title { font-weight: 600; display: flex; align-items: center; gap: 0.4rem; margin-bottom: 0.4rem; font-size: 0.9375rem; }
.prose .markdown-alert-title svg { width: 1em; height: 1em; flex-shrink: 0; }
.prose .markdown-alert-note { border-color: #2f81f7; background: #2f81f714; }
.prose .markdown-alert-note .markdown-alert-title { color: #2f81f7; }
.prose .markdown-alert-tip { border-color: #3fb950; background: #3fb95014; }
.prose .markdown-alert-tip .markdown-alert-title { color: #3fb950; }
.prose .markdown-alert-important { border-color: #a371f7; background: #a371f714; }
.prose .markdown-alert-important .markdown-alert-title { color: #a371f7; }
.prose .markdown-alert-warning { border-color: #d29922; background: #d2992214; }
.prose .markdown-alert-warning .markdown-alert-title { color: #d29922; }
.prose .markdown-alert-caution { border-color: #f85149; background: #f8514914; }
.prose .markdown-alert-caution .markdown-alert-title { color: #f85149; }
.prose a { color: var(--c-primary); text-decoration: none; font-weight: 500; border-bottom: 1px solid transparent; transition: border 0.1s; }
.prose a:hover { border-bottom-color: var(--c-primary); }
.prose table { border-collapse: collapse; width: 100%; margin: 2rem 0; font-size: 0.95rem; }
.prose th { background: var(--c-bg-subtle); font-weight: 600; font-size: 0.875rem; color: var(--c-text-secondary); text-align: left; padding: 0.75rem; border-bottom: 1px solid var(--c-border); }
.prose td { padding: 0.75rem; border-bottom: 1px solid var(--c-border); color: #334155; }
.prose tr:hover td { background: var(--c-bg-subtle); }

/* Backlinks */
.backlinks-section { margin-top: 4rem; padding-top: 2rem; border-top: 1px solid var(--c-border); }
.backlinks-title { font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--c-text-tertiary); font-weight: 600; margin: 0 0 1.5rem 0; }
.backlinks-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 1rem; }
.backlink-card { display: flex; flex-direction: column; gap: 0.25rem; text-decoration: none; background: #fff; border: 1px solid var(--c-border); border-radius: 8px; padding: 1rem; transition: all 0.15s ease; box-shadow: var(--shadow-sm); }
.backlink-card:hover { border-color: var(--c-border-hover); transform: translateY(-1px); box-shadow: var(--shadow-md); }
.backlink-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.25rem; }
.backlink-id { font-family: var(--font-mono); font-size: 0.75rem; font-weight: 600; color: var(--c-text-secondary); }
.backlink-rel { font-size: 0.65rem; font-weight: 600; text-transform: uppercase; letter-spacing: 0.03em; background: var(--c-bg-subtle); padding: 0.125rem 0.375rem; border-radius: 99px; color: var(--c-text-tertiary); border: 1px solid var(--c-border); }
.backlink-title { font-size: 0.9375rem; font-weight: 500; color: var(--c-text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

/* Diagrams */
.mermaid { margin: 1.5rem 0; text-align: center; }
.mermaid svg { max-width: 100%; height: auto; }
.diagram { margin: 1.5rem 0; text-align: center; background: var(--c-bg-subtle); border-radius: 8px; padding: 1.5rem; border: 1px solid var(--c-border); }
.diagram svg { max-width: 100%; height: auto; }
.diagram-loading { color: var(--c-text-tertiary); font-size: 0.875rem; padding: 2rem; }
"#;

/// Scripts for rendering Mermaid diagrams, D2 diagrams (via kroki.io), and MathJax.
pub(crate) const DIAGRAM_SCRIPTS: &str = r##"
<script>
MathJax = {
  tex: {
    inlineMath: [['$','$'], ['\\(','\\)']],
    displayMath: [['$$','$$'], ['\\[','\\]']],
    processEscapes: true
  },
  options: { skipHtmlTags: ['code','pre','script','style','textarea'] }
};
</script>
<script async src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js"></script>
<script type="module">
import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.esm.min.mjs';
mermaid.initialize({ startOnLoad: false, theme: 'neutral', fontFamily: 'system-ui, sans-serif' });

// Convert mermaid code blocks to renderable divs
var blocks = document.querySelectorAll('pre code.language-mermaid');
for (var i = 0; i < blocks.length; i++) {
  var el = blocks[i];
  var pre = el.parentElement;
  var div = document.createElement('div');
  div.className = 'mermaid';
  div.textContent = el.textContent;
  pre.replaceWith(div);
}
if (blocks.length > 0) await mermaid.run();

// Render D2 diagrams via kroki.io
var d2 = document.querySelectorAll('pre code.language-d2');
for (var j = 0; j < d2.length; j++) {
  (function(el) {
    var pre = el.parentElement;
    var loading = document.createElement('div');
    loading.className = 'diagram diagram-loading';
    loading.textContent = 'Rendering diagram\u2026';
    pre.replaceWith(loading);
    fetch('https://kroki.io/d2/svg', {
      method: 'POST',
      headers: { 'Content-Type': 'text/plain' },
      body: el.textContent
    }).then(function(r) {
      if (!r.ok) throw new Error(r.status);
      return r.text();
    }).then(function(svg) {
      loading.className = 'diagram';
      loading.textContent = '';
      loading.innerHTML = svg;
      var s = loading.querySelector('svg');
      if (s) {
        s.style.maxWidth = '100%'; s.style.height = 'auto';
        // D2's outer SVG clips inner content that uses negative viewBox offsets
        s.style.overflow = 'visible';
      }
    }).catch(function() {
      // Fallback: restore original code block
      loading.replaceWith(pre);
    });
  })(d2[j]);
}
</script>
"##;

/// Standalone CSS for the index page — matches document page design system.
pub(crate) const INDEX_CSS: &str = r#"
:root {
  --font-sans: "Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  --font-mono: ui-monospace, SFMono-Regular, Menlo, monospace;
  --c-bg: #f8fafc; --c-text: #0f172a; --c-text-secondary: #64748b; --c-border: #e2e8f0; --c-primary: #2563eb;
}
*, *:before, *:after { box-sizing: border-box; }
body { font-family: var(--font-sans); color: var(--c-text); background: var(--c-bg); line-height: 1.6; margin: 0; -webkit-font-smoothing: antialiased; }
.index-header { background: rgba(255,255,255,0.8); backdrop-filter: blur(12px); -webkit-backdrop-filter: blur(12px); border-bottom: 1px solid var(--c-border); padding: 1rem 2rem; display: flex; justify-content: space-between; align-items: center; position: sticky; top: 0; z-index: 100; }
.index-header h1 { font-size: 1.125rem; font-weight: 700; margin: 0; letter-spacing: -0.01em; color: var(--c-text); }
.index-header .doc-count { font-weight: 400; color: var(--c-text-secondary); font-size: 0.875rem; margin-left: 0.75rem; background: #f1f5f9; padding: 0.1rem 0.5rem; border-radius: 99px; }
.index-header nav { display: flex; gap: 0.5rem; }
.index-header nav a { font-size: 0.875rem; font-weight: 500; color: var(--c-text-secondary); text-decoration: none; padding: 0.375rem 0.875rem; border-radius: 6px; transition: all 0.15s; }
.index-header nav a:hover { background: #f1f5f9; color: var(--c-text); }

/* Flat list fallback */
.type-header { font-size: 0.75rem; font-weight: 700; color: var(--c-text-secondary); text-transform: uppercase; letter-spacing: 0.05em; margin: 2rem 0 0.75rem; padding-left: 1rem; }
.type-header .count { display: inline-block; margin-left: 0.5rem; font-size: 0.7rem; color: #94a3b8; font-weight: 500; }
.doc-list { list-style: none; padding: 0; margin: 0 1rem 2rem; background: #fff; border: 1px solid var(--c-border); border-radius: 8px; box-shadow: 0 1px 2px rgba(0,0,0,0.05); overflow: hidden; }
.doc-list li:not(:last-child) { border-bottom: 1px solid var(--c-border); }
.doc-row { display: flex; align-items: center; padding: 0.75rem 1rem; text-decoration: none; color: inherit; transition: background 0.1s; }
.doc-row:hover { background: #f8fafc; }
.doc-row:hover .doc-title { color: var(--c-primary); }
.doc-id { flex-shrink: 0; width: 5rem; font-family: var(--font-mono); font-size: 0.75rem; color: var(--c-text-secondary); font-weight: 500; }
.doc-title { font-weight: 500; color: var(--c-text); font-size: 0.9375rem; transition: color 0.1s; }
"#;

/// CSS for the dependency graph view on the index page.
/// Cards have solid background + high z-index so arrows can pass behind them.
pub(crate) const GRAPH_CSS: &str = r#"
.graph-container { position: relative; margin: 3rem 0; isolation: isolate; }
.graph-arrows { position: absolute; top: 0; left: 0; width: 100%; height: 100%; z-index: 0; pointer-events: none; }
.graph-grid { position: relative; z-index: 10; display: flex; flex-direction: column; gap: 3rem; padding-left: 7rem; max-width: 64rem; margin: 0 auto; }
.timeline-quarter { position: relative; font-size: 0.85rem; font-weight: 700; color: #64748b; letter-spacing: 0.03em; margin-left: -7rem; padding: 0.5rem 0; border-bottom: 1px solid #e2e8f0; margin-top: 1rem; width: calc(100% + 7rem); }
.graph-rank { display: flex; flex-wrap: wrap; gap: 2rem; width: 100%; }

/* Card design — left color accent per type */
.graph-card { width: 17rem; display: flex; flex-direction: column; border: 1px solid #e2e8f0; border-radius: 8px; background: #fff; border-left: 3px solid #94a3b8; transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1); position: relative; z-index: 20; }
.graph-card:hover { transform: translateY(-2px); box-shadow: 0 8px 16px -4px rgba(0,0,0,0.1), 0 2px 4px -2px rgba(0,0,0,0.06); border-color: #cbd5e1; z-index: 50; }
.graph-card.adr { border-left-color: #3b82f6; }
.graph-card.pol { border-left-color: #8b5cf6; }
.graph-card.opp { border-left-color: #10b981; }
.graph-card.inc { border-left-color: #ef4444; }

.graph-container.has-hover .graph-card { opacity: 0.2; filter: grayscale(0.8); transition: opacity 0.2s; }
.graph-container.has-hover .graph-card.is-active,
.graph-container.has-hover .graph-card.is-related { opacity: 1; filter: none; }
.graph-container.has-hover .edge-group { opacity: 0.05; }
.graph-container.has-hover .edge-group.is-active { opacity: 1; }
.graph-container.has-hover .edge-group.is-active path { stroke: #475569; stroke-width: 2.5; }

.card-link { display: flex; flex-direction: column; height: 100%; padding: 1rem; text-decoration: none; color: inherit; }
.card-top { display: flex; justify-content: space-between; align-items: center; margin-bottom: 0.5rem; }
.card-id { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 0.7rem; font-weight: 600; color: #64748b; }

/* Status dot in card */
.card-status { display: flex; align-items: center; gap: 0.25rem; font-size: 0.65rem; font-weight: 600; text-transform: uppercase; color: #64748b; background: #f1f5f9; padding: 0.15rem 0.4rem; border-radius: 99px; }
.card-status::before { content: ""; display: block; width: 0.35rem; height: 0.35rem; border-radius: 50%; background: #cbd5e1; }
.card-status.status-accepted::before, .card-status.status-active::before, .card-status.status-completed::before, .card-status.status-resolved::before, .card-status.status-mitigated::before { background: #22c55e; }
.card-status.status-accepted, .card-status.status-active, .card-status.status-completed, .card-status.status-resolved, .card-status.status-mitigated { background: #dcfce7; color: #15803d; }
.card-status.status-validating::before, .card-status.status-pursuing::before, .card-status.status-investigating::before { background: #f59e0b; }
.card-status.status-validating, .card-status.status-pursuing, .card-status.status-investigating { background: #fef3c7; color: #b45309; }
.card-status.status-open::before, .card-status.status-rejected::before, .card-status.status-declined::before { background: #ef4444; }
.card-status.status-open, .card-status.status-rejected, .card-status.status-declined { background: #fee2e2; color: #b91c1c; }

.graph-card.is-deprecated { opacity: 0.6; filter: grayscale(0.8); }
.graph-container.has-hover .graph-card.is-deprecated.is-related { opacity: 0.75; }

.card-title { font-size: 0.95rem; font-weight: 600; color: #1e293b; line-height: 1.4; display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }

.arrow-label { font-size: 0.7rem; fill: #64748b; font-family: system-ui; font-weight: 600; text-anchor: middle; paint-order: stroke; stroke: #f8fafc; stroke-width: 4px; pointer-events: none; }
.timeline-quarter.other-docs { margin-top: 2rem; border-top: 1px dashed #cbd5e1; border-bottom: 1px solid #e2e8f0; }
"#;

/// JavaScript for dependency graph arrows with hover highlighting.
/// Uses distributed port allocation on both source and target cards to prevent
/// arrow overlapping. Sorts edges by target/source X to minimize crossings.
pub(crate) const GRAPH_JS: &str = r##"
document.addEventListener('DOMContentLoaded', function() {
  var container = document.querySelector('.graph-container');
  var svg = document.getElementById('graph-arrows');
  var edgeEl = document.getElementById('graph-edges');
  if (!svg || !edgeEl) return;
  var rawEdges = [];
  try { rawEdges = JSON.parse(edgeEl.textContent); } catch(e) {}
  if (!rawEdges.length) return;

  // --- Load inverse-pair lookup from schema ---
  var inverses = {};
  try { inverses = JSON.parse(document.getElementById('graph-inverses').textContent); } catch(x) {}
  var inverseLookup = {};
  for (var ik in inverses) { inverseLookup[ik] = inverses[ik]; inverseLookup[inverses[ik]] = ik; }

  // --- Merge bidirectional edges (A->B + B->A) ---
  var edgeMap = {};
  for (var i = 0; i < rawEdges.length; i++) {
    var e = rawEdges[i];
    var key = e.from < e.to ? e.from + '::' + e.to : e.to + '::' + e.from;
    if (!edgeMap[key]) {
      edgeMap[key] = {from: e.from, to: e.to, label: e.label, bidir: false};
    } else {
      edgeMap[key].bidir = true;
      if (edgeMap[key].label !== e.label) {
        if (inverseLookup[edgeMap[key].label] === e.label) {
          if (inverses[e.label]) edgeMap[key].label = e.label;
        } else {
          edgeMap[key].label = 'related';
        }
      }
    }
  }
  var edges = [];
  for (var k in edgeMap) edges.push(edgeMap[k]);

  var GUTTER_STEP = 12;

  function bezierXY(t, sx, sy, c1x, c1y, c2x, c2y, ex, ey) {
    var u = 1 - t;
    return {
      x: u*u*u*sx + 3*u*u*t*c1x + 3*u*t*t*c2x + t*t*t*ex,
      y: u*u*u*sy + 3*u*u*t*c1y + 3*u*t*t*c2y + t*t*t*ey
    };
  }

  function getCards() {
    var map = {};
    var cRect = container.getBoundingClientRect();
    var els = container.querySelectorAll('.graph-card');
    for (var c = 0; c < els.length; c++) {
      var el = els[c];
      var id = el.id.replace('card-', '').toUpperCase();
      var r = el.getBoundingClientRect();
      map[id] = {
        el: el, id: id,
        x: r.left - cRect.left, y: r.top - cRect.top,
        w: r.width, h: r.height,
        cx: r.left - cRect.left + r.width / 2,
        cy: r.top - cRect.top + r.height / 2
      };
    }
    return map;
  }

  // --- Hover highlighting ---
  var hoverBound = false;
  function bindHover(cards) {
    if (hoverBound) return;
    hoverBound = true;
    for (var id in cards) {
      (function(nodeId) {
        cards[nodeId].el.addEventListener('mouseenter', function() { highlight(nodeId, true); });
        cards[nodeId].el.addEventListener('mouseleave', function() { highlight(nodeId, false); });
      })(id);
    }
  }

  function highlight(id, active) {
    if (active) container.classList.add('has-hover');
    else container.classList.remove('has-hover');
    var actives = container.querySelectorAll('.is-active, .is-related');
    for (var j = 0; j < actives.length; j++) {
      actives[j].classList.remove('is-active', 'is-related');
    }
    if (!active) return;
    var cardEl = document.getElementById('card-' + id.toLowerCase());
    if (cardEl) cardEl.classList.add('is-active');
    var neighbors = {};
    for (var i = 0; i < edges.length; i++) {
      var e = edges[i];
      if (e.from === id || e.to === id) {
        var g = document.getElementById('edge-' + i);
        if (g) g.classList.add('is-active');
        neighbors[e.from] = 1; neighbors[e.to] = 1;
      }
    }
    for (var nid in neighbors) {
      var nel = document.getElementById('card-' + nid.toLowerCase());
      if (nel) nel.classList.add('is-related');
    }
  }

  // --- Port assignment: distribute start/end points across card edges ---
  function assignPorts(edges, cards) {
    var outEdges = {}, inEdges = {};
    for (var id in cards) { outEdges[id] = []; inEdges[id] = []; }
    for (var i = 0; i < edges.length; i++) {
      var e = edges[i];
      if (outEdges[e.from]) outEdges[e.from].push(e);
      if (inEdges[e.to]) inEdges[e.to].push(e);
    }
    // Sort outputs by target X position to uncross lines
    for (var id in outEdges) {
      outEdges[id].sort(function(a, b) {
        var cA = cards[a.to], cB = cards[b.to];
        return (cA ? cA.cx : 0) - (cB ? cB.cx : 0);
      });
      for (var i = 0; i < outEdges[id].length; i++) {
        outEdges[id][i].outIdx = i;
        outEdges[id][i].outTot = outEdges[id].length;
      }
    }
    // Sort inputs by source X position
    for (var id in inEdges) {
      inEdges[id].sort(function(a, b) {
        var cA = cards[a.from], cB = cards[b.from];
        return (cA ? cA.cx : 0) - (cB ? cB.cx : 0);
      });
      for (var i = 0; i < inEdges[id].length; i++) {
        inEdges[id][i].inIdx = i;
        inEdges[id][i].inTot = inEdges[id].length;
      }
    }
  }

  function draw() {
    var cards = getCards();
    assignPorts(edges, cards);
    bindHover(cards);
    var cRect = container.getBoundingClientRect();
    svg.setAttribute('width', cRect.width);
    svg.setAttribute('height', cRect.height);
    svg.innerHTML = '<defs>'
      + '<marker id="arrow" markerWidth="8" markerHeight="6" refX="7" refY="3" orient="auto">'
      + '<path d="M0,0 L8,3 L0,6" fill="#94a3b8"/></marker>'
      + '<marker id="arrow-rev" markerWidth="8" markerHeight="6" refX="1" refY="3" orient="auto">'
      + '<path d="M8,0 L0,3 L8,6" fill="#94a3b8"/></marker>'
      + '</defs>';

    var allCards = [];
    for (var cid in cards) allCards.push(cards[cid]);

    // Check if a direct path between two points overlaps any intermediate card
    function isBlocked(p1x, p1y, p2x, p2y, srcId, tgtId) {
      var PAD = 8;
      var top = Math.min(p1y, p2y);
      var bottom = Math.max(p1y, p2y);
      var left = Math.min(p1x, p2x) - PAD;
      var right = Math.max(p1x, p2x) + PAD;
      for (var ci = 0; ci < allCards.length; ci++) {
        var c = allCards[ci];
        if (c.id === srcId || c.id === tgtId) continue;
        if (c.y < bottom && (c.y + c.h) > top && c.x < right && (c.x + c.w) > left) return true;
      }
      return false;
    }

    // Lane tracking for gutter-routed edges
    var lanes = { left: [], right: [] };
    function allocLane(side, minY, maxY) {
      var active = lanes[side];
      var lane = 0;
      while (true) {
        var conflict = false;
        for (var i = 0; i < active.length; i++) {
          if (active[i].lane === lane && !(active[i].max < minY - 10 || active[i].min > maxY + 10)) {
            conflict = true; break;
          }
        }
        if (!conflict) break;
        lane++;
      }
      active.push({lane: lane, min: minY, max: maxY});
      return lane;
    }

    for (var i = 0; i < edges.length; i++) {
      var e = edges[i];
      var src = cards[e.from], tgt = cards[e.to];
      if (!src || !tgt) continue;

      // Calculate distributed port positions (spread across 80% of card width)
      var outW = src.w * 0.8, inW = tgt.w * 0.8;
      var outStep = outW / (e.outTot + 1);
      var inStep = inW / (e.inTot + 1);
      var startX = src.x + (src.w - outW) / 2 + outStep * (e.outIdx + 1);
      var endX = tgt.x + (tgt.w - inW) / 2 + inStep * (e.inIdx + 1);

      var isDown = tgt.y > src.y + 15;
      var isSame = Math.abs(src.cy - tgt.cy) < 20;
      var isUp = !isDown && !isSame;
      var blockedDown = isDown && isBlocked(startX, src.y + src.h, endX, tgt.y, e.from, e.to);
      var blockedUp = isUp && isBlocked(endX, tgt.y + tgt.h, startX, src.y, e.from, e.to);

      var d = '', labelX = 0, labelY = 0, labelAnchor = 'middle';

      if (isSame) {
        // --- Horizontal bezier ---
        var ltr = src.x < tgt.x;
        var sx = ltr ? src.x + src.w : src.x;
        var ex = ltr ? tgt.x : tgt.x + tgt.w;
        var sy = src.cy + (e.outIdx - (e.outTot - 1) / 2) * 5;
        var ey = tgt.cy + (e.inIdx - (e.inTot - 1) / 2) * 5;
        var cx1 = ltr ? sx + 30 : sx - 30;
        var cx2 = ltr ? ex - 30 : ex + 30;
        d = 'M' + sx + ',' + sy + ' C' + cx1 + ',' + sy + ' ' + cx2 + ',' + ey + ' ' + ex + ',' + ey;
        labelX = (sx + ex) / 2;
        labelY = Math.min(sy, ey) - 8;

      } else if (isDown && !blockedDown) {
        // --- Downstream bezier with distributed ports ---
        var y1 = src.y + src.h;
        var y2 = tgt.y;
        var tension = Math.max(Math.min((y2 - y1) / 2, 120), 40);
        d = 'M' + startX + ',' + y1 + ' C' + startX + ',' + (y1 + tension)
          + ' ' + endX + ',' + (y2 - tension) + ' ' + endX + ',' + y2;
        var mid = bezierXY(0.5, startX, y1, startX, y1 + tension, endX, y2 - tension, endX, y2);
        labelX = mid.x;
        labelY = mid.y;

      } else if (isUp && !blockedUp) {
        // --- Upstream bezier (direct, no gutter) ---
        var y1 = src.y;
        var y2 = tgt.y + tgt.h;
        var tension = Math.max(Math.min((y1 - y2) / 2, 120), 40);
        d = 'M' + startX + ',' + y1 + ' C' + startX + ',' + (y1 - tension)
          + ' ' + endX + ',' + (y2 + tension) + ' ' + endX + ',' + y2;
        var mid = bezierXY(0.5, startX, y1, startX, y1 - tension, endX, y2 + tension, endX, y2);
        labelX = mid.x;
        labelY = mid.y;

      } else {
        // --- Gutter routing (upstream or blocked downstream) ---
        // Pick side based on relative card positions (route away from center)
        var side = tgt.cx > src.cx ? 'right' : 'left';

        // Vertically offset side ports slightly per edge to prevent overlap
        var sy = src.cy + (e.outIdx - (e.outTot - 1) / 2) * 6;
        var ey = tgt.cy + (e.inIdx - (e.inTot - 1) / 2) * 6;
        var minY = Math.min(sy, ey);
        var maxY = Math.max(sy, ey);
        var lane = allocLane(side, minY, maxY);

        var railX = side === 'right'
          ? Math.max(src.x + src.w, tgt.x + tgt.w) + 20 + lane * GUTTER_STEP
          : Math.min(src.x, tgt.x) - 20 - lane * GUTTER_STEP;
        var sx = side === 'right' ? src.x + src.w : src.x;
        var ex = side === 'right' ? tgt.x + tgt.w : tgt.x;
        var R = 8;
        var dir = side === 'right' ? 1 : -1;
        var goDown = ey > sy;

        d = 'M' + sx + ',' + sy
          + ' L' + (railX - R * dir) + ',' + sy
          + ' Q' + railX + ',' + sy + ' ' + railX + ',' + (sy + (goDown ? R : -R))
          + ' L' + railX + ',' + (ey + (goDown ? -R : R))
          + ' Q' + railX + ',' + ey + ' ' + (railX - R * dir) + ',' + ey
          + ' L' + ex + ',' + ey;

        labelX = railX + (side === 'right' ? 6 : -6);
        labelY = (sy + ey) / 2;
        labelAnchor = side === 'right' ? 'start' : 'end';
      }

      var g = document.createElementNS('http://www.w3.org/2000/svg', 'g');
      g.setAttribute('id', 'edge-' + i);
      g.setAttribute('class', 'edge-group');
      var path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      path.setAttribute('d', d);
      path.setAttribute('fill', 'none');
      path.setAttribute('stroke', '#94a3b8');
      path.setAttribute('stroke-width', '2');
      path.setAttribute('stroke-linecap', 'round');
      if (e.bidir) {
        path.setAttribute('marker-start', 'url(#arrow-rev)');
        path.setAttribute('marker-end', 'url(#arrow)');
      } else {
        path.setAttribute('marker-end', 'url(#arrow)');
      }
      if (isUp || e.label === 'related') {
        path.setAttribute('stroke-dasharray', '4 3');
      }
      g.appendChild(path);

      if (e.label) {
        var txt = document.createElementNS('http://www.w3.org/2000/svg', 'text');
        txt.setAttribute('x', labelX);
        txt.setAttribute('y', labelY + 4);
        txt.setAttribute('class', 'arrow-label');
        txt.setAttribute('text-anchor', labelAnchor);
        txt.textContent = e.label;
        g.appendChild(txt);
      }
      svg.appendChild(g);
    }
  }

  setTimeout(draw, 50);
  var timer;
  window.addEventListener('resize', function() {
    clearTimeout(timer);
    timer = setTimeout(draw, 100);
  });
});
"##;

/// Export a single document to a full HTML page.
/// Backlinks are (id, relation, title) tuples.
pub fn export_html(
    doc: &Document,
    known_ids: &[String],
    backlinks: &[(String, String, String)],
) -> String {
    let title = doc
        .frontmatter
        .as_ref()
        .and_then(|fm| fm.get_display("title"))
        .or_else(|| doc.title())
        .unwrap_or_else(|| "Untitled".to_string());

    let status = doc
        .frontmatter
        .as_ref()
        .and_then(|fm| fm.get_display("status"));

    let doc_id = doc.path.as_ref().map(|p| path_to_id(p)).unwrap_or_default();
    let doc_type = doc_id.split('-').next().unwrap_or("DOC").to_uppercase();

    let fm_html = frontmatter_meta(doc, known_ids);
    let body_html = render_markdown_to_html(&doc.body);
    let body_linked = linkify_refs(&body_html, known_ids);

    let status_badge = status
        .as_ref()
        .map(|s| {
            let class = format!("status-{}", s.to_lowercase());
            format!(
                "<span class=\"status-badge {}\">{}</span>\n",
                encode_attr(&class),
                encode_text(s),
            )
        })
        .unwrap_or_default();

    let backlinks_html = if backlinks.is_empty() {
        String::new()
    } else {
        let mut bl = String::from(
            "<section class=\"backlinks-section\">\n<h2 class=\"backlinks-title\">Referenced by</h2>\n<div class=\"backlinks-grid\">\n",
        );
        for (ref_id, ref_relation, ref_title) in backlinks {
            let lower = ref_id.to_lowercase();
            bl.push_str(&format!(
                "<a href=\"{}.html\" class=\"backlink-card\">\
                 <div class=\"backlink-header\"><span class=\"backlink-id\">{}</span><span class=\"backlink-rel\">{}</span></div>\
                 <div class=\"backlink-title\">{}</div>\
                 </a>\n",
                encode_attr(&lower),
                encode_text(ref_id),
                encode_text(ref_relation),
                encode_text(ref_title),
            ));
        }
        bl.push_str("</div>\n</section>\n");
        bl
    };

    let encoded_title = encode_text(&title);
    let encoded_doc_id = encode_text(&doc_id);
    let encoded_type = encode_text(&doc_type);
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{encoded_doc_id} — {encoded_title}</title>
<style>{CSS}</style>
</head>
<body>
<header class="site-header">
<nav class="breadcrumbs">
<a href="index.html">Index</a>
<span class="sep">/</span>
<span>{encoded_type}</span>
<span class="sep">/</span>
<span class="current">{encoded_doc_id}</span>
</nav>
</header>
<div class="container">
<header class="doc-header">
{status_badge}<h1>{encoded_title}</h1>
{fm_html}
</header>
<article class="prose">
{body_linked}
</article>
{backlinks_html}
</div>
{DIAGRAM_SCRIPTS}
</body>
</html>
"#
    )
}

/// Capitalize first letter of a word (e.g. "other" -> "Other").
pub(crate) fn titlecase_word(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
    }
}

/// Type priority for left-to-right ordering within a rank row.
fn type_sort_key(doc_type: &str) -> u8 {
    match doc_type.to_lowercase().as_str() {
        "inc" => 0,
        "adr" => 1,
        "pol" => 2,
        "opp" => 3,
        _ => 4,
    }
}

/// Parse a YYYY-MM-DD date string to (year, month) for rank grouping.
fn parse_year_month(date: &str) -> Option<(i32, u32)> {
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() >= 2 {
        let year: i32 = parts[0].parse().ok()?;
        let month: u32 = parts[1].parse().ok()?;
        if (1..=12).contains(&month) {
            return Some((year, month));
        }
    }
    None
}

/// Convert (year, month) to months-since-epoch for rank calculation.
fn to_epoch_months(year: i32, month: u32) -> i64 {
    year as i64 * 12 + month as i64
}

/// Groups cards into rows based on dependencies within a quarter.
/// If card A has an edge to card B, A appears in an earlier row than B.
/// Uses longest-path layering (iterative relaxation).
fn layer_cards_topologically<'a>(
    cards: &[&'a GraphCard],
    edges: &[crate::graph::DocEdge],
) -> Vec<Vec<&'a GraphCard>> {
    if cards.is_empty() {
        return vec![];
    }

    let id_map: std::collections::HashMap<&str, usize> = cards
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), i))
        .collect();

    // Build adjacency for edges within this card subset.
    // When bidirectional edges exist (A->B and B->A), keep only the
    // direction where the source has a lower index (earlier date).
    let mut pair_set: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let mut adj: Vec<Vec<usize>> = vec![vec![]; cards.len()];
    for edge in edges {
        if let (Some(&u), Some(&v)) = (id_map.get(edge.from.as_str()), id_map.get(edge.to.as_str()))
        {
            if u != v {
                pair_set.insert((u, v));
            }
        }
    }
    for (u, v) in &pair_set {
        // If reverse also exists, keep only the lower→higher direction
        if pair_set.contains(&(*v, *u)) {
            if u < v {
                adj[*u].push(*v);
            }
        } else {
            adj[*u].push(*v);
        }
    }

    // Longest-path layering: rank[v] = max(rank[parents]) + 1
    let mut ranks = vec![0usize; cards.len()];
    let mut changed = true;
    let mut iter = 0;
    while changed && iter < cards.len() {
        changed = false;
        for u in 0..cards.len() {
            for &v in &adj[u] {
                if ranks[v] <= ranks[u] {
                    ranks[v] = ranks[u] + 1;
                    changed = true;
                }
            }
        }
        iter += 1;
    }

    // Group by rank (preserves input order within each row)
    let max_rank = *ranks.iter().max().unwrap_or(&0);
    let mut rows: Vec<Vec<&GraphCard>> = vec![Vec::new(); max_rank + 1];
    for (i, &r) in ranks.iter().enumerate() {
        rows[r].push(cards[i]);
    }
    rows.retain(|r| !r.is_empty());
    rows
}

/// A card in the graph view.
pub(crate) struct GraphCard {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) date: String,
    pub(crate) status: Option<String>,
    pub(crate) doc_type: String,
}

/// Export an index page listing all documents, with optional dependency graph.
pub fn export_index(
    docs: &[(String, &Document)],
    graph: Option<&DocGraph>,
    schema: Option<&Schema>,
) -> String {
    let total = docs.len();

    // Extract card info for each doc
    let cards: Vec<GraphCard> = docs
        .iter()
        .map(|(id, doc)| {
            let fm = doc.frontmatter.as_ref();
            GraphCard {
                id: id.clone(),
                title: fm
                    .and_then(|f| f.get_display("title"))
                    .or_else(|| doc.title())
                    .unwrap_or_else(|| id.clone()),
                date: fm.and_then(|f| f.get_display("date")).unwrap_or_default(),
                status: fm.and_then(|f| f.get_display("status")),
                doc_type: fm
                    .and_then(|f| f.get_display("type"))
                    .or_else(|| {
                        // Infer type from ID prefix (e.g. "ADR-001" → "adr")
                        id.split('-').next().map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "other".to_string())
                    .to_lowercase(),
            }
        })
        .collect();

    // Split into dated and undated
    let mut dated: Vec<(i64, &GraphCard)> = Vec::new();
    let mut undated: Vec<&GraphCard> = Vec::new();
    for card in &cards {
        if let Some((y, m)) = parse_year_month(&card.date) {
            dated.push((to_epoch_months(y, m), card));
        } else {
            undated.push(card);
        }
    }

    // If no graph or no dated docs, fall back to flat list
    if graph.is_none() || dated.is_empty() {
        return export_index_flat(docs);
    }
    let graph = graph.unwrap();

    // Sort dated cards by epoch_months, then by type priority
    dated.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| type_sort_key(&a.1.doc_type).cmp(&type_sort_key(&b.1.doc_type)))
    });

    // Group into ranks by (year, month)
    struct Rank<'a> {
        year: i32,
        month: u32,
        cards: Vec<&'a GraphCard>,
    }
    let mut ranks: Vec<Rank> = Vec::new();
    let mut current_epoch: Option<i64> = None;
    for (epoch, card) in &dated {
        if current_epoch != Some(*epoch) {
            let (y, m) = parse_year_month(&card.date).unwrap();
            ranks.push(Rank {
                year: y,
                month: m,
                cards: Vec::new(),
            });
            current_epoch = Some(*epoch);
        }
        ranks.last_mut().unwrap().cards.push(card);
    }

    // Build graph HTML with left-side timeline
    let mut body = String::from(
        "<div class=\"graph-container\">\n\
         <svg class=\"graph-arrows\" id=\"graph-arrows\"></svg>\n\
         <div class=\"graph-grid\">\n",
    );

    // Collect cards per quarter, then layer topologically
    let mut quarter_groups: Vec<((i32, u32), Vec<&GraphCard>)> = Vec::new();
    for rank in &ranks {
        let q = (rank.month - 1) / 3 + 1;
        let quarter = (rank.year, q);
        if quarter_groups.last().map(|(k, _)| *k) != Some(quarter) {
            quarter_groups.push((quarter, Vec::new()));
        }
        quarter_groups
            .last_mut()
            .unwrap()
            .1
            .extend(rank.cards.iter().copied());
    }

    for ((year, q), cards) in &quarter_groups {
        body.push_str(&format!(
            "<div class=\"timeline-quarter\">{} Q{}</div>\n",
            year, q,
        ));
        let rows = layer_cards_topologically(cards, &graph.edges);
        for row_cards in rows {
            body.push_str("<div class=\"graph-rank\">\n");
            for card in row_cards {
                render_card(&mut body, card);
            }
            body.push_str("</div>\n");
        }
    }

    // Undated docs rendered inside the graph grid
    if !undated.is_empty() {
        body.push_str("<div class=\"timeline-quarter other-docs\">Other documents</div>\n");
        let rows = layer_cards_topologically(&undated, &graph.edges);
        for row_cards in rows {
            body.push_str("<div class=\"graph-rank\">\n");
            for card in row_cards {
                render_card(&mut body, card);
            }
            body.push_str("</div>\n");
        }
    }

    body.push_str("</div>\n"); // close graph-grid

    // Edge data as JSON
    let mut edges_json = String::from("[");
    let mut seen_pairs: std::collections::HashSet<(String, String)> =
        std::collections::HashSet::new();
    for edge in &graph.edges {
        // Skip inline_ref edges — they add noise when frontmatter edges exist
        if edge.relation == "inline_ref" {
            continue;
        }
        let pair = if edge.from < edge.to {
            (edge.from.clone(), edge.to.clone())
        } else {
            (edge.to.clone(), edge.from.clone())
        };
        // Deduplicate bidirectional "related" edges
        if edge.relation == "related" && !seen_pairs.insert(pair) {
            continue;
        }
        if edges_json.len() > 1 {
            edges_json.push(',');
        }
        edges_json.push_str(&format!(
            "{{\"from\":\"{}\",\"to\":\"{}\",\"label\":\"{}\"}}",
            encode_json_str(&edge.from),
            encode_json_str(&edge.to),
            encode_json_str(&edge.relation),
        ));
    }
    edges_json.push(']');
    body.push_str(&format!(
        "<script type=\"application/json\" id=\"graph-edges\">{edges_json}</script>\n"
    ));
    // Emit inverse-pair lookup so JS can merge bidirectional edges correctly
    if let Some(s) = schema {
        let inverses_json = build_inverses_json(s);
        body.push_str(&format!(
            "<script type=\"application/json\" id=\"graph-inverses\">{inverses_json}</script>\n"
        ));
    }
    body.push_str("</div>\n"); // close graph-container

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Document Index</title>
<style>{INDEX_CSS}{GRAPH_CSS}</style>
</head>
<body>
<header class="index-header">
<h1>Document Index<span class="doc-count">{total} docs</span></h1>
<nav><a href="roadmap.html">Roadmap &rarr;</a></nav>
</header>
{body}
<script>{GRAPH_JS}</script>
</body>
</html>
"#
    )
}

/// Render a single graph card into HTML.
pub(crate) fn render_card(out: &mut String, card: &GraphCard) {
    let lower_id = card.id.to_lowercase();
    let type_class = &card.doc_type;

    let status_lower = card
        .status
        .as_ref()
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    let is_obsolete = matches!(
        status_lower.as_str(),
        "deprecated" | "superseded" | "rejected" | "declined"
    );
    let status_html = card
        .status
        .as_ref()
        .map(|s| {
            let sc = format!("status-{}", s.to_lowercase());
            format!(
                "<span class=\"card-status {}\">{}</span>",
                encode_attr(&sc),
                encode_text(s),
            )
        })
        .unwrap_or_default();

    let extra_class = if is_obsolete { " is-deprecated" } else { "" };
    out.push_str(&format!(
        concat!(
            "<div class=\"graph-card {type_class}{extra_class}\" id=\"card-{lower_id}\">\n",
            "<a href=\"{lower_id}.html\" class=\"card-link\">\n",
            "<div class=\"card-top\">\n",
            "<span class=\"card-id\">{card_id}</span>\n",
            "{status_html}\n",
            "</div>\n",
            "<div class=\"card-title\">{title}</div>\n",
            "</a>\n",
            "</div>\n",
        ),
        type_class = encode_attr(type_class),
        extra_class = extra_class,
        lower_id = encode_attr(&lower_id),
        card_id = encode_text(&card.id),
        status_html = status_html,
        title = encode_text(&card.title),
    ));
}

/// Flat list fallback (original index layout).
fn export_index_flat(docs: &[(String, &Document)]) -> String {
    let mut by_type: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();

    for (id, doc) in docs {
        let doc_type = doc
            .frontmatter
            .as_ref()
            .and_then(|fm| fm.get_display("type"))
            .or_else(|| id.split('-').next().map(|s| s.to_lowercase()))
            .unwrap_or_else(|| "other".to_string());
        let title = doc
            .frontmatter
            .as_ref()
            .and_then(|fm| fm.get_display("title"))
            .or_else(|| doc.title())
            .unwrap_or_else(|| id.clone());
        by_type
            .entry(doc_type)
            .or_default()
            .push((id.clone(), title));
    }

    let mut body = String::new();
    let total = docs.len();

    for (doc_type, entries) in &by_type {
        let title_case = titlecase_word(doc_type);
        body.push_str(&format!(
            "<h2 class=\"type-header\">{} <span class=\"count\">{}</span></h2>\n<ul class=\"doc-list\">\n",
            encode_text(&title_case),
            entries.len()
        ));
        for (id, title) in entries {
            let lower = id.to_lowercase();
            body.push_str(&format!(
                "<li><a href=\"{}\" class=\"doc-row\"><span class=\"doc-id\">{}</span><span class=\"doc-title\">{}</span></a></li>\n",
                encode_attr(&format!("{lower}.html")),
                encode_text(id),
                encode_text(title),
            ));
        }
        body.push_str("</ul>\n");
    }

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Document Index</title>
<style>{INDEX_CSS}</style>
</head>
<body>
<header class="index-header">
<h1>Document Index<span class="doc-count">{total} docs</span></h1>
<nav><a href="roadmap.html">Roadmap &rarr;</a></nav>
</header>
{body}
</body>
</html>
"#
    )
}

/// Export all documents in a directory to HTML files in output_dir.
/// Returns the number of documents exported.
pub fn export_site(
    dir: impl AsRef<Path>,
    schema: Option<&Schema>,
    output_dir: impl AsRef<Path>,
) -> crate::error::Result<usize> {
    let dir = dir.as_ref();
    let output_dir = output_dir.as_ref();
    std::fs::create_dir_all(output_dir)
        .map_err(|_| crate::error::Error::WriteFailed(output_dir.to_path_buf()))?;

    let files = crate::discovery::discover_files(dir, None, &[], false)?;

    // Load all documents + cache titles for backlinks
    let mut docs: Vec<(String, Document)> = Vec::new();
    let mut id_to_title: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for path in &files {
        let doc = match Document::from_file(path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let id = path_to_id(path);
        let title = doc
            .frontmatter
            .as_ref()
            .and_then(|fm| fm.get_display("title"))
            .or_else(|| doc.title())
            .unwrap_or_else(|| id.clone());
        id_to_title.insert(id.clone(), title);
        docs.push((id, doc));
    }

    let known_ids: Vec<String> = docs.iter().map(|(id, _)| id.clone()).collect();

    // Build graph + backlinks map (id, relation, title) if schema provided
    let mut backlinks_map: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
    let graph = schema.and_then(|s| DocGraph::build(dir, s).ok());
    if let Some(ref g) = graph {
        for edge in &g.edges {
            let from_title = id_to_title
                .get(&edge.from)
                .cloned()
                .unwrap_or_else(|| edge.from.clone());
            backlinks_map.entry(edge.to.clone()).or_default().push((
                edge.from.clone(),
                edge.relation.clone(),
                from_title,
            ));
        }
    }

    // Export each document
    for (id, doc) in &docs {
        let backlinks = backlinks_map.get(id).cloned().unwrap_or_default();
        let html = export_html(doc, &known_ids, &backlinks);
        let filename = format!("{}.html", id.to_lowercase());
        let out_path = output_dir.join(&filename);
        std::fs::write(&out_path, &html)
            .map_err(|_| crate::error::Error::WriteFailed(out_path.clone()))?;
    }

    // Export index with dependency graph
    let doc_refs: Vec<(String, &Document)> = docs.iter().map(|(id, d)| (id.clone(), d)).collect();
    let index_html = export_index(&doc_refs, graph.as_ref(), schema);
    let index_path = output_dir.join("index.html");
    std::fs::write(&index_path, &index_html)
        .map_err(|_| crate::error::Error::WriteFailed(index_path))?;

    Ok(docs.len())
}

/// Rewrite internal markdown links (e.g. `docs/policies/pol-009.md`) to SPA routes
/// (e.g. `/policies/pol-009`). Operates on rendered HTML href attributes.
pub fn rewrite_internal_links(html: &str) -> String {
    use std::sync::LazyLock;

    static INTERNAL_LINK: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"href="\.?/?(docs/|services/)([^""]*)""#).expect("valid regex")
    });

    INTERNAL_LINK
        .replace_all(html, |caps: &regex::Captures| {
            let prefix = &caps[1]; // "docs/" or "services/"
            let rest = &caps[2]; // e.g. "policies/pol-009.md" or "api/README.md"

            // Split off #anchor if present
            let (path, anchor) = match rest.find('#') {
                Some(i) => (&rest[..i], &rest[i..]),
                None => (rest, ""),
            };

            let spa_route = if prefix == "services/" {
                // services/{slug}/README.md or services/{slug} → /services/{slug}
                let slug = path
                    .trim_end_matches('/')
                    .trim_end_matches("README.md")
                    .trim_end_matches('/');
                format!("/services/{slug}")
            } else {
                // docs/{type-folder}/{file}.md → /{spa-folder}/{stem}
                rewrite_doc_path(path)
            };

            format!("href=\"{spa_route}{anchor}\"")
        })
        .into_owned()
}

/// Map a docs-relative path like `teams/platform.md` or `policies/pol-009.md`
/// to an SPA route like `/org/teams/platform` or `/policies/pol-009`.
fn rewrite_doc_path(path: &str) -> String {
    let mut parts = path.splitn(2, '/');
    let folder = parts.next().unwrap_or("");
    let file = parts.next().unwrap_or("");
    let stem = file
        .trim_end_matches(".md")
        .trim_end_matches('/')
        .trim_end_matches("README.md")
        .trim_end_matches('/');

    if folder == "teams" {
        return format!("/org/teams/{stem}");
    }

    let spa_folder = match folder {
        "specs" => "specifications",
        other => other,
    };

    format!("/{spa_folder}/{stem}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_markdown_to_html() {
        let md = "# Hello\n\nWorld **bold**.\n";
        let html = render_markdown_to_html(md);
        assert!(html.contains("<h1>"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn test_render_markdown_table() {
        let md = "\
| Service | Stack | Description |
|---------|-------|-------------|
| **Core** | Rails 7 | Admin backend |
| **Frontline** | Rails 7 | Customer web app |
";
        let html = render_markdown_to_html(md);
        assert!(
            html.contains("<table"),
            "GFM pipe table must render as <table>, got: {html}"
        );
        assert!(html.contains("<th"));
        assert!(html.contains("<td"));
        assert!(html.contains("Core"));
        assert!(html.contains("Admin backend"));
        // Must NOT contain raw pipe characters in the output
        assert!(
            !html.contains("|-"),
            "separator row must not appear as raw text"
        );
    }

    #[test]
    fn test_frontmatter_meta() {
        let doc =
            Document::from_str("---\ntitle: Test\nstatus: accepted\nauthor: onni\n---\n\nBody\n")
                .unwrap();
        let html = frontmatter_meta(&doc, &[]);
        // title and status are skipped (shown in header)
        assert!(!html.contains(">title<"));
        assert!(!html.contains(">status<"));
        // author should be present
        assert!(html.contains("author"));
        assert!(html.contains("onni"));
    }

    #[test]
    fn test_linkify_refs() {
        let html = "<p>See ADR-001 and OPP-002 for details.</p>";
        let ids = vec!["ADR-001".to_string(), "OPP-002".to_string()];
        let result = linkify_refs(html, &ids);
        assert!(result.contains("<a href=\"adr-001.html\">ADR-001</a>"));
        assert!(result.contains("<a href=\"opp-002.html\">OPP-002</a>"));
    }

    #[test]
    fn test_export_html() {
        let doc = Document::from_str(
            "---\ntitle: Use Postgres\nstatus: accepted\n---\n\n# Decision\n\nWe use PostgreSQL.\n",
        )
        .unwrap();
        let ids = vec!["ADR-001".to_string()];
        let backlinks = vec![(
            "OPP-001".to_string(),
            "enables".to_string(),
            "Real-time Collaboration".to_string(),
        )];
        let html = export_html(&doc, &ids, &backlinks);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Use Postgres"));
        assert!(html.contains("accepted"));
        assert!(html.contains("PostgreSQL"));
        assert!(html.contains("Referenced by"));
        assert!(html.contains("OPP-001"));
        assert!(html.contains("Real-time Collaboration"));
    }

    #[test]
    fn test_xss_prevention_in_status_badge() {
        let doc = Document::from_str(
            "---\ntitle: XSS Test\nstatus: '\"><script>alert(1)</script>'\n---\n\nBody\n",
        )
        .unwrap();
        let html = export_html(&doc, &[], &[]);
        assert!(
            !html.contains("<script>alert(1)</script>"),
            "raw XSS payload must be escaped in status badge"
        );
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_xss_prevention_in_backlinks() {
        let doc = Document::from_str("---\ntitle: Test\nstatus: ok\n---\n\nBody\n").unwrap();
        let backlinks = vec![(
            "\"><script>alert(1)</script>".to_string(),
            "enables".to_string(),
            "Title".to_string(),
        )];
        let html = export_html(&doc, &[], &backlinks);
        assert!(
            !html.contains("<script>alert(1)</script>"),
            "raw XSS payload must be escaped in backlinks"
        );
    }

    #[test]
    fn test_gfm_alerts_rendered() {
        let md = "> [!CAUTION]\n> Do not delete the database.\n";
        let html = render_markdown_to_html(md);
        assert!(
            html.contains("markdown-alert-caution"),
            "GFM caution alert must render, got: {html}"
        );
        assert!(html.contains("Do not delete the database."));
    }

    #[test]
    fn test_raw_html_stripped_from_markdown() {
        let md = "# Hello\n\n<script>alert('xss')</script>\n\nSafe text.\n";
        let html = render_markdown_to_html(md);
        assert!(!html.contains("<script>"), "raw HTML should be stripped");
        assert!(html.contains("Safe text."));
    }

    #[test]
    fn test_export_index_flat_fallback() {
        let doc1 = Document::from_str("---\ntitle: ADR 1\ntype: adr\n---\n\nBody\n").unwrap();
        let doc2 = Document::from_str("---\ntitle: OPP 1\ntype: opp\n---\n\nBody\n").unwrap();
        let docs = vec![
            ("ADR-001".to_string(), &doc1),
            ("OPP-001".to_string(), &doc2),
        ];
        // No graph → flat list
        let html = export_index(&docs, None, None);
        assert!(html.contains("Document Index"));
        assert!(html.contains("ADR-001"));
        assert!(html.contains("OPP-001"));
        assert!(html.contains("2 docs"));
        // Should not contain graph elements
        assert!(!html.contains("graph-container"));
    }

    #[test]
    fn test_export_index_graph_view() {
        let doc1 = Document::from_str(
            "---\ntitle: ADR 1\ntype: adr\ndate: 2025-01-15\nstatus: accepted\n---\n\nBody\n",
        )
        .unwrap();
        let doc2 = Document::from_str(
            "---\ntitle: POL 1\ntype: pol\ndate: 2025-02-10\nstatus: active\n---\n\nBody\n",
        )
        .unwrap();
        let docs = vec![
            ("ADR-001".to_string(), &doc1),
            ("POL-001".to_string(), &doc2),
        ];

        let mut nodes = BTreeMap::new();
        nodes.insert(
            "ADR-001".to_string(),
            crate::graph::DocNode {
                id: "ADR-001".to_string(),
                path: "adr-001.md".into(),
                doc_type: Some("adr".to_string()),
                title: Some("ADR 1".to_string()),
                status: Some("accepted".to_string()),
            },
        );
        nodes.insert(
            "POL-001".to_string(),
            crate::graph::DocNode {
                id: "POL-001".to_string(),
                path: "pol-001.md".into(),
                doc_type: Some("pol".to_string()),
                title: Some("POL 1".to_string()),
                status: Some("active".to_string()),
            },
        );
        let edges = vec![crate::graph::DocEdge {
            from: "ADR-001".to_string(),
            to: "POL-001".to_string(),
            relation: "triggers".to_string(),
        }];
        let graph = DocGraph { nodes, edges };

        let html = export_index(&docs, Some(&graph), None);
        assert!(html.contains("graph-container"));
        assert!(html.contains("graph-card"));
        assert!(html.contains("card-adr-001"));
        assert!(html.contains("card-pol-001"));
        assert!(html.contains("graph-edges"));
        assert!(html.contains("triggers"));
        assert!(html.contains("2025 Q1"));
        assert!(html.contains("graph-rank"));
        assert!(!html.contains("timeline-month"));
    }

    #[test]
    fn test_export_site() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("input");
        let output = dir.path().join("output");
        std::fs::create_dir_all(&input).unwrap();

        std::fs::write(
            input.join("adr-001.md"),
            "---\ntitle: Test ADR\nstatus: accepted\ntype: adr\n---\n\n# Decision\n\nDone.\n",
        )
        .unwrap();

        let count = export_site(&input, None, &output).unwrap();
        assert_eq!(count, 1);
        assert!(output.join("index.html").exists());
        assert!(output.join("adr-001.html").exists());
    }

    #[test]
    fn test_rewrite_internal_links_policies() {
        let html = r#"<a href="docs/policies/pol-009.md">POL-009</a>"#;
        let result = rewrite_internal_links(html);
        assert_eq!(result, r#"<a href="/policies/pol-009">POL-009</a>"#);
    }

    #[test]
    fn test_rewrite_internal_links_architecture() {
        let html = r#"<a href="docs/architecture/adr-001.md">ADR-001</a>"#;
        let result = rewrite_internal_links(html);
        assert_eq!(result, r#"<a href="/architecture/adr-001">ADR-001</a>"#);
    }

    #[test]
    fn test_rewrite_internal_links_specs_to_specifications() {
        let html = r#"<a href="docs/specs/spec-001.md">SPEC-001</a>"#;
        let result = rewrite_internal_links(html);
        assert_eq!(result, r#"<a href="/specifications/spec-001">SPEC-001</a>"#);
    }

    #[test]
    fn test_rewrite_internal_links_teams() {
        let html = r#"<a href="docs/teams/platform.md">Platform</a>"#;
        let result = rewrite_internal_links(html);
        assert_eq!(result, r#"<a href="/org/teams/platform">Platform</a>"#);
    }

    #[test]
    fn test_rewrite_internal_links_services() {
        let html = r#"<a href="services/api/README.md">API</a>"#;
        let result = rewrite_internal_links(html);
        assert_eq!(result, r#"<a href="/services/api">API</a>"#);
    }

    #[test]
    fn test_rewrite_internal_links_with_anchor() {
        let html = r#"<a href="docs/policies/pol-009.md#section">POL-009</a>"#;
        let result = rewrite_internal_links(html);
        assert_eq!(result, r#"<a href="/policies/pol-009#section">POL-009</a>"#);
    }

    #[test]
    fn test_rewrite_internal_links_leading_dot_slash() {
        let html = r#"<a href="./docs/policies/pol-009.md">POL-009</a>"#;
        let result = rewrite_internal_links(html);
        assert_eq!(result, r#"<a href="/policies/pol-009">POL-009</a>"#);
    }

    #[test]
    fn test_rewrite_internal_links_external_unchanged() {
        let html = r#"<a href="https://example.com">External</a>"#;
        let result = rewrite_internal_links(html);
        assert_eq!(result, html);
    }

    #[test]
    fn test_rewrite_internal_links_no_match_passthrough() {
        let html = r#"<a href="other/file.md">Other</a>"#;
        let result = rewrite_internal_links(html);
        assert_eq!(result, html);
    }

    #[test]
    fn test_rewrite_internal_links_multiple() {
        let html = r#"<a href="docs/policies/pol-009.md">POL-009</a> and <a href="docs/architecture/adr-001.md">ADR-001</a>"#;
        let result = rewrite_internal_links(html);
        assert!(result.contains(r#"href="/policies/pol-009""#));
        assert!(result.contains(r#"href="/architecture/adr-001""#));
    }
}

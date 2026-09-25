//! Link checks for markdown bodies (issue #20).
//!
//! Local links/images (`[x](../other.md)`, `![](img.png)`, `<img src>`,
//! `<a href>`) are resolved against the document's directory:
//!
//! - C020 (error): target file does not exist
//! - C021 (warning): target resolves outside the git repo root
//! - C022 (warning): absolute filesystem path — hint shows the relative path,
//!   `dg fmt` rewrites paths inside the repo automatically
//! - C023 (warning): attachment (image, csv, xlsx, pdf, ...) outside the repo —
//!   suggest moving it into `docs/assets/`
//!
//! External `http(s)` links are checked only on request (`--check-links`):
//!
//! - C024 (error): hostname does not resolve (typo domain)
//! - C025 (warning): DNS lookup timed out
//!
//! TODO: verify other links non-intrusively — `#anchor` fragments against
//! heading slugs, `mailto:` address syntax, and opt-in cached HTTP HEAD checks
//! for dead external pages (rate-limited, results cached in `.dg/.cache.json`
//! so normal validation stays offline and fast).

use std::collections::{BTreeMap, BTreeSet};
use std::net::ToSocketAddrs;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use std::sync::LazyLock;
use std::time::Duration;

use comrak::nodes::NodeValue;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use rayon::prelude::*;
use regex::Regex;

use super::{Diagnostic, FileResult, Severity};
use crate::document::Document;

/// Extensions treated as attachments that belong in `docs/assets/`.
const ATTACHMENT_EXTENSIONS: &[&str] = &[
    "png",
    "jpg",
    "jpeg",
    "gif",
    "svg",
    "webp",
    "bmp",
    "ico",
    "avif",
    "tif",
    "tiff",
    "heic",
    "csv",
    "tsv",
    "xls",
    "xlsx",
    "ods",
    "numbers",
    "doc",
    "docx",
    "odt",
    "pages",
    "ppt",
    "pptx",
    "odp",
    "key",
    "pdf",
    "zip",
    "tar",
    "gz",
    "mp4",
    "mov",
    "webm",
    "mp3",
    "wav",
    "drawio",
    "excalidraw",
    "sketch",
    "fig",
];

/// Characters re-encoded when writing a rewritten link back into markdown.
const PATH_ENCODE: &AsciiSet = &CONTROLS.add(b' ').add(b'(').add(b')').add(b'<').add(b'>');

/// `<img src="...">` / `<a href="...">` inside raw HTML.
static HTML_LINK_RE: LazyLock<Option<Regex>> =
    LazyLock::new(|| Regex::new(r#"<(img|a)\s[^>]*?\b(?:src|href)\s*=\s*["']([^"']+)["']"#).ok());

/// A link or image reference found in a markdown body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyLink {
    /// Destination as written (after markdown unescaping).
    pub url: String,
    /// 1-based line in the body.
    pub line: usize,
    /// `![](...)` or `<img>`.
    pub is_image: bool,
}

/// Extract all links and images from a markdown body, including raw HTML
/// `<img src>` / `<a href>`. Code blocks and inline code are ignored.
pub fn extract_body_links(body: &str) -> Vec<BodyLink> {
    let arena = comrak::Arena::new();
    let root = crate::ast_util::parse_md(&arena, body);
    let mut links = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        let line = data.sourcepos.start.line;
        match &data.value {
            NodeValue::Link(link) => links.push(BodyLink {
                url: link.url.clone(),
                line,
                is_image: false,
            }),
            NodeValue::Image(link) => links.push(BodyLink {
                url: link.url.clone(),
                line,
                is_image: true,
            }),
            NodeValue::HtmlBlock(block) => push_html_links(&block.literal, line, &mut links),
            NodeValue::HtmlInline(html) => push_html_links(html, line, &mut links),
            _ => {}
        }
    }
    links
}

fn push_html_links(html: &str, start_line: usize, out: &mut Vec<BodyLink>) {
    let Some(re) = HTML_LINK_RE.as_ref() else {
        return;
    };
    for cap in re.captures_iter(html) {
        let (Some(tag), Some(url)) = (cap.get(1), cap.get(2)) else {
            continue;
        };
        let offset_lines = html[..url.start()].matches('\n').count();
        out.push(BodyLink {
            url: url.as_str().to_string(),
            line: start_line + offset_lines,
            is_image: tag.as_str().eq_ignore_ascii_case("img"),
        });
    }
}

/// What a link destination points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// `http(s)://host/...` or `//host/...`
    External { host: String },
    /// Local filesystem path (decoded, without `#fragment` / `?query`).
    Local { path: String, absolute: bool },
    /// Anchors, `mailto:`, other schemes, empty links.
    Skip,
}

/// Classify a link destination.
pub fn classify_link(url: &str) -> LinkTarget {
    let url = url.trim();
    if url.is_empty() || url.starts_with('#') {
        return LinkTarget::Skip;
    }
    if let Some(rest) = url.strip_prefix("//") {
        return external(rest);
    }
    if let Some((scheme, rest)) = split_scheme(url) {
        return match scheme.to_ascii_lowercase().as_str() {
            "http" | "https" => match rest.strip_prefix("//") {
                Some(r) => external(r),
                None => LinkTarget::Skip,
            },
            "file" => {
                // file:///abs/path or file://localhost/abs/path
                let rest = rest.trim_start_matches("//");
                let rest = rest.strip_prefix("localhost").unwrap_or(rest);
                local(rest)
            }
            _ => LinkTarget::Skip,
        };
    }
    local(url)
}

/// Split `scheme:rest`. Requires a 2+ char scheme so Windows drive letters
/// (`C:\...`) aren't mistaken for schemes.
fn split_scheme(url: &str) -> Option<(&str, &str)> {
    let colon = url.find(':')?;
    let scheme = &url[..colon];
    let valid = scheme.len() >= 2
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then(|| (scheme, &url[colon + 1..]))
}

fn external(authority_and_path: &str) -> LinkTarget {
    let authority = authority_and_path
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    // Strip userinfo
    let hostport = authority.rsplit('@').next().unwrap_or(authority);
    let host = if let Some(v6) = hostport.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        hostport.split(':').next().unwrap_or("")
    };
    if host.is_empty() {
        return LinkTarget::Skip;
    }
    LinkTarget::External {
        host: host.trim_end_matches('.').to_ascii_lowercase(),
    }
}

fn local(raw: &str) -> LinkTarget {
    let path = raw.split(['#', '?']).next().unwrap_or("");
    if path.is_empty() {
        return LinkTarget::Skip;
    }
    let path = percent_decode_str(path)
        .decode_utf8()
        .map(|p| p.into_owned())
        .unwrap_or_else(|_| path.to_string());
    let absolute = Path::new(&path).is_absolute();
    LinkTarget::Local { path, absolute }
}

/// Find the root of the top-level git repository containing `start`.
///
/// Walks up past git submodules so a docs project living in a submodule still
/// treats the superproject as "the repo". Outside git, falls back to the
/// nearest ancestor holding a `.dg/` directory, else `start` itself.
pub fn find_repo_root(start: &Path) -> PathBuf {
    let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    #[cfg(feature = "git")]
    if let Some(root) = git_superproject_root(&start) {
        return root;
    }
    start
        .ancestors()
        .find(|d| d.join(".dg").is_dir())
        .map(Path::to_path_buf)
        .unwrap_or(start)
}

#[cfg(feature = "git")]
fn git_superproject_root(start: &Path) -> Option<PathBuf> {
    let repo = git2::Repository::discover(start).ok()?;
    let mut root = repo.workdir()?.canonicalize().ok()?;
    while let Some(parent) = root.parent() {
        let Ok(outer) = git2::Repository::discover(parent) else {
            break;
        };
        let Some(outer_root) = outer.workdir().and_then(|w| w.canonicalize().ok()) else {
            break;
        };
        let is_submodule = outer.submodules().is_ok_and(|subs| {
            subs.iter().any(|s| {
                outer_root
                    .join(s.path())
                    .canonicalize()
                    .is_ok_and(|p| p == root)
            })
        });
        if !is_submodule {
            break;
        }
        root = outer_root;
    }
    Some(root)
}

/// Lexically normalize a path (resolve `.` and `..` without touching disk).
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Relative path from directory `from` to `to` (both absolute + normalized).
fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from: Vec<_> = from.components().collect();
    let to_c: Vec<_> = to.components().collect();
    let common = from.iter().zip(&to_c).take_while(|(a, b)| a == b).count();
    let mut rel = PathBuf::new();
    for _ in common..from.len() {
        rel.push("..");
    }
    for c in &to_c[common..] {
        rel.push(c.as_os_str());
    }
    rel
}

fn is_attachment(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| ATTACHMENT_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Check local link targets in a document body.
///
/// `doc_dir` and `repo_root` must be absolute and canonical. `is_doc_id`
/// recognizes bare document IDs used as links (e.g. `[x](ADR-001)`), which
/// are graph references rather than paths.
pub fn check_local_links(
    body: &str,
    doc_dir: &Path,
    repo_root: &Path,
    is_doc_id: &dyn Fn(&str) -> bool,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for link in extract_body_links(body) {
        let LinkTarget::Local { path, absolute } = classify_link(&link.url) else {
            continue;
        };
        if !absolute && !path.contains('/') && is_doc_id(&path) {
            continue;
        }
        let location = format!("body line {}", link.line);
        let url = &link.url;

        let target = if absolute {
            let fs_path = normalize(Path::new(&path));
            if !fs_path.starts_with(repo_root) && !fs_path.exists() {
                // GitHub-style repo-root-relative link: `/docs/assets/x.png`
                let repo_rel = normalize(&repo_root.join(path.trim_start_matches('/')));
                if repo_rel.exists() {
                    continue;
                }
            }
            fs_path
        } else {
            normalize(&doc_dir.join(&path))
        };

        let exists = target.exists();
        let inside = target.starts_with(repo_root);

        if !exists {
            let mut hint = format!("resolved to: {}", target.display());
            if !inside {
                hint.push_str(" (outside the git repository)");
            }
            diags.push(Diagnostic {
                severity: Severity::Error,
                code: "C020".into(),
                message: format!("broken link \"{url}\": file not found"),
                location,
                hint: Some(hint),
            });
            continue;
        }

        if !inside {
            let name = file_name(&target);
            if link.is_image || is_attachment(&target) {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "C023".into(),
                    message: format!("attachment \"{url}\" is outside the git repository"),
                    location,
                    hint: Some(format!(
                        "move it into the repo (e.g. docs/assets/{name}) and link it relatively"
                    )),
                });
            } else {
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    code: "C021".into(),
                    message: format!("link \"{url}\" points outside the git repository"),
                    location,
                    hint: Some(format!(
                        "resolved to {} — readers of the repo can't follow it",
                        target.display()
                    )),
                });
            }
            continue;
        }

        if absolute {
            let rel = relative_path(doc_dir, &target);
            diags.push(Diagnostic {
                severity: Severity::Warning,
                code: "C022".into(),
                message: format!("absolute path \"{url}\" in link"),
                location,
                hint: Some(format!(
                    "use the relative path \"{}\" (`dg fmt` rewrites it)",
                    rel.display()
                )),
            });
        }
    }
    diags
}

/// Rewrite absolute filesystem links that point inside the repo into paths
/// relative to `doc_dir`. Returns the new body and the number of rewritten
/// links, or `None` when nothing changed.
pub fn rewrite_absolute_links(
    body: &str,
    doc_dir: &Path,
    repo_root: &Path,
) -> Option<(String, usize)> {
    let mut replacements: BTreeMap<String, String> = BTreeMap::new();
    for link in extract_body_links(body) {
        let LinkTarget::Local {
            path,
            absolute: true,
        } = classify_link(&link.url)
        else {
            continue;
        };
        let target = normalize(Path::new(&path));
        if !target.starts_with(repo_root) {
            continue;
        }
        let rel = relative_path(doc_dir, &target);
        let rel = rel.to_string_lossy();
        let rel = if rel.is_empty() { "." } else { rel.as_ref() };
        let suffix = link
            .url
            .find(['#', '?'])
            .map(|i| &link.url[i..])
            .unwrap_or("");
        let new_url = format!("{}{suffix}", utf8_percent_encode(rel, PATH_ENCODE));
        replacements.insert(link.url, new_url);
    }

    let mut out = body.to_string();
    let mut count = 0;
    for (old, new) in &replacements {
        let wrappers: [(&str, &str); 8] = [
            ("](", ")"),
            ("](", " "),
            ("](<", ">"),
            ("=\"", "\""),
            ("='", "'"),
            ("]: ", "\n"),
            ("]: ", " "),
            ("]: <", ">"),
        ];
        for (pre, post) in wrappers {
            let from = format!("{pre}{old}{post}");
            if out.contains(&from) {
                count += out.matches(&from).count();
                out = out.replace(&from, &format!("{pre}{new}{post}"));
            }
        }
        // Reference definition at the very end of the body (no trailing newline)
        let tail = format!("]: {old}");
        if out.ends_with(&tail) {
            let cut = out.len() - old.len();
            out.replace_range(cut.., new);
            count += 1;
        }
    }
    (count > 0).then_some((out, count))
}

// ---------------------------------------------------------------------------
// External link DNS checks (opt-in)
// ---------------------------------------------------------------------------

/// Outcome of resolving a hostname.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostLookup {
    Resolved,
    NotFound(String),
    TimedOut,
}

/// Hostname resolver — a trait so tests can run offline.
pub trait HostResolver: Sync {
    fn resolve(&self, host: &str) -> HostLookup;
}

/// Resolver backed by the OS (`getaddrinfo` via `ToSocketAddrs`), which
/// follows CNAMEs and returns A/AAAA records. Lookups exceeding `timeout`
/// report [`HostLookup::TimedOut`].
pub struct SystemResolver {
    pub timeout: Duration,
}

impl Default for SystemResolver {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
        }
    }
}

impl HostResolver for SystemResolver {
    fn resolve(&self, host: &str) -> HostLookup {
        let (tx, rx) = mpsc::channel();
        let host = host.to_string();
        // getaddrinfo has no timeout; run it on a detached thread.
        std::thread::spawn(move || {
            let res = (host.as_str(), 443)
                .to_socket_addrs()
                .map(|mut addrs| addrs.next().is_some());
            let _ = tx.send(res);
        });
        match rx.recv_timeout(self.timeout) {
            Ok(Ok(true)) => HostLookup::Resolved,
            Ok(Ok(false)) => HostLookup::NotFound("no A/AAAA records".into()),
            Ok(Err(e)) => HostLookup::NotFound(e.to_string()),
            Err(_) => HostLookup::TimedOut,
        }
    }
}

/// Hosts that never resolve publicly by design (RFC 2606/6761) or are
/// internal single-label names — skipped to avoid noise.
fn skip_host(host: &str) -> bool {
    const RESERVED_TLDS: &[&str] = &["test", "example", "invalid", "localhost", "local"];
    const EXAMPLE_DOMAINS: &[&str] = &["example.com", "example.net", "example.org"];
    if host.parse::<std::net::IpAddr>().is_ok() || !host.contains('.') {
        return true;
    }
    let tld = host.rsplit('.').next().unwrap_or("");
    RESERVED_TLDS.contains(&tld)
        || EXAMPLE_DOMAINS
            .iter()
            .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Resolve every external link hostname in `docs` (path, body) and report
/// unresolvable ones. Hosts are deduplicated and resolved in parallel.
pub fn check_external_hosts(
    docs: &[(String, String)],
    resolver: &dyn HostResolver,
) -> Vec<FileResult> {
    // host -> [(doc index, line, url)]
    let mut by_host: BTreeMap<String, Vec<(usize, usize, String)>> = BTreeMap::new();
    for (i, (_, body)) in docs.iter().enumerate() {
        for link in extract_body_links(body) {
            if let LinkTarget::External { host } = classify_link(&link.url) {
                if !skip_host(&host) {
                    by_host
                        .entry(host)
                        .or_default()
                        .push((i, link.line, link.url));
                }
            }
        }
    }

    let hosts: Vec<&String> = by_host.keys().collect();
    let lookup = |h: &&String| ((*h).clone(), resolver.resolve(h));
    // DNS is IO-bound: use a wider pool than the CPU-sized global one.
    let outcomes: Vec<(String, HostLookup)> = match rayon::ThreadPoolBuilder::new()
        .num_threads(hosts.len().clamp(1, 32))
        .build()
    {
        Ok(pool) => pool.install(|| hosts.par_iter().map(lookup).collect()),
        Err(_) => hosts.par_iter().map(lookup).collect(),
    };

    let mut per_doc: BTreeMap<usize, Vec<Diagnostic>> = BTreeMap::new();
    for (host, outcome) in outcomes {
        let (severity, code, reason) = match outcome {
            HostLookup::Resolved => continue,
            HostLookup::NotFound(e) => (Severity::Error, "C024", format!("does not resolve ({e})")),
            HostLookup::TimedOut => (
                Severity::Warning,
                "C025",
                "DNS lookup timed out".to_string(),
            ),
        };
        let mut seen = BTreeSet::new();
        for (i, line, url) in &by_host[&host] {
            if !seen.insert((*i, *line)) {
                continue;
            }
            per_doc.entry(*i).or_default().push(Diagnostic {
                severity,
                code: code.into(),
                message: format!("host \"{host}\" {reason}"),
                location: format!("body line {line}"),
                hint: Some(format!("check the domain spelling in {url}")),
            });
        }
    }

    per_doc
        .into_iter()
        .map(|(i, diagnostics)| FileResult {
            path: docs[i].0.clone(),
            diagnostics,
        })
        .collect()
}

/// Discover markdown files under `dir` and run [`check_external_hosts`].
/// Paths in results are relative to `dir`.
pub fn check_external_links(
    dir: &Path,
    pattern: Option<&str>,
    resolver: &dyn HostResolver,
) -> crate::error::Result<Vec<FileResult>> {
    let files = crate::discovery::discover_files(dir, pattern, &[], false)?;
    let docs: Vec<(String, String)> = files
        .par_iter()
        .filter_map(|path| {
            let doc = Document::from_file(path).ok()?;
            let rel = path.strip_prefix(dir).unwrap_or(path);
            Some((rel.display().to_string(), doc.body))
        })
        .collect();
    Ok(check_external_hosts(&docs, resolver))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn no_ids(_: &str) -> bool {
        false
    }

    /// Temp repo layout: <tmp>/repo/docs/decisions/doc.md, plus <tmp>/outside/.
    struct Fixture {
        _tmp: tempfile::TempDir,
        repo: PathBuf,
        doc_dir: PathBuf,
        outside: PathBuf,
    }

    fn fixture() -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().canonicalize().unwrap();
        let repo = base.join("repo");
        let doc_dir = repo.join("docs/decisions");
        let outside = base.join("outside");
        fs::create_dir_all(&doc_dir).unwrap();
        fs::create_dir_all(repo.join("docs/assets")).unwrap();
        fs::create_dir_all(&outside).unwrap();
        fs::write(doc_dir.join("other.md"), "# Other\n").unwrap();
        fs::write(repo.join("docs/assets/arch.png"), "png").unwrap();
        fs::write(outside.join("notes.md"), "# Notes\n").unwrap();
        fs::write(outside.join("report.xlsx"), "xlsx").unwrap();
        fs::write(outside.join("shot.png"), "png").unwrap();
        Fixture {
            _tmp: tmp,
            repo,
            doc_dir,
            outside,
        }
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn classify_variants() {
        assert_eq!(classify_link("#anchor"), LinkTarget::Skip);
        assert_eq!(classify_link("mailto:a@b.c"), LinkTarget::Skip);
        assert_eq!(
            classify_link("https://User@Docs.Rs:8443/x?y#z"),
            LinkTarget::External {
                host: "docs.rs".into()
            }
        );
        assert_eq!(
            classify_link("//cdn.example.com/a.js"),
            LinkTarget::External {
                host: "cdn.example.com".into()
            }
        );
        assert_eq!(
            classify_link("../a%20b.md#sec"),
            LinkTarget::Local {
                path: "../a b.md".into(),
                absolute: false
            }
        );
        assert_eq!(
            classify_link("file:///tmp/x.pdf"),
            LinkTarget::Local {
                path: "/tmp/x.pdf".into(),
                absolute: true
            }
        );
    }

    #[test]
    fn extracts_md_html_and_skips_code() {
        let body = "# T\n\n[a](a.md) ![b](b.png)\n\n<img src=\"c.png\">\n\n```\n[no](no.md)\n```\n\nText <a href=\"d.md\">d</a>\n";
        let links = extract_body_links(body);
        let urls: Vec<_> = links
            .iter()
            .map(|l| (l.url.as_str(), l.line, l.is_image))
            .collect();
        assert_eq!(
            urls,
            vec![
                ("a.md", 3, false),
                ("b.png", 3, true),
                ("c.png", 5, true),
                ("d.md", 11, false)
            ]
        );
    }

    #[test]
    fn existing_relative_links_ok() {
        let f = fixture();
        let body =
            "[o](other.md) [o2](./other.md#x) ![a](../assets/arch.png) [dir](../) [id](ADR-001)\n";
        let diags = check_local_links(body, &f.doc_dir, &f.repo, &|s| s == "ADR-001");
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn missing_file_error() {
        let f = fixture();
        let body = "See [missing](../../nope.md) and ![img](../assets/gone.png).\n";
        let diags = check_local_links(body, &f.doc_dir, &f.repo, &no_ids);
        assert_eq!(codes(&diags), vec!["C020", "C020"]);
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(diags[0].location, "body line 1");
    }

    #[test]
    fn outside_repo_warning() {
        let f = fixture();
        let body = "[notes](../../../outside/notes.md)\n";
        let diags = check_local_links(body, &f.doc_dir, &f.repo, &no_ids);
        assert_eq!(codes(&diags), vec!["C021"]);
        assert_eq!(diags[0].severity, Severity::Warning);
    }

    #[test]
    fn attachment_outside_repo_suggests_assets() {
        let f = fixture();
        let body = format!(
            "[data](../../../outside/report.xlsx)\n\n![shot]({})\n",
            f.outside.join("shot.png").display()
        );
        let diags = check_local_links(&body, &f.doc_dir, &f.repo, &no_ids);
        assert_eq!(codes(&diags), vec!["C023", "C023"]);
        assert!(diags[0]
            .hint
            .as_deref()
            .unwrap()
            .contains("docs/assets/report.xlsx"));
    }

    #[test]
    fn absolute_path_inside_repo_warns_with_relative_hint() {
        let f = fixture();
        let body = format!("![a]({})\n", f.repo.join("docs/assets/arch.png").display());
        let diags = check_local_links(&body, &f.doc_dir, &f.repo, &no_ids);
        assert_eq!(codes(&diags), vec!["C022"]);
        assert!(diags[0]
            .hint
            .as_deref()
            .unwrap()
            .contains("\"../assets/arch.png\""));
    }

    #[test]
    fn repo_root_relative_link_ok() {
        let f = fixture();
        let body = "![a](/docs/assets/arch.png)\n";
        let diags = check_local_links(body, &f.doc_dir, &f.repo, &no_ids);
        assert!(diags.is_empty(), "{diags:?}");
    }

    #[test]
    fn rewrite_absolute_links_to_relative() {
        let f = fixture();
        let abs = f.repo.join("docs/assets/arch.png").display().to_string();
        let outside = f.outside.join("shot.png").display().to_string();
        let body = format!(
            "![a]({abs})\n\n<img src=\"{abs}\">\n\n[o]({}#sec)\n\n![x]({outside})\n",
            f.doc_dir.join("other.md").display()
        );
        let (out, n) = rewrite_absolute_links(&body, &f.doc_dir, &f.repo).unwrap();
        assert_eq!(n, 3);
        assert!(out.contains("![a](../assets/arch.png)"));
        assert!(out.contains("<img src=\"../assets/arch.png\">"));
        assert!(out.contains("[o](other.md#sec)"));
        // Outside-repo path is left alone
        assert!(out.contains(&format!("![x]({outside})")));
        assert!(rewrite_absolute_links(&out, &f.doc_dir, &f.repo).is_none());
    }

    #[test]
    fn repo_root_falls_back_to_start_without_git() {
        let f = fixture();
        // tempdirs are not inside a git repo
        if git2_discovers(&f.repo) {
            return;
        }
        assert_eq!(find_repo_root(&f.doc_dir), f.doc_dir);
        fs::create_dir_all(f.repo.join(".dg")).unwrap();
        assert_eq!(find_repo_root(&f.doc_dir), f.repo);
    }

    #[cfg(feature = "git")]
    fn git2_discovers(p: &Path) -> bool {
        git2::Repository::discover(p).is_ok()
    }

    #[cfg(not(feature = "git"))]
    fn git2_discovers(_: &Path) -> bool {
        false
    }

    #[cfg(feature = "git")]
    #[test]
    fn repo_root_from_git_subdir() {
        let f = fixture();
        git2::Repository::init(&f.repo).unwrap();
        assert_eq!(find_repo_root(&f.doc_dir), f.repo);
    }

    struct MockResolver;
    impl HostResolver for MockResolver {
        fn resolve(&self, host: &str) -> HostLookup {
            match host {
                "gihtub.com" => HostLookup::NotFound("nxdomain".into()),
                "slow.dev" => HostLookup::TimedOut,
                "localhost" | "example.com" => panic!("reserved host should be skipped"),
                _ => HostLookup::Resolved,
            }
        }
    }

    #[test]
    fn dns_check_reports_unresolvable_hosts() {
        let docs = vec![
            (
                "a.md".to_string(),
                "[ok](https://github.com/x)\n\n[typo](https://gihtub.com/x)\n\n[t2](https://gihtub.com/y)\n"
                    .to_string(),
            ),
            (
                "b.md".to_string(),
                "[slow](http://slow.dev) [l](http://localhost:3000) [e](https://example.com)\n"
                    .to_string(),
            ),
        ];
        let results = check_external_hosts(&docs, &MockResolver);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path, "a.md");
        assert_eq!(codes(&results[0].diagnostics), vec!["C024", "C024"]);
        assert_eq!(results[0].diagnostics[1].location, "body line 5");
        assert_eq!(codes(&results[1].diagnostics), vec!["C025"]);
        assert_eq!(results[1].diagnostics[0].severity, Severity::Warning);
    }

    #[test]
    fn skip_host_rules() {
        assert!(skip_host("localhost"));
        assert!(skip_host("127.0.0.1"));
        assert!(skip_host("api.test"));
        assert!(skip_host("www.example.org"));
        assert!(skip_host("intranet"));
        assert!(!skip_host("docs.rs"));
    }
}

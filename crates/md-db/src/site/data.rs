//! JSON data generation for the SvelteKit SPA.
//!
//! Generates `data/*.json` files consumed client-side by the SPA.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::document::Document;
use crate::export::render_markdown_to_html;
use crate::graph::DocGraph;
use crate::schema::Schema;
use crate::users::OrgConfig;

// ── docs.json ──────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DocsJson {
    pub types: BTreeMap<String, TypeInfo>,
    pub docs: Vec<DocJson>,
}

#[derive(Serialize)]
pub struct TypeInfo {
    pub display: String,
    pub folder: String,
}

#[derive(Serialize)]
pub struct DocJson {
    pub id: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub title: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Extra frontmatter fields not captured above (effort, impact, etc.)
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub meta: BTreeMap<String, String>,
    /// User-type fields: role → list of handles (author, owner, reviewers, etc.)
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub people: BTreeMap<String, Vec<String>>,
    pub body_html: String,
    pub links: BTreeMap<String, Vec<String>>,
    pub backlinks: Vec<BacklinkJson>,
    #[serde(skip_serializing_if = "is_zero")]
    pub open_questions: usize,
    /// Relative path to source markdown file (e.g. "docs/architecture/adr-001.md")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

#[derive(Serialize)]
pub struct BacklinkJson {
    pub id: String,
    pub relation: String,
    pub title: String,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

// ── graph.json ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct GraphJson {
    pub nodes: Vec<GraphNodeJson>,
    pub edges: Vec<GraphEdgeJson>,
}

#[derive(Serialize)]
pub struct GraphNodeJson {
    pub id: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub title: String,
    pub status: String,
}

#[derive(Serialize)]
pub struct GraphEdgeJson {
    pub source: String,
    pub target: String,
    pub relation: String,
}

// ── org.json ───────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct OrgJson {
    pub teams: BTreeMap<String, TeamJson>,
    pub users: BTreeMap<String, UserJson>,
    pub orgs: BTreeMap<String, OrgDefJson>,
}

#[derive(Serialize)]
pub struct TeamJson {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lead: Option<String>,
    pub members: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
}

#[derive(Serialize)]
pub struct UserJson {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub teams: Vec<String>,
    pub status: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
}

#[derive(Serialize)]
pub struct OrgDefJson {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    pub children: Vec<String>,
}

// ── nav.json ───────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct NavItemJson {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<NavItemJson>,
}

// ── search-index.json ──────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SearchEntryJson {
    pub id: String,
    pub title: String,
    pub body: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    /// Secondary text shown below title (e.g. job title for users, member count for teams)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// Explicit href when it differs from the doc-type based convention
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

// ── assignments.json ──────────────────────────────────────────────────

/// Per-user assignments: services owned, authored docs, table action items, etc.
#[derive(Serialize)]
pub struct AssignmentsJson {
    /// Map from user handle (without @) to list of assignments.
    pub users: BTreeMap<String, Vec<AssignmentJson>>,
}

#[derive(Serialize)]
pub struct AssignmentJson {
    pub doc_id: String,
    pub doc_type: String,
    pub doc_title: String,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
}

// ── services.json ─────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ServicesJson {
    pub services: Vec<ServiceJson>,
    pub devicon_urls: BTreeMap<String, String>,
}

#[derive(Serialize)]
pub struct ServiceJson {
    pub slug: String,
    pub name: String,
    pub kind: String,
    pub status: String,
    pub owner: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_team: Option<String>,
    pub description: String,
    pub readme_path: String,
    pub body_html: String,
    pub primary_language: String,
    pub languages: Vec<ServiceLanguageJson>,
    pub frameworks: Vec<String>,
    pub framework_versions: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deployment_platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_of_code: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_commit_at: Option<String>,
    pub has_linter: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linter_tool: Option<String>,
    pub has_tests: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test_framework: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_commands: Option<DevCommandsJson>,
    /// Web URL for the source repository (from git submodule or main repo)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[cfg(feature = "avatars")]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub eol_warnings: Vec<crate::eol::EolWarning>,
}

#[derive(Serialize)]
pub struct DevCommandsJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lint: Option<String>,
}

#[derive(Serialize)]
pub struct ServiceLanguageJson {
    pub name: String,
    pub percentage: f64,
}

// ── roadmap.json ───────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct RoadmapJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
}

// ── site-meta.json ────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SiteMetaJson {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readme_html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo_url: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub jira: Vec<JiraConfigJson>,
    /// GitHub/GitLab edit URL prefix (e.g. "https://github.com/org/repo/edit/main/")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit_url_prefix: Option<String>,
    /// True when served via `dg serve` (enables local file opening)
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub is_local_dev: bool,
    /// Source file for the intro/README page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// Git clone URL for the project (derived from edit_url_prefix)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clone_url: Option<String>,
    /// True if project has a CLAUDE.md file
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub has_claude_md: bool,
    /// True if project uses git submodules
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub has_submodules: bool,
}

#[derive(Serialize)]
pub struct JiraConfigJson {
    pub prefix: String,
    pub url: String,
}

#[derive(Serialize)]
pub struct ReadmeJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

// ── schema.json ───────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SchemaJson {
    pub types: BTreeMap<String, SchemaTypeJson>,
}

#[derive(Serialize)]
pub struct SchemaTypeJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub statuses: Vec<SchemaEnumValueJson>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, SchemaFieldJson>,
}

#[derive(Serialize)]
pub struct SchemaEnumValueJson {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub transitions: Vec<String>,
}

#[derive(Serialize)]
pub struct SchemaFieldJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<SchemaEnumValueJson>,
}

// ── Builders ───────────────────────────────────────────────────────────

pub fn build_docs_json(
    docs: &[(String, Document)],
    backlinks_map: &BTreeMap<String, Vec<(String, String, String)>>,
    schema: &Schema,
    project_dir: &Path,
) -> DocsJson {
    let mut types = BTreeMap::new();
    for (key, folder, display) in schema.nav_types() {
        types.insert(key.to_string(), TypeInfo { display, folder });
    }

    let relation_names = schema.all_relation_field_names();

    let docs_json: Vec<DocJson> = docs
        .iter()
        .map(|(id, doc)| {
            let fm = doc.frontmatter.as_ref();
            let doc_type = fm
                .and_then(|f| f.get_display("type"))
                .or_else(|| id.split('-').next().map(|s| s.to_lowercase()))
                .unwrap_or_else(|| "other".to_string())
                .to_lowercase();
            let title = fm
                .and_then(|f| f.get_display("title"))
                .or_else(|| doc.title())
                .unwrap_or_else(|| id.clone());
            let status = fm.and_then(|f| f.get_display("status")).unwrap_or_default();
            let author =
                fm.and_then(|f| f.get_display("authors").or_else(|| f.get_display("author")));
            let date = fm.and_then(|f| f.get_display("date"));
            let tags = fm
                .and_then(|f| f.get("tags"))
                .map(yaml_to_string_list)
                .unwrap_or_default();
            let severity = fm.and_then(|f| f.get_display("severity"));
            let priority = fm.and_then(|f| f.get_display("priority"));
            let category = fm.and_then(|f| f.get_display("category"));

            // Collect user-typed field names from schema so we can skip them in meta
            let user_field_names: Vec<&str> = schema
                .get_type(&doc_type)
                .map(|td| {
                    td.fields
                        .iter()
                        .filter(|f| {
                            matches!(
                                f.field_type,
                                crate::schema::FieldType::User
                                    | crate::schema::FieldType::UserArray
                            )
                        })
                        .map(|f| f.name.as_str())
                        .collect()
                })
                .unwrap_or_default();

            // Collect extra scalar frontmatter fields not already captured
            let skip_keys: &[&str] = &[
                "title", "status", "type", "date", "tags", "severity", "priority", "category",
                "links",
            ];
            let mut meta: BTreeMap<String, String> = BTreeMap::new();
            if let Some(f) = fm {
                for key in f.keys() {
                    if skip_keys.contains(&key.as_str()) {
                        continue;
                    }
                    // Skip relation fields (they go into links)
                    if relation_names.contains(&key.as_str()) {
                        continue;
                    }
                    // Skip user-typed fields (they go into people)
                    if user_field_names.contains(&key.as_str()) {
                        continue;
                    }
                    if let Some(val) = f.get_display(key) {
                        // Skip arrays/objects (already handled or not useful as single string)
                        if matches!(
                            f.get(key),
                            Some(serde_yaml::Value::Sequence(_) | serde_yaml::Value::Mapping(_))
                        ) {
                            continue;
                        }
                        meta.insert(key.clone(), val);
                    }
                }
            }

            // Extract user-type fields from schema
            let mut people: BTreeMap<String, Vec<String>> = BTreeMap::new();
            if let Some(td) = schema.get_type(&doc_type) {
                if let Some(f) = fm {
                    for field in &td.fields {
                        match &field.field_type {
                            crate::schema::FieldType::User => {
                                if let Some(val) = f.get_display(&field.name) {
                                    let h = val.strip_prefix('@').unwrap_or(&val).to_string();
                                    if !h.is_empty() {
                                        people.insert(field.name.clone(), vec![h]);
                                    }
                                }
                            }
                            crate::schema::FieldType::UserArray => {
                                if let Some(serde_yaml::Value::Sequence(seq)) = f.get(&field.name) {
                                    let handles: Vec<String> = seq
                                        .iter()
                                        .filter_map(|v| v.as_str())
                                        .map(|s| s.strip_prefix('@').unwrap_or(s).to_string())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                    if !handles.is_empty() {
                                        people.insert(field.name.clone(), handles);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Extract links from frontmatter
            let mut links: BTreeMap<String, Vec<String>> = BTreeMap::new();
            if let Some(f) = fm {
                // Check "links.*" fields
                if let Some(serde_yaml::Value::Mapping(link_map)) = f.get("links") {
                    for (k, v) in link_map {
                        if let Some(key_str) = k.as_str() {
                            let refs = yaml_to_string_list(v);
                            if !refs.is_empty() {
                                links.insert(key_str.to_string(), refs);
                            }
                        }
                    }
                }
                // Check relation fields at top level
                for rel in &relation_names {
                    if let Some(val) = f.get(rel) {
                        let refs = yaml_to_string_list(val);
                        if !refs.is_empty() {
                            links.insert(rel.to_string(), refs);
                        }
                    }
                }
            }

            let backlinks = backlinks_map
                .get(id)
                .map(|bls| {
                    bls.iter()
                        .map(|(from_id, relation, from_title)| BacklinkJson {
                            id: from_id.clone(),
                            relation: relation.clone(),
                            title: from_title.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();

            let body_html = strip_leading_h1(&render_markdown_to_html(&doc.body), &title);
            // Rewrite relative image/link src paths to be root-relative
            let body_html = if let Some(doc_path) = doc.path.as_ref() {
                let doc_dir = doc_path
                    .parent()
                    .and_then(|p| p.strip_prefix(project_dir).ok());
                if let Some(dir) = doc_dir {
                    rewrite_relative_asset_paths(&body_html, dir)
                } else {
                    body_html
                }
            } else {
                body_html
            };

            let open_questions = crate::questions::extract_questions(doc)
                .iter()
                .filter(|q| !q.done)
                .count();

            let source_path = doc
                .path
                .as_ref()
                .and_then(|p| p.strip_prefix(project_dir).ok())
                .and_then(|p| p.to_str())
                .map(|s| s.to_string());

            DocJson {
                id: id.clone(),
                doc_type,
                title,
                status,
                author,
                date,
                tags,
                severity,
                priority,
                category,
                meta,
                people,
                body_html,
                links,
                backlinks,
                open_questions,
                source_path,
            }
        })
        .collect();

    DocsJson {
        types,
        docs: docs_json,
    }
}

pub fn build_graph_json(graph: &DocGraph, schema: &Schema) -> GraphJson {
    let known_prefixes: std::collections::HashSet<String> = schema
        .types
        .iter()
        .flat_map(|t| {
            let mut p = vec![t.name.to_uppercase()];
            for a in &t.aliases {
                p.push(a.to_uppercase());
            }
            p
        })
        .collect();

    let node_ids: std::collections::HashSet<String> = graph
        .nodes
        .values()
        .filter(|n| {
            let prefix = n.id.split('-').next().unwrap_or("");
            known_prefixes.contains(prefix)
        })
        .map(|n| n.id.clone())
        .collect();

    let nodes: Vec<GraphNodeJson> = graph
        .nodes
        .values()
        .filter(|n| node_ids.contains(&n.id))
        .map(|n| GraphNodeJson {
            id: n.id.clone(),
            doc_type: n.doc_type.clone().unwrap_or_default(),
            title: n.title.clone().unwrap_or_else(|| n.id.clone()),
            status: n.status.clone().unwrap_or_default(),
        })
        .collect();

    let edges: Vec<GraphEdgeJson> = graph
        .edges
        .iter()
        .filter(|e| node_ids.contains(&e.from) && node_ids.contains(&e.to))
        .map(|e| GraphEdgeJson {
            source: e.from.clone(),
            target: e.to.clone(),
            relation: e.relation.clone(),
        })
        .collect();

    GraphJson { nodes, edges }
}

pub fn build_org_json(
    org: Option<&OrgConfig>,
    avatar_map: &std::collections::HashMap<String, String>,
    project_dir: &Path,
) -> OrgJson {
    let org = match org {
        Some(o) => o,
        None => {
            return OrgJson {
                teams: BTreeMap::new(),
                users: BTreeMap::new(),
                orgs: BTreeMap::new(),
            }
        }
    };

    let teams: BTreeMap<String, TeamJson> = org
        .teams
        .iter()
        .map(|(id, t)| {
            // Members = users who list this team in their teams field
            let members: Vec<String> = org
                .users
                .iter()
                .filter(|(_, u)| u.teams.contains(id))
                .map(|(handle, _)| handle.clone())
                .collect();

            // Children = teams whose parent is this team
            let children: Vec<String> = org
                .teams
                .iter()
                .filter(|(_, child)| child.parent.as_deref() == Some(id.as_str()))
                .map(|(child_id, _)| child_id.clone())
                .collect();

            let (description, body_html, source_path) = read_team_doc(project_dir, id);

            (
                id.clone(),
                TeamJson {
                    name: t.name.clone().unwrap_or_else(|| id.clone()),
                    lead: t.lead.clone(),
                    members,
                    parent: t.parent.clone(),
                    children,
                    org: t.org.clone(),
                    status: t.status.to_string(),
                    description,
                    body_html,
                    source_path,
                    extra: t.extra.clone(),
                },
            )
        })
        .collect();

    let users: BTreeMap<String, UserJson> = org
        .users
        .iter()
        .map(|(handle, u)| {
            (
                handle.clone(),
                UserJson {
                    name: u.name.clone().unwrap_or_else(|| handle.clone()),
                    title: u.title.clone(),
                    email: u.email.clone(),
                    teams: u.teams.clone(),
                    status: u.status.to_string(),
                    kind: u.kind.to_string(),
                    org: u.org.clone(),
                    avatar_url: avatar_map.get(handle).cloned(),
                    started: u.extra.get("started").cloned(),
                },
            )
        })
        .collect();

    let orgs: BTreeMap<String, OrgDefJson> = org
        .orgs
        .iter()
        .map(|(id, o)| {
            let children: Vec<String> = org
                .orgs
                .iter()
                .filter(|(_, child)| child.parent.as_deref() == Some(id.as_str()))
                .map(|(child_id, _)| child_id.clone())
                .collect();
            (
                id.clone(),
                OrgDefJson {
                    name: o.name.clone().unwrap_or_else(|| id.clone()),
                    parent: o.parent.clone(),
                    children,
                },
            )
        })
        .collect();

    OrgJson { teams, users, orgs }
}

/// Read `docs/teams/{id}.md` and return (first_paragraph, full_html).
fn read_team_doc(
    project_dir: &Path,
    team_id: &str,
) -> (Option<String>, Option<String>, Option<String>) {
    let rel = format!("docs/teams/{team_id}.md");
    let path = project_dir.join(&rel);
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return (None, None, None),
    };

    let body = strip_yaml_frontmatter(&raw);
    let html = render_markdown_to_html(body);
    let desc = extract_first_paragraph(body);

    (desc, Some(html), Some(rel))
}

/// Strip the first `<h1>` from rendered HTML if its text matches the document title.
/// This prevents the title from appearing twice (once in the card header, once in the body).
fn strip_leading_h1(html: &str, title: &str) -> String {
    let trimmed = html.trim_start();
    if let Some(rest) = trimmed.strip_prefix("<h1") {
        // Find closing </h1>
        if let Some(end) = rest.find("</h1>") {
            // Extract text content between > and </h1>, stripping any inner tags
            if let Some(gt) = rest[..end].find('>') {
                let inner = &rest[gt + 1..end];
                // Strip HTML tags to get plain text
                let plain: String = inner
                    .split('<')
                    .enumerate()
                    .map(|(i, part)| {
                        if i == 0 {
                            part.to_string()
                        } else {
                            part.split_once('>')
                                .map(|(_, t)| t.to_string())
                                .unwrap_or_default()
                        }
                    })
                    .collect::<String>();
                let decoded = plain
                    .replace("&amp;", "&")
                    .replace("&lt;", "<")
                    .replace("&gt;", ">")
                    .replace("&quot;", "\"")
                    .replace("&#39;", "'");
                if decoded.trim() == title.trim() {
                    let after = &rest[end + 5..]; // skip past </h1>
                    return after.trim_start_matches(['\r', '\n']).to_string();
                }
            }
        }
    }
    html.to_string()
}

/// Rewrite relative `src` and `href` attributes in HTML to root-relative paths.
///
/// Given a doc at `docs/processes/proc-001.md` referencing `../assets/img.png`,
/// this resolves to `/docs/assets/img.png` so the SPA can serve it regardless
/// of the current URL path. Absolute URLs and anchors are left unchanged.
fn rewrite_relative_asset_paths(html: &str, doc_dir: &std::path::Path) -> String {
    use regex::Regex;
    use std::sync::LazyLock;

    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(src|href)="([^"]*?)""#).unwrap());

    RE.replace_all(html, |caps: &regex::Captures| {
        let attr = &caps[1];
        let path = &caps[2];

        // Skip absolute URLs, anchors, data URIs, and already-root-relative paths
        if path.starts_with("http://")
            || path.starts_with("https://")
            || path.starts_with('#')
            || path.starts_with("data:")
            || path.starts_with('/')
        {
            return caps[0].to_string();
        }

        // Resolve relative path against the doc directory
        let resolved = doc_dir.join(path);
        // Normalize away ../
        let mut parts: Vec<&str> = Vec::new();
        for component in resolved.components() {
            match component {
                std::path::Component::ParentDir => {
                    parts.pop();
                }
                std::path::Component::Normal(s) => {
                    if let Some(s) = s.to_str() {
                        parts.push(s);
                    }
                }
                _ => {}
            }
        }
        let normalized = parts.join("/");
        format!("{attr}=\"/{normalized}\"")
    })
    .to_string()
}

/// Strip optional YAML frontmatter (--- delimited) from markdown content.
fn strip_yaml_frontmatter(raw: &str) -> &str {
    let trimmed = raw.trim_start();
    if !trimmed.starts_with("---") {
        return raw;
    }
    // Find closing ---
    if let Some(end) = trimmed[3..].find("\n---") {
        let after = &trimmed[3 + end + 4..]; // skip past "\n---"
        after.trim_start_matches(['\r', '\n'])
    } else {
        raw
    }
}

/// Extract the first non-heading, non-empty paragraph from markdown source.
fn extract_first_paragraph(body: &str) -> Option<String> {
    let mut lines = Vec::new();
    let mut in_para = false;

    for line in body.lines() {
        let trimmed = line.trim();

        // Skip headings and blank lines before paragraph starts
        if !in_para {
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            in_para = true;
        }

        // End paragraph on blank line or heading
        if in_para && (trimmed.is_empty() || trimmed.starts_with('#')) {
            break;
        }

        lines.push(trimmed);
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join(" "))
    }
}

pub fn build_nav_json(
    by_type: &BTreeMap<String, Vec<(String, &Document)>>,
    org: Option<&OrgConfig>,
    config: &super::SiteConfig,
    services: &[ServiceJson],
    schema: &Schema,
) -> Vec<NavItemJson> {
    use super::nav;
    let nav_services: Vec<nav::NavService> = services
        .iter()
        .map(|s| nav::NavService {
            slug: s.slug.clone(),
            name: s.name.clone(),
            kind: s.kind.clone(),
            status: s.status.clone(),
        })
        .collect();
    let nav_tree = nav::build_nav_tree(by_type, org, config, &nav_services, schema);
    nav_tree.iter().map(nav_item_to_json).collect()
}

fn nav_item_to_json(item: &super::nav::NavItem) -> NavItemJson {
    NavItemJson {
        label: item.label.clone(),
        href: item.href.clone(),
        children: item.children.iter().map(nav_item_to_json).collect(),
    }
}

pub fn build_search_json(
    docs: &[(String, Document)],
    org: Option<&OrgConfig>,
    services: &[ServiceJson],
) -> Vec<SearchEntryJson> {
    let mut entries: Vec<SearchEntryJson> = docs
        .iter()
        .map(|(id, doc)| {
            let title = doc
                .frontmatter
                .as_ref()
                .and_then(|fm| fm.get_display("title"))
                .or_else(|| doc.title())
                .unwrap_or_else(|| id.clone());
            let doc_type = doc
                .frontmatter
                .as_ref()
                .and_then(|fm| fm.get_display("type"))
                .or_else(|| id.split('-').next().map(|s| s.to_lowercase()))
                .unwrap_or_else(|| "other".to_string())
                .to_lowercase();
            let body: String = doc.body.chars().take(500).collect();

            SearchEntryJson {
                id: id.clone(),
                title,
                body,
                doc_type,
                subtitle: None,
                href: None,
            }
        })
        .collect();

    // Add org users, teams, entities
    if let Some(org) = org {
        for (handle, user) in &org.users {
            let name = user.name.as_deref().unwrap_or(handle);
            let mut parts: Vec<String> = Vec::new();
            if let Some(title) = &user.title {
                parts.push(title.clone());
            }
            if !user.teams.is_empty() {
                parts.push(user.teams.join(", "));
            }
            let subtitle = parts.join(" · ");

            entries.push(SearchEntryJson {
                id: format!("@{handle}"),
                title: name.to_string(),
                body: subtitle.clone(),
                doc_type: "user".to_string(),
                subtitle: if subtitle.is_empty() {
                    None
                } else {
                    Some(subtitle)
                },
                href: Some(format!("/org/users/{handle}")),
            });
        }

        for (id, team) in &org.teams {
            let name = team.name.as_deref().unwrap_or(id);
            let member_count = org.users.values().filter(|u| u.teams.contains(id)).count();
            let subtitle_parts: Vec<String> = [
                Some(format!(
                    "{member_count} {}",
                    if member_count == 1 {
                        "member"
                    } else {
                        "members"
                    }
                )),
                team.lead.as_ref().map(|l| format!("Lead: @{l}")),
            ]
            .into_iter()
            .flatten()
            .collect();
            let subtitle = subtitle_parts.join(" · ");

            entries.push(SearchEntryJson {
                id: id.clone(),
                title: name.to_string(),
                body: subtitle.clone(),
                doc_type: "team".to_string(),
                subtitle: Some(subtitle),
                href: Some(format!("/org/teams/{id}")),
            });
        }

        for (id, org_def) in &org.orgs {
            let name = org_def.name.as_deref().unwrap_or(id);
            let subtitle = org_def.parent.as_ref().map(|p| format!("Part of {p}"));

            entries.push(SearchEntryJson {
                id: id.clone(),
                title: name.to_string(),
                body: subtitle.clone().unwrap_or_default(),
                doc_type: "entity".to_string(),
                subtitle,
                href: Some(format!("/org/{id}")),
            });
        }
    }

    // Add services
    for svc in services {
        let mut subtitle_parts: Vec<String> = Vec::new();
        if !svc.status.is_empty() {
            subtitle_parts.push(svc.status.clone());
        }
        if !svc.owner.is_empty() {
            subtitle_parts.push(format!("Owner: {}", svc.owner));
        }
        let subtitle = if subtitle_parts.is_empty() {
            None
        } else {
            Some(subtitle_parts.join(" · "))
        };

        entries.push(SearchEntryJson {
            id: svc.slug.clone(),
            title: svc.name.clone(),
            body: svc.description.chars().take(300).collect(),
            doc_type: "service".to_string(),
            subtitle,
            href: Some(format!("/services/{}", svc.slug)),
        });
    }

    entries
}

pub fn build_roadmap_json(roadmap_html: Option<&str>, generated_at: Option<&str>) -> RoadmapJson {
    RoadmapJson {
        html: roadmap_html.map(|s| s.to_string()),
        generated_at: generated_at.map(|s| s.to_string()),
    }
}

/// Build schema.json: type metadata, status enums with descriptions, other enum fields.
pub fn build_schema_json(schema: &Schema) -> SchemaJson {
    use crate::schema::FieldType;

    let mut types = BTreeMap::new();

    for td in &schema.types {
        // Skip singletons (service-readme, etc.)
        if td.singleton {
            continue;
        }

        let mut statuses = Vec::new();
        let mut fields = BTreeMap::new();

        for field in &td.fields {
            if let FieldType::Enum(ref vals) = field.field_type {
                let enum_vals: Vec<SchemaEnumValueJson> = vals
                    .iter()
                    .map(|v| SchemaEnumValueJson {
                        name: v.clone(),
                        description: None,
                        transitions: Vec::new(),
                    })
                    .collect();

                if field.name == "status" {
                    // Use KDL-defined transitions if present.
                    // No fallback — showing incorrect transitions is worse than none.
                    statuses = vals
                        .iter()
                        .map(|v| {
                            let transitions = field
                                .transitions
                                .iter()
                                .find(|(from, _)| from == v)
                                .map(|(_, to)| to.clone())
                                .unwrap_or_default();
                            SchemaEnumValueJson {
                                name: v.clone(),
                                description: None,
                                transitions,
                            }
                        })
                        .collect();
                } else {
                    fields.insert(
                        field.name.clone(),
                        SchemaFieldJson {
                            description: field.description.clone(),
                            values: enum_vals,
                        },
                    );
                }
            }
        }

        types.insert(
            td.name.clone(),
            SchemaTypeJson {
                description: td.description.clone(),
                statuses,
                fields,
            },
        );
    }

    SchemaJson { types }
}

/// Build assignments.json: extract user references from service READMEs,
/// frontmatter user fields, and table user columns.
pub fn build_assignments_json(
    docs: &[(String, Document)],
    schema: &Schema,
    dir: &Path,
) -> AssignmentsJson {
    use crate::schema::FieldType;

    let mut map: BTreeMap<String, Vec<AssignmentJson>> = BTreeMap::new();

    let mut insert = |handle: &str, assignment: AssignmentJson| {
        let h = handle.strip_prefix('@').unwrap_or(handle).to_lowercase();
        if !h.is_empty() {
            map.entry(h).or_default().push(assignment);
        }
    };

    // 1. Service README owners
    if let Ok(readmes) = crate::service::discover_service_readmes(dir) {
        for readme_path in &readmes {
            if let Ok(meta) = crate::service::extract_service_metadata(readme_path, dir) {
                if meta.owner != "Unknown" {
                    insert(
                        &meta.owner,
                        AssignmentJson {
                            doc_id: meta.readme_path.clone(),
                            doc_type: "service".to_string(),
                            doc_title: meta.name.clone(),
                            role: "service_owner".to_string(),
                            description: Some(meta.name),
                            status: Some(meta.status),
                            due_date: None,
                            section: None,
                        },
                    );
                }
            }
        }
    }

    // Also check apps/
    if let Ok(readmes) = crate::service::discover_app_readmes(dir) {
        for readme_path in &readmes {
            if let Ok(meta) = crate::service::extract_service_metadata(readme_path, dir) {
                if meta.owner != "Unknown" {
                    insert(
                        &meta.owner,
                        AssignmentJson {
                            doc_id: meta.readme_path.clone(),
                            doc_type: "app".to_string(),
                            doc_title: meta.name.clone(),
                            role: "service_owner".to_string(),
                            description: Some(meta.name),
                            status: Some(meta.status),
                            due_date: None,
                            section: None,
                        },
                    );
                }
            }
        }
    }

    // Also check infra/
    if let Ok(readmes) = crate::service::discover_infra_readmes(dir) {
        for readme_path in &readmes {
            if let Ok(meta) = crate::service::extract_service_metadata(readme_path, dir) {
                if meta.owner != "Unknown" {
                    insert(
                        &meta.owner,
                        AssignmentJson {
                            doc_id: meta.readme_path.clone(),
                            doc_type: "infra".to_string(),
                            doc_title: meta.name.clone(),
                            role: "service_owner".to_string(),
                            description: Some(meta.name),
                            status: Some(meta.status),
                            due_date: None,
                            section: None,
                        },
                    );
                }
            }
        }
    }

    // 2+3. For each document, extract frontmatter user fields + table user columns
    for (id, doc) in docs {
        let fm = match doc.frontmatter.as_ref() {
            Some(f) => f,
            None => continue,
        };

        let doc_type = fm
            .get_display("type")
            .or_else(|| id.split('-').next().map(|s| s.to_lowercase()))
            .unwrap_or_else(|| "other".to_string())
            .to_lowercase();
        let doc_title = fm
            .get_display("title")
            .or_else(|| doc.title())
            .unwrap_or_else(|| id.clone());
        let doc_status = fm.get_display("status");

        // Find matching TypeDef
        let type_def = schema.get_type(&doc_type);

        // 2. Frontmatter user fields
        if let Some(td) = type_def {
            for field in &td.fields {
                match &field.field_type {
                    FieldType::User => {
                        if let Some(val) = fm.get_display(&field.name) {
                            insert(
                                &val,
                                AssignmentJson {
                                    doc_id: id.clone(),
                                    doc_type: doc_type.clone(),
                                    doc_title: doc_title.clone(),
                                    role: field.name.clone(),
                                    description: None,
                                    status: doc_status.clone(),
                                    due_date: None,
                                    section: None,
                                },
                            );
                        }
                    }
                    FieldType::UserArray => {
                        if let Some(serde_yaml::Value::Sequence(seq)) = fm.get(&field.name) {
                            for item in seq {
                                if let Some(handle) = item.as_str() {
                                    insert(
                                        handle,
                                        AssignmentJson {
                                            doc_id: id.clone(),
                                            doc_type: doc_type.clone(),
                                            doc_title: doc_title.clone(),
                                            role: field.name.clone(),
                                            description: None,
                                            status: doc_status.clone(),
                                            due_date: None,
                                            section: None,
                                        },
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }

            // 3. Table user columns from sections
            let parsed = doc.parse_body();
            extract_table_assignments(
                td,
                &td.sections,
                &parsed.sections,
                id,
                &doc_type,
                &doc_title,
                &mut insert,
            );

            // Also check rule-injected section tables
            if let Some(status_val) = doc_status.as_deref() {
                for rule in &td.rules {
                    if rule.matches(status_val) {
                        for override_def in &rule.then_section_table {
                            if let Some(ps) = parsed.find_section(&override_def.section) {
                                extract_table_user_columns_from_section(
                                    &override_def.table.columns,
                                    ps,
                                    id,
                                    &doc_type,
                                    &doc_title,
                                    &override_def.section,
                                    &mut insert,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    AssignmentsJson { users: map }
}

/// Recursively extract user assignments from schema section definitions
/// matched against parsed body sections.
fn extract_table_assignments(
    _td: &crate::schema::TypeDef,
    section_defs: &[crate::schema::SectionDef],
    parsed_sections: &[crate::document::ParsedSection],
    doc_id: &str,
    doc_type: &str,
    doc_title: &str,
    insert: &mut impl FnMut(&str, AssignmentJson),
) {
    for sec_def in section_defs {
        // Find matching parsed section (case-insensitive)
        let target = sec_def.name.trim().to_lowercase();
        let ps = parsed_sections
            .iter()
            .find(|s| s.heading.trim().to_lowercase() == target);

        if let Some(ps) = ps {
            // Check if this section has a table definition with user columns
            if let Some(ref table_def) = sec_def.table {
                extract_table_user_columns_from_section(
                    &table_def.columns,
                    ps,
                    doc_id,
                    doc_type,
                    doc_title,
                    &sec_def.name,
                    insert,
                );
            }

            // Recurse into children
            if !sec_def.children.is_empty() {
                extract_table_assignments(
                    _td,
                    &sec_def.children,
                    &ps.children,
                    doc_id,
                    doc_type,
                    doc_title,
                    insert,
                );
            }
        }
    }
}

/// Extract user handles from table rows where schema defines user-typed columns.
fn extract_table_user_columns_from_section(
    columns: &[crate::schema::ColumnDef],
    ps: &crate::document::ParsedSection,
    doc_id: &str,
    doc_type: &str,
    doc_title: &str,
    section_name: &str,
    insert: &mut impl FnMut(&str, AssignmentJson),
) {
    use crate::schema::FieldType;

    // Find user-typed column indices from schema
    let user_cols: Vec<&str> = columns
        .iter()
        .filter(|c| c.col_type == FieldType::User)
        .map(|c| c.name.as_str())
        .collect();

    if user_cols.is_empty() {
        return;
    }

    // Find the primary text column for description (first non-user, non-status,
    // non-date string col — i.e. the "Action" or "Requirement" column).
    let desc_col = columns
        .iter()
        .find(|c| {
            if c.col_type != FieldType::String || user_cols.contains(&c.name.as_str()) {
                return false;
            }
            let l = c.name.to_lowercase();
            l != "status" && !l.contains("date") && !l.contains("due")
        })
        .map(|c| c.name.as_str());

    // Find status and date columns by name heuristic
    let status_col = columns
        .iter()
        .find(|c| c.name.to_lowercase() == "status")
        .map(|c| c.name.as_str());
    let date_col = columns
        .iter()
        .find(|c| {
            let l = c.name.to_lowercase();
            l.contains("date") || l.contains("due")
        })
        .map(|c| c.name.as_str());

    for table in &ps.tables {
        for (row_idx, _row) in table.rows().iter().enumerate() {
            for &user_col in &user_cols {
                if let Some(handle) = table.get_cell(user_col, row_idx) {
                    let handle = handle.trim();
                    if handle.is_empty() || handle == "-" || handle == "TBD" {
                        continue;
                    }
                    let description = desc_col
                        .and_then(|c| table.get_cell(c, row_idx))
                        .map(|s| s.to_string())
                        .filter(|s| !s.is_empty());
                    let status = status_col
                        .and_then(|c| table.get_cell(c, row_idx))
                        .map(|s| s.to_string())
                        .filter(|s| !s.is_empty());
                    let due_date = date_col
                        .and_then(|c| table.get_cell(c, row_idx))
                        .map(|s| s.to_string())
                        .filter(|s| !s.is_empty());

                    insert(
                        handle,
                        AssignmentJson {
                            doc_id: doc_id.to_string(),
                            doc_type: doc_type.to_string(),
                            doc_title: doc_title.to_string(),
                            role: format!(
                                "table_{}",
                                section_name.to_lowercase().replace(' ', "_")
                            ),
                            description,
                            status,
                            due_date,
                            section: Some(section_name.to_string()),
                        },
                    );
                }
            }
        }
    }
}

/// Strip H1, Status, and Owner sections from a service README body.
/// Keeps everything from the first content section onward (Architecture, Dependencies, etc.).
fn strip_service_header_sections(body: &str) -> String {
    let skip_headings: &[&str] = &["status", "owner"];
    let mut result = Vec::new();
    let mut skip = false;
    let mut past_header = false;
    let mut in_code_fence = false;

    for line in body.lines() {
        // Track code fence state — don't interpret headings inside fences
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_code_fence = !in_code_fence;
        }

        if in_code_fence {
            if !skip && past_header {
                result.push(line);
            }
            continue;
        }

        // Skip H1 headings (the service name)
        if line.starts_with("# ") && !line.starts_with("## ") {
            skip = true;
            continue;
        }

        // Check for H2 headings
        if line.starts_with("## ") {
            let heading = line.trim_start_matches('#').trim().to_lowercase();
            if skip_headings.contains(&heading.as_str()) {
                skip = true;
                continue;
            }
            // First non-skipped H2 — we're past the header
            skip = false;
            past_header = true;
        }

        if skip {
            continue;
        }

        // Skip preamble text before any heading (description already extracted)
        if !past_header && !line.starts_with('#') {
            continue;
        }

        result.push(line);
    }

    result.join("\n")
}

#[cfg(feature = "avatars")]
fn today_str() -> String {
    // Same civil-date-from-epoch approach used in template.rs
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (secs / 86400) as i64;
    // Algorithm from https://howardhinnant.github.io/date_algorithms.html
    let z = days + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn build_services_json(project_dir: &Path, org: Option<&OrgConfig>) -> ServicesJson {
    let mut services = Vec::new();

    // Collect known team IDs for owner resolution
    let team_ids: std::collections::HashSet<&str> = org
        .map(|o| o.teams.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();

    // Detect submodule URLs for source links
    #[cfg(feature = "git")]
    let submodule_urls = crate::code_refs::detect_submodule_urls(project_dir);
    #[cfg(not(feature = "git"))]
    let submodule_urls: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();

    // Discover services, apps, and infra — tagged by kind
    let mut tagged_readmes: Vec<(PathBuf, &str)> = Vec::new();
    if let Ok(svc) = crate::service::discover_service_readmes(project_dir) {
        tagged_readmes.extend(svc.into_iter().map(|p| (p, "service")));
    }
    if let Ok(apps) = crate::service::discover_app_readmes(project_dir) {
        tagged_readmes.extend(apps.into_iter().map(|p| (p, "app")));
    }
    if let Ok(infra) = crate::service::discover_infra_readmes(project_dir) {
        tagged_readmes.extend(infra.into_iter().map(|p| (p, "infra")));
    }

    for (readme_path, kind) in &tagged_readmes {
        let meta = match crate::service::extract_service_metadata(readme_path, project_dir) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let slug = readme_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let raw_owner = meta
            .owner
            .strip_prefix('@')
            .unwrap_or(&meta.owner)
            .to_string();

        // Resolve: if the owner handle matches a team ID, it's a team owner
        let (owner, owner_team) = if team_ids.contains(raw_owner.as_str()) {
            (raw_owner.clone(), Some(raw_owner))
        } else {
            (raw_owner, None)
        };

        let languages: Vec<ServiceLanguageJson> = meta
            .tech_stack
            .languages
            .iter()
            .map(|l| ServiceLanguageJson {
                name: l.name.clone(),
                percentage: l.percentage,
            })
            .collect();

        // Render README body to HTML, stripping H1/Status/Owner sections
        let body_html = match crate::document::Document::from_file(readme_path) {
            Ok(doc) => {
                let body = strip_service_header_sections(&doc.body);
                render_markdown_to_html(&body)
            }
            Err(_) => String::new(),
        };

        // Extract dev commands from README
        let dev_cmds = crate::service::extract_dev_commands(readme_path);
        let dev_commands = if dev_cmds.has_any() {
            Some(DevCommandsJson {
                setup: dev_cmds.setup,
                build: dev_cmds.build,
                test: dev_cmds.test,
                run: dev_cmds.run,
                lint: dev_cmds.lint,
            })
        } else {
            None
        };

        #[cfg(feature = "avatars")]
        let eol_warnings = {
            let cache_dir = project_dir.join(".dg").join("cache");
            let today = today_str();
            crate::eol::check_service_eol(&meta.tech_stack, &cache_dir, &today)
        };

        // Resolve source URL: check if service dir is a git submodule
        let source_url = readme_path
            .parent()
            .and_then(|svc_dir| svc_dir.strip_prefix(project_dir).ok())
            .and_then(|rel| rel.to_str())
            .and_then(|rel_str| {
                submodule_urls
                    .get(rel_str)
                    .map(|(web_url, _branch)| web_url.clone())
            });

        services.push(ServiceJson {
            slug,
            name: meta.name,
            kind: (*kind).to_string(),
            status: meta.status,
            owner,
            owner_team,
            description: meta.description,
            readme_path: meta.readme_path,
            body_html,
            primary_language: meta.tech_stack.primary_language,
            languages,
            frameworks: meta.tech_stack.frameworks,
            framework_versions: meta.tech_stack.framework_versions,
            deployment_platform: meta.tech_stack.deployment.map(|d| d.platform),
            database: meta.tech_stack.database,
            lines_of_code: meta.tech_stack.lines_of_code,
            dependencies_count: meta.tech_stack.dependencies_count,
            repo_size: meta.tech_stack.repo_size,
            language_version: meta.tech_stack.language_version,
            created_at: meta.created_at,
            commit_count: meta.commit_count,
            last_commit_at: meta.last_commit_at,
            has_linter: meta.practices.has_linter,
            linter_tool: meta.practices.linter_tool,
            has_tests: meta.practices.has_tests,
            test_framework: meta.practices.test_framework,
            dev_commands,
            source_url,
            #[cfg(feature = "avatars")]
            eol_warnings,
        });
    }

    let devicon_urls = crate::devicons::build_cdn_url_map();

    ServicesJson {
        services,
        devicon_urls,
    }
}

/// Write all data JSON files to `output_dir/data/`.
#[allow(clippy::too_many_arguments)]
pub fn generate_data_files(
    output_dir: &Path,
    docs: &[(String, Document)],
    graph: &DocGraph,
    org: Option<&OrgConfig>,
    schema: &Schema,
    by_type: &BTreeMap<String, Vec<(String, &Document)>>,
    config: &super::SiteConfig,
    roadmap_html: Option<&str>,
    avatar_map: &std::collections::HashMap<String, String>,
    project_dir: &Path,
) -> crate::error::Result<usize> {
    let data_dir = output_dir.join("data");
    std::fs::create_dir_all(&data_dir)
        .map_err(|_| crate::error::Error::WriteFailed(data_dir.clone()))?;

    // Build backlinks map
    let mut id_to_title: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for (id, doc) in docs {
        let title = doc
            .frontmatter
            .as_ref()
            .and_then(|fm| fm.get_display("title"))
            .or_else(|| doc.title())
            .unwrap_or_else(|| id.clone());
        id_to_title.insert(id.clone(), title);
    }

    let mut backlinks_map: BTreeMap<String, Vec<(String, String, String)>> = BTreeMap::new();
    let mut seen_backlinks: std::collections::HashSet<(String, String, String)> =
        std::collections::HashSet::new();
    for edge in &graph.edges {
        let key = (edge.to.clone(), edge.from.clone(), edge.relation.clone());
        if !seen_backlinks.insert(key) {
            continue;
        }
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

    let mut count = 0;

    // docs.json
    let docs_data = build_docs_json(docs, &backlinks_map, schema, project_dir);
    write_json(&data_dir.join("docs.json"), &docs_data)?;
    count += 1;

    // graph.json
    let graph_data = build_graph_json(graph, schema);
    write_json(&data_dir.join("graph.json"), &graph_data)?;
    count += 1;

    // org.json
    let org_data = build_org_json(org, avatar_map, project_dir);
    write_json(&data_dir.join("org.json"), &org_data)?;
    count += 1;

    // services.json (built before nav+search so they can reference service data)
    let services_data = build_services_json(project_dir, org);
    write_json(&data_dir.join("services.json"), &services_data)?;
    count += 1;

    // nav.json
    let nav_data = build_nav_json(by_type, org, config, &services_data.services, schema);
    write_json(&data_dir.join("nav.json"), &nav_data)?;
    count += 1;

    // search-index.json
    let search_data = build_search_json(docs, org, &services_data.services);
    write_json(&data_dir.join("search-index.json"), &search_data)?;
    count += 1;

    // roadmap.json
    let roadmap_data = build_roadmap_json(roadmap_html, config.roadmap_generated_at.as_deref());
    write_json(&data_dir.join("roadmap.json"), &roadmap_data)?;
    count += 1;

    // Copy logo file if configured
    let logo_url = if let Some(logo_path) = &config.logo_path {
        if logo_path.is_file() {
            let ext = logo_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("svg");
            let dest = data_dir.join(format!("logo.{ext}"));
            std::fs::copy(logo_path, &dest)
                .map_err(|_| crate::error::Error::WriteFailed(dest.clone()))?;
            Some(format!("data/logo.{ext}"))
        } else {
            None
        }
    } else {
        None
    };

    // site-meta.json
    let jira_configs: Vec<JiraConfigJson> = org
        .map(|o| {
            o.jira
                .iter()
                .map(|j| JiraConfigJson {
                    prefix: j.prefix.clone(),
                    url: j.url.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    // Detect README source path
    let readme_source = if config.readme_html.is_some() {
        Some("README.md".to_string())
    } else {
        None
    };

    let has_claude_md = project_dir.join("CLAUDE.md").is_file();
    let has_submodules = project_dir.join(".gitmodules").is_file();
    // Derive clone URL from edit_url_prefix by stripping /-/edit/<branch>/ or /edit/<branch>/
    let clone_url = config.edit_url_prefix.as_deref().and_then(|prefix| {
        let trimmed = prefix.trim_end_matches('/');
        // GitLab: "https://gitlab.example.com/org/repo/-/edit/master" → strip "/-/edit/master"
        if let Some(pos) = trimmed.find("/-/edit/") {
            return Some(format!("{}.git", &trimmed[..pos]));
        }
        // GitHub: "https://github.com/org/repo/edit/main" → strip "/edit/main"
        if let Some(pos) = trimmed.find("/edit/") {
            return Some(format!("{}.git", &trimmed[..pos]));
        }
        None
    });

    let meta = SiteMetaJson {
        title: config.title.clone(),
        readme_html: None, // served separately in readme.json for faster initial load
        logo_url,
        jira: jira_configs,
        edit_url_prefix: config.edit_url_prefix.clone(),
        is_local_dev: config.is_local_dev,
        source_path: readme_source.clone(),
        clone_url,
        has_claude_md,
        has_submodules,
    };
    write_json(&data_dir.join("site-meta.json"), &meta)?;
    count += 1;

    // readme.json — separate file so site-meta.json stays tiny for fast header render
    let readme_data = ReadmeJson {
        html: config.readme_html.clone(),
        source_path: readme_source,
    };
    write_json(&data_dir.join("readme.json"), &readme_data)?;
    count += 1;

    // assignments.json
    let assignments_data = build_assignments_json(docs, schema, project_dir);
    write_json(&data_dir.join("assignments.json"), &assignments_data)?;
    count += 1;

    // code-refs.json
    let code_refs_data = build_code_refs_json(project_dir, schema);
    write_json(&data_dir.join("code-refs.json"), &code_refs_data)?;
    count += 1;

    // schema.json
    let schema_data = build_schema_json(schema);
    write_json(&data_dir.join("schema.json"), &schema_data)?;
    count += 1;

    Ok(count)
}

// ── code-refs.json ──────────────────────────────────────────────────

#[derive(Serialize)]
struct CodeRefsJson {
    refs: BTreeMap<String, CodeRefsEntryJson>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_url_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_url_prefix: Option<String>,
}

#[derive(Serialize)]
struct CodeRefsEntryJson {
    code: Vec<CodeRefItemJson>,
    commits: Vec<CommitRefItemJson>,
}

#[derive(Serialize)]
struct CodeRefItemJson {
    file: String,
    line: usize,
    text: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    context_before: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    context_after: Vec<String>,
    /// Full URL override for files inside git submodules.
    #[serde(skip_serializing_if = "Option::is_none")]
    file_url: Option<String>,
}

#[derive(Serialize)]
struct CommitRefItemJson {
    sha: String,
    subject: String,
    date: String,
    author: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body_context: Option<String>,
}

fn build_code_refs_json(project_dir: &Path, schema: &Schema) -> CodeRefsJson {
    let cache_path = project_dir.join(".dg").join("cache").join("code-refs.json");
    let mut cache = crate::code_refs::CodeRefCache::load(&cache_path);

    // Always run incremental scans — they're fast no-ops when nothing changed
    crate::code_refs::scan_code_refs(project_dir, schema, &mut cache);
    crate::code_refs::scan_commit_refs(project_dir, schema, &mut cache);
    if cache.is_dirty() {
        let _ = cache.save(&cache_path);
    }

    // Detect git remote for commit/file links
    let (commit_url_prefix, file_url_prefix) =
        match crate::code_refs::detect_repo_web_url(project_dir) {
            Some((base_url, branch)) => {
                let is_gitlab = base_url.contains("gitlab");
                if is_gitlab {
                    (
                        Some(format!("{base_url}/-/commit/")),
                        Some(format!("{base_url}/-/blob/{branch}/")),
                    )
                } else {
                    (
                        Some(format!("{base_url}/commit/")),
                        Some(format!("{base_url}/blob/{branch}/")),
                    )
                }
            }
            None => (None, None),
        };

    // Detect submodule URLs for resolving file links
    let submodules = crate::code_refs::detect_submodule_urls(project_dir);

    let refs = cache
        .index
        .iter()
        .map(|(doc_id, doc_refs)| {
            let code = doc_refs
                .code
                .iter()
                .map(|r| {
                    // Check if file is inside a git submodule
                    let file_url = submodules
                        .iter()
                        .find_map(|(sm_path, (sm_url, sm_branch))| {
                            r.file.strip_prefix(sm_path).map(|rest| {
                                let rest = rest.strip_prefix('/').unwrap_or(rest);
                                let is_gitlab = sm_url.contains("gitlab");
                                let blob = if is_gitlab { "/-/blob/" } else { "/blob/" };
                                format!("{sm_url}{blob}{sm_branch}/{rest}")
                            })
                        });
                    CodeRefItemJson {
                        file: r.file.clone(),
                        line: r.line,
                        text: r.text.clone(),
                        context_before: r.context_before.clone(),
                        context_after: r.context_after.clone(),
                        file_url,
                    }
                })
                .collect();
            let commits = doc_refs
                .commits
                .iter()
                .map(|c| CommitRefItemJson {
                    sha: c.sha.clone(),
                    subject: c.subject.clone(),
                    date: c.date.clone(),
                    author: c.author.clone(),
                    body_context: c.body_context.clone(),
                })
                .collect();
            (doc_id.clone(), CodeRefsEntryJson { code, commits })
        })
        .collect();

    CodeRefsJson {
        refs,
        commit_url_prefix,
        file_url_prefix,
    }
}

fn write_json<T: Serialize>(path: &Path, data: &T) -> crate::error::Result<()> {
    let json = serde_json::to_string(data)?;
    crate::site::write_atomic(path, json.as_bytes())
}

/// Extract string list from a YAML value (scalar → 1-item list, sequence → list).
pub(crate) fn yaml_to_string_list(val: &serde_yaml::Value) -> Vec<String> {
    match val {
        serde_yaml::Value::String(s) => {
            if s.is_empty() {
                vec![]
            } else {
                vec![s.clone()]
            }
        }
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        serde_yaml::Value::Null => vec![],
        _ => vec![crate::frontmatter::yaml_value_to_string(val)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
    }

    fn load_schema() -> Schema {
        Schema::from_file(fixtures_dir().join("schema.kdl")).unwrap()
    }

    #[test]
    fn schema_json_has_status_transitions() {
        let schema = load_schema();
        let json = build_schema_json(&schema);

        // Every non-singleton type with a status field should have transitions
        for (type_name, type_json) in &json.types {
            if type_json.statuses.is_empty() {
                continue;
            }

            // At least one status should have transitions (all except the last)
            let with_transitions: Vec<_> = type_json
                .statuses
                .iter()
                .filter(|s| !s.transitions.is_empty())
                .collect();

            assert!(
                !with_transitions.is_empty(),
                "type '{type_name}' has statuses but no transitions — LifecycleFlow won't render"
            );

            // Last status should be terminal (no transitions)
            let last = type_json.statuses.last().unwrap();
            assert!(
                last.transitions.is_empty(),
                "type '{type_name}': last status '{}' should be terminal",
                last.name
            );
        }
    }

    #[test]
    fn schema_json_inc_status_transitions() {
        let schema = load_schema();
        let json = build_schema_json(&schema);
        let inc = &json.types["inc"];

        // INC statuses: open → mitigated → resolved
        assert_eq!(inc.statuses.len(), 3);
        assert_eq!(inc.statuses[0].name, "open");
        assert_eq!(inc.statuses[1].name, "mitigated");
        assert_eq!(inc.statuses[2].name, "resolved");
        assert!(inc.statuses[2].transitions.is_empty());
    }

    #[test]
    fn schema_json_adr_transitions_are_branching() {
        let schema = load_schema();
        let json = build_schema_json(&schema);
        let adr = &json.types["adr"];

        // ADR: proposed → accepted, rejected  (branching, NOT linear)
        //      accepted → deprecated, superseded
        let proposed = adr.statuses.iter().find(|s| s.name == "proposed").unwrap();
        assert!(
            proposed.transitions.contains(&"accepted".to_string()),
            "proposed should transition to accepted"
        );
        assert!(
            proposed.transitions.contains(&"rejected".to_string()),
            "proposed should transition to rejected"
        );
        assert_eq!(
            proposed.transitions.len(),
            2,
            "proposed should have exactly 2 transitions, got: {:?}",
            proposed.transitions
        );

        let accepted = adr.statuses.iter().find(|s| s.name == "accepted").unwrap();
        assert!(
            accepted.transitions.contains(&"deprecated".to_string()),
            "accepted should transition to deprecated"
        );
        assert!(
            accepted.transitions.contains(&"superseded".to_string()),
            "accepted should transition to superseded"
        );
        assert_eq!(
            accepted.transitions.len(),
            2,
            "accepted should have exactly 2 transitions, got: {:?}",
            accepted.transitions
        );

        // Terminal statuses: rejected, deprecated, superseded should have no transitions
        for terminal in &["rejected", "deprecated", "superseded"] {
            let s = adr.statuses.iter().find(|s| s.name == *terminal).unwrap();
            assert!(
                s.transitions.is_empty(),
                "'{terminal}' should be terminal (no outgoing transitions), got: {:?}",
                s.transitions
            );
        }
    }

    #[test]
    fn schema_json_all_types_present() {
        let schema = load_schema();
        let json = build_schema_json(&schema);

        assert!(json.types.contains_key("adr"), "missing adr type");
        assert!(json.types.contains_key("opp"), "missing opp type");
        assert!(json.types.contains_key("pol"), "missing pol type");
        assert!(json.types.contains_key("inc"), "missing inc type");
    }

    #[test]
    fn schema_json_statuses_not_empty() {
        let schema = load_schema();
        let json = build_schema_json(&schema);

        for type_name in &["adr", "opp", "pol", "inc"] {
            let type_json = &json.types[*type_name];
            assert!(
                !type_json.statuses.is_empty(),
                "type '{type_name}' should have statuses"
            );
        }
    }

    // ── strip_service_header_sections tests ───────────────────────────

    #[test]
    fn strip_preserves_code_blocks_with_hash_comments() {
        let body = r#"# My Service

Description paragraph.

## Deployment

Deploy instructions.

```bash
# Deploy to staging
heroku login
heroku container:login
```

## Architecture

More content here.
"#;
        let result = strip_service_header_sections(body);
        assert!(
            result.contains("# Deploy to staging"),
            "bash comment inside code fence was stripped: {result}"
        );
        assert!(
            result.contains("heroku login"),
            "code block content was stripped"
        );
        assert!(
            result.contains("## Architecture"),
            "section after code block was stripped"
        );
        assert!(
            result.contains("More content here."),
            "content after code block was stripped"
        );
    }

    #[test]
    fn strip_preserves_closed_code_fences() {
        let body = r#"# Service

Preamble.

## API

```bash
# Generate docs
bundle exec rails swagger
```

## Development

Dev setup.
"#;
        let result = strip_service_header_sections(body);
        // Count code fences — should be even (properly closed)
        let fence_count = result.matches("```").count();
        assert_eq!(
            fence_count % 2,
            0,
            "unbalanced code fences ({fence_count}): {result}"
        );
    }

    #[test]
    fn strip_removes_h1_and_preamble() {
        let body = "# Service Name\n\nDescription.\n\n## Section\n\nContent.\n";
        let result = strip_service_header_sections(body);
        assert!(!result.contains("# Service Name"));
        assert!(!result.contains("Description."));
        assert!(result.contains("## Section"));
        assert!(result.contains("Content."));
    }

    #[test]
    fn strip_removes_status_and_owner_sections() {
        let body = "# Svc\n\n## Status\n\nLive\n\n## Owner\n\n@team\n\n## Docs\n\nReal content.\n";
        let result = strip_service_header_sections(body);
        assert!(!result.contains("## Status"));
        assert!(!result.contains("## Owner"));
        assert!(result.contains("## Docs"));
        assert!(result.contains("Real content."));
    }

    #[test]
    fn strip_handles_mermaid_code_blocks() {
        let body = r#"# App

Desc.

## Architecture

```mermaid
graph TB
    A["Node A<br/>(detail)"] --> B
```

After diagram.
"#;
        let result = strip_service_header_sections(body);
        assert!(result.contains("```mermaid"));
        assert!(result.contains("```\n"));
        assert!(result.contains("After diagram."));
    }

    #[test]
    fn strip_handles_nested_code_and_headings() {
        let body = r#"# Svc

Desc.

## Setup

```bash
# This is NOT a heading
## Neither is this
### Or this
echo "hello"
```

## Real Section

Content.
"#;
        let result = strip_service_header_sections(body);
        assert!(
            result.contains("# This is NOT a heading"),
            "hash comment inside code block was stripped"
        );
        assert!(
            result.contains("## Neither is this"),
            "double-hash comment inside code block was stripped"
        );
        assert!(result.contains("## Real Section"));
        assert!(result.contains("Content."));
    }

    #[test]
    fn strip_handles_tilde_code_fences() {
        let body =
            "# Svc\n\nDesc.\n\n## Code\n\n~~~bash\n# comment\necho hi\n~~~\n\n## Next\n\nOk.\n";
        let result = strip_service_header_sections(body);
        assert!(result.contains("# comment"));
        assert!(result.contains("## Next"));
    }

    #[test]
    fn strip_multiple_code_blocks_all_closed() {
        let body = r#"# Svc

Desc.

## Deploy

```bash
# step 1
deploy-staging
```

## API

```bash
# generate docs
bundle exec rails swagger
```

## Dev

Setup.
"#;
        let result = strip_service_header_sections(body);
        let fence_count = result.matches("```").count();
        assert_eq!(
            fence_count, 4,
            "expected 4 fences (2 blocks), got {fence_count}: {result}"
        );
        assert!(result.contains("# step 1"));
        assert!(result.contains("# generate docs"));
        assert!(result.contains("## Dev"));
        assert!(result.contains("Setup."));
    }

    #[test]
    fn strip_leading_h1_matching() {
        let html = "<h1>My Title</h1>\n<h2>Section</h2>\n<p>Content</p>";
        let result = strip_leading_h1(html, "My Title");
        assert!(!result.contains("<h1>"));
        assert!(result.contains("<h2>Section</h2>"));
    }

    #[test]
    fn strip_leading_h1_non_matching() {
        let html = "<h1>Different Title</h1>\n<p>Content</p>";
        let result = strip_leading_h1(html, "My Title");
        assert!(result.contains("<h1>Different Title</h1>"));
    }

    #[test]
    fn strip_leading_h1_with_anchor() {
        // comrak renders H1 with id and anchor link inside
        let html = r##"<h1><a href="#my-title" aria-hidden="true"></a>My Title</h1>
<p>Content</p>"##;
        let result = strip_leading_h1(html, "My Title");
        assert!(!result.contains("<h1>"), "H1 should be stripped: {result}");
        assert!(result.contains("<p>Content</p>"));
    }

    #[test]
    fn strip_leading_h1_with_html_entities() {
        let html = "<h1>Migrate to Elixir &amp; Phoenix</h1>\n<p>Content</p>";
        let result = strip_leading_h1(html, "Migrate to Elixir & Phoenix");
        assert!(
            !result.contains("<h1>"),
            "H1 with &amp; should be stripped: {result}"
        );
        assert!(result.contains("<p>Content</p>"));
    }

    #[test]
    fn strip_leading_h1_preserves_non_h1_start() {
        let html = "<p>Intro</p>\n<h1>Title</h1>";
        let result = strip_leading_h1(html, "Title");
        assert_eq!(result, html, "should not strip H1 that isn't first element");
    }
}

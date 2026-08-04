use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::ast_util;
use crate::document::Document;
use crate::error::Result;
use crate::schema::Schema;

/// A structural diagnostic found during graph health checks.
#[derive(Debug, Clone)]
pub struct GraphDiagnostic {
    /// Diagnostic code: G010 (cycle), G011 (self-ref), G020 (orphan), G021 (disconnected), G030 (dangling ref)
    pub code: String,
    /// "error", "warning", or "info"
    pub severity: String,
    /// Human-readable description
    pub message: String,
}

/// A node in the document graph.
#[derive(Debug, Clone)]
pub struct DocNode {
    /// Canonical ID derived from filename (e.g. "ADR-001")
    pub id: String,
    pub path: PathBuf,
    pub doc_type: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
}

/// A directed edge (reference) between two documents.
#[derive(Debug, Clone)]
pub struct DocEdge {
    pub from: String,
    pub to: String,
    /// The relation field name (e.g. "supersedes", "enables", "related")
    pub relation: String,
}

/// The document graph built from a directory of markdown files.
#[derive(Debug)]
pub struct DocGraph {
    pub nodes: BTreeMap<String, DocNode>,
    pub edges: Vec<DocEdge>,
}

impl DocGraph {
    /// Build a graph from all markdown files in a directory.
    pub fn build(dir: impl AsRef<Path>, schema: &Schema) -> Result<Self> {
        let files = crate::discovery::discover_files(&dir, None, &[], false)?;
        let relation_names = schema.all_relation_field_names();

        let mut nodes = BTreeMap::new();
        let mut edges = Vec::new();

        for path in &files {
            let doc = match Document::from_file(path) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let id = path_to_id_with_schema(path, schema);
            let fm = match &doc.frontmatter {
                Some(fm) => fm,
                None => {
                    // Check if this is a singleton type
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let singleton_type = schema
                        .types
                        .iter()
                        .find(|t| t.singleton && t.match_pattern.as_deref() == Some(filename));
                    if let Some(type_def) = singleton_type {
                        let id = path_to_id_with_schema(path, schema);
                        nodes.insert(
                            id.clone(),
                            DocNode {
                                id: id.clone(),
                                path: path.clone(),
                                doc_type: Some(type_def.name.clone()),
                                title: None,
                                status: None,
                            },
                        );
                    }
                    continue;
                }
            };

            let doc_type = fm
                .get_display("type")
                .or_else(|| schema.type_name_for_doc_id(&id));
            // Prefer the frontmatter title: doc.title() returns the FIRST heading
            // of any level, which for docs without an H1 is a section like "Story"
            let title = fm.get_display("title").or_else(|| doc.title());
            let status = fm.get_display("status");

            nodes.insert(
                id.clone(),
                DocNode {
                    id: id.clone(),
                    path: path.clone(),
                    doc_type,
                    title,
                    status,
                },
            );

            // Extract outgoing refs from relation fields
            for rel_name in &relation_names {
                if let Some(val) = fm.get(rel_name) {
                    let refs = extract_refs(val);
                    for target in refs {
                        edges.push(DocEdge {
                            from: id.clone(),
                            to: target,
                            relation: rel_name.to_string(),
                        });
                    }
                }
            }

            // Extract inline links from document body
            let inline_links = ast_util::extract_links(&doc.body);
            let doc_dir = path.parent();
            for url in inline_links {
                let target_id = if url.ends_with(".md") {
                    // Relative .md path — resolve against doc directory
                    let link_path = if let Some(dir) = doc_dir {
                        dir.join(&url)
                    } else {
                        PathBuf::from(&url)
                    };
                    path_to_id(&link_path)
                } else if schema.is_valid_id(&url) {
                    // String ID pattern like "ADR-001"
                    url.to_uppercase()
                } else {
                    // External or unrecognized link — skip
                    continue;
                };

                // Deduplicate: skip if a frontmatter edge already exists for this pair
                let already_exists = edges.iter().any(|e| e.from == id && e.to == target_id);
                if !already_exists {
                    edges.push(DocEdge {
                        from: id.clone(),
                        to: target_id,
                        relation: "inline_ref".to_string(),
                    });
                }
            }
        }

        Ok(DocGraph { nodes, edges })
    }

    /// Build a graph using a cache for incremental parsing.
    /// Only re-parses files whose cache entries are stale.
    /// Inline link edges are omitted (requires body AST parse);
    /// only frontmatter relation edges are cached.
    pub fn build_cached(
        dir: impl AsRef<Path>,
        schema: &Schema,
        cache: &mut crate::cache::DocCache,
    ) -> Result<Self> {
        let files = crate::discovery::discover_files(&dir, None, &[], false)?;

        // Refresh stale entries
        for path in &files {
            if cache.is_stale(path) {
                // Ignore errors for individual files (same as build())
                let _ = cache.refresh_for_graph(path, schema);
            } else if cache.get(path).and_then(|e| e.doc_id.as_ref()).is_none() {
                // Entry exists but has no graph fields — needs graph refresh
                let _ = cache.refresh_for_graph(path, schema);
            }
        }

        let mut nodes = BTreeMap::new();
        let mut edges = Vec::new();

        for path in &files {
            let entry = match cache.get(path) {
                Some(e) => e,
                None => continue,
            };

            let id = match &entry.doc_id {
                Some(id) => id.clone(),
                None => path_to_id_with_schema(path, schema),
            };

            // Skip entries with no frontmatter and no singleton type
            if entry.frontmatter.is_none() && entry.doc_type.is_none() {
                continue;
            }

            nodes.insert(
                id.clone(),
                DocNode {
                    id: id.clone(),
                    path: path.clone(),
                    doc_type: entry.doc_type.clone(),
                    title: entry.title.clone(),
                    status: entry.status.clone(),
                },
            );

            for (target, relation) in &entry.edges {
                edges.push(DocEdge {
                    from: id.clone(),
                    to: target.clone(),
                    relation: relation.clone(),
                });
            }
        }

        cache.prune_missing();

        Ok(DocGraph { nodes, edges })
    }

    /// Get all outgoing refs from a document.
    pub fn refs_from(&self, id: &str) -> Vec<&DocEdge> {
        let id_upper = id.to_uppercase();
        self.edges.iter().filter(|e| e.from == id_upper).collect()
    }

    /// Get all incoming refs (backlinks) to a document.
    pub fn refs_to(&self, id: &str) -> Vec<&DocEdge> {
        let id_upper = id.to_uppercase();
        self.edges.iter().filter(|e| e.to == id_upper).collect()
    }

    /// Transitive forward refs from a document up to a depth limit.
    /// Returns (depth, edge) pairs.
    pub fn refs_from_transitive(&self, id: &str, max_depth: usize) -> Vec<(usize, &DocEdge)> {
        self.transitive_walk(id, max_depth, |g, id| g.refs_from(id), |e| &e.to)
    }

    /// Transitive backlinks to a document up to a depth limit.
    pub fn refs_to_transitive(&self, id: &str, max_depth: usize) -> Vec<(usize, &DocEdge)> {
        self.transitive_walk(id, max_depth, |g, id| g.refs_to(id), |e| &e.from)
    }

    /// Generic BFS walk collecting edges transitively.
    /// `get_edges` returns edges for a given node ID.
    /// `next_id` extracts the ID to follow from an edge.
    fn transitive_walk<'a>(
        &'a self,
        id: &str,
        max_depth: usize,
        get_edges: impl Fn(&'a Self, &str) -> Vec<&'a DocEdge>,
        next_id: impl Fn(&DocEdge) -> &String,
    ) -> Vec<(usize, &'a DocEdge)> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((id.to_uppercase(), 0usize));

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            for edge in get_edges(self, &current) {
                if visited.insert((edge.from.clone(), edge.to.clone(), edge.relation.clone())) {
                    result.push((depth + 1, edge));
                    queue.push_back((next_id(edge).clone(), depth + 1));
                }
            }
        }

        result
    }

    /// Collect node IDs matching the filter type, or all node IDs if no filter.
    fn active_ids(&self, filter_type: Option<&str>) -> HashSet<&str> {
        if let Some(ft) = filter_type {
            self.nodes
                .iter()
                .filter(|(_, n)| n.doc_type.as_deref() == Some(ft))
                .map(|(id, _)| id.as_str())
                .collect()
        } else {
            self.nodes.keys().map(|s| s.as_str()).collect()
        }
    }

    /// Export graph as mermaid diagram.
    pub fn to_mermaid(&self, filter_type: Option<&str>) -> String {
        let mut out = String::from("graph LR\n");
        let active_ids = self.active_ids(filter_type);

        // Node declarations
        for (id, node) in &self.nodes {
            if !active_ids.contains(id.as_str()) {
                continue;
            }
            let label = node.title.as_deref().unwrap_or(id.as_str());
            let shape = if node.status.as_deref() == Some("deprecated")
                || node.status.as_deref() == Some("superseded")
            {
                format!("  {id}[/\"{label}\"/]")
            } else {
                format!("  {id}[\"{label}\"]")
            };
            out.push_str(&shape);
            out.push('\n');
        }

        // Edges
        for edge in &self.edges {
            if !active_ids.contains(edge.from.as_str()) && filter_type.is_some() {
                continue;
            }
            let label = &edge.relation;
            out.push_str(&format!("  {} -->|{}| {}\n", edge.from, label, edge.to));
        }

        out
    }

    /// Export graph as DOT (graphviz) format.
    pub fn to_dot(&self, filter_type: Option<&str>) -> String {
        let mut out = String::from("digraph docs {\n  rankdir=LR;\n  node [shape=box];\n\n");
        let active_ids = self.active_ids(filter_type);

        for (id, node) in &self.nodes {
            if !active_ids.contains(id.as_str()) {
                continue;
            }
            let label = node.title.as_deref().unwrap_or(id.as_str());
            let style = if node.status.as_deref() == Some("deprecated")
                || node.status.as_deref() == Some("superseded")
            {
                " style=dashed"
            } else {
                ""
            };
            out.push_str(&format!("  \"{id}\" [label=\"{label}\"{style}];\n"));
        }

        out.push('\n');

        for edge in &self.edges {
            if !active_ids.contains(edge.from.as_str()) && filter_type.is_some() {
                continue;
            }
            out.push_str(&format!(
                "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
                edge.from, edge.to, edge.relation
            ));
        }

        out.push_str("}\n");
        out
    }

    /// Run all structural health checks and return diagnostics.
    pub fn check_health(&self, schema: &Schema) -> Vec<GraphDiagnostic> {
        let mut diags = Vec::new();
        self.check_self_references(&mut diags);
        self.check_cycles(schema, &mut diags);
        self.check_orphans(&mut diags);
        self.check_disconnected(&mut diags);
        self.check_dangling_refs(&mut diags);
        diags
    }

    /// G011: edges where from == to.
    fn check_self_references(&self, diags: &mut Vec<GraphDiagnostic>) {
        for edge in &self.edges {
            if edge.from == edge.to {
                diags.push(GraphDiagnostic {
                    code: "G011".into(),
                    severity: "warning".into(),
                    message: format!("{} has self-reference via '{}'", edge.from, edge.relation),
                });
            }
        }
    }

    /// G010: cycles in relations marked acyclic=true.
    /// Uses DFS with visited + recursion-stack per acyclic relation.
    fn check_cycles(&self, schema: &Schema, diags: &mut Vec<GraphDiagnostic>) {
        // Collect acyclic relation names (include inverse names too)
        let acyclic_names: HashSet<&str> = schema
            .relations
            .iter()
            .filter(|r| r.acyclic == Some(true))
            .flat_map(|r| {
                let mut names = vec![r.name.as_str()];
                if let Some(ref inv) = r.inverse {
                    names.push(inv.as_str());
                }
                names
            })
            .collect();

        if acyclic_names.is_empty() {
            return;
        }

        // Build adjacency list for acyclic edges only
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.edges {
            if acyclic_names.contains(edge.relation.as_str()) && edge.from != edge.to {
                adj.entry(edge.from.as_str())
                    .or_default()
                    .push(edge.to.as_str());
            }
        }

        // DFS cycle detection
        let mut visited: HashSet<&str> = HashSet::new();
        let mut rec_stack: HashSet<&str> = HashSet::new();
        let mut path: Vec<&str> = Vec::new();

        for start in self.nodes.keys() {
            if !visited.contains(start.as_str()) {
                self.dfs_cycle(
                    start.as_str(),
                    &adj,
                    &mut visited,
                    &mut rec_stack,
                    &mut path,
                    diags,
                );
            }
        }
    }

    fn dfs_cycle<'a>(
        &'a self,
        node: &'a str,
        adj: &HashMap<&'a str, Vec<&'a str>>,
        visited: &mut HashSet<&'a str>,
        rec_stack: &mut HashSet<&'a str>,
        path: &mut Vec<&'a str>,
        diags: &mut Vec<GraphDiagnostic>,
    ) {
        visited.insert(node);
        rec_stack.insert(node);
        path.push(node);

        if let Some(neighbors) = adj.get(node) {
            for &neighbor in neighbors {
                if !visited.contains(neighbor) {
                    self.dfs_cycle(neighbor, adj, visited, rec_stack, path, diags);
                } else if rec_stack.contains(neighbor) {
                    // Found cycle — extract it from path
                    let cycle_start = path.iter().position(|&n| n == neighbor).unwrap();
                    let cycle: Vec<&str> = path[cycle_start..].to_vec();
                    let cycle_str = cycle.join(" -> ");
                    diags.push(GraphDiagnostic {
                        code: "G010".into(),
                        severity: "error".into(),
                        message: format!(
                            "cycle detected in acyclic relation: {} -> {}",
                            cycle_str, neighbor
                        ),
                    });
                }
            }
        }

        path.pop();
        rec_stack.remove(node);
    }

    /// G020: nodes with zero incoming AND zero outgoing edges.
    fn check_orphans(&self, diags: &mut Vec<GraphDiagnostic>) {
        let mut has_edge: HashSet<&str> = HashSet::new();
        for edge in &self.edges {
            has_edge.insert(edge.from.as_str());
            has_edge.insert(edge.to.as_str());
        }

        for id in self.nodes.keys() {
            if !has_edge.contains(id.as_str()) {
                diags.push(GraphDiagnostic {
                    code: "G020".into(),
                    severity: "info".into(),
                    message: format!("{id} is an orphan (no incoming or outgoing edges)"),
                });
            }
        }
    }

    /// G021: more than one connected component (treating edges as undirected).
    fn check_disconnected(&self, diags: &mut Vec<GraphDiagnostic>) {
        if self.nodes.is_empty() {
            return;
        }

        // Build undirected adjacency
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for edge in &self.edges {
            adj.entry(edge.from.as_str())
                .or_default()
                .push(edge.to.as_str());
            adj.entry(edge.to.as_str())
                .or_default()
                .push(edge.from.as_str());
        }

        let mut visited: HashSet<&str> = HashSet::new();
        let mut components: Vec<Vec<&str>> = Vec::new();

        for id in self.nodes.keys() {
            if visited.contains(id.as_str()) {
                continue;
            }
            // BFS from this node
            let mut component = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(id.as_str());
            visited.insert(id.as_str());

            while let Some(current) = queue.pop_front() {
                component.push(current);
                if let Some(neighbors) = adj.get(current) {
                    for &n in neighbors {
                        if self.nodes.contains_key(n) && visited.insert(n) {
                            queue.push_back(n);
                        }
                    }
                }
            }

            components.push(component);
        }

        if components.len() > 1 {
            let summary: Vec<String> = components
                .iter()
                .map(|c| {
                    if c.len() <= 3 {
                        c.join(", ")
                    } else {
                        format!("{}, ... ({} nodes)", c[..2].join(", "), c.len())
                    }
                })
                .collect();
            diags.push(GraphDiagnostic {
                code: "G021".into(),
                severity: "warning".into(),
                message: format!(
                    "graph has {} disconnected components: [{}]",
                    components.len(),
                    summary.join("] [")
                ),
            });
        }
    }

    /// G030: edges pointing to nodes that don't exist in the graph.
    fn check_dangling_refs(&self, diags: &mut Vec<GraphDiagnostic>) {
        for edge in &self.edges {
            if !self.nodes.contains_key(&edge.to) {
                diags.push(GraphDiagnostic {
                    code: "G030".into(),
                    severity: "error".into(),
                    message: format!(
                        "{} references unknown document {} via '{}'",
                        edge.from, edge.to, edge.relation
                    ),
                });
            }
        }
    }

    /// Find next available numeric ID for a type prefix (e.g. "ADR" → "ADR-005").
    pub fn next_id(&self, prefix: &str) -> String {
        let prefix_upper = prefix.to_uppercase();
        let max = self
            .nodes
            .keys()
            .filter_map(|id| {
                let parts: Vec<&str> = id.splitn(2, '-').collect();
                if parts.len() == 2 && parts[0] == prefix_upper {
                    parts[1].parse::<u32>().ok()
                } else {
                    None
                }
            })
            .max()
            .unwrap_or(0);

        format!("{}-{:03}", prefix_upper, max + 1)
    }
}

/// Derive a document ID from its file path.
/// Extracts the type-prefix + number from the filename:
///   `docs/adr-001.md` → `ADR-001`
///   `docs/adr-001-start-using-postgresql.md` → `ADR-001`
///   `docs/inc_002.md` → `INC-002`
pub fn path_to_id(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_uppercase()
        .replace('_', "-");

    extract_id_from_stem(&stem).unwrap_or(stem)
}

/// Derive a document ID using the schema's ref-format pattern for validation.
/// Falls back to `path_to_id()` behavior when the schema has no `string-id` pattern.
pub fn path_to_id_with_schema(path: &Path, schema: &crate::schema::Schema) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    schema
        .parse_id_from_filename(stem)
        .unwrap_or_else(|| path_to_id(path))
}

/// Extract PREFIX-NNN from an uppercased, underscore-normalized stem.
/// Returns None if the stem doesn't match the pattern.
pub(crate) fn extract_id_from_stem(stem: &str) -> Option<String> {
    let bytes = stem.as_bytes();
    let mut i = 0;
    // Skip alpha prefix
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    // Expect dash
    if i < bytes.len() && bytes[i] == b'-' {
        i += 1;
        let num_start = i;
        // Consume digits
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i > num_start {
            return Some(stem[..i].to_string());
        }
    }
    None
}

/// Check if a string looks like a document string-ID (e.g. "ADR-001", "opp-002").
/// This is the fallback when no schema ref-format is available.
pub(crate) fn is_string_id_fallback(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    // Must start with alphabetic chars
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i == 0 {
        return false;
    }
    // Then a dash or underscore
    if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'_') {
        i += 1;
    } else {
        return false;
    }
    let num_start = i;
    // Then digits
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    // Must have consumed digits and reached the end
    i > num_start && i == bytes.len()
}

/// Extract ref strings from a YAML value (single string or array of strings).
pub(crate) fn extract_refs(val: &serde_yaml::Value) -> Vec<String> {
    match val {
        serde_yaml::Value::String(s) => vec![s.to_uppercase()],
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_uppercase()))
            .collect(),
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_to_id() {
        assert_eq!(path_to_id(Path::new("docs/adr-001.md")), "ADR-001");
        assert_eq!(path_to_id(Path::new("inc_002.md")), "INC-002");
        assert_eq!(
            path_to_id(Path::new("docs/adr-001-start-using-postgresql.md")),
            "ADR-001"
        );
        assert_eq!(
            path_to_id(Path::new("opp-003-expand-to-europe.md")),
            "OPP-003"
        );
    }

    #[test]
    fn test_build_graph_from_fixtures() {
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&schema_content).unwrap();
        let graph = DocGraph::build("../../tests/fixtures", &schema).unwrap();

        // Should have nodes for all fixture docs
        assert!(graph.nodes.contains_key("ADR-001"));
        assert!(graph.nodes.contains_key("OPP-001"));
        assert!(graph.nodes.contains_key("POL-001"));
        assert!(graph.nodes.contains_key("INC-001"));

        // ADR-001 should have outgoing refs
        let refs = graph.refs_from("ADR-001");
        assert!(!refs.is_empty());
        let targets: Vec<&str> = refs.iter().map(|e| e.to.as_str()).collect();
        assert!(targets.contains(&"OPP-001"), "ADR-001 enables OPP-001");
        assert!(targets.contains(&"POL-001"), "ADR-001 triggers POL-001");
    }

    #[test]
    fn test_backlinks() {
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&schema_content).unwrap();
        let graph = DocGraph::build("../../tests/fixtures", &schema).unwrap();

        // OPP-001 should have backlink from ADR-001 (enables)
        let backlinks = graph.refs_to("OPP-001");
        let sources: Vec<&str> = backlinks.iter().map(|e| e.from.as_str()).collect();
        assert!(sources.contains(&"ADR-001"));
    }

    #[test]
    fn test_transitive_refs() {
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&schema_content).unwrap();
        let graph = DocGraph::build("../../tests/fixtures", &schema).unwrap();

        let transitive = graph.refs_from_transitive("ADR-001", 3);
        assert!(!transitive.is_empty());
        // Should find depth=1 refs and possibly depth=2+
        assert!(transitive.iter().any(|(d, _)| *d == 1));
    }

    #[test]
    fn test_next_id() {
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&schema_content).unwrap();
        let graph = DocGraph::build("../../tests/fixtures", &schema).unwrap();

        // Fixtures have adr-001, adr-002, adr-003
        let next = graph.next_id("ADR");
        assert_eq!(next, "ADR-004");

        // Only one OPP fixture
        let next = graph.next_id("OPP");
        assert_eq!(next, "OPP-002");
    }

    #[test]
    fn test_mermaid_output() {
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&schema_content).unwrap();
        let graph = DocGraph::build("../../tests/fixtures", &schema).unwrap();

        let mermaid = graph.to_mermaid(None);
        assert!(mermaid.starts_with("graph LR"));
        assert!(mermaid.contains("ADR-001"));
        assert!(mermaid.contains("-->"));
    }

    #[test]
    fn test_dot_output() {
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&schema_content).unwrap();
        let graph = DocGraph::build("../../tests/fixtures", &schema).unwrap();

        let dot = graph.to_dot(None);
        assert!(dot.starts_with("digraph docs"));
        assert!(dot.contains("ADR-001"));
        assert!(dot.contains("->"));
    }

    // ─── Health check tests ──────────────────────────────────────────────────

    fn make_node(id: &str) -> DocNode {
        DocNode {
            id: id.to_string(),
            path: PathBuf::from(format!("{}.md", id.to_lowercase())),
            doc_type: Some("test".into()),
            title: Some(id.into()),
            status: None,
        }
    }

    fn make_schema(acyclic_relations: &[&str]) -> Schema {
        use crate::schema::{Cardinality, RelationDef};
        Schema {
            types: vec![],
            relations: acyclic_relations
                .iter()
                .map(|name| RelationDef {
                    name: name.to_string(),
                    inverse: None,
                    cardinality: Cardinality::Many,
                    description: None,
                    acyclic: Some(true),
                })
                .collect(),
            ref_formats: vec![],
        }
    }

    fn make_schema_no_acyclic() -> Schema {
        Schema {
            types: vec![],
            relations: vec![],
            ref_formats: vec![],
        }
    }

    #[test]
    fn test_check_self_reference() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".into(), make_node("A"));

        let edges = vec![DocEdge {
            from: "A".into(),
            to: "A".into(),
            relation: "related".into(),
        }];

        let graph = DocGraph { nodes, edges };
        let schema = make_schema_no_acyclic();
        let diags = graph.check_health(&schema);

        let g011: Vec<_> = diags.iter().filter(|d| d.code == "G011").collect();
        assert_eq!(g011.len(), 1);
        assert!(g011[0].message.contains("self-reference"));
    }

    #[test]
    fn test_check_cycle_detected() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".into(), make_node("A"));
        nodes.insert("B".into(), make_node("B"));
        nodes.insert("C".into(), make_node("C"));

        let edges = vec![
            DocEdge {
                from: "A".into(),
                to: "B".into(),
                relation: "supersedes".into(),
            },
            DocEdge {
                from: "B".into(),
                to: "C".into(),
                relation: "supersedes".into(),
            },
            DocEdge {
                from: "C".into(),
                to: "A".into(),
                relation: "supersedes".into(),
            },
        ];

        let graph = DocGraph { nodes, edges };
        let schema = make_schema(&["supersedes"]);
        let diags = graph.check_health(&schema);

        let g010: Vec<_> = diags.iter().filter(|d| d.code == "G010").collect();
        assert!(!g010.is_empty(), "should detect cycle");
        assert!(g010[0].severity == "error");
    }

    #[test]
    fn test_check_no_cycle_without_acyclic() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".into(), make_node("A"));
        nodes.insert("B".into(), make_node("B"));

        let edges = vec![
            DocEdge {
                from: "A".into(),
                to: "B".into(),
                relation: "related".into(),
            },
            DocEdge {
                from: "B".into(),
                to: "A".into(),
                relation: "related".into(),
            },
        ];

        let graph = DocGraph { nodes, edges };
        let schema = make_schema_no_acyclic();
        let diags = graph.check_health(&schema);

        // No G010 because no acyclic relations
        let g010: Vec<_> = diags.iter().filter(|d| d.code == "G010").collect();
        assert!(g010.is_empty());
    }

    #[test]
    fn test_check_orphan() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".into(), make_node("A"));
        nodes.insert("B".into(), make_node("B"));
        nodes.insert("ORPHAN".into(), make_node("ORPHAN"));

        let edges = vec![DocEdge {
            from: "A".into(),
            to: "B".into(),
            relation: "related".into(),
        }];

        let graph = DocGraph { nodes, edges };
        let schema = make_schema_no_acyclic();
        let diags = graph.check_health(&schema);

        let g020: Vec<_> = diags.iter().filter(|d| d.code == "G020").collect();
        assert_eq!(g020.len(), 1);
        assert!(g020[0].message.contains("ORPHAN"));
    }

    #[test]
    fn test_check_disconnected_components() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".into(), make_node("A"));
        nodes.insert("B".into(), make_node("B"));
        nodes.insert("C".into(), make_node("C"));
        nodes.insert("D".into(), make_node("D"));

        // Two components: {A,B} and {C,D}
        let edges = vec![
            DocEdge {
                from: "A".into(),
                to: "B".into(),
                relation: "related".into(),
            },
            DocEdge {
                from: "C".into(),
                to: "D".into(),
                relation: "related".into(),
            },
        ];

        let graph = DocGraph { nodes, edges };
        let schema = make_schema_no_acyclic();
        let diags = graph.check_health(&schema);

        let g021: Vec<_> = diags.iter().filter(|d| d.code == "G021").collect();
        assert_eq!(g021.len(), 1);
        assert!(g021[0].message.contains("2 disconnected components"));
    }

    #[test]
    fn test_check_dangling_ref() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".into(), make_node("A"));

        let edges = vec![DocEdge {
            from: "A".into(),
            to: "MISSING".into(),
            relation: "supersedes".into(),
        }];

        let graph = DocGraph { nodes, edges };
        let schema = make_schema_no_acyclic();
        let diags = graph.check_health(&schema);

        let g030: Vec<_> = diags.iter().filter(|d| d.code == "G030").collect();
        assert_eq!(g030.len(), 1);
        assert!(g030[0].message.contains("MISSING"));
    }

    #[test]
    fn test_check_healthy_graph() {
        let mut nodes = BTreeMap::new();
        nodes.insert("A".into(), make_node("A"));
        nodes.insert("B".into(), make_node("B"));
        nodes.insert("C".into(), make_node("C"));

        // Linear chain, all connected, no cycles, no orphans
        let edges = vec![
            DocEdge {
                from: "A".into(),
                to: "B".into(),
                relation: "enables".into(),
            },
            DocEdge {
                from: "B".into(),
                to: "C".into(),
                relation: "enables".into(),
            },
        ];

        let graph = DocGraph { nodes, edges };
        let schema = make_schema(&["enables"]);
        let diags = graph.check_health(&schema);

        assert!(
            diags.is_empty(),
            "healthy graph should have no diagnostics, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_inline_link_edges() {
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&schema_content).unwrap();
        let graph = DocGraph::build("../../tests/fixtures", &schema).unwrap();

        // ADR-003 has inline links to ./adr-001.md and ./adr-002.md
        let refs = graph.refs_from("ADR-003");
        let inline_refs: Vec<&DocEdge> = refs
            .iter()
            .filter(|e| e.relation == "inline_ref")
            .copied()
            .collect();

        let targets: Vec<&str> = inline_refs.iter().map(|e| e.to.as_str()).collect();
        assert!(
            targets.contains(&"ADR-001"),
            "ADR-003 should have inline_ref to ADR-001"
        );
        assert!(
            targets.contains(&"ADR-002"),
            "ADR-003 should have inline_ref to ADR-002"
        );
    }

    #[test]
    fn test_inline_link_dedup_with_frontmatter() {
        // If a frontmatter relation edge already exists for the same from->to pair,
        // no duplicate inline_ref edge should be created
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&schema_content).unwrap();
        let graph = DocGraph::build("../../tests/fixtures", &schema).unwrap();

        // Count all edges from ADR-003 to ADR-001
        let edges_to_adr001: Vec<&DocEdge> = graph
            .edges
            .iter()
            .filter(|e| e.from == "ADR-003" && e.to == "ADR-001")
            .collect();

        // Should not have duplicates — only one edge per unique from->to pair
        // (frontmatter edge OR inline_ref, not both)
        assert!(
            edges_to_adr001.len() <= 1,
            "Should not have duplicate edges from ADR-003 to ADR-001, found {}",
            edges_to_adr001.len()
        );
    }

    #[test]
    fn test_is_string_id_fallback() {
        assert!(super::is_string_id_fallback("ADR-001"));
        assert!(super::is_string_id_fallback("opp-002"));
        assert!(super::is_string_id_fallback("GOV_003"));
        assert!(!super::is_string_id_fallback("https://example.com"));
        assert!(!super::is_string_id_fallback("./adr-001.md"));
        assert!(!super::is_string_id_fallback("just-text"));
        assert!(!super::is_string_id_fallback(""));
    }

    #[test]
    fn test_extract_id_from_stem() {
        assert_eq!(
            super::extract_id_from_stem("ADR-001"),
            Some("ADR-001".to_string())
        );
        assert_eq!(
            super::extract_id_from_stem("ADR-001-SOME-TITLE"),
            Some("ADR-001".to_string())
        );
        assert_eq!(super::extract_id_from_stem("README"), None);
    }

    #[test]
    fn test_path_to_id_with_schema() {
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&schema_content).unwrap();

        assert_eq!(
            super::path_to_id_with_schema(Path::new("docs/adr-001.md"), &schema),
            "ADR-001"
        );
        assert_eq!(
            super::path_to_id_with_schema(Path::new("docs/opp-003-expand-to-europe.md"), &schema),
            "OPP-003"
        );
    }

    #[test]
    fn test_build_cached_matches_build() {
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&schema_content).unwrap();

        let uncached = DocGraph::build("../../tests/fixtures", &schema).unwrap();
        let mut cache = crate::cache::DocCache::new();
        let cached = DocGraph::build_cached("../../tests/fixtures", &schema, &mut cache).unwrap();

        // Same nodes
        assert_eq!(
            uncached.nodes.keys().collect::<Vec<_>>(),
            cached.nodes.keys().collect::<Vec<_>>(),
        );

        // Cached has frontmatter edges (subset of uncached which also has inline_ref)
        let cached_edge_count = cached.edges.len();
        let uncached_fm_edges: Vec<_> = uncached
            .edges
            .iter()
            .filter(|e| e.relation != "inline_ref")
            .collect();
        assert_eq!(cached_edge_count, uncached_fm_edges.len());

        // Cache is dirty after first build
        assert!(cache.is_dirty());
    }

    #[test]
    fn test_build_cached_reuses_entries() {
        let schema_content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&schema_content).unwrap();

        let mut cache = crate::cache::DocCache::new();

        // First build populates cache
        let g1 = DocGraph::build_cached("../../tests/fixtures", &schema, &mut cache).unwrap();
        assert!(!cache.is_empty());

        // Second build reuses cache (no files changed)
        let g2 = DocGraph::build_cached("../../tests/fixtures", &schema, &mut cache).unwrap();
        assert_eq!(g1.nodes.len(), g2.nodes.len());
        assert_eq!(g1.edges.len(), g2.edges.len());
    }
}

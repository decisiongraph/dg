use std::path::Path;

use anyhow::Result;
use clap::Args;
use markdown_tui::RenderOptions;
use md_db::graph::{DocEdge, DocGraph};
use md_db::schema::Schema;

#[derive(Args)]
pub struct RefsArgs {
    /// Document ID to show references for (e.g. ADR-001)
    pub id: String,

    /// Show backlinks (what references this doc) instead of forward refs
    #[arg(long)]
    pub backlinks: bool,

    /// Max traversal depth for transitive refs (default: 1 = direct only)
    #[arg(long, default_value = "1")]
    pub depth: usize,

    /// Output format (text, json, mermaid, dot)
    #[arg(long, default_value = "text")]
    pub format: String,
}

pub fn run(
    root: &Path,
    schema: &Schema,
    args: &RefsArgs,
    cache: &mut md_db::cache::DocCache,
) -> Result<()> {
    let graph = DocGraph::build_cached(root, schema, cache)?;
    let id = args.id.to_uppercase();

    if !graph.nodes.contains_key(&id) {
        anyhow::bail!("document {id} not found in graph");
    }

    match args.format.as_str() {
        "json" => print_json(&graph, &id, args),
        "mermaid" => print_mermaid(&graph, &id, args),
        "dot" => print_dot(&graph, &id, args),
        _ => print_text(&graph, &id, args),
    }
}

/// Resolve edges based on direction (backlinks or forward refs).
fn resolve_edges<'a>(graph: &'a DocGraph, id: &str, args: &RefsArgs) -> Vec<(usize, &'a DocEdge)> {
    if args.backlinks {
        graph.refs_to_transitive(id, args.depth)
    } else {
        graph.refs_from_transitive(id, args.depth)
    }
}

/// Get the peer ID from an edge (the other end, depending on direction).
fn peer_id(edge: &DocEdge, backlinks: bool) -> &str {
    if backlinks {
        &edge.from
    } else {
        &edge.to
    }
}

/// Look up a node title from the graph, falling back to the ID.
fn node_title<'a>(graph: &'a DocGraph, node_id: &'a str) -> &'a str {
    graph
        .nodes
        .get(node_id)
        .and_then(|n| n.title.as_deref())
        .unwrap_or(node_id)
}

fn print_text(graph: &DocGraph, id: &str, args: &RefsArgs) -> Result<()> {
    let edges = resolve_edges(graph, id, args);

    if edges.is_empty() {
        let mode = if args.backlinks { "backlinks" } else { "refs" };
        println!("No {mode} found for {id}.");
        return Ok(());
    }

    let mode_label = if args.backlinks {
        "Backlinks to"
    } else {
        "Refs from"
    };
    println!("{mode_label} {id}:");

    let headers = &["ID", "Relation", "Title", "Depth"];
    let rows: Vec<Vec<String>> = edges
        .iter()
        .map(|(depth, edge)| {
            let pid = peer_id(edge, args.backlinks);
            vec![
                pid.to_string(),
                edge.relation.clone(),
                node_title(graph, pid).to_string(),
                depth.to_string(),
            ]
        })
        .collect();

    let width = crossterm::terminal::size()
        .map(|(cols, _)| cols as usize)
        .unwrap_or(80);
    let opts = RenderOptions {
        width,
        ..Default::default()
    };
    let rendered = markdown_tui::render_table(headers, &rows, &opts);
    print!("{rendered}");

    Ok(())
}

fn print_json(graph: &DocGraph, id: &str, args: &RefsArgs) -> Result<()> {
    let edges = resolve_edges(graph, id, args);
    let mode = if args.backlinks { "backlinks" } else { "refs" };

    let results: Vec<serde_json::Value> = edges
        .iter()
        .map(|(depth, edge)| {
            let pid = peer_id(edge, args.backlinks);
            let node = graph.nodes.get(pid);
            serde_json::json!({
                "id": pid,
                "relation": edge.relation,
                "depth": depth,
                "type": node.and_then(|n| n.doc_type.as_deref()),
                "title": node.and_then(|n| n.title.as_deref()),
            })
        })
        .collect();

    let output = serde_json::json!({
        "id": id,
        "mode": mode,
        "results": results,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn print_mermaid(graph: &DocGraph, id: &str, args: &RefsArgs) -> Result<()> {
    let edges = resolve_edges(graph, id, args);

    let mut out = String::from("graph LR\n");
    out.push_str(&format!(
        "  {id}[\"{}\"]:::highlight\n",
        node_title(graph, id)
    ));

    for (_depth, edge) in &edges {
        let pid = peer_id(edge, args.backlinks);
        let title = node_title(graph, pid);
        out.push_str(&format!("  {pid}[\"{title}\"]\n"));
        out.push_str(&format!(
            "  {} -->|{}| {}\n",
            edge.from, edge.relation, edge.to
        ));
    }

    out.push_str("  classDef highlight fill:#f9f,stroke:#333\n");
    print!("{out}");
    Ok(())
}

fn print_dot(graph: &DocGraph, id: &str, args: &RefsArgs) -> Result<()> {
    let edges = resolve_edges(graph, id, args);

    let mut out = String::from("digraph refs {\n  rankdir=LR;\n  node [shape=box];\n\n");
    out.push_str(&format!(
        "  \"{id}\" [label=\"{}\" style=bold];\n",
        node_title(graph, id)
    ));

    for (_depth, edge) in &edges {
        let pid = peer_id(edge, args.backlinks);
        let title = node_title(graph, pid);
        out.push_str(&format!("  \"{pid}\" [label=\"{title}\"];\n"));
        out.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
            edge.from, edge.to, edge.relation
        ));
    }

    out.push_str("}\n");
    print!("{out}");
    Ok(())
}

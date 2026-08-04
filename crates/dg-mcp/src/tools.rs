//! MCP tool implementations.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use md_db::discovery::{self, Filter};
use md_db::document::Document;
use md_db::frontmatter::Frontmatter;
use md_db::graph::{path_to_id, DocGraph};
use md_db::output;
use md_db::template;
use md_db::validation;

use crate::args::*;
use crate::schema_json::*;

pub fn handle_tool_call(name: &str, args: &Value) -> Result<Value> {
    match name {
        "dg-validate" => tool_validate(args),
        "dg-get" => tool_get(args),
        "dg-list" => tool_list_docs(args),
        "dg-inspect" => tool_inspect(args),
        "dg-describe" => tool_describe(args),
        "dg-set" => tool_set(args),
        "dg-new" => tool_new(args),
        "dg-refs" => tool_refs(args),
        "dg-graph" => tool_graph(args),
        "dg-deprecate" => tool_deprecate(args),
        _ => bail!("unknown tool: {name}"),
    }
}

fn tool_validate(args: &Value) -> Result<Value> {
    let schema = load_schema(args)?;
    let user_config = load_org_config(args)?;
    let pattern = str_arg(args, "pattern");

    let result = if let Some(file) = str_arg(args, "file") {
        let content = std::fs::read_to_string(&file).with_context(|| format!("read {file}"))?;
        let doc = Document::from_str(&content)?;
        let fr = validation::validate_document(
            &doc,
            &schema,
            &HashSet::new(),
            &HashSet::new(),
            user_config.as_ref(),
        );
        validation::ValidationResult {
            file_results: vec![fr],
        }
    } else if let Some(dir) = str_arg(args, "dir") {
        validation::validate_directory(
            PathBuf::from(&dir),
            &schema,
            pattern.as_deref(),
            user_config.as_ref(),
        )?
    } else {
        bail!("provide 'dir' or 'file'");
    };

    Ok(validate_result_to_json(&result))
}

fn validate_result_to_json(result: &validation::ValidationResult) -> Value {
    let files: Vec<Value> = result
        .file_results
        .iter()
        .filter(|f| !f.diagnostics.is_empty())
        .map(|f| {
            let diags: Vec<Value> = f.diagnostics.iter().map(diagnostic_to_json).collect();
            json!({ "path": f.path, "diagnostics": diags })
        })
        .collect();

    json!({
        "files": files,
        "errors": result.total_errors(),
        "warnings": result.total_warnings(),
        "ok": result.is_ok(),
    })
}

fn tool_get(args: &Value) -> Result<Value> {
    let file = require_str(args, "file")?;
    let doc = Document::from_file(PathBuf::from(&file))?;

    if let Some(field_key) = str_arg(args, "field") {
        let fm = doc.frontmatter()?;
        let val = fm
            .get(&field_key)
            .with_context(|| format!("field not found: {field_key}"))?;
        return Ok(json!({
            "field": field_key,
            "value": output::format_field_value(val, output::OutputFormat::Text),
        }));
    }

    if bool_arg(args, "frontmatter") {
        let fm = doc.frontmatter()?;
        return Ok(fm.to_json());
    }

    if let Some(heading) = str_arg(args, "section") {
        let section = doc.get_section(&heading)?;

        if let Some(table_idx) = int_arg(args, "table") {
            let tables = section.tables();
            let table = tables
                .get(table_idx)
                .with_context(|| format!("table index {table_idx} not found"))?;

            if let Some(cell_spec) = str_arg(args, "cell") {
                let (col, row) = parse_cell_spec(&cell_spec)?;
                let val = table.get_cell_or_err(&col, row)?;
                return Ok(json!({ "cell": cell_spec, "value": val }));
            }

            return Ok(json!({
                "table_index": table_idx,
                "markdown": output::format_table(table, output::OutputFormat::Markdown),
            }));
        }

        return Ok(json!({
            "heading": section.heading.trim(),
            "level": section.level,
            "content": section.content,
        }));
    }

    Ok(doc.to_json())
}

fn tool_list_docs(args: &Value) -> Result<Value> {
    let dir = require_str(args, "dir")?;
    let pattern = str_arg(args, "pattern");
    let field_filters = str_array_arg(args, "fields");

    let mut filters = Vec::new();
    for f in &field_filters {
        if let Some((key, value)) = f.split_once('=') {
            filters.push(Filter::FieldEquals {
                key: key.to_string(),
                value: value.to_string(),
            });
        }
    }

    let mut files =
        discovery::discover_files(PathBuf::from(&dir), pattern.as_deref(), &filters, false)?;

    if let Some(sort_spec) = str_arg(args, "sort") {
        let (sort_key, descending) = if let Some(key) = sort_spec.strip_prefix('-') {
            (key.to_string(), true)
        } else {
            (sort_spec, false)
        };

        let mut file_vals: Vec<(PathBuf, Option<String>)> = files
            .into_iter()
            .map(|path| {
                let val = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|content| Frontmatter::try_parse(&content).ok())
                    .and_then(|(fm, _)| fm)
                    .and_then(|fm| fm.get_display(&sort_key));
                (path, val)
            })
            .collect();

        file_vals.sort_by(|a, b| {
            let cmp =
                a.1.as_deref()
                    .unwrap_or("")
                    .cmp(b.1.as_deref().unwrap_or(""));
            if descending {
                cmp.reverse()
            } else {
                cmp
            }
        });

        files = file_vals.into_iter().map(|(path, _)| path).collect();
    }

    let entries: Vec<Value> = files
        .iter()
        .map(|path| {
            let fm_json = std::fs::read_to_string(path)
                .ok()
                .and_then(|content| Frontmatter::try_parse(&content).ok())
                .and_then(|(fm, _)| fm.map(|f| f.to_json()));
            json!({
                "path": path.display().to_string(),
                "frontmatter": fm_json,
            })
        })
        .collect();

    Ok(json!({ "files": entries, "count": entries.len() }))
}

fn tool_inspect(args: &Value) -> Result<Value> {
    let file = require_str(args, "file")?;
    let schema = load_schema(args)?;
    let user_config = load_org_config(args)?;
    let doc = Document::from_file(PathBuf::from(&file))?;

    let file_result = validation::validate_document(
        &doc,
        &schema,
        &HashSet::new(),
        &HashSet::new(),
        user_config.as_ref(),
    );

    let frontmatter = doc
        .frontmatter
        .as_ref()
        .map(|fm| fm.to_json())
        .unwrap_or(Value::Null);

    let sections: Vec<Value> = doc
        .sections()
        .iter()
        .map(|s| {
            json!({
                "heading": s.heading.trim(),
                "level": s.level,
                "content_length": s.content.len(),
            })
        })
        .collect();

    let diagnostics: Vec<Value> = file_result
        .diagnostics
        .iter()
        .map(diagnostic_to_json)
        .collect();

    Ok(json!({
        "path": file,
        "frontmatter": frontmatter,
        "sections": sections,
        "diagnostics": diagnostics,
        "errors": file_result.errors(),
        "warnings": file_result.warnings(),
        "valid": file_result.errors() == 0,
    }))
}

fn tool_describe(args: &Value) -> Result<Value> {
    let schema = load_schema(args)?;

    if bool_arg(args, "export") {
        return Ok(export_schema_json(&schema));
    }

    if bool_arg(args, "relations") {
        return Ok(relations_to_json(&schema));
    }

    if let Some(type_name) = str_arg(args, "type") {
        let type_def = schema
            .get_type(&type_name)
            .with_context(|| format!("unknown type: {type_name}"))?;

        if let Some(field_name) = str_arg(args, "field") {
            let field_def = type_def
                .fields
                .iter()
                .find(|f| f.name == field_name)
                .with_context(|| format!("unknown field: {field_name}"))?;
            return Ok(field_to_json(field_def));
        }

        return Ok(type_to_json(type_def));
    }

    let types: Vec<Value> = schema
        .types
        .iter()
        .map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "fields": t.fields.len(),
                "sections": t.sections.len(),
                "folder": t.folder,
                "max_count": t.max_count,
            })
        })
        .collect();

    Ok(json!({
        "types": types,
        "relations": relations_to_json(&schema),
    }))
}

fn tool_set(args: &Value) -> Result<Value> {
    let file = require_str(args, "file")?;
    let dry_run = bool_arg(args, "dry_run");
    let mut doc = Document::from_file(PathBuf::from(&file))?;

    for field_str in str_array_arg(args, "fields") {
        if let Some((k, v)) = field_str.split_once("+=") {
            doc.append_field_from_str(k, v);
        } else if let Some((k, v)) = field_str.split_once('=') {
            doc.set_field_from_str(k, v);
        } else {
            anyhow::bail!("invalid field format: {field_str}");
        }
    }

    for ss in str_array_arg(args, "section_sets") {
        let (heading, content) = ss
            .split_once('=')
            .with_context(|| format!("invalid section-set: {ss}"))?;
        doc.replace_section_content(heading.trim(), &format!("\n{}\n", content.trim()))?;
    }

    if let Some(heading) = str_arg(args, "section") {
        if let Some(content) = str_arg(args, "content") {
            doc.replace_section_content(&heading, &format!("{content}\n"))?;
        }
        if let Some(text) = str_arg(args, "append") {
            doc.append_to_section(&heading, &text)?;
        }
        if let Some(table_idx) = int_arg(args, "table") {
            if let Some(cell_spec) = str_arg(args, "cell") {
                let value = require_str(args, "value")?;
                let (col, row) = parse_cell_spec(&cell_spec)?;
                doc.set_table_cell(&heading, table_idx, &col, row, &value)?;
            }
            if let Some(row_str) = str_arg(args, "add_row") {
                let values: Vec<String> =
                    row_str.split(',').map(|s| s.trim().to_string()).collect();
                doc.add_table_row(&heading, table_idx, values)?;
            }
        }
    }

    if dry_run {
        Ok(json!({ "content": doc.raw, "written": false }))
    } else {
        doc.save()?;
        Ok(json!({ "path": file, "written": true }))
    }
}

fn tool_new(args: &Value) -> Result<Value> {
    let doc_type = require_str(args, "type")?;
    let schema = load_schema(args)?;

    let type_def = schema
        .get_type(&doc_type)
        .with_context(|| format!("unknown type: {doc_type}"))?;
    let canonical_type = &type_def.name;

    let field_strs = str_array_arg(args, "fields");
    let mut set_fields = Vec::new();
    let mut append_fields = Vec::new();
    for s in &field_strs {
        if let Some((k, v)) = s.split_once("+=") {
            append_fields.push((k.to_string(), v.to_string()));
        } else if let Some((k, v)) = s.split_once('=') {
            set_fields.push((k.to_string(), v.to_string()));
        } else {
            anyhow::bail!("invalid field: {s}");
        }
    }

    // `title` feeds the template's "title" override (-> H1) and the auto-id slug.
    // An explicit `title=` in `fields` wins over the dedicated `title` arg.
    let title = str_arg(args, "title");
    if let Some(ref t) = title {
        if !set_fields.iter().any(|(k, _)| k == "title") {
            set_fields.push(("title".to_string(), t.clone()));
        }
    }
    // LAST `title=` wins, matching the template's H1 (BTreeMap insert overwrites),
    // so the slug and the H1 cannot disagree.
    let effective_title = set_fields
        .iter()
        .rev()
        .find(|(k, _)| k == "title")
        .map(|(_, v)| v.clone());

    let fill = bool_arg(args, "fill");
    let auto_id = bool_arg(args, "auto_id");

    let output_path = if auto_id {
        let dir = require_str(args, "dir")?;
        let graph = DocGraph::build(PathBuf::from(&dir), &schema)?;
        let next_id = graph.next_id(canonical_type);
        let folder = type_def.folder.as_deref().unwrap_or(".");
        let slug = effective_title
            .as_deref()
            .map(template::slugify)
            .unwrap_or_default();
        // Match the CLI exactly: under auto_id a title is required and must yield a
        // non-empty slug. A missing title — or one that slugifies to nothing
        // (punctuation- or non-ASCII-only) — would produce a bare `{id}.md` that
        // fails F011, so refuse it loudly instead of writing an invalid document.
        if slug.is_empty() {
            match &effective_title {
                None => anyhow::bail!(
                    "title is required under auto_id (used for the filename slug); \
                     pass a `title` with ASCII letters or digits"
                ),
                Some(t) => anyhow::bail!(
                    "title {t:?} produces an empty filename slug (only punctuation or \
                     non-ASCII characters); provide a title with ASCII letters or digits"
                ),
            }
        }
        let filename = format!("{}-{slug}.md", next_id.to_lowercase());
        let base = PathBuf::from(&dir);
        if folder != "." && base.ends_with(folder) {
            Some(base.join(filename))
        } else {
            Some(base.join(folder).join(filename))
        }
    } else {
        str_arg(args, "output").map(PathBuf::from)
    };

    let content = template::generate_document_opts(type_def, &schema, &set_fields, &[], fill);
    let mut doc = Document::from_str(&content)?;

    for (k, v) in &append_fields {
        doc.append_field_from_str(k, v);
    }

    for ss in str_array_arg(args, "section_sets") {
        let (heading, content) = ss
            .split_once('=')
            .with_context(|| format!("invalid section-set: {ss}"))?;
        doc.replace_section_content(heading.trim(), &format!("\n{}\n", content.trim()))?;
    }

    let final_content = &doc.raw;

    if let Some(ref path) = output_path {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, final_content)?;
        Ok(json!({ "path": path.display().to_string(), "content": final_content }))
    } else {
        Ok(json!({ "content": final_content }))
    }
}

fn tool_refs(args: &Value) -> Result<Value> {
    let dir = require_str(args, "dir")?;
    let schema = load_schema(args)?;
    let graph = DocGraph::build(PathBuf::from(&dir), &schema)?;
    let depth = int_arg(args, "depth").unwrap_or(1);

    if let Some(target) = str_arg(args, "to") {
        let id = normalize_id(&target);
        let items = build_ref_items(&graph, &id, depth, RefDirection::To);
        return Ok(
            json!({ "id": id, "mode": "backlinks", "results": items, "count": items.len() }),
        );
    }

    if let Some(source) = str_arg(args, "from") {
        let id = normalize_id(&source);
        let items = build_ref_items(&graph, &id, depth, RefDirection::From);
        return Ok(json!({ "id": id, "mode": "refs", "results": items, "count": items.len() }));
    }

    bail!("provide 'from' or 'to'");
}

enum RefDirection {
    From,
    To,
}

fn build_ref_items(
    graph: &DocGraph,
    id: &str,
    depth: usize,
    direction: RefDirection,
) -> Vec<Value> {
    let edges = match direction {
        RefDirection::To => {
            if depth > 1 {
                graph.refs_to_transitive(id, depth)
            } else {
                graph.refs_to(id).into_iter().map(|e| (1usize, e)).collect()
            }
        }
        RefDirection::From => {
            if depth > 1 {
                graph.refs_from_transitive(id, depth)
            } else {
                graph
                    .refs_from(id)
                    .into_iter()
                    .map(|e| (1usize, e))
                    .collect()
            }
        }
    };

    edges
        .iter()
        .map(|(d, e)| {
            let (peer_id, node) = match direction {
                RefDirection::To => (&e.from, graph.nodes.get(&e.from)),
                RefDirection::From => (&e.to, graph.nodes.get(&e.to)),
            };
            json!({
                "id": peer_id,
                "relation": e.relation,
                "depth": d,
                "type": node.and_then(|n| n.doc_type.as_deref()),
                "title": node.and_then(|n| n.title.as_deref()),
            })
        })
        .collect()
}

fn tool_graph(args: &Value) -> Result<Value> {
    let dir = require_str(args, "dir")?;
    let schema = load_schema(args)?;
    let graph = DocGraph::build(PathBuf::from(&dir), &schema)?;
    let filter_type =
        str_arg(args, "type").map(|t| schema.get_type(&t).map(|td| td.name.clone()).unwrap_or(t));

    let nodes: Vec<Value> = graph
        .nodes
        .values()
        .filter(|n| {
            filter_type
                .as_deref()
                .map(|ft| n.doc_type.as_deref() == Some(ft))
                .unwrap_or(true)
        })
        .map(|n| {
            json!({
                "id": n.id,
                "type": n.doc_type,
                "title": n.title,
                "status": n.status,
                "path": n.path.display().to_string(),
            })
        })
        .collect();

    let edges: Vec<Value> = graph
        .edges
        .iter()
        .map(|e| json!({ "from": e.from, "to": e.to, "relation": e.relation }))
        .collect();

    Ok(json!({
        "nodes": nodes,
        "edges": edges,
        "node_count": nodes.len(),
        "edge_count": edges.len(),
    }))
}

fn tool_deprecate(args: &Value) -> Result<Value> {
    let file = require_str(args, "file")?;
    let schema = load_schema(args)?;
    let dry_run = bool_arg(args, "dry_run");

    let mut doc = Document::from_file(PathBuf::from(&file))?;
    let doc_id = path_to_id(std::path::Path::new(&file));

    if let Some(replacement) = str_arg(args, "superseded_by") {
        doc.set_field_from_str("status", "superseded");
        doc.set_field_from_str("superseded_by", &replacement);
    } else {
        doc.set_field_from_str("status", "deprecated");
    }

    if dry_run {
        return Ok(json!({ "id": doc_id, "content": doc.raw, "written": false }));
    }

    doc.save()?;

    let mut backlinks = Vec::new();
    if let Some(dir) = str_arg(args, "dir") {
        let graph = DocGraph::build(PathBuf::from(&dir), &schema)?;
        for edge in graph.refs_to(&doc_id) {
            if edge.from != doc_id {
                backlinks.push(json!({ "from": edge.from, "relation": edge.relation }));
            }
        }
    }

    Ok(json!({
        "id": doc_id,
        "written": true,
        "backlinks": backlinks,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FIXTURE_SCHEMA: &str = "../../tests/fixtures/schema.kdl";

    fn new_adr(dir: &std::path::Path, args: serde_json::Value) -> Result<Value> {
        let mut base = json!({
            "type": "adr",
            "schema": FIXTURE_SCHEMA,
            "auto_id": true,
            "dir": dir.to_str().unwrap(),
            "fields": ["author=example", "status=proposed"],
        });
        let obj = base.as_object_mut().unwrap();
        for (k, v) in args.as_object().unwrap() {
            obj.insert(k.clone(), v.clone());
        }
        tool_new(&base)
    }

    #[test]
    fn title_arg_sets_h1_and_slug() {
        let tmp = tempfile::tempdir().unwrap();
        let out = new_adr(tmp.path(), json!({ "title": "SDK Architecture" })).unwrap();
        let path = out["path"].as_str().unwrap();
        assert!(
            path.ends_with("docs/architecture/adr-001-sdk-architecture.md"),
            "{path}"
        );
        assert!(out["content"]
            .as_str()
            .unwrap()
            .contains("# SDK Architecture"));
    }

    #[test]
    fn explicit_field_title_wins_over_title_arg() {
        let tmp = tempfile::tempdir().unwrap();
        let out = new_adr(
            tmp.path(),
            json!({ "title": "From Arg", "fields": ["author=example", "title=From Field"] }),
        )
        .unwrap();
        assert!(out["path"]
            .as_str()
            .unwrap()
            .ends_with("adr-001-from-field.md"));
        assert!(out["content"].as_str().unwrap().contains("# From Field"));
    }

    #[test]
    fn duplicate_title_slug_matches_h1() {
        let tmp = tempfile::tempdir().unwrap();
        let out = new_adr(
            tmp.path(),
            json!({ "fields": ["author=example", "title=First Alpha", "title=Second Beta"] }),
        )
        .unwrap();
        // last title wins for BOTH the H1 and the slug
        assert!(out["path"]
            .as_str()
            .unwrap()
            .ends_with("adr-001-second-beta.md"));
        assert!(out["content"].as_str().unwrap().contains("# Second Beta"));
    }

    #[test]
    fn empty_slug_title_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        // non-ASCII-only title slugifies to nothing -> must error, not write {id}.md
        let err = new_adr(tmp.path(), json!({ "title": "データベース移行" })).unwrap_err();
        assert!(err.to_string().contains("empty filename slug"), "{err}");
        // and nothing was written
        assert!(!tmp.path().join("docs/architecture/adr-001.md").exists());
    }

    #[test]
    fn no_title_under_auto_id_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        // under auto_id a title is required (CLI parity); a bare {id}.md fails F011
        let err = new_adr(tmp.path(), json!({})).unwrap_err();
        assert!(err.to_string().contains("title is required"), "{err}");
        assert!(!tmp.path().join("docs/architecture/adr-001.md").exists());
    }
}

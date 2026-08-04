//! Minimal MCP (Model Context Protocol) server over stdio.
//!
//! Reads JSON-RPC 2.0 requests line-by-line from stdin, dispatches to md-db
//! library functions, and writes JSON-RPC responses to stdout.

mod args;
mod schema_json;
mod tools;

use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

// ── Tool descriptors ────────────────────────────────────────────────────────

fn tool_list() -> Value {
    json!([
        {
            "name": "dg-validate",
            "description": "Validate markdown documents against a KDL schema. Returns diagnostics.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dir":     { "type": "string", "description": "Directory to validate" },
                    "schema":  { "type": "string", "description": "Path to KDL schema file" },
                    "file":    { "type": "string", "description": "Single file to validate (instead of dir)" },
                    "pattern": { "type": "string", "description": "Glob pattern (default *.md)" },
                    "users":   { "type": "string", "description": "Path to user/team config KDL" }
                },
                "required": ["schema"]
            }
        },
        {
            "name": "dg-get",
            "description": "Read a field, section, table, or cell from a markdown document.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file":        { "type": "string",  "description": "Path to the markdown file" },
                    "field":       { "type": "string",  "description": "Frontmatter field key (dotted paths supported)" },
                    "frontmatter": { "type": "boolean", "description": "Return full frontmatter" },
                    "section":     { "type": "string",  "description": "Section heading" },
                    "table":       { "type": "integer", "description": "Table index within section (0-based)" },
                    "cell":        { "type": "string",  "description": "Cell spec: Column,Row" }
                },
                "required": ["file"]
            }
        },
        {
            "name": "dg-list",
            "description": "List and filter markdown documents by frontmatter fields.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dir":     { "type": "string", "description": "Directory to search" },
                    "pattern": { "type": "string", "description": "Glob pattern (default *.md)" },
                    "fields":  { "type": "array",  "items": { "type": "string" }, "description": "Filters: key=value" },
                    "sort":    { "type": "string", "description": "Sort by field (prefix - for descending)" }
                },
                "required": ["dir"]
            }
        },
        {
            "name": "dg-inspect",
            "description": "Inspect a document: frontmatter, sections, validation diagnostics.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file":   { "type": "string", "description": "Path to the markdown file" },
                    "schema": { "type": "string", "description": "Path to KDL schema file" },
                    "users":  { "type": "string", "description": "Path to user/team config KDL" }
                },
                "required": ["file", "schema"]
            }
        },
        {
            "name": "dg-describe",
            "description": "Describe schema types, fields, sections, and relations.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "schema":    { "type": "string",  "description": "Path to KDL schema file" },
                    "type":      { "type": "string",  "description": "Show details for a specific type" },
                    "field":     { "type": "string",  "description": "Show details for a field (requires type)" },
                    "relations": { "type": "boolean", "description": "Show all relations" },
                    "export":    { "type": "boolean", "description": "Export full schema as JSON" }
                },
                "required": ["schema"]
            }
        },
        {
            "name": "dg-set",
            "description": "Set/update fields, sections, or table cells in a markdown document.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file":         { "type": "string",  "description": "Path to the markdown file" },
                    "fields":       { "type": "array",   "items": { "type": "string" }, "description": "Field updates: key=value or key+=value to append" },
                    "section":      { "type": "string",  "description": "Target section heading" },
                    "content":      { "type": "string",  "description": "Replace section content" },
                    "append":       { "type": "string",  "description": "Append to section" },
                    "table":        { "type": "integer", "description": "Table index (0-based)" },
                    "cell":         { "type": "string",  "description": "Cell spec: Column,Row" },
                    "value":        { "type": "string",  "description": "Value for --cell" },
                    "add_row":      { "type": "string",  "description": "Add row (comma-separated)" },
                    "section_sets": { "type": "array",   "items": { "type": "string" }, "description": "Batch: Heading=content" },
                    "dry_run":      { "type": "boolean", "description": "Return result without writing" }
                },
                "required": ["file"]
            }
        },
        {
            "name": "dg-new",
            "description": "Create a new document from a schema type definition.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "type":    { "type": "string",  "description": "Document type name" },
                    "title":   { "type": "string",  "description": "Document title — sets the H1 heading and (under auto_id) the filename slug. Under auto_id a title is REQUIRED and must contain ASCII letters/digits: a missing title, or one that slugifies to nothing (punctuation-/non-ASCII-only), is rejected, matching the CLI. With an explicit `output` path the title is optional and only sets the H1." },
                    "schema":  { "type": "string",  "description": "Path to KDL schema file" },
                    "output":  { "type": "string",  "description": "Output file path" },
                    "dir":     { "type": "string",  "description": "Directory for auto-ID" },
                    "fields":  { "type": "array",   "items": { "type": "string" }, "description": "Pre-fill: key=value or key+=value to append" },
                    "fill":         { "type": "boolean", "description": "Expand template variables" },
                    "auto_id":      { "type": "boolean", "description": "Auto-generate path using next ID" },
                    "section_sets": { "type": "array",   "items": { "type": "string" }, "description": "Batch: Heading=content" }
                },
                "required": ["type", "schema"]
            }
        },
        {
            "name": "dg-refs",
            "description": "Show forward refs or backlinks for a document.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dir":    { "type": "string",  "description": "Directory containing markdown files" },
                    "schema": { "type": "string",  "description": "Path to KDL schema file" },
                    "from":   { "type": "string",  "description": "Show outgoing refs from this ID/file" },
                    "to":     { "type": "string",  "description": "Show backlinks to this ID" },
                    "depth":  { "type": "integer", "description": "Transitive depth (default 1)" }
                },
                "required": ["dir", "schema"]
            }
        },
        {
            "name": "dg-graph",
            "description": "Export the document link graph as JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "dir":    { "type": "string", "description": "Directory containing markdown files" },
                    "schema": { "type": "string", "description": "Path to KDL schema file" },
                    "type":   { "type": "string", "description": "Filter by document type" }
                },
                "required": ["dir", "schema"]
            }
        },
        {
            "name": "dg-deprecate",
            "description": "Mark a document as deprecated or superseded.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "file":          { "type": "string",  "description": "Path to the markdown file" },
                    "schema":        { "type": "string",  "description": "Path to KDL schema file" },
                    "superseded_by": { "type": "string",  "description": "Replacement document ID" },
                    "dir":           { "type": "string",  "description": "Directory for backlink scanning" },
                    "dry_run":       { "type": "boolean", "description": "Print result without writing" }
                },
                "required": ["file", "schema"]
            }
        }
    ])
}

// ── JSON-RPC helpers ────────────────────────────────────────────────────────

fn jsonrpc_ok(id: &Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn jsonrpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

fn text_content(text: &str) -> Value {
    json!([{ "type": "text", "text": text }])
}

// ── Main ────────────────────────────────────────────────────────────────────

/// Parse --root <path> from argv, returning the resolved absolute path.
fn parse_root_arg() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2).find(|w| w[0] == "--root").and_then(|w| {
        std::fs::canonicalize(&w[1])
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .or_else(|| Some(w[1].clone()))
    })
}

/// Inject default values into tool args when they aren't provided by the caller.
/// `dir` defaults to `root` so tools work without explicit path arguments.
fn apply_defaults(args: Value, root: &Option<String>) -> Value {
    let Some(root) = root else { return args };
    let mut obj = match args {
        Value::Object(m) => m,
        other => return other,
    };
    if !obj.contains_key("dir") {
        obj.insert("dir".to_string(), Value::String(root.clone()));
    }
    Value::Object(obj)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = parse_root_arg();

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();

    let mut initialized = false;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                let resp = jsonrpc_error(&Value::Null, -32700, &format!("parse error: {e}"));
                writeln!(writer, "{}", resp)?;
                writer.flush()?;
                continue;
            }
        };

        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(json!({}));

        let response = match method {
            "initialize" => {
                initialized = true;
                jsonrpc_ok(
                    &id,
                    json!({
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": { "listChanged": false }
                        },
                        "serverInfo": {
                            "name": "dg-mcp",
                            "version": env!("CARGO_PKG_VERSION"),
                        }
                    }),
                )
            }
            "notifications/initialized" => continue,
            "tools/list" => {
                if !initialized {
                    jsonrpc_error(&id, -32600, "not initialized")
                } else {
                    jsonrpc_ok(&id, json!({ "tools": tool_list() }))
                }
            }
            "tools/call" => {
                if !initialized {
                    jsonrpc_error(&id, -32600, "not initialized")
                } else {
                    let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let raw_args = params.get("arguments").cloned().unwrap_or(json!({}));
                    let tool_args = apply_defaults(raw_args, &root);

                    match tools::handle_tool_call(tool_name, &tool_args) {
                        Ok(result) => {
                            let text = serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| result.to_string());
                            jsonrpc_ok(
                                &id,
                                json!({
                                    "content": text_content(&text),
                                    "isError": false,
                                }),
                            )
                        }
                        Err(e) => jsonrpc_ok(
                            &id,
                            json!({
                                "content": text_content(&format!("{e:#}")),
                                "isError": true,
                            }),
                        ),
                    }
                }
            }
            "ping" => jsonrpc_ok(&id, json!({})),
            _ => jsonrpc_error(&id, -32601, &format!("unknown method: {method}")),
        };

        writeln!(writer, "{}", response)?;
        writer.flush()?;
    }

    Ok(())
}

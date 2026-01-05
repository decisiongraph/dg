//! JSON argument extraction helpers for MCP tool calls.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::Value;

use md_db::schema::Schema;
use md_db::users::OrgConfig;

pub fn str_arg(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

pub fn bool_arg(args: &Value, key: &str) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(false)
}

pub fn int_arg(args: &Value, key: &str) -> Option<usize> {
    args.get(key).and_then(|v| v.as_u64()).map(|n| n as usize)
}

pub fn str_array_arg(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn require_str(args: &Value, key: &str) -> Result<String> {
    str_arg(args, key).context(format!("missing required argument: {key}"))
}

/// Load a Schema from the "schema" argument, falling back to built-in schema.
pub fn load_schema(args: &Value) -> Result<Schema> {
    let path = require_str(args, "schema")?;
    let path = PathBuf::from(&path);
    if path.is_file() {
        Schema::from_file(&path).context("failed to load schema")
    } else {
        Schema::from_str(dg_schemas::SCHEMA).context("failed to parse built-in schema")
    }
}

/// Optionally load an OrgConfig from the "users" argument.
pub fn load_org_config(args: &Value) -> Result<Option<OrgConfig>> {
    str_arg(args, "users")
        .map(|p| OrgConfig::from_file(PathBuf::from(p)))
        .transpose()
        .context("failed to load org config")
}

/// Parse a cell spec like "Column,Row" into (column_name, row_index).
pub fn parse_cell_spec(spec: &str) -> Result<(String, usize)> {
    let (col, row_str) = spec
        .split_once(',')
        .context(format!("invalid cell spec '{spec}', expected Column,Row"))?;
    let row: usize = row_str.parse().context("invalid row number in cell spec")?;
    Ok((col.to_string(), row))
}

/// Normalize a document ID from a source string (path or plain ID).
pub fn normalize_id(source: &str) -> String {
    if source.contains('/') || source.ends_with(".md") {
        md_db::graph::path_to_id(std::path::Path::new(source))
    } else {
        source.to_uppercase().replace('_', "-")
    }
}

use std::collections::BTreeMap;

use gray_matter::{engine::YAML, Matter};
use serde_yaml::Value;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Frontmatter {
    data: BTreeMap<String, Value>,
}

impl Frontmatter {
    /// Parse frontmatter from raw file content. Returns (Frontmatter, body).
    pub fn parse(raw: &str) -> Result<(Self, String)> {
        let matter = Matter::<YAML>::new();
        let result = matter.parse(raw);

        let data: BTreeMap<String, Value> = match result.data {
            Some(pod) => match pod.deserialize::<BTreeMap<String, Value>>() {
                Ok(map) => map,
                Err(_) => {
                    // null/~ YAML is equivalent to empty frontmatter
                    let trimmed = result.matter.trim();
                    if trimmed == "null" || trimmed == "~" || trimmed.is_empty() {
                        BTreeMap::new()
                    } else {
                        return Err(Error::FrontmatterParse(format!(
                            "invalid YAML syntax in frontmatter (near: {})",
                            result.matter.lines().next().unwrap_or("unknown")
                        )));
                    }
                }
            },
            None => {
                // gray_matter found no frontmatter delimiters, OR empty ---\n---
                if raw.starts_with("---") {
                    BTreeMap::new()
                } else {
                    return Err(Error::NoFrontmatter);
                }
            }
        };

        Ok((Self { data }, result.content))
    }

    /// Try to parse frontmatter; returns (None, full_content) if no frontmatter found.
    pub fn try_parse(raw: &str) -> Result<(Option<Self>, String)> {
        match Self::parse(raw) {
            Ok((fm, body)) => Ok((Some(fm), body)),
            Err(Error::NoFrontmatter) => Ok((None, raw.to_string())),
            Err(e) => Err(e),
        }
    }

    /// Get a value by dotted path (e.g. "links.superseded_by").
    pub fn get(&self, path: &str) -> Option<&Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current: &Value = self.data.get(parts[0])?;

        for part in &parts[1..] {
            match current {
                Value::Mapping(map) => {
                    current = map.get(Value::String(part.to_string()))?;
                }
                _ => return None,
            }
        }

        Some(current)
    }

    /// Get all keys at the top level.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.data.keys()
    }

    /// Check if a top-level field exists.
    pub fn has_field(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Get the underlying data map.
    pub fn data(&self) -> &BTreeMap<String, Value> {
        &self.data
    }

    /// Serialize to YAML string.
    pub fn to_yaml(&self) -> std::result::Result<String, serde_yaml::Error> {
        serde_yaml::to_string(&self.data)
    }

    /// Convert to JSON value.
    pub fn to_json(&self) -> serde_json::Value {
        let obj: serde_json::Map<String, serde_json::Value> = self
            .data
            .iter()
            .map(|(k, v)| (k.clone(), yaml_to_json(v)))
            .collect();
        serde_json::Value::Object(obj)
    }

    /// Get a field value as a plain string (for display).
    pub fn get_display(&self, path: &str) -> Option<String> {
        self.get(path).map(yaml_value_to_string)
    }

    /// Construct from an existing data map.
    pub fn from_data(data: BTreeMap<String, Value>) -> Self {
        Self { data }
    }

    /// Get a mutable reference to the underlying data map.
    pub fn data_mut(&mut self) -> &mut BTreeMap<String, Value> {
        &mut self.data
    }

    /// Set a top-level field.
    pub fn set(&mut self, key: &str, value: Value) {
        self.data.insert(key.to_string(), value);
    }

    /// Parse a string as a YAML value and set the field.
    pub fn set_from_str(&mut self, key: &str, raw: &str) {
        self.set(key, parse_yaml_value(raw));
    }

    /// Remove a top-level field, returning its previous value.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.data.remove(key)
    }

    /// Append a value to a sequence field. If the field is a scalar, wraps it
    /// in a sequence first. If the field is absent, creates a new sequence.
    pub fn append(&mut self, key: &str, value: Value) {
        match self.data.get_mut(key) {
            Some(Value::Sequence(seq)) => seq.push(value),
            Some(_) => {
                let existing = self.data.remove(key).unwrap();
                self.data
                    .insert(key.to_string(), Value::Sequence(vec![existing, value]));
            }
            None => {
                self.data
                    .insert(key.to_string(), Value::Sequence(vec![value]));
            }
        }
    }

    /// Parse a string as a YAML value and append to a sequence field.
    pub fn append_from_str(&mut self, key: &str, raw: &str) {
        self.append(key, parse_yaml_value(raw));
    }

    /// Serialize as YAML string (infallible for BTreeMap).
    pub fn to_yaml_string(&self) -> String {
        serde_yaml::to_string(&self.data).unwrap_or_default()
    }

    /// Serialize as grouped YAML string with blank lines between field groups.
    ///
    /// Groups (in order):
    /// 1. Core identity: status, user-type fields, date-pattern fields — in schema order
    /// 2. Scalar properties: remaining non-array fields — in schema order
    /// 3. Array metadata + relations: non-relation arrays, then relation fields
    ///
    /// Fields not in the schema are appended at the end.
    pub fn to_grouped_yaml(
        &self,
        type_def: &crate::schema::TypeDef,
        schema: &crate::schema::Schema,
    ) -> String {
        use crate::schema::FieldType;

        let relation_names: Vec<&str> = schema.all_relation_field_names();

        // Classify schema fields into groups
        let mut core_keys = Vec::new();
        let mut prop_keys = Vec::new();
        let mut array_keys = Vec::new();

        for field in &type_def.fields {
            let is_core = field.name == "status"
                || matches!(field.field_type, FieldType::User | FieldType::Org)
                || field
                    .pattern
                    .as_deref()
                    .is_some_and(|p| p.contains("\\d{4}-\\d{2}-\\d{2}"));

            if is_core {
                core_keys.push(field.name.as_str());
            } else if field.field_type.is_array() {
                array_keys.push(field.name.as_str());
            } else {
                prop_keys.push(field.name.as_str());
            }
        }

        // Relation keys present in this frontmatter, in schema relation order
        let mut rel_keys = Vec::new();
        for name in &relation_names {
            if self.data.contains_key(*name) {
                rel_keys.push(*name);
            }
        }

        let mut out = String::new();
        let mut emitted = std::collections::HashSet::new();

        let wrote_core = emit_field_group(&mut out, &core_keys, &self.data, &mut emitted);

        if wrote_core && prop_keys.iter().any(|k| self.data.contains_key(*k)) {
            out.push('\n');
        }
        let wrote_props = emit_field_group(&mut out, &prop_keys, &self.data, &mut emitted);

        let has_arrays = array_keys.iter().any(|k| {
            self.data
                .get(*k)
                .is_some_and(|v| !v.as_sequence().is_some_and(|s| s.is_empty()))
        }) || !rel_keys.is_empty();
        if (wrote_core || wrote_props) && has_arrays {
            out.push('\n');
        }
        emit_field_group(&mut out, &array_keys, &self.data, &mut emitted);
        emit_field_group(&mut out, &rel_keys, &self.data, &mut emitted);

        // Any remaining keys not in schema (unknown fields)
        let mut remaining: Vec<&String> = self
            .data
            .keys()
            .filter(|k| !emitted.contains(k.as_str()))
            .collect();
        remaining.sort();
        if !remaining.is_empty() && !out.is_empty() {
            out.push('\n');
        }
        for key in remaining {
            if let Some(val) = self.data.get(key) {
                emit_yaml_field(&mut out, key, val);
            }
        }

        out
    }
}

/// Emit all fields from `keys` that exist in `data`, tracking emitted keys.
fn emit_field_group(
    out: &mut String,
    keys: &[&str],
    data: &BTreeMap<String, Value>,
    emitted: &mut std::collections::HashSet<String>,
) -> bool {
    let mut wrote = false;
    for &key in keys {
        if let Some(val) = data.get(key) {
            emit_yaml_field(out, key, val);
            emitted.insert(key.to_string());
            wrote = true;
        }
    }
    wrote
}

/// Emit a single YAML field to `out`. Scalars use inline format, sequences use
/// block `- item` style.
fn emit_yaml_field(out: &mut String, key: &str, val: &Value) {
    match val {
        Value::Sequence(items) => {
            if items.is_empty() {
                return; // omit empty arrays — absent field is equivalent
            }
            out.push_str(key);
            out.push_str(":\n");
            for item in items {
                out.push_str("  - ");
                out.push_str(&emit_yaml_scalar(item));
                out.push('\n');
            }
        }
        _ => {
            out.push_str(key);
            out.push_str(": ");
            out.push_str(&emit_yaml_scalar(val));
            out.push('\n');
        }
    }
}

/// Emit a scalar YAML value. Strings containing special chars are quoted.
fn emit_yaml_scalar(val: &Value) -> String {
    match val {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            // Quote if contains YAML-special chars or looks like a ref/date/bool
            let needs_quote = s.contains(':')
                || s.contains('#')
                || s.contains('@')
                || s.contains('{')
                || s.contains('}')
                || s.contains('[')
                || s.contains(']')
                || s.contains('\'')
                || s.contains('"')
                || s.contains('|')
                || s.contains('>')
                || s.contains('*')
                || s.contains('&')
                || s.contains('!')
                || s.contains('%')
                || s.contains(',')
                || s.contains('?')
                || s.starts_with(' ')
                || s.ends_with(' ')
                || s.is_empty()
                || s == "true"
                || s == "false"
                || s == "null"
                || s == "yes"
                || s == "no"
                || s.parse::<f64>().is_ok();
            if needs_quote {
                format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
            } else {
                s.clone()
            }
        }
        Value::Sequence(_) | Value::Mapping(_) => serde_yaml::to_string(val)
            .unwrap_or_default()
            .trim()
            .to_string(),
        Value::Tagged(tagged) => emit_yaml_scalar(&tagged.value),
    }
}

pub fn yaml_value_to_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Sequence(seq) => {
            let items: Vec<String> = seq.iter().map(yaml_value_to_string).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Mapping(_) => serde_yaml::to_string(v).unwrap_or_default(),
        Value::Tagged(tagged) => yaml_value_to_string(&tagged.value),
    }
}

/// Parse a string into a YAML value, trying bool/number/sequence before falling back to string.
pub fn parse_yaml_value(s: &str) -> Value {
    let trimmed = s.trim();

    // Booleans
    match trimmed {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }

    // Integer
    if let Ok(n) = trimmed.parse::<i64>() {
        return Value::Number(n.into());
    }

    // Float (only if contains '.')
    if trimmed.contains('.') {
        if let Ok(f) = trimmed.parse::<f64>() {
            return Value::Number(serde_yaml::Number::from(f));
        }
    }

    // YAML sequence syntax [a, b, c]
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        if let Ok(val) = serde_yaml::from_str::<Value>(trimmed) {
            return val;
        }
    }

    // Default: string
    Value::String(s.to_string())
}

pub fn yaml_to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(serde_json::Value::Number)
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            }
        }
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Sequence(seq) => serde_json::Value::Array(seq.iter().map(yaml_to_json).collect()),
        Value::Mapping(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| {
                    let key = match k {
                        Value::String(s) => s.clone(),
                        other => yaml_value_to_string(other),
                    };
                    (key, yaml_to_json(v))
                })
                .collect();
            serde_json::Value::Object(obj)
        }
        Value::Tagged(tagged) => yaml_to_json(&tagged.value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = "---\ntitle: Test\nstatus: accepted\n---\n\n# Body\n";
        let (fm, body) = Frontmatter::parse(content).unwrap();
        assert_eq!(fm.get_display("title").unwrap(), "Test");
        assert_eq!(fm.get_display("status").unwrap(), "accepted");
        assert!(body.contains("# Body"));
    }

    #[test]
    fn test_dotted_path() {
        let content = "---\nlinks:\n  superseded_by: ADR-005\n---\nbody";
        let (fm, _) = Frontmatter::parse(content).unwrap();
        assert_eq!(fm.get_display("links.superseded_by").unwrap(), "ADR-005");
    }

    #[test]
    fn test_no_frontmatter() {
        let content = "# Just a heading\n\nNo frontmatter here.";
        let (fm, body) = Frontmatter::try_parse(content).unwrap();
        assert!(fm.is_none());
        assert!(body.contains("Just a heading"));
    }

    #[test]
    fn test_to_json() {
        let content = "---\ntitle: Test\ncount: 42\ntags:\n  - a\n  - b\n---\nbody";
        let (fm, _) = Frontmatter::parse(content).unwrap();
        let json = fm.to_json();
        assert_eq!(json["title"], "Test");
        assert_eq!(json["count"], 42);
        assert_eq!(json["tags"][0], "a");
    }

    #[test]
    fn test_has_field() {
        let content = "---\ntitle: Test\n---\nbody";
        let (fm, _) = Frontmatter::parse(content).unwrap();
        assert!(fm.has_field("title"));
        assert!(!fm.has_field("missing"));
    }

    #[test]
    fn test_set_and_remove() {
        let content = "---\ntitle: Test\n---\nbody";
        let (mut fm, _) = Frontmatter::parse(content).unwrap();
        fm.set("status", Value::String("accepted".into()));
        assert_eq!(fm.get_display("status").unwrap(), "accepted");
        let removed = fm.remove("status");
        assert!(removed.is_some());
        assert!(!fm.has_field("status"));
    }

    #[test]
    fn test_set_from_str() {
        let mut fm = Frontmatter::from_data(BTreeMap::new());
        fm.set_from_str("count", "42");
        fm.set_from_str("active", "true");
        fm.set_from_str("name", "hello");
        fm.set_from_str("tags", "[a, b]");
        assert!(matches!(fm.get("count").unwrap(), Value::Number(_)));
        assert!(matches!(fm.get("active").unwrap(), Value::Bool(true)));
        assert_eq!(fm.get_display("name").unwrap(), "hello");
        assert!(matches!(fm.get("tags").unwrap(), Value::Sequence(_)));
    }

    #[test]
    fn test_append_to_sequence() {
        let content = "---\ntags:\n  - silver\n---\nbody";
        let (mut fm, _) = Frontmatter::parse(content).unwrap();
        fm.append("tags", Value::String("backend".into()));
        match fm.get("tags").unwrap() {
            Value::Sequence(seq) => {
                assert_eq!(seq.len(), 2);
                assert_eq!(seq[1], Value::String("backend".into()));
            }
            _ => panic!("expected sequence"),
        }
    }

    #[test]
    fn test_append_to_scalar() {
        let content = "---\nowner: alice\n---\nbody";
        let (mut fm, _) = Frontmatter::parse(content).unwrap();
        fm.append("owner", Value::String("bob".into()));
        match fm.get("owner").unwrap() {
            Value::Sequence(seq) => {
                assert_eq!(seq.len(), 2);
                assert_eq!(seq[0], Value::String("alice".into()));
                assert_eq!(seq[1], Value::String("bob".into()));
            }
            _ => panic!("expected sequence"),
        }
    }

    #[test]
    fn test_append_to_missing() {
        let mut fm = Frontmatter::from_data(BTreeMap::new());
        fm.append("tags", Value::String("new".into()));
        match fm.get("tags").unwrap() {
            Value::Sequence(seq) => {
                assert_eq!(seq.len(), 1);
                assert_eq!(seq[0], Value::String("new".into()));
            }
            _ => panic!("expected sequence"),
        }
    }

    #[test]
    fn test_append_from_str() {
        let content = "---\ntags:\n  - a\n---\nbody";
        let (mut fm, _) = Frontmatter::parse(content).unwrap();
        fm.append_from_str("tags", "b");
        match fm.get("tags").unwrap() {
            Value::Sequence(seq) => assert_eq!(seq.len(), 2),
            _ => panic!("expected sequence"),
        }
    }

    #[test]
    fn test_from_data() {
        let mut data = BTreeMap::new();
        data.insert("type".into(), Value::String("adr".into()));
        let fm = Frontmatter::from_data(data);
        assert_eq!(fm.get_display("type").unwrap(), "adr");
    }

    #[test]
    fn test_to_yaml_string() {
        let content = "---\ntitle: Test\nstatus: ok\n---\nbody";
        let (fm, _) = Frontmatter::parse(content).unwrap();
        let yaml = fm.to_yaml_string();
        assert!(yaml.contains("title:"));
        assert!(yaml.contains("status:"));
    }

    #[test]
    fn test_at_prefixed_value_returns_error() {
        let content = "---\nresponders: [@onni]\n---\nbody";
        let result = Frontmatter::parse(content);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid YAML syntax"), "got: {err}");
    }

    #[test]
    fn test_empty_frontmatter_is_ok() {
        let content = "---\n---\nbody";
        let (fm, body) = Frontmatter::parse(content).unwrap();
        assert!(fm.data().is_empty());
        assert!(body.contains("body"));
    }

    #[test]
    fn test_null_frontmatter_is_ok() {
        let content = "---\nnull\n---\nbody";
        let (fm, body) = Frontmatter::parse(content).unwrap();
        assert!(fm.data().is_empty());
        assert!(body.contains("body"));
    }

    #[test]
    fn test_quoted_at_value_parses_correctly() {
        let content = "---\nresponders:\n  - \"@onni\"\n---\nbody";
        let (fm, _) = Frontmatter::parse(content).unwrap();
        let val = fm.get_display("responders").unwrap();
        assert!(val.contains("@onni"));
    }

    #[test]
    fn test_parse_yaml_value() {
        assert_eq!(parse_yaml_value("true"), Value::Bool(true));
        assert_eq!(parse_yaml_value("false"), Value::Bool(false));
        assert!(matches!(parse_yaml_value("42"), Value::Number(_)));
        assert!(matches!(parse_yaml_value("3.14"), Value::Number(_)));
        assert_eq!(parse_yaml_value("hello"), Value::String("hello".into()));
        assert!(matches!(parse_yaml_value("[a, b]"), Value::Sequence(_)));
    }
}

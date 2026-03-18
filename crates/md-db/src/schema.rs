use std::path::Path;

use kdl::{KdlDocument, KdlNode, KdlValue};
use regex::Regex;

use crate::error::{Error, Result};

/// A parsed schema containing document type definitions and relation vocabulary.
#[derive(Debug, Clone)]
pub struct Schema {
    pub types: Vec<TypeDef>,
    pub relations: Vec<RelationDef>,
    pub ref_formats: Vec<RefFormat>,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: String,
    pub description: Option<String>,
    /// Alternative names for this type (e.g. "architecture" for "adr").
    pub aliases: Vec<String>,
    /// Default folder for documents of this type (e.g. "docs/architecture")
    pub folder: Option<String>,
    /// Maximum number of documents allowed for this type (e.g. 1 for README.md)
    pub max_count: Option<usize>,
    /// Whether this is a singleton doc identified by filename, not frontmatter type field.
    pub singleton: bool,
    /// Filename pattern to match singleton docs (e.g. "README.md").
    pub match_pattern: Option<String>,
    pub fields: Vec<FieldDef>,
    pub sections: Vec<SectionDef>,
    pub rules: Vec<RuleDef>,
}

#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
    pub pattern: Option<String>,
    pub description: Option<String>,
    pub default: Option<String>,
    /// Status transitions: from → [to1, to2, ...]. Only meaningful for enum "status" fields.
    pub transitions: Vec<(String, Vec<String>)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    String,
    Number,
    Bool,
    Date,
    Enum(Vec<String>),
    Ref,
    StringArray,
    RefArray,
    User,
    UserArray,
    Org,
    OrgArray,
}

impl FieldType {
    /// True if this field type stores an array/list of values.
    pub fn is_array(&self) -> bool {
        matches!(
            self,
            FieldType::StringArray
                | FieldType::RefArray
                | FieldType::UserArray
                | FieldType::OrgArray
        )
    }
}

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::String => write!(f, "string"),
            FieldType::Number => write!(f, "number"),
            FieldType::Bool => write!(f, "bool"),
            FieldType::Date => write!(f, "date"),
            FieldType::Enum(vals) => write!(f, "enum({})", vals.join(", ")),
            FieldType::Ref => write!(f, "ref"),
            FieldType::StringArray => write!(f, "string[]"),
            FieldType::RefArray => write!(f, "ref[]"),
            FieldType::User => write!(f, "user"),
            FieldType::UserArray => write!(f, "user[]"),
            FieldType::Org => write!(f, "org"),
            FieldType::OrgArray => write!(f, "org[]"),
        }
    }
}

/// A conditional validation rule: when a field equals a value, other fields become required
/// and/or sections must contain tables.
#[derive(Debug, Clone)]
pub struct RuleDef {
    pub name: String,
    pub when_field: String,
    pub when_equals: String,
    pub when_equals_any: Vec<String>,
    pub then_required: Vec<String>,
    pub then_section_table: Vec<SectionTableOverride>,
}

impl RuleDef {
    /// Check if this rule matches a given field value.
    pub fn matches(&self, val: &str) -> bool {
        (!self.when_equals.is_empty() && val == self.when_equals)
            || self.when_equals_any.iter().any(|v| v == val)
    }

    /// Human-readable description of the match condition.
    pub fn condition_display(&self) -> String {
        if !self.when_equals.is_empty() {
            format!("{}={}", self.when_field, self.when_equals)
        } else {
            format!("{}={}", self.when_field, self.when_equals_any.join("|"))
        }
    }
}

/// Override requiring a specific section to contain a table when rule conditions are met.
#[derive(Debug, Clone)]
pub struct SectionTableOverride {
    pub section: String,
    pub table: TableDef,
}

#[derive(Debug, Clone)]
pub struct SectionDef {
    pub name: String,
    pub required: bool,
    pub description: Option<String>,
    pub children: Vec<SectionDef>,
    pub table: Option<TableDef>,
    pub content: Option<ContentDef>,
    pub list: Option<ListDef>,
    pub diagram: Option<DiagramDef>,
    /// Minimum number of subsections (child headings) required.
    pub min_subsections: Option<usize>,
    /// Whether a GFM callout (`> [!NOTE]`, `> [!WARNING]`, etc.) is required.
    pub callout_required: bool,
}

#[derive(Debug, Clone)]
pub struct ContentDef {
    pub min_paragraphs: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct ListDef {
    pub required: bool,
    pub min_items: Option<usize>,
    pub ordered: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct DiagramDef {
    pub required: bool,
    pub diagram_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TableDef {
    pub required: bool,
    pub description: Option<String>,
    pub columns: Vec<ColumnDef>,
}

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub col_type: FieldType,
    pub required: bool,
    pub description: Option<String>,
}

/// A user-defined relationship type. Defined once at schema level,
/// available as frontmatter fields on all document types.
#[derive(Debug, Clone)]
pub struct RelationDef {
    /// The frontmatter field name (e.g. "supersedes").
    pub name: String,
    /// The inverse relation name (e.g. "superseded_by"). Optional for symmetric relations.
    pub inverse: Option<String>,
    /// "one" or "many" — determines if the field is `ref` or `ref[]`.
    pub cardinality: Cardinality,
    pub description: Option<String>,
    /// If true, cycles through this relation are reported as errors.
    pub acyclic: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    One,
    Many,
}

impl RelationDef {
    /// The FieldType this relation maps to based on cardinality.
    pub fn field_type(&self) -> FieldType {
        match self.cardinality {
            Cardinality::One => FieldType::Ref,
            Cardinality::Many => FieldType::RefArray,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RefFormat {
    pub name: String,
    pub pattern: String,
}

impl Schema {
    /// Parse a KDL schema from a file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(Error::FileNotFound(path.to_path_buf()));
        }
        let content = std::fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    /// Parse a KDL schema from a string.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(content: &str) -> Result<Self> {
        let doc: KdlDocument = content
            .parse()
            .map_err(|e: kdl::KdlError| Error::SchemaParse(format!("{e:#}")))?;

        let mut types = Vec::new();
        let mut relations = Vec::new();
        let mut ref_formats = Vec::new();

        for node in doc.nodes() {
            match node.name().value() {
                "type" => types.push(parse_type_def(node)?),
                "relation" => relations.push(parse_relation_def(node)?),
                "ref-format" => ref_formats.extend(parse_ref_formats(node)?),
                other => {
                    return Err(Error::SchemaParse(format!(
                        "unknown top-level node: '{other}'"
                    )));
                }
            }
        }

        Ok(Self {
            types,
            relations,
            ref_formats,
        })
    }

    /// Look up a type definition by name or alias.
    pub fn get_type(&self, name: &str) -> Option<&TypeDef> {
        self.types
            .iter()
            .find(|t| t.name == name || t.aliases.iter().any(|a| a == name))
    }

    /// Look up a type definition by document ID (e.g. "ADR-001" → adr type).
    pub fn get_type_for_doc_id(&self, doc_id: &str) -> Option<&TypeDef> {
        let prefix = doc_id.split('-').next()?.to_lowercase();
        self.get_type(&prefix)
    }

    /// Get the canonical type name for a document ID (e.g. "ADR-001" → "adr").
    pub fn type_name_for_doc_id(&self, doc_id: &str) -> Option<String> {
        self.get_type_for_doc_id(doc_id).map(|t| t.name.clone())
    }

    /// Get uppercase type name prefixes (e.g. ["ADR", "OPP", "POL", "INC"]).
    pub fn type_prefixes(&self) -> Vec<String> {
        self.types.iter().map(|t| t.name.to_uppercase()).collect()
    }

    /// Get all relation field names (both direct names and inverse names).
    /// These are valid frontmatter fields on any document type.
    pub fn all_relation_field_names(&self) -> Vec<&str> {
        let mut names = Vec::new();
        for r in &self.relations {
            names.push(r.name.as_str());
            if let Some(ref inv) = r.inverse {
                names.push(inv.as_str());
            }
        }
        names
    }

    /// Find a relation definition by field name (checks both name and inverse).
    pub fn find_relation(&self, field_name: &str) -> Option<(&RelationDef, bool)> {
        for r in &self.relations {
            if r.name == field_name {
                return Some((r, false));
            }
            if let Some(ref inv) = r.inverse {
                if inv == field_name {
                    return Some((r, true));
                }
            }
        }
        None
    }

    /// Get the cardinality for a relation field name.
    /// Inverse relations inherit the parent's cardinality.
    pub fn relation_cardinality(&self, field_name: &str) -> Option<Cardinality> {
        self.find_relation(field_name).map(|(r, _)| r.cardinality)
    }

    /// Compile the `string-id` ref-format pattern into a Regex, if defined.
    pub fn id_regex(&self) -> Option<Regex> {
        self.ref_formats
            .iter()
            .find(|rf| rf.name == "string-id")
            .and_then(|rf| Regex::new(&rf.pattern).ok())
    }

    /// Parse a document ID from a filename stem using the schema's ref-format pattern.
    /// Falls back to byte-level extraction if no schema pattern matches.
    pub fn parse_id_from_filename(&self, stem: &str) -> Option<String> {
        let upper = stem.to_uppercase().replace('_', "-");
        if let Some(re) = self.id_regex() {
            if let Some(m) = re.find(&upper) {
                return Some(m.as_str().to_string());
            }
        }
        // Fallback: extract PREFIX-NNN from the beginning
        crate::graph::extract_id_from_stem(&upper)
    }

    /// Check if a string is a valid document ID according to the schema's ref-format.
    /// Falls back to structural check if no schema pattern is defined.
    pub fn is_valid_id(&self, s: &str) -> bool {
        if let Some(re) = self.id_regex() {
            re.is_match(&s.to_uppercase())
        } else {
            crate::graph::is_string_id_fallback(s)
        }
    }
}

fn parse_type_def(node: &KdlNode) -> Result<TypeDef> {
    let name = node
        .entries()
        .iter()
        .find(|e| e.name().is_none())
        .and_then(|e| e.value().as_string())
        .ok_or_else(|| Error::SchemaParse("type node missing name argument".into()))?
        .to_string();

    let description = get_string_prop(node, "description");
    let folder = get_string_prop(node, "folder");
    let max_count = get_i64_prop(node, "max_count").map(|n| n as usize);
    let singleton = get_bool_prop(node, "singleton").unwrap_or(false);

    let children = node
        .children()
        .ok_or_else(|| Error::SchemaParse(format!("type '{name}' has no body")))?;

    let mut aliases = Vec::new();
    let mut fields = Vec::new();
    let mut sections = Vec::new();
    let mut match_pattern = None;
    let mut rules = Vec::new();
    for child in children.nodes() {
        match child.name().value() {
            "alias" => {
                for entry in child.entries() {
                    if entry.name().is_none() {
                        if let Some(s) = entry.value().as_string() {
                            aliases.push(s.to_string());
                        }
                    }
                }
            }
            "field" => {
                if singleton {
                    return Err(Error::SchemaParse(format!(
                        "singleton type '{name}' cannot have field definitions"
                    )));
                }
                fields.push(parse_field_def(child)?);
            }
            "section" => sections.push(parse_section_def(child)?),
            "match" => {
                match_pattern = get_string_arg(child);
                if match_pattern.is_none() {
                    return Err(Error::SchemaParse(format!(
                        "match node in type '{name}' missing pattern argument"
                    )));
                }
            }
            "rule" => rules.push(parse_rule_def(child)?),
            other => {
                return Err(Error::SchemaParse(format!(
                    "unknown node in type '{name}': '{other}'"
                )));
            }
        }
    }

    if singleton && match_pattern.is_none() {
        return Err(Error::SchemaParse(format!(
            "singleton type '{name}' requires a match pattern"
        )));
    }

    Ok(TypeDef {
        name,
        description,
        aliases,
        folder,
        max_count,
        singleton,
        match_pattern,
        fields,
        sections,
        rules,
    })
}

fn parse_field_def(node: &KdlNode) -> Result<FieldDef> {
    let name =
        get_string_arg(node).ok_or_else(|| Error::SchemaParse("field node missing name".into()))?;

    let type_str = get_string_prop(node, "type").unwrap_or("string".into());
    let required = get_bool_prop(node, "required").unwrap_or(false);
    let pattern = get_string_prop(node, "pattern");
    let description = get_string_prop(node, "description");
    let default = get_string_prop(node, "default");

    let field_type = parse_field_type(&type_str, node)?;

    // Parse transition nodes: `transition "from" "to1" "to2" ...`
    let transitions = node
        .children()
        .map(|c| {
            c.nodes()
                .iter()
                .filter(|n| n.name().value() == "transition")
                .filter_map(|t| {
                    let args: Vec<String> = t
                        .entries()
                        .iter()
                        .filter(|e| e.name().is_none())
                        .filter_map(|e| e.value().as_string().map(|s| s.to_string()))
                        .collect();
                    if args.len() >= 2 {
                        let from = args[0].clone();
                        let to = args[1..].to_vec();
                        Some((from, to))
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(FieldDef {
        name,
        field_type,
        required,
        pattern,
        description,
        default,
        transitions,
    })
}

fn parse_field_type(type_str: &str, node: &KdlNode) -> Result<FieldType> {
    match type_str {
        "string" => Ok(FieldType::String),
        "number" => Ok(FieldType::Number),
        "bool" => Ok(FieldType::Bool),
        "date" => Ok(FieldType::Date),
        "ref" => Ok(FieldType::Ref),
        "string[]" => Ok(FieldType::StringArray),
        "ref[]" => Ok(FieldType::RefArray),
        "user" => Ok(FieldType::User),
        "user[]" => Ok(FieldType::UserArray),
        "org" => Ok(FieldType::Org),
        "org[]" => Ok(FieldType::OrgArray),
        "enum" => {
            let values = node
                .children()
                .and_then(|c| {
                    c.nodes()
                        .iter()
                        .find(|n| n.name().value() == "values")
                        .map(|values_node| {
                            values_node
                                .entries()
                                .iter()
                                .filter(|e| e.name().is_none())
                                .filter_map(|e| e.value().as_string().map(|s| s.to_string()))
                                .collect::<Vec<_>>()
                        })
                })
                .unwrap_or_default();

            if values.is_empty() {
                return Err(Error::SchemaParse(
                    "enum field has no values defined".into(),
                ));
            }

            Ok(FieldType::Enum(values))
        }
        other => Err(Error::SchemaParse(format!("unknown field type: '{other}'"))),
    }
}

fn parse_section_def(node: &KdlNode) -> Result<SectionDef> {
    let name = get_string_arg(node)
        .ok_or_else(|| Error::SchemaParse("section node missing name".into()))?;
    let required = get_bool_prop(node, "required").unwrap_or(false);
    let description = get_string_prop(node, "description");
    let min_subsections = get_i64_prop(node, "min-subsections").map(|n| n as usize);
    let callout_required = get_bool_prop(node, "callout").unwrap_or(false);

    let mut children = Vec::new();
    let mut table = None;
    let mut content = None;
    let mut list = None;
    let mut diagram = None;

    if let Some(body) = node.children() {
        for child in body.nodes() {
            match child.name().value() {
                "section" => children.push(parse_section_def(child)?),
                "table" => table = Some(parse_table_def(child)?),
                "content" => content = Some(parse_content_def(child)?),
                "list" => list = Some(parse_list_def(child)?),
                "diagram" => diagram = Some(parse_diagram_def(child)?),
                other => {
                    return Err(Error::SchemaParse(format!(
                        "unknown node in section '{name}': '{other}'"
                    )));
                }
            }
        }
    }

    Ok(SectionDef {
        name,
        required,
        description,
        children,
        table,
        content,
        list,
        diagram,
        min_subsections,
        callout_required,
    })
}

fn parse_table_def(node: &KdlNode) -> Result<TableDef> {
    let required = get_bool_prop(node, "required").unwrap_or(false);
    let description = get_string_prop(node, "description");
    let mut columns = Vec::new();

    if let Some(body) = node.children() {
        for child in body.nodes() {
            if child.name().value() == "column" {
                columns.push(parse_column_def(child)?);
            }
        }
    }

    Ok(TableDef {
        required,
        description,
        columns,
    })
}

fn parse_column_def(node: &KdlNode) -> Result<ColumnDef> {
    let name = get_string_arg(node)
        .ok_or_else(|| Error::SchemaParse("column node missing name".into()))?;
    let type_str = get_string_prop(node, "type").unwrap_or("string".into());
    let required = get_bool_prop(node, "required").unwrap_or(false);
    let description = get_string_prop(node, "description");

    let col_type = match type_str.as_str() {
        "string" => FieldType::String,
        "number" => FieldType::Number,
        "date" => FieldType::Date,
        "user" => FieldType::User,
        "org" => FieldType::Org,
        "enum" => {
            let values = node
                .children()
                .and_then(|c| {
                    c.nodes()
                        .iter()
                        .find(|n| n.name().value() == "values")
                        .map(|values_node| {
                            values_node
                                .entries()
                                .iter()
                                .filter(|e| e.name().is_none())
                                .filter_map(|e| e.value().as_string().map(|s| s.to_string()))
                                .collect::<Vec<_>>()
                        })
                })
                .unwrap_or_default();
            if values.is_empty() {
                return Err(Error::SchemaParse(
                    "enum column has no values defined".into(),
                ));
            }
            FieldType::Enum(values)
        }
        other => {
            return Err(Error::SchemaParse(format!(
                "unknown column type: '{other}'"
            )));
        }
    };

    Ok(ColumnDef {
        name,
        col_type,
        required,
        description,
    })
}

fn parse_relation_def(node: &KdlNode) -> Result<RelationDef> {
    let name = get_string_arg(node)
        .ok_or_else(|| Error::SchemaParse("relation node missing name".into()))?;

    let inverse = get_string_prop(node, "inverse");
    let description = get_string_prop(node, "description");
    let acyclic = get_bool_prop(node, "acyclic");

    let cardinality_str = get_string_prop(node, "cardinality").unwrap_or("many".into());
    let cardinality = match cardinality_str.as_str() {
        "one" => Cardinality::One,
        "many" => Cardinality::Many,
        other => {
            return Err(Error::SchemaParse(format!(
                "unknown cardinality '{other}' for relation '{name}', expected 'one' or 'many'"
            )));
        }
    };

    Ok(RelationDef {
        name,
        inverse,
        cardinality,
        description,
        acyclic,
    })
}

fn parse_content_def(node: &KdlNode) -> Result<ContentDef> {
    Ok(ContentDef {
        min_paragraphs: get_i64_prop(node, "min-paragraphs").map(|n| n as usize),
    })
}

fn parse_list_def(node: &KdlNode) -> Result<ListDef> {
    Ok(ListDef {
        required: get_bool_prop(node, "required").unwrap_or(true),
        min_items: get_i64_prop(node, "min-items").map(|n| n as usize),
        ordered: get_bool_prop(node, "ordered"),
    })
}

fn parse_diagram_def(node: &KdlNode) -> Result<DiagramDef> {
    Ok(DiagramDef {
        required: get_bool_prop(node, "required").unwrap_or(true),
        diagram_type: get_string_prop(node, "type"),
    })
}

fn parse_rule_def(node: &KdlNode) -> Result<RuleDef> {
    let name = get_string_arg(node)
        .ok_or_else(|| Error::SchemaParse("rule node missing name argument".into()))?;

    let mut when_field = String::new();
    let mut when_equals = String::new();
    let mut when_equals_any = Vec::new();
    let mut then_required = Vec::new();
    let mut then_section_table = Vec::new();

    if let Some(body) = node.children() {
        for child in body.nodes() {
            match child.name().value() {
                "when" => {
                    when_field = get_string_arg(child).unwrap_or_default();
                    when_equals = get_string_prop(child, "equals").unwrap_or_default();
                    if let Some(any_str) = get_string_prop(child, "equals-any") {
                        when_equals_any =
                            any_str.split(',').map(|s| s.trim().to_string()).collect();
                    }
                }
                "then-required" => {
                    if let Some(field_name) = get_string_arg(child) {
                        then_required.push(field_name);
                    }
                }
                "then-section-table" => {
                    let section = get_string_arg(child).ok_or_else(|| {
                        Error::SchemaParse(format!(
                            "then-section-table in rule '{name}' missing section name"
                        ))
                    })?;
                    let table_node = child
                        .children()
                        .and_then(|c| {
                            c.nodes()
                                .iter()
                                .find(|n| n.name().value() == "table")
                        })
                        .ok_or_else(|| {
                            Error::SchemaParse(format!(
                                "then-section-table '{section}' in rule '{name}' missing table definition"
                            ))
                        })?;
                    let table = parse_table_def(table_node)?;
                    then_section_table.push(SectionTableOverride { section, table });
                }
                other => {
                    return Err(Error::SchemaParse(format!(
                        "unknown node in rule '{name}': '{other}'"
                    )));
                }
            }
        }
    }

    if when_field.is_empty() {
        return Err(Error::SchemaParse(format!(
            "rule '{name}' missing 'when' clause"
        )));
    }
    if then_required.is_empty() && then_section_table.is_empty() {
        return Err(Error::SchemaParse(format!(
            "rule '{name}' missing 'then-required' or 'then-section-table' clause"
        )));
    }

    Ok(RuleDef {
        name,
        when_field,
        when_equals,
        when_equals_any,
        then_required,
        then_section_table,
    })
}

fn parse_ref_formats(node: &KdlNode) -> Result<Vec<RefFormat>> {
    let mut formats = Vec::new();
    if let Some(body) = node.children() {
        for child in body.nodes() {
            let name = child.name().value().to_string();
            let pattern = get_string_prop(child, "pattern").ok_or_else(|| {
                Error::SchemaParse(format!("ref-format '{name}' missing pattern"))
            })?;
            formats.push(RefFormat { name, pattern });
        }
    }
    Ok(formats)
}

// ─── KDL helper functions ────────────────────────────────────────────────────

fn get_string_arg(node: &KdlNode) -> Option<String> {
    node.entries()
        .iter()
        .find(|e| e.name().is_none())
        .and_then(|e| e.value().as_string())
        .map(|s| s.to_string())
}

fn get_string_prop(node: &KdlNode, key: &str) -> Option<String> {
    node.entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some(key))
        .and_then(|e| e.value().as_string())
        .map(|s| s.to_string())
}

fn get_bool_prop(node: &KdlNode, key: &str) -> Option<bool> {
    node.entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some(key))
        .and_then(|e| match e.value() {
            KdlValue::Bool(b) => Some(*b),
            _ => None,
        })
}

fn get_i64_prop(node: &KdlNode, key: &str) -> Option<i64> {
    node.entries()
        .iter()
        .find(|e| e.name().map(|n| n.value()) == Some(key))
        .and_then(|e| e.value().as_integer())
        .map(|n| n as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_kdl_parse() {
        let simple = "node \"hello\"";
        let result: std::result::Result<KdlDocument, _> = simple.parse();
        assert!(result.is_ok(), "simple: {result:?}");

        let prop = "node key=\"val\"";
        let result: std::result::Result<KdlDocument, _> = prop.parse();
        assert!(result.is_ok(), "prop: {result:?}");

        let bool_prop = "node required=#true";
        let result: std::result::Result<KdlDocument, _> = bool_prop.parse();
        assert!(result.is_ok(), "bool: {result:?}");

        let children = "parent {\n    child \"val\"\n}";
        let result: std::result::Result<KdlDocument, _> = children.parse();
        assert!(result.is_ok(), "children: {result:?}");

        let combined = "type \"test\" {\n    field \"title\" type=\"string\" required=#true\n}";
        let result: std::result::Result<KdlDocument, _> = combined.parse();
        assert!(result.is_ok(), "combined: {result:?}");
    }

    #[test]
    fn test_parse_minimal_schema() {
        let kdl = r#"
type "test" {
    field "title" type="string" required=#true
    field "status" type="enum" required=#true {
        values "a" "b" "c"
    }
    section "Body" required=#true
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        assert_eq!(schema.types.len(), 1);
        let t = &schema.types[0];
        assert_eq!(t.name, "test");
        assert_eq!(t.fields.len(), 2);
        assert_eq!(t.fields[0].name, "title");
        assert!(t.fields[0].required);
        assert_eq!(
            t.fields[1].field_type,
            FieldType::Enum(vec!["a".into(), "b".into(), "c".into()])
        );
        assert_eq!(t.sections.len(), 1);
        assert!(t.sections[0].required);
    }

    #[test]
    fn test_parse_nested_sections() {
        let kdl = r#"
type "doc" {
    section "Parent" required=#true {
        section "Child1" required=#true
        section "Child2"
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let sec = &schema.types[0].sections[0];
        assert_eq!(sec.children.len(), 2);
        assert!(sec.children[0].required);
        assert!(!sec.children[1].required);
    }

    #[test]
    fn test_parse_table_def() {
        let kdl = r#"
type "doc" {
    section "Data" {
        table required=#true {
            column "Name" type="string" required=#true
            column "Score" type="number"
        }
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let table = schema.types[0].sections[0].table.as_ref().unwrap();
        assert!(table.required);
        assert_eq!(table.columns.len(), 2);
        assert!(table.columns[0].required);
        assert_eq!(table.columns[1].col_type, FieldType::Number);
    }

    #[test]
    fn test_parse_relations() {
        let kdl = r#"
relation "supersedes" inverse="superseded_by" cardinality="one"
relation "enables" inverse="enabled_by" cardinality="many"
relation "related" cardinality="many"

type "t" {
    field "title" type="string"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        assert_eq!(schema.relations.len(), 3);

        assert_eq!(schema.relations[0].name, "supersedes");
        assert_eq!(
            schema.relations[0].inverse.as_deref(),
            Some("superseded_by")
        );
        assert_eq!(schema.relations[0].cardinality, Cardinality::One);

        assert_eq!(schema.relations[1].name, "enables");
        assert_eq!(schema.relations[1].cardinality, Cardinality::Many);

        assert_eq!(schema.relations[2].name, "related");
        assert!(schema.relations[2].inverse.is_none());

        // all_relation_field_names includes both names and inverses
        let names = schema.all_relation_field_names();
        assert!(names.contains(&"supersedes"));
        assert!(names.contains(&"superseded_by"));
        assert!(names.contains(&"enables"));
        assert!(names.contains(&"enabled_by"));
        assert!(names.contains(&"related"));
        assert_eq!(names.len(), 5);
    }

    #[test]
    fn test_find_relation() {
        let kdl = r#"
relation "supersedes" inverse="superseded_by" cardinality="one"
type "t" { field "x" type="string" }
"#;
        let schema = Schema::from_str(kdl).unwrap();

        // Find by direct name
        let (r, is_inverse) = schema.find_relation("supersedes").unwrap();
        assert_eq!(r.name, "supersedes");
        assert!(!is_inverse);

        // Find by inverse name
        let (r, is_inverse) = schema.find_relation("superseded_by").unwrap();
        assert_eq!(r.name, "supersedes");
        assert!(is_inverse);

        // Not found
        assert!(schema.find_relation("unknown").is_none());
    }

    #[test]
    fn test_parse_ref_formats() {
        let kdl = r#"
type "t" {
    field "x" type="ref"
}
ref-format {
    string-id pattern="^ADR-\\d+$"
    relative-path pattern="\\.md$"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        assert_eq!(schema.ref_formats.len(), 2);
        assert_eq!(schema.ref_formats[0].name, "string-id");
    }

    #[test]
    fn test_parse_full_schema_file() {
        let content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        let schema = Schema::from_str(&content).unwrap();
        assert_eq!(schema.types.len(), 4);
        assert!(!schema.relations.is_empty());

        let names: Vec<&str> = schema.types.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"adr"));
        assert!(names.contains(&"opp"));
        assert!(names.contains(&"pol"));
        assert!(names.contains(&"inc"));

        // Relations are defined at schema level
        let rel_names = schema.all_relation_field_names();
        assert!(rel_names.contains(&"supersedes"));
        assert!(rel_names.contains(&"superseded_by"));
    }

    #[test]
    fn test_parse_descriptions() {
        let kdl = r#"
relation "supersedes" inverse="superseded_by" cardinality="one" description="Replaces a decision"

type "adr" description="Architecture Decision Record" {
    field "title" type="string" required=#true description="Short summary"
    section "Decision" required=#true description="The decision" {
        table description="Options" {
            column "Name" type="string" description="Option name"
        }
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let t = &schema.types[0];
        assert_eq!(
            t.description.as_deref(),
            Some("Architecture Decision Record")
        );
        assert_eq!(t.fields[0].description.as_deref(), Some("Short summary"));
        assert_eq!(t.sections[0].description.as_deref(), Some("The decision"));

        let table = t.sections[0].table.as_ref().unwrap();
        assert_eq!(table.description.as_deref(), Some("Options"));
        assert_eq!(table.columns[0].description.as_deref(), Some("Option name"));

        assert_eq!(
            schema.relations[0].description.as_deref(),
            Some("Replaces a decision")
        );
    }

    #[test]
    fn test_parse_descriptions_absent() {
        let kdl = r#"
type "t" {
    field "x" type="string"
    section "S"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        assert!(schema.types[0].description.is_none());
        assert!(schema.types[0].fields[0].description.is_none());
        assert!(schema.types[0].sections[0].description.is_none());
    }

    #[test]
    fn test_parse_defaults() {
        let kdl = r#"
type "t" {
    field "status" type="enum" default="proposed" {
        values "proposed" "accepted"
    }
    field "date" type="string" default="$TODAY"
    field "title" type="string"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        assert_eq!(
            schema.types[0].fields[0].default.as_deref(),
            Some("proposed")
        );
        assert_eq!(schema.types[0].fields[1].default.as_deref(), Some("$TODAY"));
        assert!(schema.types[0].fields[2].default.is_none());
    }

    #[test]
    fn test_parse_content_constraint() {
        let kdl = r#"
type "t" {
    section "Body" required=#true {
        content min-paragraphs=2
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let content = schema.types[0].sections[0].content.as_ref().unwrap();
        assert_eq!(content.min_paragraphs, Some(2));
    }

    #[test]
    fn test_parse_list_constraint() {
        let kdl = r#"
type "t" {
    section "Reqs" {
        list min-items=3
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let list = schema.types[0].sections[0].list.as_ref().unwrap();
        assert!(list.required);
        assert_eq!(list.min_items, Some(3));
    }

    #[test]
    fn test_parse_diagram_constraint() {
        let kdl = r#"
type "t" {
    section "Arch" {
        diagram type="mermaid"
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let diagram = schema.types[0].sections[0].diagram.as_ref().unwrap();
        assert!(diagram.required);
        assert_eq!(diagram.diagram_type.as_deref(), Some("mermaid"));
    }

    #[test]
    fn test_parse_diagram_constraint_any() {
        let kdl = r#"
type "t" {
    section "Arch" {
        diagram
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let diagram = schema.types[0].sections[0].diagram.as_ref().unwrap();
        assert!(diagram.required);
        assert!(diagram.diagram_type.is_none());
    }

    #[test]
    fn test_parse_folder_and_max_count() {
        let kdl = r#"
type "readme" folder="." max_count=1 {
    field "title" type="string" required=#true
    section "Body" required=#true
}
type "adr" folder="docs/architecture" {
    field "title" type="string"
    section "Decision"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();

        let readme = &schema.types[0];
        assert_eq!(readme.folder.as_deref(), Some("."));
        assert_eq!(readme.max_count, Some(1));

        let adr = &schema.types[1];
        assert_eq!(adr.folder.as_deref(), Some("docs/architecture"));
        assert!(adr.max_count.is_none());
    }

    #[test]
    fn test_parse_folder_absent() {
        let kdl = r#"
type "t" {
    field "x" type="string"
    section "S"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        assert!(schema.types[0].folder.is_none());
        assert!(schema.types[0].max_count.is_none());
    }

    #[test]
    fn test_parse_rules() {
        let kdl = r#"
type "adr" {
    field "status" type="enum" required=#true {
        values "proposed" "accepted" "superseded"
    }
    field "date" type="string"
    field "superseded_by" type="ref"
    section "Decision" required=#true

    rule "accepted requires date" {
        when "status" equals="accepted"
        then-required "date"
    }
    rule "superseded requires superseded_by" {
        when "status" equals="superseded"
        then-required "superseded_by"
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let t = &schema.types[0];
        assert_eq!(t.rules.len(), 2);

        assert_eq!(t.rules[0].name, "accepted requires date");
        assert_eq!(t.rules[0].when_field, "status");
        assert_eq!(t.rules[0].when_equals, "accepted");
        assert_eq!(t.rules[0].then_required, vec!["date"]);

        assert_eq!(t.rules[1].name, "superseded requires superseded_by");
        assert_eq!(t.rules[1].when_field, "status");
        assert_eq!(t.rules[1].when_equals, "superseded");
        assert_eq!(t.rules[1].then_required, vec!["superseded_by"]);
    }

    #[test]
    fn test_parse_rule_multiple_then_required() {
        let kdl = r#"
type "t" {
    field "status" type="string"
    field "a" type="string"
    field "b" type="string"
    section "S"

    rule "active requires a and b" {
        when "status" equals="active"
        then-required "a"
        then-required "b"
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let rule = &schema.types[0].rules[0];
        assert_eq!(rule.then_required, vec!["a", "b"]);
    }

    #[test]
    fn test_type_without_rules() {
        let kdl = r#"
type "t" {
    field "x" type="string"
    section "S"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        assert!(schema.types[0].rules.is_empty());
    }

    #[test]
    fn test_rule_matches_equals() {
        let rule = RuleDef {
            name: "test".into(),
            when_field: "status".into(),
            when_equals: "accepted".into(),
            when_equals_any: vec![],
            then_required: vec!["date".into()],
            then_section_table: vec![],
        };
        assert!(rule.matches("accepted"));
        assert!(!rule.matches("proposed"));
        assert!(!rule.matches(""));
    }

    #[test]
    fn test_rule_matches_equals_any() {
        let rule = RuleDef {
            name: "test".into(),
            when_field: "status".into(),
            when_equals: String::new(),
            when_equals_any: vec!["pursuing".into(), "completed".into()],
            then_required: vec!["date".into()],
            then_section_table: vec![],
        };
        assert!(rule.matches("pursuing"));
        assert!(rule.matches("completed"));
        assert!(!rule.matches("proposed"));
        assert!(!rule.matches(""));
    }

    #[test]
    fn test_rule_condition_display() {
        let single = RuleDef {
            name: "test".into(),
            when_field: "status".into(),
            when_equals: "accepted".into(),
            when_equals_any: vec![],
            then_required: vec![],
            then_section_table: vec![],
        };
        assert_eq!(single.condition_display(), "status=accepted");

        let multi = RuleDef {
            name: "test".into(),
            when_field: "status".into(),
            when_equals: String::new(),
            when_equals_any: vec!["pursuing".into(), "completed".into()],
            then_required: vec![],
            then_section_table: vec![],
        };
        assert_eq!(multi.condition_display(), "status=pursuing|completed");
    }

    #[test]
    fn test_parse_rule_equals_any() {
        let kdl = r#"
type "opp" {
    field "status" type="string"
    field "date" type="string"
    section "S"

    rule "active needs date" {
        when "status" equals-any="pursuing,completed"
        then-required "date"
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let rule = &schema.types[0].rules[0];
        assert!(rule.when_equals.is_empty());
        assert_eq!(rule.when_equals_any, vec!["pursuing", "completed"]);
        assert!(rule.matches("pursuing"));
        assert!(rule.matches("completed"));
        assert!(!rule.matches("exploring"));
    }

    #[test]
    fn test_parse_rule_then_section_table() {
        let kdl = r#"
type "opp" {
    field "status" type="string"
    section "Action Items"

    rule "pursuing needs table" {
        when "status" equals="pursuing"
        then-section-table "Action Items" {
            table {
                column "Status" type="string" required=#true
                column "Item" type="string" required=#true
            }
        }
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let rule = &schema.types[0].rules[0];
        assert_eq!(rule.then_section_table.len(), 1);
        assert_eq!(rule.then_section_table[0].section, "Action Items");
        assert_eq!(rule.then_section_table[0].table.columns.len(), 2);
        assert_eq!(rule.then_section_table[0].table.columns[0].name, "Status");
        assert!(rule.then_section_table[0].table.columns[0].required);
    }

    #[test]
    fn test_parse_enum_column_type() {
        let kdl = r#"
type "t" {
    field "x" type="string"
    section "Data" {
        table {
            column "Priority" type="enum" {
                values "high" "medium" "low"
            }
        }
    }
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let col = &schema.types[0].sections[0].table.as_ref().unwrap().columns[0];
        assert_eq!(
            col.col_type,
            FieldType::Enum(vec!["high".into(), "medium".into(), "low".into()])
        );
    }

    #[test]
    fn test_parse_enum_column_no_values_error() {
        let kdl = r#"
type "t" {
    field "x" type="string"
    section "Data" {
        table {
            column "Priority" type="enum"
        }
    }
}
"#;
        let result = Schema::from_str(kdl);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("no values"));
    }

    #[test]
    fn test_parse_alias() {
        let kdl = r#"
type "adr" {
    alias "architecture" "tech"
    field "title" type="string"
    section "Decision"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let t = &schema.types[0];
        assert_eq!(t.aliases, vec!["architecture", "tech"]);
    }

    #[test]
    fn test_get_type_by_alias() {
        let kdl = r#"
type "adr" {
    alias "architecture" "tech"
    field "title" type="string"
    section "Decision"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        assert!(schema.get_type("architecture").is_some());
        assert_eq!(schema.get_type("architecture").unwrap().name, "adr");
        assert!(schema.get_type("tech").is_some());
        assert_eq!(schema.get_type("tech").unwrap().name, "adr");
        assert!(schema.get_type("adr").is_some());
        assert!(schema.get_type("nonexistent").is_none());
    }

    #[test]
    fn test_no_aliases() {
        let kdl = r#"
type "t" {
    field "x" type="string"
    section "S"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        assert!(schema.types[0].aliases.is_empty());
    }
}

#[cfg(test)]
mod id_parser_tests {
    use super::*;

    fn fixture_schema() -> Schema {
        let content = std::fs::read_to_string("../../tests/fixtures/schema.kdl").unwrap();
        Schema::from_str(&content).unwrap()
    }

    #[test]
    fn test_id_regex_from_fixture() {
        let schema = fixture_schema();
        let re = schema
            .id_regex()
            .expect("fixture schema should have string-id ref-format");
        assert!(re.is_match("ADR-001"));
        assert!(re.is_match("OPP-042"));
        assert!(re.is_match("POL-100"));
        assert!(re.is_match("INC-999"));
        assert!(!re.is_match("CUSTOM-001"));
        assert!(!re.is_match("adr-001")); // pattern is uppercase
        assert!(!re.is_match("ADR001"));
    }

    #[test]
    fn test_parse_id_from_filename() {
        let schema = fixture_schema();
        assert_eq!(
            schema.parse_id_from_filename("adr-001"),
            Some("ADR-001".to_string())
        );
        assert_eq!(
            schema.parse_id_from_filename("opp-003-expand-to-europe"),
            Some("OPP-003".to_string())
        );
        assert_eq!(
            schema.parse_id_from_filename("inc_002"),
            Some("INC-002".to_string())
        );
    }

    #[test]
    fn test_is_valid_id() {
        let schema = fixture_schema();
        assert!(schema.is_valid_id("ADR-001"));
        assert!(schema.is_valid_id("opp-042")); // case-insensitive
        assert!(!schema.is_valid_id("CUSTOM-001"));
        assert!(!schema.is_valid_id("https://example.com"));
        assert!(!schema.is_valid_id("./adr-001.md"));
    }

    #[test]
    fn test_id_regex_none_when_no_ref_format() {
        let schema = Schema {
            types: vec![],
            relations: vec![],
            ref_formats: vec![],
        };
        assert!(schema.id_regex().is_none());
    }

    #[test]
    fn test_is_valid_id_fallback_without_schema() {
        let schema = Schema {
            types: vec![],
            relations: vec![],
            ref_formats: vec![],
        };
        // Falls back to structural check
        assert!(schema.is_valid_id("ADR-001"));
        assert!(schema.is_valid_id("GOV_003"));
        assert!(!schema.is_valid_id("just-text"));
    }
}

#[cfg(test)]
mod singleton_tests {
    use super::*;

    #[test]
    fn test_parse_singleton_type() {
        let kdl = r#"
type "readme" folder="." max_count=1 singleton=#true {
    match "README.md"
    section "Install" required=#true
    section "Usage" required=#true
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        let t = &schema.types[0];
        assert_eq!(t.name, "readme");
        assert!(t.singleton);
        assert_eq!(t.match_pattern.as_deref(), Some("README.md"));
        assert!(t.fields.is_empty());
        assert_eq!(t.sections.len(), 2);
        assert_eq!(t.max_count, Some(1));
    }

    #[test]
    fn test_singleton_rejects_fields() {
        let kdl = r#"
type "readme" singleton=#true {
    match "README.md"
    field "title" type="string"
}
"#;
        let result = Schema::from_str(kdl);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("cannot have field definitions"));
    }

    #[test]
    fn test_singleton_requires_match() {
        let kdl = r#"
type "readme" singleton=#true {
    section "Body"
}
"#;
        let result = Schema::from_str(kdl);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("requires a match pattern"));
    }

    #[test]
    fn test_non_singleton_has_defaults() {
        let kdl = r#"
type "doc" {
    field "title" type="string"
    section "Body"
}
"#;
        let schema = Schema::from_str(kdl).unwrap();
        assert!(!schema.types[0].singleton);
        assert!(schema.types[0].match_pattern.is_none());
    }
}

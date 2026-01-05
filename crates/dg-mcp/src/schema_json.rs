//! Schema-to-JSON serialization helpers.

use md_db::schema::{FieldDef, FieldType, Schema, SectionDef, TypeDef};
use md_db::validation::Diagnostic;
use serde_json::{json, Value};

pub fn field_type_short(ft: &FieldType) -> &'static str {
    match ft {
        FieldType::String => "string",
        FieldType::Number => "number",
        FieldType::Bool => "bool",
        FieldType::Date => "date",
        FieldType::Enum(_) => "enum",
        FieldType::Ref => "ref",
        FieldType::StringArray => "string[]",
        FieldType::RefArray => "ref[]",
        FieldType::User => "user",
        FieldType::UserArray => "user[]",
        FieldType::Org => "org",
        FieldType::OrgArray => "org[]",
    }
}

pub fn field_to_json(f: &FieldDef) -> Value {
    let mut obj = json!({
        "name": f.name,
        "type": field_type_short(&f.field_type),
        "required": f.required,
    });
    if let Some(ref desc) = f.description {
        obj["description"] = Value::String(desc.clone());
    }
    if let Some(ref pat) = f.pattern {
        obj["pattern"] = Value::String(pat.clone());
    }
    if let Some(ref def) = f.default {
        obj["default"] = Value::String(def.clone());
    }
    if let FieldType::Enum(ref vals) = f.field_type {
        obj["values"] = json!(vals);
    }
    obj
}

pub fn section_to_json(s: &SectionDef) -> Value {
    let mut obj = json!({ "name": s.name, "required": s.required });
    if let Some(ref desc) = s.description {
        obj["description"] = Value::String(desc.clone());
    }
    if !s.children.is_empty() {
        let children: Vec<Value> = s.children.iter().map(section_to_json).collect();
        obj["children"] = json!(children);
    }
    obj
}

pub fn type_to_json(type_def: &TypeDef) -> Value {
    let fields: Vec<Value> = type_def.fields.iter().map(field_to_json).collect();
    let sections: Vec<Value> = type_def.sections.iter().map(section_to_json).collect();
    json!({
        "name": type_def.name,
        "description": type_def.description,
        "folder": type_def.folder,
        "max_count": type_def.max_count,
        "fields": fields,
        "sections": sections,
    })
}

pub fn export_schema_json(schema: &Schema) -> Value {
    let types: Vec<Value> = schema.types.iter().map(type_to_json).collect();
    json!({ "types": types, "relations": relations_to_json(schema) })
}

pub fn relations_to_json(schema: &Schema) -> Value {
    let rels: Vec<Value> = schema
        .relations
        .iter()
        .map(|r| {
            json!({
                "name": r.name,
                "inverse": r.inverse,
                "cardinality": match r.cardinality {
                    md_db::schema::Cardinality::One => "one",
                    md_db::schema::Cardinality::Many => "many",
                },
                "description": r.description,
                "acyclic": r.acyclic,
            })
        })
        .collect();
    json!(rels)
}

/// Convert a Diagnostic to its JSON representation.
pub fn diagnostic_to_json(d: &Diagnostic) -> Value {
    json!({
        "severity": d.severity.to_string(),
        "code": d.code,
        "message": d.message,
        "location": d.location,
        "hint": d.hint,
    })
}

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use regex::{Regex, RegexBuilder};

use crate::document::{Document, ParsedBody};
use crate::schema::{FieldDef, FieldType, Schema, SectionDef, TableDef, TypeDef};
use crate::users::OrgConfig;

use super::{Diagnostic, FileResult, Severity};

/// Validate a single document against its type definition in the schema.
pub fn validate_document(
    doc: &Document,
    schema: &Schema,
    known_files: &HashSet<PathBuf>,
    known_ids: &HashSet<String>,
    user_config: Option<&OrgConfig>,
) -> FileResult {
    let path = doc
        .path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<string>".to_string());

    let mut diagnostics = Vec::new();

    // Must have frontmatter
    let fm = match &doc.frontmatter {
        Some(fm) => fm,
        None => {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "F000".into(),
                message: "document has no frontmatter".into(),
                location: "frontmatter".into(),
                hint: Some("add YAML frontmatter between --- delimiters".into()),
            });
            return FileResult { path, diagnostics };
        }
    };

    // Infer type from frontmatter (legacy), or from filename/path
    let type_name = fm.get_display("type").or_else(|| {
        let doc_id = doc
            .path
            .as_ref()
            .map(|p| crate::graph::path_to_id(p))
            .unwrap_or_default();
        schema.type_name_for_doc_id(&doc_id)
    });

    let type_name = match type_name {
        Some(t) => t,
        None => {
            let known: Vec<&str> = schema.types.iter().map(|t| t.name.as_str()).collect();
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "F001".into(),
                message: "cannot determine document type from filename".into(),
                location: "filename".into(),
                hint: Some(format!(
                    "filename must start with a known type prefix: {}",
                    known.join(", ")
                )),
            });
            return FileResult { path, diagnostics };
        }
    };

    // Look up type definition
    let type_def = match schema.get_type(&type_name) {
        Some(t) => t,
        None => {
            let known: Vec<&str> = schema.types.iter().map(|t| t.name.as_str()).collect();
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "F002".into(),
                message: format!("unknown document type \"{type_name}\""),
                location: "frontmatter.type".into(),
                hint: Some(format!("known types: {}", known.join(", "))),
            });
            return FileResult { path, diagnostics };
        }
    };

    // Validate fields
    validate_fields(
        fm,
        type_def,
        schema,
        known_files,
        known_ids,
        &doc.path,
        user_config,
        &mut diagnostics,
    );

    // Validate conditional rules (if/then constraints)
    validate_rules(fm, type_def, &mut diagnostics);

    // Validate relation fields (defined at schema level, not per-type)
    validate_relation_fields(
        fm,
        schema,
        known_files,
        known_ids,
        &doc.path,
        &mut diagnostics,
    );

    // Warn on undefined frontmatter keys
    check_undefined_keys(fm, type_def, schema, &mut diagnostics);

    // Validate H1 heading (exactly one required, serves as title)
    {
        let arena = comrak::Arena::new();
        let root = crate::ast_util::parse_md(&arena, &doc.body);
        let h1s = crate::ast_util::find_headings(root, Some(1));
        if h1s.is_empty() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "C010".into(),
                message: "document has no H1 heading (title)".into(),
                location: "body".into(),
                hint: Some("add a single '# Title' heading as the document title".into()),
            });
        } else if h1s.len() > 1 {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "C011".into(),
                message: format!("document has {} H1 headings, expected exactly 1", h1s.len()),
                location: "body".into(),
                hint: Some(
                    "use a single '# Title' for the document title; use ## for sections".into(),
                ),
            });
        }
    }

    let parsed = doc.parse_body();

    // Validate folder-type consistency (e.g. ADR-002 must not be in docs/opportunities/)
    if let (Some(ref doc_path), Some(ref expected_folder)) = (&doc.path, &type_def.folder) {
        let doc_id = crate::graph::path_to_id(doc_path);
        let id_type = schema.type_name_for_doc_id(&doc_id);
        if let Some(id_type) = id_type {
            if let Some(id_type_def) = schema.get_type(&id_type) {
                if let Some(ref id_folder) = id_type_def.folder {
                    if id_folder != expected_folder {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            code: "F010".into(),
                            message: format!(
                                "{doc_id} has type prefix '{}' but is in '{expected_folder}/' (expected '{id_folder}/')",
                                id_type.to_uppercase()
                            ),
                            location: "filename".into(),
                            hint: Some(format!("move this file to {id_folder}/")),
                        });
                    }
                }
            }
        }
    }

    // Validate filename has slug (e.g. opp-001.md is invalid, must be opp-001-sell-llama-milk.md)
    if let Some(ref doc_path) = doc.path {
        let stem = doc_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let doc_id = crate::graph::path_to_id(doc_path);
        let id_lower = doc_id.to_lowercase();
        // Check if stem is just the bare ID (no slug)
        let normalized_stem = stem.to_lowercase().replace('_', "-");
        if normalized_stem == id_lower {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "F011".into(),
                message: format!(
                    "filename '{stem}.md' is missing a descriptive slug"
                ),
                location: "filename".into(),
                hint: Some(format!(
                    "rename to '{id_lower}-descriptive-name.md' (e.g. '{id_lower}-use-postgresql.md')"
                )),
            });
        }
    }

    validate_sections_parsed(
        &parsed,
        &type_def.sections,
        &[],
        user_config,
        &mut diagnostics,
    );

    // Validate conditional section table overrides from rules
    validate_rule_sections(fm, &parsed, type_def, user_config, &mut diagnostics);

    // Check for broken markdown tables (pipe syntax that failed to parse)
    super::content::check_broken_tables(&doc.body, &mut diagnostics);

    // Check that local image paths point to docs/assets/
    super::content::check_image_paths(&doc.body, &mut diagnostics);

    // Validate diagram code blocks by attempting to render them
    #[cfg(feature = "diagrams")]
    {
        let allow_cycles = fm
            .get_display("allow_diagram_cycles")
            .is_some_and(|v| v == "true");
        validate_all_diagrams(&parsed, allow_cycles, &mut diagnostics);
    }

    // Validate gherkin code blocks
    #[cfg(feature = "gherkin")]
    validate_gherkin_blocks(&parsed, &path, &mut diagnostics);

    FileResult { path, diagnostics }
}

#[allow(clippy::too_many_arguments)]
fn validate_fields(
    fm: &crate::frontmatter::Frontmatter,
    type_def: &TypeDef,
    schema: &Schema,
    known_files: &HashSet<PathBuf>,
    known_ids: &HashSet<String>,
    doc_path: &Option<PathBuf>,
    user_config: Option<&OrgConfig>,
    diags: &mut Vec<Diagnostic>,
) {
    for field_def in &type_def.fields {
        let val = fm.get(&field_def.name);

        // Required check
        if field_def.required && val.is_none() {
            let mut hint = format!(
                "add '{}: <{}>' to frontmatter",
                field_def.name, field_def.field_type
            );
            if let Some(ref desc) = field_def.description {
                hint.push_str(&format!(" — {desc}"));
            }
            diags.push(Diagnostic {
                severity: Severity::Error,
                code: "F010".into(),
                message: format!("missing required field \"{}\"", field_def.name),
                location: "frontmatter".into(),
                hint: Some(hint),
            });
            continue;
        }

        let val = match val {
            Some(v) => v,
            None => continue,
        };

        // Type check
        validate_field_value(
            &field_def.name,
            val,
            field_def,
            schema,
            known_files,
            known_ids,
            doc_path,
            user_config,
            diags,
        );
    }
}

/// Validate conditional rules: when a field matches a value, other fields become required.
fn validate_rules(
    fm: &crate::frontmatter::Frontmatter,
    type_def: &TypeDef,
    diags: &mut Vec<Diagnostic>,
) {
    for rule in &type_def.rules {
        if let Some(val) = fm.get(&rule.when_field) {
            let val_str = match val.as_str() {
                Some(s) => s,
                None => continue,
            };
            if rule.matches(val_str) {
                let cond = rule.condition_display();
                for required_field in &rule.then_required {
                    if fm.get(required_field).is_none() {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "F040".into(),
                            message: format!("field \"{}\" required when {}", required_field, cond),
                            location: format!("frontmatter.{}", required_field),
                            hint: Some(format!(
                                "add '{}' to frontmatter (required by rule \"{}\")",
                                required_field, rule.name
                            )),
                        });
                    }
                }
            }
        }
    }
}

/// Validate conditional section table overrides: when a rule's condition matches,
/// require specified sections to contain a table with the defined columns.
fn validate_rule_sections(
    fm: &crate::frontmatter::Frontmatter,
    parsed: &ParsedBody,
    type_def: &TypeDef,
    user_config: Option<&OrgConfig>,
    diags: &mut Vec<Diagnostic>,
) {
    for rule in &type_def.rules {
        if rule.then_section_table.is_empty() {
            continue;
        }
        let val = match fm.get(&rule.when_field).and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };
        if !rule.matches(val) {
            continue;
        }
        let cond = rule.condition_display();
        for override_def in &rule.then_section_table {
            match parsed.find_section(&override_def.section) {
                Some(section) => {
                    if section.tables.is_empty() {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "S040".into(),
                            message: format!(
                                "section \"{}\" requires a table when {}",
                                override_def.section, cond
                            ),
                            location: format!("section \"{}\"", override_def.section),
                            hint: Some(format!(
                                "add a table with columns: {} (required by rule \"{}\")",
                                override_def
                                    .table
                                    .columns
                                    .iter()
                                    .map(|c| c.name.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", "),
                                rule.name
                            )),
                        });
                    } else if let Some(table) = section.tables.first() {
                        validate_table_columns(
                            table,
                            &override_def.table,
                            &override_def.section,
                            user_config,
                            diags,
                        );
                    }
                }
                None => {
                    // Section itself is missing — only report if the section is also
                    // defined as required in the schema; otherwise, S010 already covers it.
                    // For rules, we report a specific S040 so the user knows *why* it's needed.
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        code: "S040".into(),
                        message: format!(
                            "section \"{}\" required when {}",
                            override_def.section, cond
                        ),
                        location: format!("section \"{}\"", override_def.section),
                        hint: Some(format!(
                            "add section with a table (required by rule \"{}\")",
                            rule.name
                        )),
                    });
                }
            }
        }
    }
}

/// Validate relation fields. Relations are defined at schema level and apply to all types.
/// Any frontmatter field matching a relation name/inverse is validated as a ref.
#[allow(clippy::too_many_arguments)]
fn validate_relation_fields(
    fm: &crate::frontmatter::Frontmatter,
    schema: &Schema,
    known_files: &HashSet<PathBuf>,
    known_ids: &HashSet<String>,
    doc_path: &Option<PathBuf>,
    diags: &mut Vec<Diagnostic>,
) {
    // Extract this document's own ID for self-reference detection
    let self_id = doc_path.as_ref().map(|p| crate::graph::path_to_id(p));

    for key in fm.keys() {
        if let Some((rel_def, _is_inverse)) = schema.find_relation(key) {
            let val = match fm.get(key) {
                Some(v) => v,
                None => continue,
            };

            match rel_def.cardinality {
                crate::schema::Cardinality::One => {
                    // Single ref
                    if let Some(s) = val.as_str() {
                        check_self_ref(key, s, &self_id, diags);
                        validate_ref(key, s, schema, known_files, known_ids, doc_path, diags);
                    } else {
                        diags.push(type_mismatch(key, "ref (string)", val));
                    }
                }
                crate::schema::Cardinality::Many => {
                    // Array of refs
                    match val.as_sequence() {
                        Some(seq) => {
                            for (i, item) in seq.iter().enumerate() {
                                if let Some(s) = item.as_str() {
                                    let loc = format!("{key}[{i}]");
                                    check_self_ref(&loc, s, &self_id, diags);
                                    validate_ref(
                                        &loc,
                                        s,
                                        schema,
                                        known_files,
                                        known_ids,
                                        doc_path,
                                        diags,
                                    );
                                } else {
                                    diags.push(Diagnostic {
                                        severity: Severity::Error,
                                        code: "F020".into(),
                                        message: format!(
                                            "relation \"{key}[{i}]\" expected ref (string), got {}",
                                            yaml_type_name(item)
                                        ),
                                        location: format!("frontmatter.{key}[{i}]"),
                                        hint: None,
                                    });
                                }
                            }
                        }
                        None => {
                            // Allow single string for cardinality=many (auto-wrap)
                            if let Some(s) = val.as_str() {
                                check_self_ref(key, s, &self_id, diags);
                                validate_ref(
                                    key,
                                    s,
                                    schema,
                                    known_files,
                                    known_ids,
                                    doc_path,
                                    diags,
                                );
                            } else {
                                diags.push(type_mismatch(key, "ref[]", val));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Warn on frontmatter keys not defined in schema (type fields + relations + builtins).
fn check_undefined_keys(
    fm: &crate::frontmatter::Frontmatter,
    type_def: &TypeDef,
    schema: &Schema,
    diags: &mut Vec<Diagnostic>,
) {
    // `type` is read by the type resolver itself (see validate_document) and
    // `allow_diagram_cycles` suppresses D002, so neither may be flagged as
    // unknown. Everything else is schema-defined.
    const BUILTINS: &[&str] = &["type", "allow_diagram_cycles"];

    // Keys from removed dg features. Checked only after the schema lookups so
    // a project that defines its own field with the same name is unaffected.
    const DEPRECATED_KEYS: &[(&str, &str)] = &[(
        "code_paths",
        "code_paths was removed — delete this key and reference the doc ID from \
         source code comments instead (code refs are scanned automatically; see `dg refs`)",
    )];

    for key in fm.keys() {
        // Check type-specific fields
        if type_def.fields.iter().any(|f| f.name == *key) {
            continue;
        }
        // Check relation fields (direct + inverse)
        if schema.find_relation(key).is_some() {
            continue;
        }
        // Check builtins
        if BUILTINS.contains(&key.as_str()) {
            continue;
        }
        if let Some((_, hint)) = DEPRECATED_KEYS.iter().find(|(k, _)| k == key) {
            diags.push(Diagnostic {
                severity: Severity::Error,
                code: "F023".into(),
                message: format!(
                    "deprecated frontmatter key \"{key}\" for type \"{}\"",
                    type_def.name
                ),
                location: format!("frontmatter.{key}"),
                hint: Some((*hint).to_string()),
            });
            continue;
        }
        diags.push(Diagnostic {
            severity: Severity::Warning,
            code: "F020".into(),
            message: format!(
                "unknown frontmatter key \"{key}\" for type \"{}\"",
                type_def.name
            ),
            location: format!("frontmatter.{key}"),
            hint: Some(format!(
                "remove this key or add field \"{key}\" to type \"{}\" in schema.kdl",
                type_def.name
            )),
        });
    }
}

/// Emit an error if a relation ref points to the document itself.
fn check_self_ref(
    field_name: &str,
    value: &str,
    self_id: &Option<String>,
    diags: &mut Vec<Diagnostic>,
) {
    if let Some(ref id) = self_id {
        if value.eq_ignore_ascii_case(id) {
            diags.push(Diagnostic {
                severity: Severity::Error,
                code: "R020".into(),
                message: format!("self-referential relation \"{value}\" in \"{field_name}\""),
                location: format!("frontmatter.{field_name}"),
                hint: Some("a document cannot reference itself".into()),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_field_value(
    field_name: &str,
    val: &serde_yaml::Value,
    field_def: &FieldDef,
    schema: &Schema,
    known_files: &HashSet<PathBuf>,
    known_ids: &HashSet<String>,
    doc_path: &Option<PathBuf>,
    user_config: Option<&OrgConfig>,
    diags: &mut Vec<Diagnostic>,
) {
    match &field_def.field_type {
        FieldType::String => {
            if !val.is_string() {
                diags.push(type_mismatch(field_name, "string", val));
            } else if let Some(ref pattern) = field_def.pattern {
                check_pattern(field_name, val.as_str().unwrap(), pattern, diags);
            }
        }
        FieldType::Number => {
            if !val.is_number() {
                diags.push(type_mismatch(field_name, "number", val));
            }
        }
        FieldType::Bool => {
            if !val.is_bool() {
                diags.push(type_mismatch(field_name, "bool", val));
            }
        }
        FieldType::Date => {
            if let Some(s) = val.as_str() {
                if !is_valid_date(s) {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        code: "F022".into(),
                        message: format!("field \"{field_name}\" has invalid date \"{s}\""),
                        location: format!("frontmatter.{field_name}"),
                        hint: Some("expected YYYY-MM-DD format (e.g. 2026-06-01)".into()),
                    });
                }
            } else {
                diags.push(type_mismatch(field_name, "date (string)", val));
            }
        }
        FieldType::Enum(allowed) => match val.as_str() {
            Some(s) => {
                if !allowed.contains(&s.to_string()) {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        code: "F021".into(),
                        message: format!("field \"{field_name}\" has invalid value \"{s}\""),
                        location: format!("frontmatter.{field_name}"),
                        hint: Some(format!("allowed values: {}", allowed.join(", "))),
                    });
                }
            }
            None => {
                diags.push(type_mismatch(field_name, "enum (string)", val));
            }
        },
        FieldType::Ref => {
            if let Some(s) = val.as_str() {
                validate_ref(
                    field_name,
                    s,
                    schema,
                    known_files,
                    known_ids,
                    doc_path,
                    diags,
                );
            } else {
                diags.push(type_mismatch(field_name, "ref (string)", val));
            }
        }
        FieldType::StringArray => match val.as_sequence() {
            Some(seq) => {
                for (i, item) in seq.iter().enumerate() {
                    if !item.is_string() {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "F020".into(),
                            message: format!(
                                "field \"{field_name}[{i}]\" expected string, got {}",
                                yaml_type_name(item)
                            ),
                            location: format!("frontmatter.{field_name}[{i}]"),
                            hint: None,
                        });
                    }
                }
                if let Some(ref pattern) = field_def.pattern {
                    for (i, item) in seq.iter().enumerate() {
                        if let Some(s) = item.as_str() {
                            check_pattern(&format!("{field_name}[{i}]"), s, pattern, diags);
                        }
                    }
                }
            }
            None => {
                diags.push(type_mismatch(field_name, "string[]", val));
            }
        },
        FieldType::RefArray => match val.as_sequence() {
            Some(seq) => {
                for (i, item) in seq.iter().enumerate() {
                    if let Some(s) = item.as_str() {
                        validate_ref(
                            &format!("{field_name}[{i}]"),
                            s,
                            schema,
                            known_files,
                            known_ids,
                            doc_path,
                            diags,
                        );
                    } else {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "F020".into(),
                            message: format!(
                                "field \"{field_name}[{i}]\" expected ref (string), got {}",
                                yaml_type_name(item)
                            ),
                            location: format!("frontmatter.{field_name}[{i}]"),
                            hint: None,
                        });
                    }
                }
            }
            None => {
                diags.push(type_mismatch(field_name, "ref[]", val));
            }
        },
        FieldType::User => {
            if let Some(s) = val.as_str() {
                validate_user_ref(field_name, s, user_config, false, false, diags);
            } else {
                diags.push(type_mismatch(field_name, "user (@handle)", val));
            }
        }
        FieldType::UserArray => match val.as_sequence() {
            Some(seq) => {
                for (i, item) in seq.iter().enumerate() {
                    if let Some(s) = item.as_str() {
                        validate_user_ref(
                            &format!("{field_name}[{i}]"),
                            s,
                            user_config,
                            false,
                            false,
                            diags,
                        );
                    } else {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "F020".into(),
                            message: format!(
                                "field \"{field_name}[{i}]\" expected user (@handle), got {}",
                                yaml_type_name(item)
                            ),
                            location: format!("frontmatter.{field_name}[{i}]"),
                            hint: None,
                        });
                    }
                }
            }
            None => {
                diags.push(type_mismatch(field_name, "user[]", val));
            }
        },
        FieldType::Org => {
            if let Some(s) = val.as_str() {
                validate_org_ref(field_name, s, user_config, false, diags);
            } else {
                diags.push(type_mismatch(field_name, "org (@org/name)", val));
            }
        }
        FieldType::OrgArray => match val.as_sequence() {
            Some(seq) => {
                for (i, item) in seq.iter().enumerate() {
                    if let Some(s) = item.as_str() {
                        validate_org_ref(
                            &format!("{field_name}[{i}]"),
                            s,
                            user_config,
                            false,
                            diags,
                        );
                    } else {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "F020".into(),
                            message: format!(
                                "field \"{field_name}[{i}]\" expected org (@org/name), got {}",
                                yaml_type_name(item)
                            ),
                            location: format!("frontmatter.{field_name}[{i}]"),
                            hint: None,
                        });
                    }
                }
            }
            None => {
                diags.push(type_mismatch(field_name, "org[]", val));
            }
        },
    }
}

fn validate_ref(
    field_name: &str,
    value: &str,
    schema: &Schema,
    known_files: &HashSet<PathBuf>,
    known_ids: &HashSet<String>,
    doc_path: &Option<PathBuf>,
    diags: &mut Vec<Diagnostic>,
) {
    // Check if it matches any ref-format pattern
    let matches_format = schema.ref_formats.iter().any(|rf| {
        safe_regex(&rf.pattern)
            .map(|re| re.is_match(value))
            .unwrap_or(false)
    });

    if !matches_format && !schema.ref_formats.is_empty() {
        let patterns: Vec<&str> = schema
            .ref_formats
            .iter()
            .map(|rf| rf.pattern.as_str())
            .collect();
        diags.push(Diagnostic {
            severity: Severity::Warning,
            code: "R001".into(),
            message: format!("ref \"{value}\" in \"{field_name}\" doesn't match any ref-format"),
            location: format!("frontmatter.{field_name}"),
            hint: Some(format!("expected patterns: {}", patterns.join(", "))),
        });
        return;
    }

    // If it looks like a relative path, check file existence
    if value.ends_with(".md") {
        if let Some(ref base) = doc_path {
            if let Some(dir) = base.parent() {
                let target = dir.join(value);
                if !known_files.contains(&target) {
                    // Try canonical
                    let canonical = target
                        .canonicalize()
                        .ok()
                        .map(|p| known_files.contains(&p))
                        .unwrap_or(false);
                    if !canonical {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "R010".into(),
                            message: format!(
                                "broken file reference \"{value}\" in \"{field_name}\""
                            ),
                            location: format!("frontmatter.{field_name}"),
                            hint: Some(format!("resolved to: {}", target.display())),
                        });
                    }
                }
            }
        }
    } else {
        // String ID — check against known IDs
        if !known_ids.contains(value) && !known_ids.is_empty() {
            diags.push(Diagnostic {
                severity: Severity::Warning,
                code: "R011".into(),
                message: format!("unresolved reference \"{value}\" in \"{field_name}\""),
                location: format!("frontmatter.{field_name}"),
                hint: Some("no document with matching ID found in scope".into()),
            });
        }
    }
}

/// Validate a user/team reference.
///
/// When `require_at` is true (markdown content: tables, text), the value must
/// start with `@`. When false (YAML frontmatter), `@` is forbidden — bare
/// handles like `onni` are required since `@` is a reserved YAML character.
fn validate_user_ref(
    field_name: &str,
    value: &str,
    user_config: Option<&OrgConfig>,
    require_at: bool,
    skip_departed: bool,
    diags: &mut Vec<Diagnostic>,
) {
    // In markdown content, @ prefix is required to distinguish user refs from text
    if require_at && !value.starts_with('@') {
        diags.push(Diagnostic {
            severity: Severity::Error,
            code: "U010".into(),
            message: format!(
                "field \"{field_name}\" value \"{value}\" is not a valid user reference"
            ),
            location: format!("frontmatter.{field_name}"),
            hint: Some("user references must start with @ (e.g. @onni, @team/platform)".into()),
        });
        return;
    }

    // In frontmatter, @ prefix is forbidden (reserved YAML character)
    if !require_at && value.starts_with('@') {
        diags.push(Diagnostic {
            severity: Severity::Error,
            code: "U013".into(),
            message: format!(
                "field \"{field_name}\" value \"{value}\" uses '@' which is a reserved YAML character"
            ),
            location: format!("frontmatter.{field_name}"),
            hint: Some(format!(
                "remove the '@' prefix: use '{}' instead of '{value}'",
                value.trim_start_matches('@')
            )),
        });
        return;
    }

    // Normalize: prepend @ for lookup against org config
    let normalized = if value.starts_with('@') {
        value.to_string()
    } else {
        format!("@{value}")
    };

    // If user config is provided, validate the reference resolves
    if let Some(config) = user_config {
        if !config.is_valid_ref(&normalized) {
            let mut all_refs = config.all_user_handles();
            all_refs.extend(config.all_team_names());
            diags.push(Diagnostic {
                severity: Severity::Error,
                code: "U011".into(),
                message: format!("field \"{field_name}\" references unknown user/team \"{value}\""),
                location: format!("frontmatter.{field_name}"),
                hint: if all_refs.is_empty() {
                    None
                } else {
                    Some(format!("known: {}", all_refs.join(", ")))
                },
            });
        } else if !skip_departed && config.is_departed_user(&normalized) {
            diags.push(Diagnostic {
                severity: Severity::Warning,
                code: "U012".into(),
                message: format!("field \"{field_name}\" references departed user \"{value}\""),
                location: format!("frontmatter.{field_name}"),
                hint: Some(
                    "consider reassigning to an active user or deprecating this document".into(),
                ),
            });
        }
    }
}

/// Sections whose data is inherently historical — departed users are expected.
fn is_historical_section(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("history") || lower == "timeline"
}

/// Validate an org reference.
///
/// When `require_prefix` is true (markdown content), the value must use `@org/` prefix.
/// When false (YAML frontmatter), bare org ids like `acme-eu` are accepted.
/// If user config has no orgs, validation is a no-op (inert).
fn validate_org_ref(
    field_name: &str,
    value: &str,
    user_config: Option<&OrgConfig>,
    require_prefix: bool,
    diags: &mut Vec<Diagnostic>,
) {
    // In markdown content, @org/ prefix is required
    if require_prefix && !value.starts_with("@org/") {
        diags.push(Diagnostic {
            severity: Severity::Error,
            code: "U020".into(),
            message: format!(
                "field \"{field_name}\" value \"{value}\" is not a valid org reference"
            ),
            location: format!("frontmatter.{field_name}"),
            hint: Some("org references must use @org/ prefix (e.g. @org/acme)".into()),
        });
        return;
    }

    // In frontmatter, @ prefix is forbidden (reserved YAML character)
    if !require_prefix && value.starts_with('@') {
        diags.push(Diagnostic {
            severity: Severity::Error,
            code: "U013".into(),
            message: format!(
                "field \"{field_name}\" value \"{value}\" uses '@' which is a reserved YAML character"
            ),
            location: format!("frontmatter.{field_name}"),
            hint: Some(format!(
                "remove the '@' prefix: use '{}' instead of '{value}'",
                value.trim_start_matches('@')
            )),
        });
        return;
    }

    // Normalize: prepend @org/ if missing
    let normalized = if value.starts_with("@org/") {
        value.to_string()
    } else {
        format!("@org/{value}")
    };

    // If user config is provided and has orgs, validate the reference resolves
    if let Some(config) = user_config {
        if config.orgs.is_empty() {
            return; // inert when no orgs defined
        }
        if !config.is_valid_org(&normalized) {
            let all_orgs = config.all_org_names();
            diags.push(Diagnostic {
                severity: Severity::Error,
                code: "U021".into(),
                message: format!("field \"{field_name}\" references unknown org \"{value}\""),
                location: format!("frontmatter.{field_name}"),
                hint: if all_orgs.is_empty() {
                    None
                } else {
                    Some(format!("known: {}", all_orgs.join(", ")))
                },
            });
        }
    }
}

fn validate_sections_parsed(
    parsed: &ParsedBody,
    section_defs: &[SectionDef],
    parent_path: &[&str],
    user_config: Option<&OrgConfig>,
    diags: &mut Vec<Diagnostic>,
) {
    for sec_def in section_defs {
        // Look up section from pre-parsed data
        let found = if parent_path.is_empty() {
            parsed.find_section(&sec_def.name)
        } else {
            // Walk the path: find the root section, then drill into children
            let root = parsed.find_section(parent_path[0]);
            if parent_path.len() == 1 {
                root.and_then(|r| r.find_child(&sec_def.name))
            } else {
                root.and_then(|r| r.find_by_path(&parent_path[1..]))
                    .and_then(|p| p.find_child(&sec_def.name))
            }
        };

        match found {
            Some(section) => {
                // Validate table if defined
                if let Some(ref table_def) = sec_def.table {
                    if section.tables.is_empty() && table_def.required {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "S020".into(),
                            message: format!(
                                "section \"{}\" requires a table but none found",
                                sec_def.name
                            ),
                            location: format!("section \"{}\"", sec_def.name),
                            hint: Some("add a markdown table to this section".into()),
                        });
                    } else if let Some(table) = section.tables.first() {
                        validate_table_columns(table, table_def, &sec_def.name, user_config, diags);
                    }
                }

                // Content constraint — use pre-extracted paragraph count
                if let Some(ref content_def) = sec_def.content {
                    if let Some(min) = content_def.min_paragraphs {
                        if section.paragraph_count < min {
                            diags.push(Diagnostic {
                                severity: Severity::Error,
                                code: "S030".into(),
                                message: format!(
                                    "section \"{}\" requires at least {} paragraph(s), found {}",
                                    sec_def.name, min, section.paragraph_count
                                ),
                                location: format!("section \"{}\"", sec_def.name),
                                hint: Some("add prose content to this section".into()),
                            });
                        }
                    }
                }

                // List constraint — use pre-extracted list info
                if let Some(ref list_def) = sec_def.list {
                    if !section.list_has_list && list_def.required {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "S031".into(),
                            message: format!(
                                "section \"{}\" requires a list but none found",
                                sec_def.name
                            ),
                            location: format!("section \"{}\"", sec_def.name),
                            hint: Some("add a markdown list (- item) to this section".into()),
                        });
                    } else if let Some(min_items) = list_def.min_items {
                        if section.list_total_items < min_items {
                            diags.push(Diagnostic {
                                severity: Severity::Error,
                                code: "S031".into(),
                                message: format!(
                                    "section \"{}\" requires at least {} list item(s), found {}",
                                    sec_def.name, min_items, section.list_total_items
                                ),
                                location: format!("section \"{}\"", sec_def.name),
                                hint: Some(format!("add at least {min_items} list items")),
                            });
                        }
                    }

                    // Ordered/unordered constraint
                    if section.list_has_list {
                        if let Some(ordered) = list_def.ordered {
                            if ordered && !section.list_is_ordered {
                                diags.push(Diagnostic {
                                    severity: Severity::Error,
                                    code: "S033".into(),
                                    message: format!(
                                        "section \"{}\" requires an ordered list (1. 2. …) but found an unordered list",
                                        sec_def.name
                                    ),
                                    location: format!("section \"{}\"", sec_def.name),
                                    hint: Some("use numbered list items (1. item) instead of bullets (- item)".into()),
                                });
                            } else if !ordered && section.list_is_ordered {
                                diags.push(Diagnostic {
                                    severity: Severity::Error,
                                    code: "S033".into(),
                                    message: format!(
                                        "section \"{}\" requires an unordered list but found an ordered list",
                                        sec_def.name
                                    ),
                                    location: format!("section \"{}\"", sec_def.name),
                                    hint: Some("use bullet list items (- item) instead of numbers (1. item)".into()),
                                });
                            }
                        }
                    }
                }

                // Diagram constraint — use pre-extracted code block languages
                if let Some(ref diagram_def) = sec_def.diagram {
                    let has_diagram = if let Some(ref expected_type) = diagram_def.diagram_type {
                        let expected = expected_type.to_lowercase();
                        section
                            .code_block_languages
                            .iter()
                            .any(|info| info == &expected)
                    } else {
                        section
                            .code_block_languages
                            .iter()
                            .any(|info| DIAGRAM_LANGUAGES.iter().any(|lang| info == lang))
                    };

                    if !has_diagram && diagram_def.required {
                        let hint = if let Some(ref dt) = diagram_def.diagram_type {
                            format!("add a ```{dt} code block to this section")
                        } else {
                            format!(
                                "add a fenced code block with a diagram language ({})",
                                DIAGRAM_LANGUAGES.join(", ")
                            )
                        };
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "S032".into(),
                            message: format!(
                                "section \"{}\" requires a diagram but none found",
                                sec_def.name
                            ),
                            location: format!("section \"{}\"", sec_def.name),
                            hint: Some(hint),
                        });
                    }
                }

                // Min-subsections constraint — check child heading count
                if let Some(min) = sec_def.min_subsections {
                    if section.children.len() < min {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "S034".into(),
                            message: format!(
                                "section \"{}\" requires at least {} subsection(s), found {}",
                                sec_def.name,
                                min,
                                section.children.len()
                            ),
                            location: format!("section \"{}\"", sec_def.name),
                            hint: Some(format!(
                                "add at least {min} subsection(s) with ### headings"
                            )),
                        });
                    }
                }

                // Callout constraint — check for GFM callout blockquote
                if sec_def.callout_required && !section.has_callout {
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        code: "S035".into(),
                        message: format!(
                            "section \"{}\" requires a GFM callout but none found",
                            sec_def.name
                        ),
                        location: format!("section \"{}\"", sec_def.name),
                        hint: Some("add a callout like: > [!WARNING]\\n> Risk summary here".into()),
                    });
                }

                // Recurse into child sections
                if !sec_def.children.is_empty() {
                    let mut path: Vec<&str> = parent_path.to_vec();
                    path.push(&sec_def.name);
                    validate_sections_parsed(parsed, &sec_def.children, &path, user_config, diags);
                }
            }
            None => {
                if sec_def.required {
                    let full_name = if parent_path.is_empty() {
                        sec_def.name.clone()
                    } else {
                        format!("{} > {}", parent_path.join(" > "), sec_def.name)
                    };
                    let mut hint = format!("add a \"## {}\" section", sec_def.name);
                    if let Some(ref desc) = sec_def.description {
                        hint.push_str(&format!(" — {desc}"));
                    }
                    diags.push(Diagnostic {
                        severity: Severity::Error,
                        code: "S010".into(),
                        message: format!("missing required section \"{full_name}\""),
                        location: "document body".into(),
                        hint: Some(hint),
                    });
                }
            }
        }
    }
}

/// Validate table columns: required columns present + user type columns.
fn validate_table_columns(
    table: &crate::table::Table,
    table_def: &TableDef,
    section_name: &str,
    user_config: Option<&OrgConfig>,
    diags: &mut Vec<Diagnostic>,
) {
    for col_def in &table_def.columns {
        if col_def.required && !table.headers().iter().any(|h| h == &col_def.name) {
            diags.push(Diagnostic {
                severity: Severity::Error,
                code: "S021".into(),
                message: format!(
                    "table in \"{}\" missing required column \"{}\"",
                    section_name, col_def.name
                ),
                location: format!("section \"{section_name}\" > table"),
                hint: None,
            });
            continue;
        }

        // Validate user-typed column cells
        if col_def.col_type == FieldType::User {
            if let Some(col_values) = table.get_column(&col_def.name) {
                for (row_idx, cell) in col_values.iter().enumerate() {
                    let cell = cell.trim();
                    if cell.is_empty() {
                        if col_def.required {
                            diags.push(Diagnostic {
                                severity: Severity::Error,
                                code: "S022".into(),
                                message: format!(
                                    "table in \"{section_name}\" column \"{}\" row {row_idx} is empty but required",
                                    col_def.name
                                ),
                                location: format!("section \"{section_name}\" > table > {}[{row_idx}]", col_def.name),
                                hint: None,
                            });
                        }
                        continue;
                    }
                    let skip_departed = is_historical_section(section_name)
                        || table
                            .get_cell("Status", row_idx)
                            .is_some_and(|s| s.trim().eq_ignore_ascii_case("completed"));
                    validate_user_ref(
                        &format!("table:{section_name}.{}.row{row_idx}", col_def.name),
                        cell,
                        user_config,
                        true,
                        skip_departed,
                        diags,
                    );
                }
            }
        }

        // Validate org-typed column cells
        if col_def.col_type == FieldType::Org {
            if let Some(col_values) = table.get_column(&col_def.name) {
                for (row_idx, cell) in col_values.iter().enumerate() {
                    let cell = cell.trim();
                    if cell.is_empty() {
                        if col_def.required {
                            diags.push(Diagnostic {
                                severity: Severity::Error,
                                code: "S022".into(),
                                message: format!(
                                    "table in \"{section_name}\" column \"{}\" row {row_idx} is empty but required",
                                    col_def.name
                                ),
                                location: format!("section \"{section_name}\" > table > {}[{row_idx}]", col_def.name),
                                hint: None,
                            });
                        }
                        continue;
                    }
                    validate_org_ref(
                        &format!("table:{section_name}.{}.row{row_idx}", col_def.name),
                        cell,
                        user_config,
                        true,
                        diags,
                    );
                }
            }
        }

        // Validate date-typed column cells (YYYY-MM-DD)
        if col_def.col_type == FieldType::Date {
            if let Some(col_values) = table.get_column(&col_def.name) {
                for (row_idx, cell) in col_values.iter().enumerate() {
                    let cell = cell.trim();
                    if cell.is_empty() {
                        if col_def.required {
                            diags.push(Diagnostic {
                                severity: Severity::Error,
                                code: "S024".into(),
                                message: format!(
                                    "table in \"{section_name}\" column \"{}\" row {row_idx} is empty but required",
                                    col_def.name
                                ),
                                location: format!("section \"{section_name}\" > table > {}[{row_idx}]", col_def.name),
                                hint: Some("expected date in YYYY-MM-DD format".into()),
                            });
                        }
                        continue;
                    }
                    if !is_valid_date(cell) {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "S024".into(),
                            message: format!(
                                "table in \"{section_name}\" column \"{}\" row {row_idx} has invalid date \"{cell}\"",
                                col_def.name
                            ),
                            location: format!("section \"{section_name}\" > table > {}[{row_idx}]", col_def.name),
                            hint: Some("expected date in YYYY-MM-DD format (e.g. 2026-06-01)".into()),
                        });
                    }
                }
            }
        }

        // Validate enum-typed column cells
        if let FieldType::Enum(ref allowed) = col_def.col_type {
            if let Some(col_values) = table.get_column(&col_def.name) {
                for (row_idx, cell) in col_values.iter().enumerate() {
                    let cell = cell.trim();
                    if cell.is_empty() {
                        if col_def.required {
                            diags.push(Diagnostic {
                                severity: Severity::Error,
                                code: "S023".into(),
                                message: format!(
                                    "table in \"{section_name}\" column \"{}\" row {row_idx} is empty but required",
                                    col_def.name
                                ),
                                location: format!("section \"{section_name}\" > table > {}[{row_idx}]", col_def.name),
                                hint: Some(format!("allowed values: {}", allowed.join(", "))),
                            });
                        }
                        continue;
                    }
                    if !allowed.iter().any(|a| a == cell) {
                        diags.push(Diagnostic {
                            severity: Severity::Error,
                            code: "S023".into(),
                            message: format!(
                                "table in \"{section_name}\" column \"{}\" row {row_idx} has invalid value \"{cell}\"",
                                col_def.name
                            ),
                            location: format!("section \"{section_name}\" > table > {}[{row_idx}]", col_def.name),
                            hint: Some(format!("allowed values: {}", allowed.join(", "))),
                        });
                    }
                }
            }
        }
    }
}

/// Infer document type from file path based on schema folder definitions.
/// E.g. a file in `docs/incidents/` matches type "inc" if the schema defines
/// `type "inc" folder="docs/incidents"`.
pub fn infer_type_from_path(path: &Path, root: &Path, schema: &Schema) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parent = rel.parent()?;
    for type_def in &schema.types {
        if let Some(ref folder) = type_def.folder {
            if parent == Path::new(folder) {
                return Some(type_def.name.clone());
            }
        }
    }
    None
}

/// Known diagram languages for fenced code blocks.
const DIAGRAM_LANGUAGES: &[&str] = &["mermaid", "d2", "plantuml", "graphviz", "dot"];

/// Validate all diagram code blocks in the document by attempting to parse them.
/// Reports parse errors as D001 and structural warnings (cycles) as D002.
#[cfg(feature = "diagrams")]
fn validate_all_diagrams(parsed: &ParsedBody, allow_cycles: bool, diags: &mut Vec<Diagnostic>) {
    use crate::document::ParsedSection;

    // A parent section's content byte-range includes its child sections, so a
    // fenced diagram under a child heading appears in the parent's code_blocks
    // too. Only the deepest owning section reports it.
    fn child_owns_block(sections: &[ParsedSection], lang: &str, code: &str) -> bool {
        sections.iter().any(|s| {
            s.code_blocks.iter().any(|(l, c)| l == lang && c == code)
                || child_owns_block(&s.children, lang, code)
        })
    }

    fn walk_sections(sections: &[ParsedSection], allow_cycles: bool, diags: &mut Vec<Diagnostic>) {
        for section in sections {
            for (lang, code) in &section.code_blocks {
                if !graphs_tui::is_supported(lang) {
                    continue;
                }
                if child_owns_block(&section.children, lang, code) {
                    continue;
                }

                let location = format!("section \"{}\" ({} diagram)", section.heading, lang);

                match graphs_tui::check(lang, code) {
                    Ok(warnings) => {
                        for warning in &warnings {
                            if allow_cycles
                                && matches!(
                                    warning,
                                    graphs_tui::DiagramWarning::CycleDetected { .. }
                                )
                            {
                                continue;
                            }
                            diags.push(Diagnostic {
                                severity: Severity::Warning,
                                code: "D002".into(),
                                message: warning.to_string(),
                                location: location.clone(),
                                hint: if matches!(
                                    warning,
                                    graphs_tui::DiagramWarning::CycleDetected { .. }
                                ) {
                                    Some(
                                        "intentional loop? set 'allow_diagram_cycles: true' \
                                         in frontmatter"
                                            .into(),
                                    )
                                } else {
                                    None
                                },
                            });
                        }
                    }
                    Err(e) => {
                        // Warning, not error: the diagram parser is incomplete
                        // and must not block validation of valid diagrams.
                        diags.push(Diagnostic {
                            severity: Severity::Warning,
                            code: "D001".into(),
                            message: format!("diagram parse error: {e}"),
                            location,
                            hint: Some(format!("check the {lang} syntax in this code block")),
                        });
                    }
                }
            }

            // Recurse into child sections
            walk_sections(&section.children, allow_cycles, diags);
        }
    }

    walk_sections(&parsed.sections, allow_cycles, diags);
}

/// Gherkin code block languages.
#[cfg(feature = "gherkin")]
const GHERKIN_LANGUAGES: &[&str] = &["gherkin", "feature"];

/// Validate all gherkin code blocks by parsing and running semantic checks.
/// Reports parse errors as G001 and semantic warnings as G002.
#[cfg(feature = "gherkin")]
fn validate_gherkin_blocks(parsed: &ParsedBody, doc_path: &str, diags: &mut Vec<Diagnostic>) {
    fn collect_gherkin(
        sections: &[crate::document::ParsedSection],
        out: &mut Vec<(String, String)>,
    ) {
        for section in sections {
            for (lang, code) in &section.code_blocks {
                let lower = lang.to_lowercase();
                if GHERKIN_LANGUAGES.contains(&lower.as_str()) {
                    out.push((section.heading.clone(), code.clone()));
                }
            }
            collect_gherkin(&section.children, out);
        }
    }

    let mut gherkin_blocks: Vec<(String, String)> = Vec::new();
    collect_gherkin(&parsed.sections, &mut gherkin_blocks);

    if gherkin_blocks.is_empty() {
        return;
    }

    let contents: Vec<String> = gherkin_blocks.iter().map(|(_, c)| c.clone()).collect();

    match dg_gherkin::parse::parse_gherkin_blocks(&contents, doc_path) {
        Ok(features) => {
            let validation = dg_gherkin::validate::validate_features(&features);
            for warning in &validation.warnings {
                let severity = match warning.severity {
                    dg_gherkin::Severity::Warning => Severity::Warning,
                    dg_gherkin::Severity::Info => Severity::Warning, // promote to Warning for schema validation
                };
                diags.push(Diagnostic {
                    severity,
                    code: "G002".into(),
                    message: warning.to_string(),
                    location: doc_path.to_string(),
                    hint: None,
                });
            }
        }
        Err(e) => {
            let location = match e {
                dg_gherkin::GherkinError::ParseError { block_index, .. } => {
                    if let Some((heading, _)) = gherkin_blocks.get(block_index) {
                        format!("{doc_path} section \"{heading}\"")
                    } else {
                        doc_path.to_string()
                    }
                }
            };
            diags.push(Diagnostic {
                severity: Severity::Error,
                code: "G001".into(),
                message: format!("{e}"),
                location,
                hint: Some("check the Gherkin syntax in this code block".into()),
            });
        }
    }
}

/// Compile a regex with a size limit to prevent excessive compilation time from
/// pathological patterns in user-provided schemas.
/// Validate YYYY-MM-DD date format with basic range checks.
fn is_valid_date(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let Ok(year) = s[0..4].parse::<u16>() else {
        return false;
    };
    let Ok(month) = s[5..7].parse::<u8>() else {
        return false;
    };
    let Ok(day) = s[8..10].parse::<u8>() else {
        return false;
    };
    year >= 1970 && (1..=12).contains(&month) && (1..=31).contains(&day)
}

fn safe_regex(pattern: &str) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .size_limit(1 << 20) // 1 MiB compiled NFA limit
        .build()
}

fn check_pattern(field_name: &str, value: &str, pattern: &str, diags: &mut Vec<Diagnostic>) {
    match safe_regex(pattern) {
        Ok(re) => {
            if !re.is_match(value) {
                diags.push(Diagnostic {
                    severity: Severity::Error,
                    code: "F030".into(),
                    message: format!(
                        "field \"{field_name}\" value \"{value}\" doesn't match pattern"
                    ),
                    location: format!("frontmatter.{field_name}"),
                    hint: Some(format!("expected pattern: {pattern}")),
                });
            }
        }
        Err(e) => {
            diags.push(Diagnostic {
                severity: Severity::Warning,
                code: "S000".into(),
                message: format!("invalid regex pattern in schema for \"{field_name}\": {e}"),
                location: "schema".into(),
                hint: None,
            });
        }
    }
}

fn type_mismatch(field_name: &str, expected: &str, got: &serde_yaml::Value) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code: "F020".into(),
        message: format!(
            "field \"{field_name}\" expected {expected}, got {}",
            yaml_type_name(got)
        ),
        location: format!("frontmatter.{field_name}"),
        hint: None,
    }
}

/// Check if a singleton type definition matches a file, considering both
/// filename and optional folder scope.
///
/// - `folder = None` → match anywhere (backward compat)
/// - `folder = Some(".")` → only root-level files (rel_path has no directory component)
/// - `folder = Some(x)` → rel_path parent starts with `x/` or equals `x`
pub(crate) fn singleton_matches(type_def: &TypeDef, filename: &str, rel_path: &Path) -> bool {
    if type_def.match_pattern.as_deref() != Some(filename) {
        return false;
    }
    match type_def.folder.as_deref() {
        None => true,
        Some(".") => rel_path.parent().is_none_or(|p| p == Path::new("")),
        Some(folder) => rel_path.parent().is_some_and(|p| {
            // Match folder/*/filename but not folder/a/b/filename
            // i.e. parent must be exactly folder/<one-component>
            p.starts_with(folder)
                && p.components().count() == Path::new(folder).components().count() + 1
        }),
    }
}

/// Validate a singleton document (no frontmatter required, section-only validation).
pub fn validate_singleton(
    doc: &Document,
    type_def: &TypeDef,
    user_config: Option<&OrgConfig>,
) -> FileResult {
    let path = doc
        .path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<string>".to_string());

    let mut diagnostics = Vec::new();

    let parsed = doc.parse_body();

    validate_sections_parsed(
        &parsed,
        &type_def.sections,
        &[],
        user_config,
        &mut diagnostics,
    );

    FileResult { path, diagnostics }
}

/// Create a FileResult for a file-level error with specific error codes.
pub fn error_diagnostic(path: &Path, err: &crate::error::Error) -> FileResult {
    use crate::error::Error;
    let (code, hint) = match err {
        Error::Io(io_err) => (
            "E001".to_string(),
            Some(format!("IO error: {}", io_err.kind())),
        ),
        Error::FrontmatterParse(_) => (
            "E002".to_string(),
            Some("check YAML frontmatter syntax".into()),
        ),
        Error::FileNotFound(_) => (
            "E003".to_string(),
            Some("file may have been deleted during validation".into()),
        ),
        _ => ("E000".to_string(), None),
    };
    FileResult {
        path: path.display().to_string(),
        diagnostics: vec![Diagnostic {
            severity: Severity::Error,
            code,
            message: format!("failed to parse: {err}"),
            location: "file".into(),
            hint,
        }],
    }
}

fn yaml_type_name(v: &serde_yaml::Value) -> &'static str {
    match v {
        serde_yaml::Value::Null => "null",
        serde_yaml::Value::Bool(_) => "bool",
        serde_yaml::Value::Number(_) => "number",
        serde_yaml::Value::String(_) => "string",
        serde_yaml::Value::Sequence(_) => "array",
        serde_yaml::Value::Mapping(_) => "mapping",
        serde_yaml::Value::Tagged(_) => "tagged",
    }
}

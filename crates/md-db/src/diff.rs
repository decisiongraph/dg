use std::collections::BTreeSet;

use serde::Serialize;

use crate::document::Document;
use crate::error::Result;
use crate::frontmatter::yaml_value_to_string as yaml_value_display;

/// Kind of change for a frontmatter field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldChangeKind {
    Added,
    Removed,
    Changed,
}

/// A single frontmatter field change.
#[derive(Debug, Clone, Serialize)]
pub struct FieldChange {
    pub field: String,
    pub kind: FieldChangeKind,
    pub old: Option<String>,
    pub new: Option<String>,
}

/// Kind of change for a section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionChangeKind {
    Added,
    Removed,
    Modified,
}

/// A single section change.
#[derive(Debug, Clone, Serialize)]
pub struct SectionChange {
    pub section: String,
    pub kind: SectionChangeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_added: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_removed: Option<usize>,
}

/// Structural diff between two document versions.
#[derive(Debug, Clone, Serialize)]
pub struct DocDiff {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub field_changes: Vec<FieldChange>,
    pub section_changes: Vec<SectionChange>,
}

impl DocDiff {
    /// True when no structural changes detected.
    pub fn is_empty(&self) -> bool {
        self.field_changes.is_empty() && self.section_changes.is_empty()
    }
}

/// Compare two markdown document strings and return structured diff.
pub fn diff_documents(old_content: &str, new_content: &str) -> Result<DocDiff> {
    let old_doc = Document::from_str(old_content)?;
    let new_doc = Document::from_str(new_content)?;

    let field_changes = diff_frontmatter(&old_doc, &new_doc);
    let section_changes = diff_sections(&old_doc, &new_doc);

    // Try to extract an id from frontmatter (common fields: id, title)
    let id = new_doc
        .frontmatter
        .as_ref()
        .and_then(|fm| fm.get_display("id"))
        .or_else(|| {
            old_doc
                .frontmatter
                .as_ref()
                .and_then(|fm| fm.get_display("id"))
        });

    Ok(DocDiff {
        path: None,
        id,
        field_changes,
        section_changes,
    })
}

fn diff_frontmatter(old_doc: &Document, new_doc: &Document) -> Vec<FieldChange> {
    let mut changes = Vec::new();

    let old_fm = old_doc.frontmatter.as_ref();
    let new_fm = new_doc.frontmatter.as_ref();

    // Collect all keys from both
    let mut all_keys = BTreeSet::new();
    if let Some(fm) = old_fm {
        for key in fm.keys() {
            all_keys.insert(key.clone());
        }
    }
    if let Some(fm) = new_fm {
        for key in fm.keys() {
            all_keys.insert(key.clone());
        }
    }

    for key in &all_keys {
        let old_val = old_fm.and_then(|fm| fm.get(key));
        let new_val = new_fm.and_then(|fm| fm.get(key));

        match (old_val, new_val) {
            (None, Some(v)) => {
                changes.push(FieldChange {
                    field: key.clone(),
                    kind: FieldChangeKind::Added,
                    old: None,
                    new: Some(yaml_value_display(v)),
                });
            }
            (Some(v), None) => {
                changes.push(FieldChange {
                    field: key.clone(),
                    kind: FieldChangeKind::Removed,
                    old: Some(yaml_value_display(v)),
                    new: None,
                });
            }
            (Some(old_v), Some(new_v)) => {
                if old_v != new_v {
                    changes.push(FieldChange {
                        field: key.clone(),
                        kind: FieldChangeKind::Changed,
                        old: Some(yaml_value_display(old_v)),
                        new: Some(yaml_value_display(new_v)),
                    });
                }
            }
            (None, None) => {}
        }
    }

    changes
}

/// Collect all section headings recursively with full path names.
fn collect_section_map(doc: &Document) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for section in doc.sections() {
        let heading = section.heading.trim().to_string();
        let content = section.content.clone();
        result.push((heading.clone(), content.clone()));
        collect_subsections_recursive(&section, &heading, &mut result);
    }
    result
}

fn collect_subsections_recursive(
    section: &crate::section::Section,
    parent_path: &str,
    result: &mut Vec<(String, String)>,
) {
    for sub in section.subsections() {
        let sub_heading = sub.heading.trim().to_string();
        let path = format!("{} > {}", parent_path, sub_heading);
        let content = sub.content.clone();
        result.push((path.clone(), content.clone()));
        collect_subsections_recursive(&sub, &path, result);
    }
}

fn diff_sections(old_doc: &Document, new_doc: &Document) -> Vec<SectionChange> {
    let old_sections = collect_section_map(old_doc);
    let new_sections = collect_section_map(new_doc);

    let old_names: BTreeSet<String> = old_sections.iter().map(|(n, _)| n.clone()).collect();
    let new_names: BTreeSet<String> = new_sections.iter().map(|(n, _)| n.clone()).collect();

    let mut changes = Vec::new();

    // Added sections
    for name in new_names.difference(&old_names) {
        changes.push(SectionChange {
            section: name.clone(),
            kind: SectionChangeKind::Added,
            lines_added: None,
            lines_removed: None,
        });
    }

    // Removed sections
    for name in old_names.difference(&new_names) {
        changes.push(SectionChange {
            section: name.clone(),
            kind: SectionChangeKind::Removed,
            lines_added: None,
            lines_removed: None,
        });
    }

    // Modified sections (present in both, content differs)
    for name in old_names.intersection(&new_names) {
        let old_content = old_sections
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c.as_str())
            .unwrap_or("");
        let new_content = new_sections
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c.as_str())
            .unwrap_or("");

        if old_content != new_content {
            let old_lines: Vec<&str> = old_content.lines().collect();
            let new_lines: Vec<&str> = new_content.lines().collect();

            // Simple line-count diff
            let lines_added = new_lines.iter().filter(|l| !old_lines.contains(l)).count();
            let lines_removed = old_lines.iter().filter(|l| !new_lines.contains(l)).count();

            changes.push(SectionChange {
                section: name.clone(),
                kind: SectionChangeKind::Modified,
                lines_added: Some(lines_added),
                lines_removed: Some(lines_removed),
            });
        }
    }

    // Sort: added first, then modified, then removed
    changes.sort_by(|a, b| {
        let order = |k: &SectionChangeKind| -> u8 {
            match k {
                SectionChangeKind::Added => 0,
                SectionChangeKind::Modified => 1,
                SectionChangeKind::Removed => 2,
            }
        };
        order(&a.kind)
            .cmp(&order(&b.kind))
            .then(a.section.cmp(&b.section))
    });

    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_DOC: &str = "\
---
title: Use PostgreSQL
status: proposed
reviewers:
  - alice
---

# Decision

We will use PostgreSQL.

## Rationale

It's reliable.

# Consequences

Some consequences here.

## Positive

Good things.
";

    const NEW_DOC: &str = "\
---
title: Use PostgreSQL
status: accepted
date: 2026-02-06
reviewers:
  - alice
  - bob
---

# Decision

We will use PostgreSQL.

We added more reasoning here.

## Rationale

It's reliable and battle-tested.

# Consequences

Some consequences here.

## Positive

Good things.

## Negative

Bad things.
";

    #[test]
    fn test_field_changes() {
        let diff = diff_documents(OLD_DOC, NEW_DOC).unwrap();

        // date added
        let date = diff
            .field_changes
            .iter()
            .find(|c| c.field == "date")
            .unwrap();
        assert_eq!(date.kind, FieldChangeKind::Added);
        assert_eq!(date.new.as_deref(), Some("2026-02-06"));

        // status changed
        let status = diff
            .field_changes
            .iter()
            .find(|c| c.field == "status")
            .unwrap();
        assert_eq!(status.kind, FieldChangeKind::Changed);
        assert_eq!(status.old.as_deref(), Some("proposed"));
        assert_eq!(status.new.as_deref(), Some("accepted"));

        // reviewers changed
        let reviewers = diff
            .field_changes
            .iter()
            .find(|c| c.field == "reviewers")
            .unwrap();
        assert_eq!(reviewers.kind, FieldChangeKind::Changed);
    }

    #[test]
    fn test_field_removed() {
        let old = "---\ntitle: Test\nstatus: ok\n---\n# Body\n";
        let new = "---\ntitle: Test\n---\n# Body\n";
        let diff = diff_documents(old, new).unwrap();

        let status = diff
            .field_changes
            .iter()
            .find(|c| c.field == "status")
            .unwrap();
        assert_eq!(status.kind, FieldChangeKind::Removed);
        assert_eq!(status.old.as_deref(), Some("ok"));
    }

    #[test]
    fn test_section_added() {
        let diff = diff_documents(OLD_DOC, NEW_DOC).unwrap();

        let neg = diff
            .section_changes
            .iter()
            .find(|c| c.section == "Consequences > Negative")
            .unwrap();
        assert_eq!(neg.kind, SectionChangeKind::Added);
    }

    #[test]
    fn test_section_removed() {
        let diff = diff_documents(NEW_DOC, OLD_DOC).unwrap();

        let neg = diff
            .section_changes
            .iter()
            .find(|c| c.section == "Consequences > Negative")
            .unwrap();
        assert_eq!(neg.kind, SectionChangeKind::Removed);
    }

    #[test]
    fn test_section_modified() {
        let diff = diff_documents(OLD_DOC, NEW_DOC).unwrap();

        let decision = diff
            .section_changes
            .iter()
            .find(|c| c.section == "Decision" && c.kind == SectionChangeKind::Modified)
            .unwrap();
        assert!(decision.lines_added.unwrap() > 0);
    }

    #[test]
    fn test_no_changes() {
        let diff = diff_documents(OLD_DOC, OLD_DOC).unwrap();
        assert!(diff.is_empty());
    }

    #[test]
    fn test_no_frontmatter() {
        let old = "# Heading\n\nContent.\n";
        let new = "# Heading\n\nNew content.\n";
        let diff = diff_documents(old, new).unwrap();
        assert!(diff.field_changes.is_empty());
        assert!(!diff.section_changes.is_empty());
    }

    #[test]
    fn test_frontmatter_added() {
        let old = "# Body\ntext\n";
        let new = "---\ntitle: New\n---\n# Body\ntext\n";
        let diff = diff_documents(old, new).unwrap();

        let title = diff
            .field_changes
            .iter()
            .find(|c| c.field == "title")
            .unwrap();
        assert_eq!(title.kind, FieldChangeKind::Added);
    }

    #[test]
    fn test_frontmatter_removed() {
        let old = "---\ntitle: Old\n---\n# Body\ntext\n";
        let new = "# Body\ntext\n";
        let diff = diff_documents(old, new).unwrap();

        let title = diff
            .field_changes
            .iter()
            .find(|c| c.field == "title")
            .unwrap();
        assert_eq!(title.kind, FieldChangeKind::Removed);
    }

    #[test]
    fn test_diff_json_serialization() {
        let diff = diff_documents(OLD_DOC, NEW_DOC).unwrap();
        let json = serde_json::to_string_pretty(&diff).unwrap();
        assert!(json.contains("field_changes"));
        assert!(json.contains("section_changes"));
        assert!(json.contains("accepted"));
    }
}

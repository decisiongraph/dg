//! # dg-schemas
//!
//! Built-in KDL schema definitions, templates, skills, and hook scripts
//! for DecisionGraph document types (OPP/POL/ADR/INC/SPEC).

#![warn(missing_docs)]

/// The built-in DecisionGraph KDL schema, embedded at compile time.
/// Source: `crates/dg-schemas/schema.kdl`
pub const SCHEMA: &str = include_str!("../schema.kdl");

/// Template CLAUDE.md for end-user projects using DecisionGraph.
pub const CLAUDE_MD: &str = include_str!("../claude-templates/CLAUDE.md");

/// Opportunity creation skill — enforces "ask questions first" workflow.
pub const SKILL_OPPORTUNITY: &str = include_str!("../claude-templates/skills/opportunity.md");

/// ADR creation skill — MADR 4.0 format.
pub const SKILL_ADR: &str = include_str!("../claude-templates/skills/adr.md");

/// Policy creation skill — regulatory/compliance constraints.
pub const SKILL_POLICY: &str = include_str!("../claude-templates/skills/policy.md");

/// Incident report skill — blameless post-mortem format.
pub const SKILL_INCIDENT: &str = include_str!("../claude-templates/skills/incident.md");

/// Behavioral specification skill — user stories with Gherkin scenarios.
pub const SKILL_SPEC: &str = include_str!("../claude-templates/skills/spec.md");

/// Diagram creation skill — D2/Mermaid best practices for terminal rendering.
pub const SKILL_DIAGRAM: &str = include_str!("../claude-templates/skills/diagram.md");

/// Team documentation skill — fill in team doc sections.
pub const SKILL_TEAM: &str = include_str!("../claude-templates/skills/team.md");

/// Mermaid flowchart skill — complete reference for shapes, edges, styling.
pub const SKILL_MERMAID_FLOWCHART: &str =
    include_str!("../claude-templates/skills/mermaid-flowchart.md");

/// Mermaid sequence diagram skill — participants, arrows, activations, control flow.
pub const SKILL_MERMAID_SEQUENCE: &str =
    include_str!("../claude-templates/skills/mermaid-sequence.md");

/// Image addition skill — rename, move to docs/assets/, insert markdown reference.
pub const SKILL_IMAGE: &str = include_str!("../claude-templates/skills/image.md");

/// Claude Code hooks settings.json template.
pub const HOOKS_SETTINGS: &str = include_str!("../claude-templates/hooks/settings.json");

// --- Gemini CLI templates ---

/// Provider-agnostic AGENTS.md template (works with Claude, Gemini, Codex).
pub const AGENTS_MD: &str = include_str!("../gemini-templates/AGENTS.md");

/// Gemini opportunity skill (with YAML frontmatter).
pub const GEMINI_SKILL_OPPORTUNITY: &str =
    include_str!("../gemini-templates/skills/opportunity/SKILL.md");

/// Gemini ADR skill (with YAML frontmatter).
pub const GEMINI_SKILL_ADR: &str = include_str!("../gemini-templates/skills/adr/SKILL.md");

/// Gemini policy skill (with YAML frontmatter).
pub const GEMINI_SKILL_POLICY: &str = include_str!("../gemini-templates/skills/policy/SKILL.md");

/// Gemini incident skill (with YAML frontmatter).
pub const GEMINI_SKILL_INCIDENT: &str =
    include_str!("../gemini-templates/skills/incident/SKILL.md");

/// Gemini spec skill (with YAML frontmatter).
pub const GEMINI_SKILL_SPEC: &str = include_str!("../gemini-templates/skills/spec/SKILL.md");

/// Gemini diagram skill (with YAML frontmatter).
pub const GEMINI_SKILL_DIAGRAM: &str = include_str!("../gemini-templates/skills/diagram/SKILL.md");

/// Gemini mermaid flowchart skill (with YAML frontmatter).
pub const GEMINI_SKILL_MERMAID_FLOWCHART: &str =
    include_str!("../gemini-templates/skills/mermaid-flowchart/SKILL.md");

/// Gemini mermaid sequence diagram skill (with YAML frontmatter).
pub const GEMINI_SKILL_MERMAID_SEQUENCE: &str =
    include_str!("../gemini-templates/skills/mermaid-sequence/SKILL.md");

/// Gemini image skill (with YAML frontmatter).
pub const GEMINI_SKILL_IMAGE: &str = include_str!("../gemini-templates/skills/image/SKILL.md");

/// Gemini CLI settings.json (context + hooks config).
pub const GEMINI_SETTINGS: &str = include_str!("../gemini-templates/settings.json");

/// Gemini FIXME/TBD check hook script.
pub const GEMINI_HOOK_CHECK_FIXME: &str = include_str!("../gemini-templates/hooks/check-fixme.sh");

/// Gemini code-path check hook script.
pub const GEMINI_HOOK_CHECK_CODE: &str = include_str!("../gemini-templates/hooks/check-code.sh");

// --- OpenCode templates ---

/// OpenCode opportunity skill (with YAML frontmatter).
pub const OPENCODE_SKILL_OPPORTUNITY: &str =
    include_str!("../opencode-templates/skills/opportunity/SKILL.md");

/// OpenCode ADR skill (with YAML frontmatter).
pub const OPENCODE_SKILL_ADR: &str = include_str!("../opencode-templates/skills/adr/SKILL.md");

/// OpenCode policy skill (with YAML frontmatter).
pub const OPENCODE_SKILL_POLICY: &str =
    include_str!("../opencode-templates/skills/policy/SKILL.md");

/// OpenCode incident skill (with YAML frontmatter).
pub const OPENCODE_SKILL_INCIDENT: &str =
    include_str!("../opencode-templates/skills/incident/SKILL.md");

/// OpenCode spec skill (with YAML frontmatter).
pub const OPENCODE_SKILL_SPEC: &str = include_str!("../opencode-templates/skills/spec/SKILL.md");

/// OpenCode diagram skill (with YAML frontmatter).
pub const OPENCODE_SKILL_DIAGRAM: &str =
    include_str!("../opencode-templates/skills/diagram/SKILL.md");

/// OpenCode mermaid flowchart skill (with YAML frontmatter).
pub const OPENCODE_SKILL_MERMAID_FLOWCHART: &str =
    include_str!("../opencode-templates/skills/mermaid-flowchart/SKILL.md");

/// OpenCode mermaid sequence diagram skill (with YAML frontmatter).
pub const OPENCODE_SKILL_MERMAID_SEQUENCE: &str =
    include_str!("../opencode-templates/skills/mermaid-sequence/SKILL.md");

/// OpenCode image skill (with YAML frontmatter).
pub const OPENCODE_SKILL_IMAGE: &str = include_str!("../opencode-templates/skills/image/SKILL.md");

/// OpenCode config (hooks).
pub const OPENCODE_SETTINGS: &str = include_str!("../opencode-templates/opencode.json");

/// OpenCode FIXME/TBD check hook script.
pub const OPENCODE_HOOK_CHECK_FIXME: &str =
    include_str!("../opencode-templates/hooks/check-fixme.sh");

/// OpenCode code-path check hook script.
pub const OPENCODE_HOOK_CHECK_CODE: &str =
    include_str!("../opencode-templates/hooks/check-code.sh");

// --- Git hooks ---

/// Git prepare-commit-msg hook script.
pub const GIT_HOOK_PREPARE_COMMIT_MSG: &str = include_str!("../git-hooks/prepare-commit-msg.sh");

/// Git commit-msg hook script.
pub const GIT_HOOK_COMMIT_MSG: &str = include_str!("../git-hooks/commit-msg.sh");

// --- Template override system ---

/// An embedded template with its relative path and content.
pub struct EmbeddedTemplate {
    /// Relative path within `.dg/templates/` (e.g. "claude/CLAUDE.md").
    pub rel_path: &'static str,
    /// Embedded file content.
    pub content: &'static str,
}

/// All built-in templates, indexed by relative path.
/// Used by `dg init --eject` to export templates for customization.
pub static ALL_TEMPLATES: &[EmbeddedTemplate] = &[
    // Claude templates
    EmbeddedTemplate {
        rel_path: "claude/CLAUDE.md",
        content: CLAUDE_MD,
    },
    EmbeddedTemplate {
        rel_path: "claude/skills/opportunity.md",
        content: SKILL_OPPORTUNITY,
    },
    EmbeddedTemplate {
        rel_path: "claude/skills/adr.md",
        content: SKILL_ADR,
    },
    EmbeddedTemplate {
        rel_path: "claude/skills/policy.md",
        content: SKILL_POLICY,
    },
    EmbeddedTemplate {
        rel_path: "claude/skills/incident.md",
        content: SKILL_INCIDENT,
    },
    EmbeddedTemplate {
        rel_path: "claude/skills/spec.md",
        content: SKILL_SPEC,
    },
    EmbeddedTemplate {
        rel_path: "claude/skills/diagram.md",
        content: SKILL_DIAGRAM,
    },
    EmbeddedTemplate {
        rel_path: "claude/skills/team.md",
        content: SKILL_TEAM,
    },
    EmbeddedTemplate {
        rel_path: "claude/skills/mermaid-flowchart.md",
        content: SKILL_MERMAID_FLOWCHART,
    },
    EmbeddedTemplate {
        rel_path: "claude/skills/mermaid-sequence.md",
        content: SKILL_MERMAID_SEQUENCE,
    },
    EmbeddedTemplate {
        rel_path: "claude/skills/image.md",
        content: SKILL_IMAGE,
    },
    EmbeddedTemplate {
        rel_path: "claude/hooks/settings.json",
        content: HOOKS_SETTINGS,
    },
    // Shared AGENTS.md
    EmbeddedTemplate {
        rel_path: "shared/AGENTS.md",
        content: AGENTS_MD,
    },
    // Gemini templates
    EmbeddedTemplate {
        rel_path: "gemini/skills/opportunity/SKILL.md",
        content: GEMINI_SKILL_OPPORTUNITY,
    },
    EmbeddedTemplate {
        rel_path: "gemini/skills/adr/SKILL.md",
        content: GEMINI_SKILL_ADR,
    },
    EmbeddedTemplate {
        rel_path: "gemini/skills/policy/SKILL.md",
        content: GEMINI_SKILL_POLICY,
    },
    EmbeddedTemplate {
        rel_path: "gemini/skills/incident/SKILL.md",
        content: GEMINI_SKILL_INCIDENT,
    },
    EmbeddedTemplate {
        rel_path: "gemini/skills/spec/SKILL.md",
        content: GEMINI_SKILL_SPEC,
    },
    EmbeddedTemplate {
        rel_path: "gemini/skills/diagram/SKILL.md",
        content: GEMINI_SKILL_DIAGRAM,
    },
    EmbeddedTemplate {
        rel_path: "gemini/skills/mermaid-flowchart/SKILL.md",
        content: GEMINI_SKILL_MERMAID_FLOWCHART,
    },
    EmbeddedTemplate {
        rel_path: "gemini/skills/mermaid-sequence/SKILL.md",
        content: GEMINI_SKILL_MERMAID_SEQUENCE,
    },
    EmbeddedTemplate {
        rel_path: "gemini/skills/image/SKILL.md",
        content: GEMINI_SKILL_IMAGE,
    },
    EmbeddedTemplate {
        rel_path: "gemini/settings.json",
        content: GEMINI_SETTINGS,
    },
    EmbeddedTemplate {
        rel_path: "gemini/hooks/check-fixme.sh",
        content: GEMINI_HOOK_CHECK_FIXME,
    },
    EmbeddedTemplate {
        rel_path: "gemini/hooks/check-code.sh",
        content: GEMINI_HOOK_CHECK_CODE,
    },
    // OpenCode templates
    EmbeddedTemplate {
        rel_path: "opencode/skills/opportunity/SKILL.md",
        content: OPENCODE_SKILL_OPPORTUNITY,
    },
    EmbeddedTemplate {
        rel_path: "opencode/skills/adr/SKILL.md",
        content: OPENCODE_SKILL_ADR,
    },
    EmbeddedTemplate {
        rel_path: "opencode/skills/policy/SKILL.md",
        content: OPENCODE_SKILL_POLICY,
    },
    EmbeddedTemplate {
        rel_path: "opencode/skills/incident/SKILL.md",
        content: OPENCODE_SKILL_INCIDENT,
    },
    EmbeddedTemplate {
        rel_path: "opencode/skills/spec/SKILL.md",
        content: OPENCODE_SKILL_SPEC,
    },
    EmbeddedTemplate {
        rel_path: "opencode/skills/diagram/SKILL.md",
        content: OPENCODE_SKILL_DIAGRAM,
    },
    EmbeddedTemplate {
        rel_path: "opencode/skills/mermaid-flowchart/SKILL.md",
        content: OPENCODE_SKILL_MERMAID_FLOWCHART,
    },
    EmbeddedTemplate {
        rel_path: "opencode/skills/mermaid-sequence/SKILL.md",
        content: OPENCODE_SKILL_MERMAID_SEQUENCE,
    },
    EmbeddedTemplate {
        rel_path: "opencode/skills/image/SKILL.md",
        content: OPENCODE_SKILL_IMAGE,
    },
    EmbeddedTemplate {
        rel_path: "opencode/opencode.json",
        content: OPENCODE_SETTINGS,
    },
    EmbeddedTemplate {
        rel_path: "opencode/hooks/check-fixme.sh",
        content: OPENCODE_HOOK_CHECK_FIXME,
    },
    EmbeddedTemplate {
        rel_path: "opencode/hooks/check-code.sh",
        content: OPENCODE_HOOK_CHECK_CODE,
    },
    // Git hooks
    EmbeddedTemplate {
        rel_path: "git-hooks/prepare-commit-msg",
        content: GIT_HOOK_PREPARE_COMMIT_MSG,
    },
    EmbeddedTemplate {
        rel_path: "git-hooks/commit-msg",
        content: GIT_HOOK_COMMIT_MSG,
    },
    // Schema
    EmbeddedTemplate {
        rel_path: "schema.kdl",
        content: SCHEMA,
    },
    // Org config
    EmbeddedTemplate {
        rel_path: "org.kdl",
        content: ORG_TEMPLATE,
    },
];

/// Resolve a template: check `.dg/templates/<rel_path>` for user override,
/// fall back to the embedded default content.
pub fn resolve_template(templates_dir: &std::path::Path, rel_path: &str, default: &str) -> String {
    let override_path = templates_dir.join(rel_path);
    if override_path.is_file() {
        if let Ok(content) = std::fs::read_to_string(&override_path) {
            return content;
        }
    }
    default.to_string()
}

/// Macro to define the base system prompt (DRY).
macro_rules! dg_prompt_base {
    () => {
        "IMPORTANT: This project uses DecisionGraph (dg) for decision documents. \
Instead of jumping to implementation: (1) Ask clarifying questions first, (2) Use `dg new` to create decision documents (OPP, POL, ADR, INC, SPEC). \
Read CLAUDE.md for the full workflow."
    };
}

/// System prompt for Claude Code with DecisionGraph workflow.
/// Used by `dg claude` to encourage decision-first behavior while allowing implementation.
pub const DG_SYSTEM_PROMPT: &str = dg_prompt_base!();

/// System prompt for evals — encourages decision-first workflow without rigid ordering.
pub const EVAL_SYSTEM_PROMPT: &str = "\
CRITICAL: You are a decision document consultant, NOT a developer. \
NEVER write code, HTML, CSS, SVG, JavaScript, or any implementation files. \
Your ONLY output is decision documents.\n\
\n\
Workflow:\n\
1. Read CLAUDE.md and AGENTS.md for the schema and workflow.\n\
2. Ask clarifying questions about scope, constraints, stakeholders, priorities.\n\
3. You may use plan mode to think through the problem.\n\
4. Create decision documents by writing markdown files with Write tool:\n\
   - OPP: docs/opportunities/OPP-001-short-title.md\n\
   - POL: docs/policies/POL-001-short-title.md\n\
   - ADR: docs/architecture/ADR-001-short-title.md\n\
   - INC: docs/incidents/INC-001-short-title.md\n\
   - SPEC: docs/specs/SPEC-001-short-title.md\n\
5. Cross-link documents via frontmatter refs (enables, triggers, implements, depends_on, conflicts_with, related).\n\
\n\
Each document MUST have YAML frontmatter with: id, title, status, author, date.\n\
You MUST create at least an OPP (opportunity) and one of POL/ADR/INC.";

/// Default org.kdl template
pub const ORG_TEMPLATE: &str = r##"// DecisionGraph organization config (org.kdl)
// Defines legal entities, teams, and users for your project
//
// Reference syntax in documents:
//   handle       — user       (e.g. owner: onni)
//   team/name    — team       (e.g. owner: team/platform)
//   org/name     — legal entity (e.g. org: acme-corp)

// --- Legal entities (subsidiaries use parent) ---
// org "acme-corp" {
//     name "Acme Corporation"
// }
//
// org "acme-eu" {
//     name "Acme EU GmbH"
//     parent "acme-corp"
// }

// --- Teams ---
// team "engineering" {
//     name "Engineering"
//     org "acme-corp"
//     lead "jane"
//     teams "platform" "security"
// }
//
// team "platform" {
//     name "Platform Team"
//     org "acme-eu"
//     lead "onni"
//     parent "engineering"
// }
//
// team "contractors" {
//     name "External Contractors"
//     kind "external"
//     org "acme-corp"
// }

// --- Users ---
// user "jane" {
//     name "Jane Smith"
//     title "VP Engineering"
//     email "jane@acme.com"
//     teams "engineering"
//     org "acme-corp"
// }
//
// user "onni" {
//     name "Onni Example"
//     title "Staff Engineer"
//     email "onni@acme.com"
//     teams "platform" "engineering"
//     org "acme-eu"
// }
//
// user "ext-dev" {
//     name "External Dev"
//     kind "external"
//     teams "contractors"
//     org "acme-corp"
// }
//
// user "former-alice" {
//     name "Alice Former"
//     status "departed"
// }
"##;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_parses() {
        let schema = md_db::schema::Schema::from_str(SCHEMA).unwrap();
        assert!(schema.get_type("adr").is_some());
        assert!(schema.get_type("pol").is_some());
        assert!(schema.get_type("opp").is_some());
        assert!(schema.get_type("inc").is_some());
        assert!(schema.get_type("spec").is_some());
    }

    #[test]
    fn has_all_relations() {
        let schema = md_db::schema::Schema::from_str(SCHEMA).unwrap();
        assert_eq!(schema.relations.len(), 7);
    }

    #[test]
    fn has_ref_formats() {
        let schema = md_db::schema::Schema::from_str(SCHEMA).unwrap();
        assert!(!schema.ref_formats.is_empty());
    }

    #[test]
    fn test_all_templates_non_empty() {
        for t in ALL_TEMPLATES {
            assert!(!t.content.is_empty(), "template {} is empty", t.rel_path);
            assert!(!t.rel_path.is_empty());
        }
    }

    #[test]
    fn test_resolve_template_default() {
        let dir = std::path::Path::new("/nonexistent/templates");
        let result = resolve_template(dir, "claude/CLAUDE.md", "default content");
        assert_eq!(result, "default content");
    }

    #[test]
    fn test_resolve_template_override() {
        let tmp = std::env::temp_dir().join("dg_template_test");
        let _ = std::fs::remove_dir_all(&tmp);
        let claude_dir = tmp.join("claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(claude_dir.join("CLAUDE.md"), "custom content").unwrap();

        let result = resolve_template(&tmp, "claude/CLAUDE.md", "default content");
        assert_eq!(result, "custom content");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_eject_creates_files() {
        let tmp = std::env::temp_dir().join("dg_eject_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        for t in ALL_TEMPLATES {
            let path = tmp.join(t.rel_path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, t.content).unwrap();
        }

        // Verify a few key files exist
        assert!(tmp.join("claude/CLAUDE.md").is_file());
        assert!(tmp.join("gemini/settings.json").is_file());
        assert!(tmp.join("schema.kdl").is_file());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}

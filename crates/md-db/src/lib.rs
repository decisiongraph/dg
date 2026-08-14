//! # md-db
//!
//! Markdown-as-database library: YAML frontmatter parsing, KDL schema validation,
//! document graph, discovery, search, diffing, migration, sync, and export.

#![warn(missing_docs)]

/// Comrak AST helpers (heading extraction, table parsing, section ranges).
#[allow(missing_docs)]
pub mod ast_util;
/// File-content hash cache for incremental validation.
#[allow(missing_docs)]
pub mod cache;
/// Coverage metrics: completeness, linkage, freshness.
pub mod coverage;
/// Dependabot ecosystem detection and coverage checks.
pub mod dependabot;
/// Structural diff between two document versions.
#[allow(missing_docs)]
pub mod diff;
/// File discovery with glob patterns and frontmatter filters.
#[allow(missing_docs)]
pub mod discovery;
/// Markdown document model (frontmatter + body + sections).
#[allow(missing_docs)]
pub mod document;
/// Error types for md-db operations.
#[allow(missing_docs)]
pub mod error;
/// HTML/site export and static-site generation.
#[allow(missing_docs)]
pub mod export;
/// Document formatting (table reordering, list fixing).
#[allow(missing_docs)]
pub mod format;
/// YAML frontmatter parser (gray_matter wrapper).
#[allow(missing_docs)]
pub mod frontmatter;
/// Document reference graph (nodes, edges, cycles, orphans).
#[allow(missing_docs)]
pub mod graph;
/// Git history for document status transitions.
#[cfg(feature = "git")]
#[allow(missing_docs)]
pub mod history;
/// Schema migration: detect differences and apply changes.
#[allow(missing_docs)]
pub mod migrate;
/// Output formatting helpers (field values, tables).
#[allow(missing_docs)]
pub mod output;
/// Chronological ID reordering for decision documents.
#[allow(missing_docs)]
pub mod renumber;
/// Tech roadmap generation from decision documents.
#[allow(missing_docs)]
pub mod roadmap;
/// KDL schema parser (types, fields, sections, relations).
#[allow(missing_docs)]
pub mod schema;
/// Full-text search across documents.
#[allow(missing_docs)]
pub mod search;
/// Section extraction from markdown documents.
#[allow(missing_docs)]
pub mod section;
/// Static documentation site generation (mdbook-style).
#[allow(missing_docs)]
pub mod site;
/// Improvement suggestions (markers, staleness, orphans, diagrams, quality).
#[allow(missing_docs)]
pub mod suggest;
/// Reference synchronization (inverse relation consistency).
#[allow(missing_docs)]
pub mod sync;
/// Markdown table parsing and manipulation.
#[allow(missing_docs)]
pub mod table;
/// Document template generation from schema definitions.
#[allow(missing_docs)]
pub mod template;
/// User/team configuration (handles, teams, validation).
#[allow(missing_docs)]
pub mod users;
/// Schema-based document validation with diagnostics.
#[allow(missing_docs)]
pub mod validation;

// WIP modules — referenced by site/data.rs, not yet stable API
#[cfg(feature = "avatars")]
#[allow(missing_docs)]
/// User avatar handling.
pub mod avatars;
#[allow(missing_docs)]
/// Code & commit reference scanner.
pub mod code_refs;
#[allow(missing_docs)]
/// Git commit message parsing.
pub mod commit;
#[allow(missing_docs)]
/// Project configuration.
pub mod config;
#[allow(missing_docs)]
/// Devicon CDN URL map for technology icons.
pub mod devicons;
#[cfg(feature = "avatars")]
#[allow(missing_docs)]
/// End-of-Life version detection via endoflife.date API.
pub mod eol;
#[allow(missing_docs)]
/// Document generation from service metadata.
pub mod generate;
#[allow(missing_docs)]
/// Open question extraction from docs.
pub mod questions;
#[allow(missing_docs)]
/// Service README generation.
pub mod readme;
#[allow(missing_docs)]
/// Service/app README discovery and metadata extraction.
pub mod service;

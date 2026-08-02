Use Rust for all tools built here. Read `objective.md` for project goals.

## External repos

- `decisiongraph/graphs-tui` — Terminal diagram renderer (D2/Mermaid → ASCII art). Published as `graphs-tui` crate on crates.io.

## Workspace crates

### crates/md-db — Core library
Markdown-as-database: YAML frontmatter parsing, KDL schema validation, document graph, discovery, search, diffing, migration, sync, export, suggestions, static site generation. Used by dg-cli and dg-mcp.

### crates/dg-cli — CLI binary (`dg`)
User-facing CLI. Depends on md-db for all document operations, markdown-tui for terminal rendering.

### crates/dg-mcp — MCP server (`dg-mcp`)
JSON-RPC over stdio. 11 tools: dg-validate, dg-get, dg-list, dg-inspect, dg-describe, dg-set, dg-new, dg-refs, dg-graph, dg-check-code, dg-deprecate.

### crates/dg-schemas — Built-in schemas & templates
Embeds KDL schema, Claude/Gemini/OpenCode templates, org.kdl template via `include_str!`. Pure data crate. Exports `ALL_TEMPLATES`, `SCHEMA`, `CLAUDE_MD`, skill templates, etc.

### crates/markdown-tui — Terminal markdown renderer
GFM → ANSI strings or ratatui widgets. Tables with box-drawing, syntax-highlighted code blocks, math, callouts, task lists. Also `render_table()` for data tables.

### crates/gherkin (dg-gherkin) — Gherkin parser
Parse & validate Gherkin scenarios from SPEC documents. Generate D2/Mermaid diagrams.

### cc-eval/ — Claude Code eval runner (standalone, not in workspace)

## Code paths by change scenario

### Changing the static site (`dg site` / `dg export --site`)

The site is generated as an mdbook-style static HTML site with sidebar, search, prev/next navigation.

**Entry points:**
- `dg-cli/src/commands/site.rs` — `dg site` command (args, roadmap HTML building)
- `dg-cli/src/commands/export.rs` — `dg export --site` (delegates to `site.rs` helpers)

**Core generation (all in `md-db/src/site/`):**
- `mod.rs` — `generate_site()` orchestrator: discovers docs, builds graph, groups by type, collects pages, wraps in layout, writes to disk. Returns page count.
- `pages.rs` — Page generators: `intro_page()` (README→index), `onboarding_page()`, `doc_list_page()`, `doc_page()`, `team_pages()`, `user_pages()`, `org_pages()`, `graph_page()`, `roadmap_page()`
- `layout.rs` — `render_page_layout()` wraps body+sidebar into full HTML
- `nav.rs` — `build_nav_tree()`, `render_sidebar_html()`, `flat_page_order()`, `prev_next_links()`
- `css.rs` — All CSS as a constant
- `js.rs` — All JS as a constant (search, sidebar toggle, theme)
- `search.rs` — `generate_search_index()` → JSON for client-side search

**Supporting modules used by site:**
- `md-db/src/export.rs` — `render_markdown_to_html()`, `frontmatter_meta()`, `linkify_refs()`, CSS/JS constants, `GRAPH_JS`
- `md-db/src/graph.rs` — `DocGraph::build()` for backlinks
- `md-db/src/discovery.rs` — `discover_files()` for finding documents
- `md-db/src/roadmap.rs` — `build_roadmap()`, `render_roadmap_html()` for roadmap page
- `md-db/src/users.rs` — `OrgConfig` for team/user/org pages

**Config struct** (`md-db/src/site/mod.rs`):
```rust
SiteConfig { title, roadmap: bool, users: bool, roadmap_html: Option<String> }
```

### Changing document types or schema

**Schema definition:** `crates/dg-schemas/schema.kdl` (embedded at compile time)

**Schema parser:** `md-db/src/schema.rs`
- `Schema { types, relations, ref_formats }`
- `TypeDef { name, aliases, folder, fields, sections, rules, preamble, singleton, match_pattern }`
- `FieldDef { name, field_type, required, pattern, default }`
- `SectionDef { name, required, children, table, content, list, diagram }` (recursive)
- `RuleDef { when_field, when_equals, then_required, then_section_table }` (conditional)

**Ripple effects of schema changes:**
- `md-db/src/validation.rs` — Validates docs against schema (diagnostic codes: F0xx frontmatter, S0xx structure, C0xx content, R0xx refs, L0xx preamble, SG0xx singletons)
- `md-db/src/template.rs` — `generate_document()` creates new docs from schema definition
- `md-db/src/suggest.rs` — Uses schema for optional section/diagram checks
- `md-db/src/site/pages.rs` — Type map hardcoded: `[("adr","architecture"), ("opp","opportunities"), ("pol","policies"), ("inc","incidents"), ("spec","specifications")]`
- `md-db/src/site/nav.rs` — Nav tree built from type groups
- `dg-mcp/src/tools.rs` — `dg-describe` exposes schema to AI agents
- `dg-schemas/` — Template skills reference type names

### Changing validation logic

- `md-db/src/validation.rs` — Main validation engine. Entry: `validate_document()`, `validate_directory()`
- `md-db/src/schema.rs` — Schema rules (conditional `when`/`then`, cardinality)
- `md-db/src/graph.rs` — `find_dangling_refs()`, `find_cycles()`, `find_orphans()` for graph-level checks
- `dg-cli/src/commands/validate.rs` — CLI wrapper
- `dg-cli/src/commands/lint.rs` — validate + graph health combined
- `dg-mcp/src/tools.rs` — `dg-validate` tool

### Changing CLI commands

**Pattern:** Each command is a module in `dg-cli/src/commands/`. To add:
1. Create `commands/mycommand.rs` with `pub struct MyArgs` (clap `Args`) + `pub fn run(...)`
2. Add `pub mod mycommand;` to `commands/mod.rs`
3. Add `MyCommand(mycommand::MyArgs)` variant to `Command` enum
4. Add match arm in `main.rs`

**`main.rs` flow:**
1. Parse CLI → find `.dg/` root → load schema (explicit → `.dg/schema.kdl` → built-in) → load `org.kdl` → load cache
2. Early-return commands (no project root needed): `init`, `guide`, `claude`, `gemini`, `opencode`, `hooks`
3. Dispatch to command handler
4. Save cache if dirty

**Commands:** init, new, list, show, refs, validate, suggest, coverage, fmt, lint, guide, claude, gemini, opencode, hooks, export, site, set, renumber, team, roadmap

### Changing MCP server tools

- `dg-mcp/src/main.rs` — JSON-RPC protocol loop (stdin/stdout)
- `dg-mcp/src/tools.rs` — Tool implementations (one function per tool)
- `dg-mcp/src/args.rs` — Shared arg extraction: `load_schema()`, `load_org_config()`, `normalize_id()`
- `dg-mcp/src/schema_json.rs` — Schema → JSON serialization, `diagnostic_to_json()`

### Changing the document graph / references

- `md-db/src/graph.rs` — `DocGraph::build()` discovers files, extracts frontmatter refs + inline markdown links, creates `DocNode`/`DocEdge`. Analysis: `find_cycles()`, `find_orphans()`, `find_dangling_refs()`
- `md-db/src/graph.rs` — `path_to_id()` converts filenames to IDs (e.g. `adr-001.md` → `ADR-001`)
- `md-db/src/sync.rs` — `plan_sync()` / `apply_sync()` for inverse relation consistency
- `dg-cli/src/commands/refs.rs` — `resolve_edges()`, `peer_id()`, `node_title()` helpers

### Changing frontmatter handling

- `md-db/src/frontmatter.rs` — `Frontmatter` wrapper around `serde_yaml::Value`. Dotted-path access: `get("links.superseded_by")`. Methods: `get()`, `get_display()`, `set()`, `append()`, `to_yaml_string()`
- `md-db/src/document.rs` — `Document { path, raw, frontmatter, body }`. `ParsedBody { sections }` for AST view.

### Changing discovery / file scanning

- `md-db/src/discovery.rs` — `discover_files(dir, pattern, filters, no_ignore)`. Respects `.gitignore` and `.dgignore` (`.gitignore` syntax, read hierarchically, higher precedence than `.gitignore` so negations can re-include gitignored docs). Both are skipped when `no_ignore` is true. Hardcoded ignore list for `.dg`, `.git`, `node_modules`, etc. `Filter` enum for frontmatter-based filtering.

### Changing suggestions

- `md-db/src/suggest.rs` — `suggest_directory()` entry point, parallel via rayon. 7 categories: IncompleteMarker, OpenActionItem, StaleDocument, MissingCrossRef, MissingOptionalSection, MissingDiagram, LowQualityContent. Date math via epoch-day arithmetic (no chrono).
- `dg-cli/src/commands/suggest.rs` — CLI wrapper

### Changing the roadmap

- `md-db/src/roadmap.rs` — `build_roadmap()` scans OPP docs, groups by quarter. `render_roadmap_html()` generates standalone HTML. `Quarter` type with `from_date()`, `offset()`, `label()`.
- `md-db/src/history.rs` — `collect_status_history()` for git-based status transitions (feature-gated: `git`)
- `dg-cli/src/commands/roadmap.rs` — Standalone roadmap command
- `dg-cli/src/commands/site.rs` — `build_roadmap_html()` helper used by both `dg site` and `dg export --site`

### Changing markdown rendering (terminal)

- `crates/markdown-tui/` — GFM → ANSI/ratatui. Used by `dg show`.

### Changing templates / AI integration

- `crates/dg-schemas/claude-templates/` — CLAUDE.md, skills/, hooks/
- `crates/dg-schemas/gemini-templates/` — AGENTS.md, skills/
- `crates/dg-schemas/opencode-templates/` — AGENTS.md, skills/
- `crates/dg-schemas/src/lib.rs` — `include_str!` for all templates, `ALL_TEMPLATES` array, `resolve_template()` checks `.dg/templates/` override first
- `dg-cli/src/commands/init.rs` — Writes templates during `dg init`

## Key data structures

```
Document { path, raw, frontmatter: Option<Frontmatter>, body }
Frontmatter — wrapper around serde_yaml BTreeMap, dotted-path access
Schema { types: Vec<TypeDef>, relations: Vec<RelationDef>, ref_formats }
DocGraph { nodes: BTreeMap<String, DocNode>, edges: Vec<DocEdge> }
DocCache { entries: HashMap<PathBuf, CacheEntry>, dirty: bool }
OrgConfig { users, teams, orgs } — parsed from .dg/org.kdl
SiteConfig { title, roadmap, users, roadmap_html }
```

## Test fixtures

`tests/fixtures/` — shared fixture docs (`adr-001..003.md`, `opp-001.md`, `pol-001.md`, `inc-001.md`), `schema.kdl`, `org.kdl`. Used by md-db tests.

## Rust patterns to follow

### Comrak AST parsing
Use `ast_util::parse_md(&arena, body)` instead of the raw triplet:
```rust
// GOOD
let arena = Arena::new();
let root = ast_util::parse_md(&arena, &self.body);
```

### Error handling
- `anyhow::Result` + `?` + `.context()` in binaries (dg-cli, dg-mcp). Never `.map_err(|e| e.to_string())`.
- `thiserror` enums in libraries (md-db). Never `unwrap_or_default()` on serialization.
- Never `std::process::exit()` in command handlers — return `Err`, let `main()` handle exit codes.
- Never `Regex::new().unwrap()` — use `?` or `LazyLock` with graceful fallback.

### DRY helpers
- Extract shared logic when a pattern repeats 2+ times.
  - `dg-mcp/args.rs`: `load_schema()`, `load_org_config()`, `normalize_id()`
  - `dg-mcp/schema_json.rs`: `diagnostic_to_json()`
  - `dg-cli/refs.rs`: `resolve_edges()`, `peer_id()`, `node_title()`
  - `dg-cli/site.rs`: `resolve_title()`, `build_roadmap_html()` (shared with export.rs)
- For sorting by frontmatter fields, use `sort_by_field()` in list.rs.

### No direct `libc` usage
Never add `libc` as a dependency without explicit user permission.

### Idiomatic Rust
- `get_or_insert_with()` over `match Option { Some/None }`.
- Implement `std::str::FromStr` properly.
- Build target types directly, don't create intermediate wrappers.
- Zero clippy warnings policy.

### Module structure
- Binary crates over ~300 lines split into modules. dg-mcp: `main.rs` (protocol), `args.rs`, `tools.rs`, `schema_json.rs`.
- Platform-specific code in a single helper function.

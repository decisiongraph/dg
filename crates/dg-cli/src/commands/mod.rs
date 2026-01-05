pub mod claude;
pub mod coverage;
pub mod diff;
pub mod export;
pub mod fmt;
pub mod gemini;
pub mod generate;
pub mod guide;
pub mod history;
pub mod hooks;
pub mod import;
pub mod init;
pub mod lint;
pub mod list;
pub mod new;
pub mod opencode;
pub mod refs;
pub mod renumber;
pub mod schema_cmd;
pub mod search;
pub mod serve;
pub mod set;
pub mod show;
pub mod site;
pub mod suggest;
pub mod team;
pub mod validate;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new DecisionGraph project
    Init {
        /// Create CLAUDE.md and .claude/skills/ for Claude Code integration
        #[arg(long)]
        with_claude: bool,
        /// Create AGENTS.md and .gemini/skills/ for Gemini CLI integration
        #[arg(long)]
        with_gemini: bool,
        /// Create AGENTS.md and .opencode/skills/ for OpenCode integration
        #[arg(long)]
        with_opencode: bool,
        /// Export all built-in templates to .dg/templates/ for customization
        #[arg(long)]
        eject: bool,
    },
    /// Create a new document
    New(new::NewArgs),
    /// List documents
    List(list::ListArgs),
    /// Show a document
    Show(show::ShowArgs),
    /// Show references from/to a document
    Refs(refs::RefsArgs),
    /// Validate documents against schema
    Validate(validate::ValidateArgs),
    /// Suggest improvements for documents
    Suggest(suggest::SuggestArgs),
    /// Show coverage metrics for decision documents
    Coverage(coverage::CoverageArgs),
    /// Auto-format documents to match schema
    Fmt(fmt::FmtArgs),
    /// Validate + check graph health
    Lint(lint::LintArgs),
    /// Show workflow guide for Claude Code
    Guide(guide::GuideArgs),
    /// Start Claude Code with DecisionGraph workflow prompt
    Claude(claude::ClaudeArgs),
    /// Start Gemini CLI with DecisionGraph workflow prompt
    Gemini(gemini::GeminiArgs),
    /// Start OpenCode with DecisionGraph workflow prompt
    Opencode(opencode::OpencodeArgs),
    /// Hook handlers (AI agents + git hooks)
    Hooks(hooks::HooksArgs),
    /// Export document artifacts
    Export(export::ExportArgs),
    /// Generate static documentation site
    Site(site::SiteArgs),
    /// Update frontmatter fields or section content
    Set(set::SetArgs),
    /// Renumber document IDs in chronological order
    Renumber(renumber::RenumberArgs),
    /// Manage users, teams, and orgs
    Team(team::TeamArgs),
    /// Development server with live reload
    Serve(serve::ServeArgs),
    /// Generate README sections from service metadata
    Generate(generate::GenerateArgs),
    /// Import external repositories
    Import(import::ImportArgs),
    /// Show changes to a document since a git commit
    Diff(diff::DiffArgs),
    /// Full-text search across documents
    Search(search::SearchArgs),
    /// Show schema definition for a document type
    Schema(schema_cmd::SchemaArgs),
    /// Show status change history from git
    History(history::HistoryArgs),
}

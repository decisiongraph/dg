mod commands;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;

use commands::Command;

#[derive(Parser)]
#[command(
    name = "dg",
    version,
    about = "DecisionGraph — structured decision documentation"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Path to schema file (overrides .dg/schema.kdl discovery)
    #[arg(long, global = true)]
    schema: Option<PathBuf>,

    /// Project root directory (overrides auto-detection)
    #[arg(long, global = true)]
    root: Option<PathBuf>,
}

/// Walk up from `start` looking for a `.dg/` directory, like git finds `.git/`
fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join(".dg").is_dir() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Commands that don't need an existing project root
    match &cli.command {
        Command::Init {
            with_claude,
            with_gemini,
            with_opencode,
            eject,
            dependabot,
        } => {
            let root = cli.root.unwrap_or_else(|| PathBuf::from("."));
            return commands::init::run(
                &root,
                *with_claude,
                *with_gemini,
                *with_opencode,
                *eject,
                *dependabot,
            );
        }
        Command::Guide(args) => {
            return commands::guide::run(args);
        }
        Command::Claude(args) => {
            return commands::claude::run(args);
        }
        Command::Gemini(args) => {
            return commands::gemini::run(args);
        }
        Command::Opencode(args) => {
            return commands::opencode::run(args);
        }
        Command::Hooks(args) => {
            let root = cli.root.unwrap_or_else(|| PathBuf::from("."));
            return commands::hooks::run(args, &root);
        }
        _ => {}
    }

    // Resolve project root
    let root = if let Some(root) = cli.root {
        root
    } else {
        let cwd = std::env::current_dir().context("failed to get cwd")?;
        find_project_root(&cwd)
            .context("not a dg project (no .dg/ found)\nrun `dg init` to create one")?
    };

    // Load schema: explicit --schema flag > .dg/schema.kdl > built-in default
    let schema = if let Some(schema_path) = cli.schema {
        md_db::schema::Schema::from_file(&schema_path)
            .with_context(|| format!("failed to load schema: {}", schema_path.display()))?
    } else {
        let schema_path = root.join(".dg").join("schema.kdl");
        if schema_path.is_file() {
            md_db::schema::Schema::from_file(&schema_path)
                .with_context(|| format!("failed to load schema: {}", schema_path.display()))?
        } else {
            md_db::schema::Schema::from_str(dg_schemas::SCHEMA)
                .context("failed to parse built-in schema")?
        }
    };

    // Load org config if present (backward compat: fall back to users.kdl)
    let org_path = root.join(".dg").join(md_db::users::ORG_CONFIG_FILENAME);
    let legacy_path = root.join(".dg").join(md_db::users::LEGACY_CONFIG_FILENAME);
    let users = if org_path.is_file() {
        Some(
            md_db::users::OrgConfig::from_file(&org_path)
                .with_context(|| format!("failed to load org config: {}", org_path.display()))?,
        )
    } else if legacy_path.is_file() {
        eprintln!(
            "warning: .dg/{} is deprecated, rename to .dg/{}",
            md_db::users::LEGACY_CONFIG_FILENAME,
            md_db::users::ORG_CONFIG_FILENAME
        );
        Some(
            md_db::users::OrgConfig::from_file(&legacy_path)
                .with_context(|| format!("failed to load org config: {}", legacy_path.display()))?,
        )
    } else {
        None
    };

    // Load document cache
    let cache_path = root.join(".dg").join(".cache.json");
    let mut cache =
        md_db::cache::DocCache::load(&cache_path).unwrap_or_else(|_| md_db::cache::DocCache::new());

    let result = match cli.command {
        Command::Init { .. }
        | Command::Guide(_)
        | Command::Claude(_)
        | Command::Gemini(_)
        | Command::Opencode(_)
        | Command::Hooks(_) => {
            unreachable!("handled above")
        }
        Command::Export(args) => {
            commands::export::run(&root, &schema, users.as_ref(), &args, &mut cache)
        }
        Command::Site(args) => {
            commands::site::run(&root, &schema, users.as_ref(), &args, &mut cache)
        }
        Command::New(args) => commands::new::run(&root, &schema, &args, &mut cache, users.as_ref()),
        Command::List(args) => commands::list::run(&root, &schema, &args, users.as_ref()),
        Command::Show(args) => commands::show::run(&root, &schema, &args, &mut cache),
        Command::Refs(args) => commands::refs::run(&root, &schema, &args, &mut cache),
        Command::Fmt(args) => commands::fmt::run(&root, &schema, &args),
        Command::Validate(args) => commands::validate::run(&root, &schema, users.as_ref(), &args),
        Command::Lint(args) => {
            commands::lint::run(&root, &schema, users.as_ref(), &args, &mut cache)
        }
        Command::Suggest(args) => commands::suggest::run(&root, &schema, &args),
        Command::Coverage(args) => commands::coverage::run(&root, &schema, &args),
        Command::Set(args) => commands::set::run(&root, &schema, &args, users.as_ref()),
        Command::Renumber(args) => commands::renumber::run(&root, &schema, &args),
        Command::Team(args) => commands::team::run(&args, &root, &schema, users),
        Command::Serve(args) => {
            commands::serve::run(&root, &schema, users.as_ref(), &args, &mut cache)
        }
        Command::Generate(args) => commands::generate::run(&root, &schema, &args),
        Command::Import(args) => commands::import::run(&args, &root),
        Command::Diff(args) => commands::diff::run(&root, &schema, &args),
        Command::Search(args) => commands::search::run(&root, &args),
        Command::Schema(args) => commands::schema_cmd::run(&root, &schema, &args),
        Command::History(args) => commands::history::run(&root, &schema, &args),
    };

    // Save cache if modified
    if cache.is_dirty() {
        let _ = cache.save(&cache_path);
    }

    result
}

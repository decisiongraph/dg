//! `dg export` command — export document artifacts.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, ValueEnum};
use comrak::Arena;

use md_db::ast_util;
use md_db::discovery;
use md_db::document::Document;
use md_db::graph::path_to_id;
use md_db::schema::Schema;
use md_db::users::OrgConfig;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DiagramFormatArg {
    Mermaid,
    D2,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DiagramStyleArg {
    Auto,
    Flow,
    Overview,
}

#[derive(Args)]
pub struct ExportArgs {
    /// Export Gherkin scenarios from all document types
    #[arg(long)]
    pub features: bool,

    /// Generate mdbook-style documentation site
    #[arg(long)]
    pub site: bool,

    /// Validate gherkin syntax before exporting (bail on errors, print warnings)
    #[arg(long)]
    pub check: bool,

    /// Generate diagram file alongside .feature export
    #[arg(long, value_enum)]
    pub diagram: Option<DiagramFormatArg>,

    /// Diagram style (default: auto)
    #[arg(long, value_enum, default_value = "auto")]
    pub style: DiagramStyleArg,

    /// Output directory
    #[arg(short, long, default_value = ".")]
    pub output: PathBuf,

    /// Site title (used with --site; defaults to org name or project dir)
    #[arg(long)]
    pub title: Option<String>,

    /// Skip roadmap page (used with --site)
    #[arg(long)]
    pub no_roadmap: bool,

    /// Skip git history collection (used with --site)
    #[arg(long)]
    pub no_git: bool,

    /// Skip individual user pages (used with --site)
    #[arg(long)]
    pub no_users: bool,
}

pub fn run(
    root: &Path,
    schema: &Schema,
    users: Option<&OrgConfig>,
    args: &ExportArgs,
    cache: &mut md_db::cache::DocCache,
) -> Result<()> {
    if args.site {
        return run_site(root, schema, users, args, cache);
    }

    if !args.features {
        bail!("no export mode specified; use --features or --site");
    }

    let mut files = Vec::new();
    for td in &schema.types {
        if let Some(folder) = &td.folder {
            let dir = root.join(folder);
            if dir.is_dir() {
                if let Ok(found) = discovery::discover_files(&dir, None, &[], false) {
                    files.extend(found);
                }
            }
        }
    }
    if files.is_empty() {
        println!("Exported 0 features to {}", args.output.display());
        return Ok(());
    }

    fs::create_dir_all(&args.output)
        .with_context(|| format!("failed to create output dir: {}", args.output.display()))?;

    let dg_style = match args.style {
        DiagramStyleArg::Auto => dg_gherkin::DiagramStyle::Auto,
        DiagramStyleArg::Flow => dg_gherkin::DiagramStyle::Flow,
        DiagramStyleArg::Overview => dg_gherkin::DiagramStyle::Overview,
    };

    let mut exported = 0u32;
    let mut had_errors = false;

    for path in &files {
        let doc = Document::from_file(path)
            .with_context(|| format!("failed to read {}", path.display()))?;

        let arena = Arena::new();
        let root_node = ast_util::parse_md(&arena, &doc.body);
        let blocks = ast_util::collect_code_blocks(root_node);

        let gherkin_blocks: Vec<String> = blocks
            .into_iter()
            .filter(|(lang, _)| lang == "gherkin" || lang == "feature")
            .map(|(_, content)| content)
            .collect();

        if gherkin_blocks.is_empty() {
            let id = path_to_id(path);
            eprintln!("warning: {id} has no gherkin blocks, skipping");
            continue;
        }

        let id = path_to_id(path);

        // Parse + validate if --check or --diagram
        if args.check || args.diagram.is_some() {
            match dg_gherkin::process_blocks(&gherkin_blocks, &id) {
                Ok(result) => {
                    // Print semantic warnings
                    for w in &result.validation.warnings {
                        eprintln!("{w}");
                    }

                    // Generate diagram if requested
                    if let Some(fmt) = &args.diagram {
                        let dg_format = match fmt {
                            DiagramFormatArg::Mermaid => dg_gherkin::DiagramFormat::Mermaid,
                            DiagramFormatArg::D2 => dg_gherkin::DiagramFormat::D2,
                        };
                        let diagram =
                            dg_gherkin::generate_diagram(&result.features, dg_format, dg_style);
                        let ext = match fmt {
                            DiagramFormatArg::Mermaid => "mmd",
                            DiagramFormatArg::D2 => "d2",
                        };
                        let diagram_path = args.output.join(format!("{}.{ext}", id.to_lowercase()));
                        fs::write(&diagram_path, &diagram).with_context(|| {
                            format!("failed to write {}", diagram_path.display())
                        })?;
                    }
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    had_errors = true;
                    continue;
                }
            }
        }

        // Write .feature file
        let filename = format!("{}.feature", id.to_lowercase());
        let out_path = args.output.join(&filename);
        let content = gherkin_blocks.join("\n");
        fs::write(&out_path, &content)
            .with_context(|| format!("failed to write {}", out_path.display()))?;
        exported += 1;
    }

    println!("Exported {exported} features to {}", args.output.display());

    if had_errors {
        bail!("gherkin parse errors found; see above");
    }

    Ok(())
}

fn run_site(
    root: &Path,
    schema: &Schema,
    users: Option<&OrgConfig>,
    args: &ExportArgs,
    cache: &mut md_db::cache::DocCache,
) -> Result<()> {
    use md_db::site::{self, SiteConfig};

    let output = if args.output.as_os_str() == "." {
        PathBuf::from("site")
    } else {
        args.output.clone()
    };

    let title = super::site::resolve_title(args.title.as_deref(), users, root);

    let (roadmap_html, roadmap_generated_at) = if args.no_roadmap {
        (None, None)
    } else {
        match super::site::build_roadmap_html(root, schema, users, args.no_git, cache) {
            Ok((html, date)) => (Some(html), Some(date)),
            Err(_) => (None, None),
        }
    };

    let edit_url_prefix = super::site::detect_edit_url_prefix(root);

    let config = SiteConfig {
        title,
        roadmap: !args.no_roadmap && roadmap_html.is_some(),
        users: !args.no_users,
        roadmap_html,
        roadmap_generated_at,
        readme_html: super::site::render_readme_html(root),
        logo_path: None,
        edit_url_prefix,
        is_local_dev: false,
    };

    let count = site::generate_site(root, schema, users, &config, &output)
        .context("failed to generate site")?;

    eprintln!("Exported {count} pages to {}", output.display());
    Ok(())
}

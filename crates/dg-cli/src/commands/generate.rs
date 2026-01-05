use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use md_db::schema::Schema;

#[derive(Args)]
pub struct GenerateArgs {
    /// Show what would be generated without writing
    #[arg(long)]
    pub dry_run: bool,

    /// Path to README (default: README.md)
    #[arg(long, default_value = "README.md")]
    pub readme: PathBuf,
}

pub fn run(root: &Path, _schema: &Schema, args: &GenerateArgs) -> Result<()> {
    generate_readme(root, args)
}

fn generate_readme(root: &Path, args: &GenerateArgs) -> Result<()> {
    let readme_path = root.join(&args.readme);

    if !readme_path.exists() {
        anyhow::bail!("README not found: {}", readme_path.display());
    }

    // Discover and extract service metadata
    let services = md_db::generate::discover_and_extract_services(root)
        .context("Failed to discover services")?;

    if services.is_empty() {
        println!("No services found in services/ directory");
        return Ok(());
    }

    println!("Found {} service(s)", services.len());

    // Generate service catalog table
    let table = md_db::generate::generate_service_table(&services);

    // Read current README
    let readme_content = std::fs::read_to_string(&readme_path).context("Failed to read README")?;

    // Try to replace marker section
    match md_db::readme::replace_marker_section(&readme_content, "services", &table) {
        Ok(new_content) => {
            if args.dry_run {
                println!("\nGenerated service catalog (dry-run):");
                println!("{}", "=".repeat(60));
                println!("{}", table);
                println!("{}", "=".repeat(60));
            } else {
                std::fs::write(&readme_path, new_content)
                    .context("Failed to write updated README")?;
                println!("✓ Updated {}", readme_path.display());
                println!("\nService catalog:");
                println!("{}", table);
            }
        }
        Err(_) => {
            // Markers not found - suggest where to add them
            println!("⚠ Service catalog markers not found in README");
            println!("\nAdd the following markers to your README.md:");
            println!("\n<!-- dg:services:start -->");
            println!("<!-- dg:services:end -->\n");

            if let Some(suggestion) = md_db::readme::suggest_marker_location(&readme_content) {
                println!("Suggested location: {}", suggestion);
            }

            if args.dry_run {
                println!("\nGenerated service catalog (dry-run):");
                println!("{}", "=".repeat(60));
                println!("{}", table);
                println!("{}", "=".repeat(60));
            }
        }
    }

    Ok(())
}

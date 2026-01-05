//! `dg services` command — detect tech stacks across services and apps.

use std::path::Path;

use anyhow::{Context, Result};
use clap::Args;

#[derive(Args)]
pub struct ServicesArgs {
    /// Output format: text (default) or json
    #[arg(long, default_value = "text")]
    pub format: String,
}

pub fn run(root: &Path, args: &ServicesArgs) -> Result<()> {
    let service_readmes = md_db::service::discover_service_readmes(root)
        .context("Failed to discover service READMEs")?;
    let app_readmes = md_db::service::discover_app_readmes(root)
        .context("Failed to discover app READMEs")?;

    if service_readmes.is_empty() && app_readmes.is_empty() {
        if args.format == "json" {
            println!("{{\"services\":[],\"apps\":[]}}");
        } else {
            println!("No services or apps found in services/ or apps/ directories.");
        }
        return Ok(());
    }

    let mut services = Vec::new();
    let mut apps = Vec::new();

    for readme in &service_readmes {
        let service_dir = readme.parent().unwrap_or(readme.as_path());
        let name = service_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let tech = md_db::service::extract_tech_stack(service_dir);
        let practices = md_db::service::detect_engineering_practices(
            service_dir,
            &tech.primary_language,
            None,
        );
        let dev_cmds = md_db::service::extract_dev_commands(readme);
        services.push((name, tech, practices, dev_cmds));
    }

    for readme in &app_readmes {
        let app_dir = readme.parent().unwrap_or(readme.as_path());
        let name = app_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let tech = md_db::service::extract_tech_stack(app_dir);
        let practices = md_db::service::detect_engineering_practices(
            app_dir,
            &tech.primary_language,
            None,
        );
        let dev_cmds = md_db::service::extract_dev_commands(readme);
        apps.push((name, tech, practices, dev_cmds));
    }

    if args.format == "json" {
        print_json(&services, &apps)?;
    } else {
        print_text(&services, &apps);
    }

    Ok(())
}

type ServiceEntry = (
    String,
    md_db::service::TechStack,
    md_db::service::EngineeringPractices,
    md_db::service::DevCommands,
);

fn entry_to_json(name: &str, tech: &md_db::service::TechStack, practices: &md_db::service::EngineeringPractices, dev_cmds: &md_db::service::DevCommands) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "name": name,
        "primary_language": tech.primary_language,
        "language_version": tech.language_version,
        "languages": tech.languages.iter().map(|l| {
            serde_json::json!({"name": l.name, "percentage": l.percentage})
        }).collect::<Vec<_>>(),
        "frameworks": tech.frameworks,
        "framework_versions": tech.framework_versions.iter().map(|(f, v)| {
            serde_json::json!({"name": f, "version": v})
        }).collect::<Vec<_>>(),
        "database": tech.database,
        "deployment": tech.deployment.as_ref().map(|d| &d.platform),
        "has_linter": practices.has_linter,
        "linter_tool": practices.linter_tool,
        "has_tests": practices.has_tests,
        "test_framework": practices.test_framework,
    });
    if dev_cmds.has_any() {
        obj["dev_commands"] = serde_json::json!({
            "setup": dev_cmds.setup,
            "build": dev_cmds.build,
            "test": dev_cmds.test,
            "run": dev_cmds.run,
            "lint": dev_cmds.lint,
        });
    }
    obj
}

fn print_json(services: &[ServiceEntry], apps: &[ServiceEntry]) -> Result<()> {
    let output = serde_json::json!({
        "services": services.iter().map(|(name, tech, practices, dev_cmds)| {
            entry_to_json(name, tech, practices, dev_cmds)
        }).collect::<Vec<_>>(),
        "apps": apps.iter().map(|(name, tech, practices, dev_cmds)| {
            entry_to_json(name, tech, practices, dev_cmds)
        }).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn print_text(services: &[ServiceEntry], apps: &[ServiceEntry]) {
    if !services.is_empty() {
        println!("## Services\n");
        for (name, tech, practices, _) in services {
            print_entry(name, tech, practices);
        }
    }

    if !apps.is_empty() {
        if !services.is_empty() {
            println!();
        }
        println!("## Apps\n");
        for (name, tech, practices, _) in apps {
            print_entry(name, tech, practices);
        }
    }
}

fn print_entry(
    name: &str,
    tech: &md_db::service::TechStack,
    practices: &md_db::service::EngineeringPractices,
) {
    print!("  {} — {}", name, tech.primary_language);
    if let Some(ver) = &tech.language_version {
        print!(" {ver}");
    }
    if !tech.frameworks.is_empty() {
        let fw_strs: Vec<String> = tech
            .frameworks
            .iter()
            .map(|f| {
                if let Some((_, ver)) = tech.framework_versions.iter().find(|(n, _)| n == f) {
                    format!("{f} {ver}")
                } else {
                    f.clone()
                }
            })
            .collect();
        print!(" ({})", fw_strs.join(", "));
    }
    if let Some(db) = &tech.database {
        print!(" + {db}");
    }
    if let Some(deploy) = &tech.deployment {
        print!(" → {}", deploy.platform);
    }
    println!();

    // Print engineering practices line
    let linter_str = if practices.has_linter {
        format!(
            "{}",
            practices
                .linter_tool
                .as_deref()
                .unwrap_or("Linter")
        )
    } else {
        "No Linter".to_string()
    };
    let linter_icon = if practices.has_linter { "✓" } else { "✗" };

    let tests_str = if practices.has_tests {
        format!(
            "{}",
            practices
                .test_framework
                .as_deref()
                .unwrap_or("Tests")
        )
    } else {
        "No Tests".to_string()
    };
    let tests_icon = if practices.has_tests { "✓" } else { "✗" };

    println!("    Linter: {linter_str} {linter_icon} | Tests: {tests_str} {tests_icon}");
}

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use md_db::schema::{FieldType, Schema};
use md_db::users::{
    EntityKind, OrgConfig, OrgDef, TeamDef, TeamStatus, UserDef, UserStatus, ORG_CONFIG_FILENAME,
};

#[derive(Args)]
pub struct TeamArgs {
    #[command(subcommand)]
    pub command: TeamCommand,
}

#[derive(Subcommand)]
pub enum TeamCommand {
    /// Add a user to the team config
    AddUser {
        /// User handle (without @)
        handle: String,
        /// Display name
        #[arg(long)]
        name: Option<String>,
        /// Job title
        #[arg(long)]
        title: Option<String>,
        /// Email address
        #[arg(long)]
        email: Option<String>,
        /// Comma-separated team IDs
        #[arg(long, value_delimiter = ',')]
        teams: Vec<String>,
        /// Org ID
        #[arg(long)]
        org: Option<String>,
        /// Entity kind (internal or external)
        #[arg(long, default_value = "internal")]
        kind: String,
    },
    /// Mark a user as departed
    DepartUser {
        /// User handle (without @)
        handle: String,
    },
    /// Add a team to the config
    AddTeam {
        /// Team ID
        id: String,
        /// Display name
        #[arg(long)]
        name: Option<String>,
        /// Team lead handle (without @)
        #[arg(long)]
        lead: Option<String>,
        /// Parent team ID
        #[arg(long)]
        parent: Option<String>,
        /// Org ID
        #[arg(long)]
        org: Option<String>,
        /// Entity kind (internal or external)
        #[arg(long, default_value = "internal")]
        kind: String,
        /// Skip creating docs/teams/{id}.md
        #[arg(long)]
        no_doc: bool,
    },
    /// Add an org to the config
    AddOrg {
        /// Org ID
        id: String,
        /// Display name
        #[arg(long)]
        name: Option<String>,
        /// Parent org ID
        #[arg(long)]
        parent: Option<String>,
    },
    /// Deprecate a team
    DeprecateTeam {
        /// Team ID
        id: String,
    },
    /// List users, teams, and orgs
    List,
}

pub fn run(args: &TeamArgs, root: &Path, schema: &Schema, users: Option<OrgConfig>) -> Result<()> {
    match &args.command {
        TeamCommand::AddUser {
            handle,
            name,
            title,
            email,
            teams,
            org,
            kind,
        } => {
            let mut config = users.unwrap_or_else(empty_config);
            let kind: EntityKind = kind.parse().context("invalid --kind value")?;
            config.add_user(UserDef {
                handle: handle.clone(),
                name: name.clone(),
                title: title.clone(),
                email: email.clone(),
                teams: teams.clone(),
                org: org.clone(),
                status: UserStatus::Active,
                kind,
                extra: BTreeMap::new(),
            });
            save_config(&config, root)?;
            println!("Added user @{handle}");
            Ok(())
        }
        TeamCommand::DepartUser { handle } => {
            let mut config = users.context(format!(
                "no {ORG_CONFIG_FILENAME} found — nothing to depart"
            ))?;
            config
                .depart_user(handle)
                .with_context(|| format!("cannot depart user '{handle}'"))?;
            save_config(&config, root)?;
            println!("Marked @{handle} as departed");

            // Scan docs for references to departed user
            let affected = find_docs_referencing_user(root, schema, handle)?;
            if affected.is_empty() {
                println!("No documents reference @{handle}");
            } else {
                println!("\nDocuments referencing @{handle}:");
                for path in &affected {
                    println!("  {}", path.strip_prefix(root).unwrap_or(path).display());
                }
                println!("\nRun `dg validate` to see U012 warnings for these files.");
            }
            Ok(())
        }
        TeamCommand::AddTeam {
            id,
            name,
            lead,
            parent,
            org,
            kind,
            no_doc,
        } => {
            let mut config = users.unwrap_or_else(empty_config);
            let kind: EntityKind = kind.parse().context("invalid --kind value")?;
            let display_name = name.clone().unwrap_or_else(|| id.clone());
            config.add_team(TeamDef {
                id: id.clone(),
                name: name.clone(),
                lead: lead.clone(),
                parent: parent.clone(),
                teams: vec![],
                org: org.clone(),
                status: TeamStatus::Active,
                kind,
                extra: BTreeMap::new(),
            });
            save_config(&config, root)?;
            println!("Added team @team/{id}");

            if !no_doc {
                create_team_doc(root, id, &display_name)?;
            }
            Ok(())
        }
        TeamCommand::AddOrg { id, name, parent } => {
            let mut config = users.unwrap_or_else(empty_config);
            config.add_org(OrgDef {
                id: id.clone(),
                name: name.clone(),
                parent: parent.clone(),
                primary: false,
                extra: BTreeMap::new(),
            });
            save_config(&config, root)?;
            println!("Added org @org/{id}");
            Ok(())
        }
        TeamCommand::DeprecateTeam { id } => {
            let mut config = users.context(format!(
                "no {ORG_CONFIG_FILENAME} found — nothing to deprecate"
            ))?;
            config
                .deprecate_team(id)
                .with_context(|| format!("cannot deprecate team '{id}'"))?;
            save_config(&config, root)?;
            println!("Deprecated team @team/{id}");
            Ok(())
        }
        TeamCommand::List => {
            let config = users
                .as_ref()
                .context(format!("no {ORG_CONFIG_FILENAME} found"))?;
            print_list(config);
            Ok(())
        }
    }
}

fn create_team_doc(root: &Path, id: &str, display_name: &str) -> Result<()> {
    let dir = root.join("docs/teams");
    std::fs::create_dir_all(&dir).context("failed to create docs/teams/")?;
    let path = dir.join(format!("{id}.md"));
    if path.exists() {
        println!(
            "Team doc already exists: {}",
            path.strip_prefix(root).unwrap_or(&path).display()
        );
        return Ok(());
    }
    let content = format!(
        "{display_name} — describe what this team owns and its core responsibilities.\n\
         \n\
         ## Charter\n\
         <!-- Why the team exists, what it owns, responsibility boundaries -->\n\
         \n\
         ## Communication\n\
         | Channel | Purpose |\n\
         |---------|---------|\n\
         | #team-{id} | General discussion |\n\
         | #team-{id}-alerts | Monitoring alerts |\n\
         \n\
         ## On-Call\n\
         <!-- Rotation schedule, escalation path, runbook links -->\n\
         \n\
         ## Getting Started\n\
         <!-- Machine setup, repo access, first-week checklist -->\n\
         \n\
         ## Processes\n\
         <!-- PR review, deploy procedures, RFC workflow -->\n\
         \n\
         ## Key Contacts\n\
         <!-- External stakeholders, partner teams, escalation paths -->\n"
    );
    std::fs::write(&path, &content)
        .with_context(|| format!("failed to write {}", path.display()))?;
    println!(
        "Created team doc: {}",
        path.strip_prefix(root).unwrap_or(&path).display()
    );
    Ok(())
}

fn empty_config() -> OrgConfig {
    OrgConfig {
        users: Default::default(),
        teams: Default::default(),
        orgs: Default::default(),
        jira: Default::default(),
    }
}

fn save_config(config: &OrgConfig, root: &Path) -> Result<()> {
    let path = root.join(".dg").join(ORG_CONFIG_FILENAME);
    config
        .save(&path)
        .with_context(|| format!("failed to write {}", path.display()))
}

/// Scan all markdown docs and return paths that reference the given user handle.
fn find_docs_referencing_user(
    root: &Path,
    schema: &Schema,
    handle: &str,
) -> Result<Vec<std::path::PathBuf>> {
    let files = md_db::discovery::discover_files(root, None, &[], false)?;
    let mut affected = Vec::new();

    // Collect all user-type field names from schema
    let user_fields: Vec<String> = schema
        .types
        .iter()
        .flat_map(|td| {
            td.fields
                .iter()
                .filter(|f| matches!(f.field_type, FieldType::User | FieldType::UserArray))
                .map(|f| f.name.clone())
        })
        .collect();

    if user_fields.is_empty() {
        return Ok(affected);
    }

    for path in &files {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let (fm, _) = match md_db::frontmatter::Frontmatter::try_parse(&content) {
            Ok(pair) => pair,
            Err(_) => continue,
        };
        let Some(fm) = fm else { continue };

        for field in &user_fields {
            if let Some(val) = fm.get_display(field) {
                let normalized = val.trim();
                let bare = normalized.strip_prefix('@').unwrap_or(normalized);
                if bare == handle {
                    affected.push(path.clone());
                    break;
                }
            }
        }
    }

    affected.sort();
    Ok(affected)
}

fn print_list(config: &OrgConfig) {
    // Orgs
    if !config.orgs.is_empty() {
        println!("Orgs:");
        let mut ids: Vec<&String> = config.orgs.keys().collect();
        ids.sort();
        for id in ids {
            let org = &config.orgs[id];
            let name = org.name.as_deref().unwrap_or("");
            let parent = org
                .parent
                .as_ref()
                .map(|p| format!(" (parent: {p})"))
                .unwrap_or_default();
            println!("  @org/{id:<16} {name}{parent}");
        }
        println!();
    }

    // Teams
    if !config.teams.is_empty() {
        println!("Teams:");
        let mut ids: Vec<&String> = config.teams.keys().collect();
        ids.sort();
        for id in ids {
            let team = &config.teams[id];
            let name = team.name.as_deref().unwrap_or("");
            let mut meta = Vec::new();
            if let Some(ref lead) = team.lead {
                meta.push(format!("lead: @{lead}"));
            }
            if let Some(ref parent) = team.parent {
                meta.push(format!("parent: {parent}"));
            }
            if team.status == TeamStatus::Deprecated {
                meta.push("deprecated".to_string());
            }
            if team.kind == EntityKind::External {
                meta.push("external".to_string());
            }
            let suffix = if meta.is_empty() {
                String::new()
            } else {
                format!(" ({})", meta.join(", "))
            };
            println!("  @team/{id:<14} {name}{suffix}");
        }
        println!();
    }

    // Users
    if !config.users.is_empty() {
        println!("Users:");
        let mut handles: Vec<&String> = config.users.keys().collect();
        handles.sort();
        for handle in handles {
            let user = &config.users[handle];
            let name = user.name.as_deref().unwrap_or("");
            let mut meta = Vec::new();
            if let Some(ref title) = user.title {
                meta.push(title.as_str().to_string());
            }
            if user.kind == EntityKind::External {
                meta.push("external".to_string());
            }
            match user.status {
                UserStatus::Active => {}
                UserStatus::Departed => meta.push("departed".to_string()),
            }
            let suffix = if meta.is_empty() {
                String::new()
            } else {
                format!(" ({})", meta.join(", "))
            };
            println!("  @{handle:<18} {name}{suffix}");
        }
    }

    if config.orgs.is_empty() && config.teams.is_empty() && config.users.is_empty() {
        println!("No users, teams, or orgs configured.");
    }
}

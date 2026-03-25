use std::collections::BTreeMap;

use crate::document::Document;
use crate::schema::Schema;
use crate::users::OrgConfig;

use super::SiteConfig;

/// A node in the navigation tree.
#[derive(Debug, Clone)]
pub struct NavItem {
    /// Display label in sidebar.
    pub label: String,
    /// Relative path to the page (e.g. "architecture/index.html").
    pub href: Option<String>,
    /// Child items (collapsible section).
    pub children: Vec<NavItem>,
}

/// The full navigation tree for the sidebar.
pub type NavTree = Vec<NavItem>;

/// Lightweight service info for nav tree building.
pub struct NavService {
    pub slug: String,
    pub name: String,
    pub kind: String,
    pub status: String,
}

/// Build the nav tree from discovered docs and org config.
pub fn build_nav_tree(
    by_type: &BTreeMap<String, Vec<(String, &Document)>>,
    org: Option<&OrgConfig>,
    config: &SiteConfig,
    services: &[NavService],
    schema: &Schema,
) -> NavTree {
    let mut tree = Vec::new();

    // Introduction (empty href → root "/" in SPA)
    tree.push(NavItem {
        label: "Introduction".to_string(),
        href: Some(String::new()),
        children: vec![],
    });

    // Getting Started
    tree.push(NavItem {
        label: "Getting Started".to_string(),
        href: Some("onboarding/".to_string()),
        children: vec![],
    });

    // Documents group — flat list of doc types (individual docs on listing pages)
    let type_sections = schema.nav_types();

    let mut doc_children: Vec<NavItem> = Vec::new();
    for (type_key, dir_name, display_name) in &type_sections {
        let count = by_type.get(*type_key).map(|d| d.len()).unwrap_or(0);
        if count == 0 {
            continue;
        }
        doc_children.push(NavItem {
            label: format!("{display_name} ({count})"),
            href: Some(format!("{dir_name}/")),
            children: vec![],
        });
    }

    if !doc_children.is_empty() {
        tree.push(NavItem {
            label: "Documents".to_string(),
            href: None,
            children: doc_children,
        });
    }

    // Software section (services, apps, infra)
    if !services.is_empty() {
        let mut sw_children = Vec::new();

        for &(kind, dir, display) in &[
            ("service", "services", "Services"),
            ("app", "apps", "Apps"),
            ("infra", "infra", "Infra"),
        ] {
            let items: Vec<&NavService> = services.iter().filter(|s| s.kind == kind).collect();
            if items.is_empty() {
                continue;
            }
            let mut children: Vec<NavItem> = items
                .iter()
                .map(|s| {
                    let label = match s.status.as_str() {
                        "deprecated" | "superseded" | "rejected" | "declined" | "sunset" => {
                            format!("deprecated:{}", s.name)
                        }
                        _ => s.name.clone(),
                    };
                    NavItem {
                        label,
                        href: Some(format!("software/{dir}/{}/", s.slug)),
                        children: vec![],
                    }
                })
                .collect();
            children.sort_by(|a, b| a.label.cmp(&b.label));

            sw_children.push(NavItem {
                label: format!("{display} ({count})", count = items.len()),
                href: Some(format!("software/{dir}/")),
                children,
            });
        }

        if !sw_children.is_empty() {
            tree.push(NavItem {
                label: "Software".to_string(),
                href: None,
                children: sw_children,
            });
        }
    }

    // Organization section
    if let Some(org_config) = org {
        let mut org_children = Vec::new();

        // Teams
        if !org_config.teams.is_empty() {
            let mut team_children: Vec<NavItem> = org_config
                .teams
                .iter()
                .map(|(id, team)| {
                    let name = team.name.as_deref().unwrap_or(id);
                    let label = if team.status == crate::users::TeamStatus::Deprecated {
                        format!("deprecated:{name}")
                    } else {
                        name.to_string()
                    };
                    NavItem {
                        label,
                        href: Some(format!("org/teams/{id}/")),
                        children: vec![],
                    }
                })
                .collect();
            team_children.sort_by(|a, b| a.label.cmp(&b.label));

            let count = team_children.len();
            org_children.push(NavItem {
                label: format!("Teams ({count})"),
                href: Some("org/teams/".to_string()),
                children: team_children,
            });
        }

        // People
        if config.users && !org_config.users.is_empty() {
            let mut user_children: Vec<NavItem> = org_config
                .users
                .iter()
                .filter(|(_, user)| {
                    !matches!(user.status, crate::users::UserStatus::Departed)
                        && !matches!(user.kind, crate::users::EntityKind::Ai)
                })
                .map(|(handle, user)| {
                    let name = user.name.as_deref().unwrap_or(handle);
                    NavItem {
                        label: name.to_string(),
                        href: Some(format!("org/users/{handle}/")),
                        children: vec![],
                    }
                })
                .collect();
            user_children.sort_by(|a, b| a.label.cmp(&b.label));

            let count = user_children.len();
            org_children.push(NavItem {
                label: format!("People ({count})"),
                href: Some("org/users/".to_string()),
                children: user_children,
            });
        }

        // Entities (legal entities / subsidiaries)
        if !org_config.orgs.is_empty() {
            let mut entity_children: Vec<NavItem> = org_config
                .orgs
                .iter()
                .map(|(id, org_def)| {
                    let name = org_def.name.as_deref().unwrap_or(id);
                    NavItem {
                        label: name.to_string(),
                        href: Some(format!("org/{id}/")),
                        children: vec![],
                    }
                })
                .collect();
            entity_children.sort_by(|a, b| a.label.cmp(&b.label));

            let count = entity_children.len();
            org_children.push(NavItem {
                label: format!("Entities ({count})"),
                href: Some("org/entities/".to_string()),
                children: entity_children,
            });
        }

        if !org_children.is_empty() {
            tree.push(NavItem {
                label: "Organization".to_string(),
                href: None,
                children: org_children,
            });
        }
    }

    // Graph
    tree.push(NavItem {
        label: "Graph".to_string(),
        href: Some("graph/".to_string()),
        children: vec![],
    });

    // Kanban
    tree.push(NavItem {
        label: "Kanban".to_string(),
        href: Some("kanban/".to_string()),
        children: vec![],
    });

    // Roadmap
    if config.roadmap {
        tree.push(NavItem {
            label: "Roadmap".to_string(),
            href: Some("roadmap/".to_string()),
            children: vec![],
        });
    }

    tree
}

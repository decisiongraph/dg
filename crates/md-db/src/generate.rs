use std::path::Path;

use crate::error::Result;
use crate::service::ServiceMetadata;

/// Generate a markdown table for the service catalog.
pub fn generate_service_table(services: &[ServiceMetadata]) -> String {
    if services.is_empty() {
        return String::new();
    }

    let mut table = String::new();
    table.push_str("| Service | Status | Owner | Stack | Description |\n");
    table.push_str("|---------|--------|-------|-------|-------------|\n");

    // Sort services alphabetically by name
    let mut sorted = services.to_vec();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    for service in sorted {
        let service_link = format!("[{}]({})", service.name, service.readme_path);
        let description = if service.description.len() > 80 {
            format!("{}...", &service.description[..77])
        } else {
            service.description.clone()
        };

        table.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            service_link,
            service.status,
            service.owner,
            service.tech_stack.render_table_cell(),
            description
        ));
    }

    table
}

/// Discover services and generate metadata for all service READMEs.
pub fn discover_and_extract_services(root: &Path) -> Result<Vec<ServiceMetadata>> {
    let readme_paths = crate::service::discover_service_readmes(root)?;
    let mut services = Vec::new();

    for path in readme_paths {
        match crate::service::extract_service_metadata(&path, root) {
            Ok(metadata) => services.push(metadata),
            Err(e) => {
                eprintln!(
                    "Warning: failed to extract metadata from {}: {}",
                    path.display(),
                    e
                );
            }
        }
    }

    Ok(services)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_empty_table() {
        let table = generate_service_table(&[]);
        assert_eq!(table, "");
    }

    #[test]
    fn test_generate_service_table() {
        use crate::service::TechStack;

        let services = vec![
            ServiceMetadata {
                name: "Core".to_string(),
                status: "Live".to_string(),
                owner: "@jacek".to_string(),
                tech_stack: TechStack::from_simple_string("Ruby"),
                description: "Admin backend, account management".to_string(),
                readme_path: "services/core/README.md".to_string(),
                created_at: None,
                commit_count: None,
                last_commit_at: None,
                practices: Default::default(),
            },
            ServiceMetadata {
                name: "API".to_string(),
                status: "Beta".to_string(),
                owner: "@alice".to_string(),
                tech_stack: TechStack::from_simple_string("JavaScript"),
                description: "Public API service".to_string(),
                readme_path: "services/api/README.md".to_string(),
                created_at: None,
                commit_count: None,
                last_commit_at: None,
                practices: Default::default(),
            },
        ];

        let table = generate_service_table(&services);

        // Should be sorted alphabetically
        assert!(table.contains("[API](services/api/README.md)"));
        assert!(table.contains("[Core](services/core/README.md)"));
        assert!(table.contains("| Service | Status | Owner | Stack | Description |"));
    }
}

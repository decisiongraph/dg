//! DecisionGraph configuration file (.dg/config.toml) parsing.
//!
//! Handles commit hook settings, validation modes, and other project-level config.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Full DecisionGraph configuration from .dg/config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[derive(Default)]
pub struct Config {
    /// Commit hook settings
    #[serde(default)]
    pub commit_hooks: CommitHooksConfig,
}

/// Commit hook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CommitHooksConfig {
    /// Enable or disable all commit hooks
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Local validation mode: "warn" (default), "off"
    #[serde(default = "default_warn_mode")]
    pub local_mode: String,

    /// Commit types that should have document references (warned if missing)
    #[serde(default = "default_recommend_refs_for")]
    pub recommend_refs_for: Vec<String>,

    /// Auto-suggest document IDs from staged files
    #[serde(default = "default_true")]
    pub auto_suggest: bool,

    /// Validate that referenced documents exist
    #[serde(default = "default_true")]
    pub check_existence: bool,

    /// Skip validation for WIP commits
    #[serde(default = "default_true")]
    pub skip_wip: bool,

    /// Skip validation for merge commits
    #[serde(default = "default_true")]
    pub skip_merge: bool,

    /// Skip validation for revert commits
    #[serde(default = "default_true")]
    pub skip_revert: bool,
}

impl Default for CommitHooksConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            local_mode: "warn".to_string(),
            recommend_refs_for: vec![
                "feat".to_string(),
                "fix".to_string(),
                "refactor".to_string(),
            ],
            auto_suggest: true,
            check_existence: true,
            skip_wip: true,
            skip_merge: true,
            skip_revert: true,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_warn_mode() -> String {
    "warn".to_string()
}

fn default_recommend_refs_for() -> Vec<String> {
    vec![
        "feat".to_string(),
        "fix".to_string(),
        "refactor".to_string(),
    ]
}

impl Config {
    /// Load configuration from .dg/config.toml, or return defaults if not found.
    pub fn load(dg_root: &Path) -> Self {
        let config_path = dg_root.join("config.toml");
        if !config_path.exists() {
            return Config::default();
        }

        match std::fs::read_to_string(&config_path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse config.toml: {}. Using defaults.",
                        e
                    );
                    Config::default()
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: Failed to read config.toml: {}. Using defaults.",
                    e
                );
                Config::default()
            }
        }
    }

    /// Generate default config.toml content as a string.
    pub fn default_toml() -> String {
        let config = Config::default();
        toml::to_string_pretty(&config)
            .unwrap_or_else(|_| String::from("# Error generating config"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.commit_hooks.enabled);
        assert_eq!(config.commit_hooks.local_mode, "warn");
        assert_eq!(config.commit_hooks.recommend_refs_for.len(), 3);
        assert!(config.commit_hooks.auto_suggest);
    }

    #[test]
    fn test_parse_config_toml() {
        let toml_str = r#"
[commit-hooks]
enabled = true
local-mode = "warn"
recommend-refs-for = ["feat", "fix"]
auto-suggest = false
"#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.commit_hooks.enabled);
        assert_eq!(config.commit_hooks.local_mode, "warn");
        assert_eq!(config.commit_hooks.recommend_refs_for, vec!["feat", "fix"]);
        assert!(!config.commit_hooks.auto_suggest);
    }

    #[test]
    fn test_default_toml_generation() {
        let toml_str = Config::default_toml();
        assert!(toml_str.contains("commit-hooks"));
        assert!(toml_str.contains("enabled"));
        assert!(toml_str.contains("local-mode"));
    }

    #[test]
    fn test_load_nonexistent_config() {
        let tmp = std::env::temp_dir().join("dg_config_test_nonexistent");
        let config = Config::load(&tmp);
        assert!(config.commit_hooks.enabled);
        assert_eq!(config.commit_hooks.local_mode, "warn");
    }

    #[test]
    fn test_load_existing_config() {
        let tmp = std::env::temp_dir().join("dg_config_test_existing");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let toml_str = r#"
[commit-hooks]
enabled = false
local-mode = "off"
"#;
        std::fs::write(tmp.join("config.toml"), toml_str).unwrap();

        let config = Config::load(&tmp);
        assert!(!config.commit_hooks.enabled);
        assert_eq!(config.commit_hooks.local_mode, "off");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}

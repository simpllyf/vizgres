//! Application settings
//!
//! Manages general configuration stored in ~/.vizgres/config.toml.
//! Settings include preview row limits, max tabs, history size,
//! and keybinding overrides.

use crate::error::ConfigResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Top-level configuration file structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub settings: SettingsInner,
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
}

/// General application settings with serde defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsInner {
    #[serde(default = "default_preview_rows")]
    pub preview_rows: usize,
    #[serde(default = "default_max_tabs")]
    pub max_tabs: usize,
    #[serde(default = "default_history_size")]
    pub history_size: usize,
    #[serde(default = "default_query_timeout_ms")]
    pub query_timeout_ms: u64,
    #[serde(default = "default_max_result_rows")]
    pub max_result_rows: usize,
    #[serde(default = "default_tree_category_limit")]
    pub tree_category_limit: usize,
    /// Server-side statement timeout in milliseconds (0 = disabled).
    /// This uses PostgreSQL's statement_timeout setting to kill queries
    /// at the server level, providing a safety net even if the client crashes.
    #[serde(default = "default_statement_timeout_ms")]
    pub statement_timeout_ms: u64,
    /// Whether to prompt for confirmation before executing destructive queries
    /// (DROP, TRUNCATE, DELETE without WHERE). Default: true.
    #[serde(default = "default_confirm_destructive")]
    pub confirm_destructive: bool,
    /// Default read-only mode for all connections. Per-connection `read_only`
    /// in connections.toml overrides this. Default: false.
    #[serde(default)]
    pub read_only: bool,
    /// Show EXPLAIN output as a visual tree with color-coded timing.
    /// When false, shows raw text output like psql. Default: true.
    #[serde(default = "default_explain_visual")]
    pub explain_visual: bool,
    /// Color theme. Options: dark, light, midnight, ember. Default: dark.
    #[serde(default = "default_theme")]
    pub theme: String,
}

/// Keybinding overrides organized by panel context
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeybindingsConfig {
    #[serde(default)]
    pub global: HashMap<String, String>,
    #[serde(default)]
    pub editor: HashMap<String, String>,
    #[serde(default)]
    pub results: HashMap<String, String>,
    #[serde(default)]
    pub tree: HashMap<String, String>,
}

fn default_preview_rows() -> usize {
    100
}

fn default_max_tabs() -> usize {
    5
}

fn default_history_size() -> usize {
    500
}

fn default_query_timeout_ms() -> u64 {
    30000 // 30 seconds, 0 = disabled
}

fn default_max_result_rows() -> usize {
    1000 // 0 = unlimited
}

fn default_tree_category_limit() -> usize {
    500 // Items per category before pagination, 0 = unlimited
}

fn default_statement_timeout_ms() -> u64 {
    60000 // 60 seconds server-side timeout, 0 = disabled
}

fn default_confirm_destructive() -> bool {
    true
}

fn default_explain_visual() -> bool {
    true
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for SettingsInner {
    fn default() -> Self {
        Self {
            preview_rows: default_preview_rows(),
            max_tabs: default_max_tabs(),
            history_size: default_history_size(),
            query_timeout_ms: default_query_timeout_ms(),
            max_result_rows: default_max_result_rows(),
            tree_category_limit: default_tree_category_limit(),
            statement_timeout_ms: default_statement_timeout_ms(),
            confirm_destructive: default_confirm_destructive(),
            read_only: false,
            explain_visual: default_explain_visual(),
            theme: default_theme(),
        }
    }
}

impl Settings {
    /// Load settings from ~/.vizgres/config.toml.
    /// Returns defaults if the file is missing. Prints a warning to stderr
    /// and returns defaults if the file exists but fails to parse.
    pub fn load() -> Self {
        let path = match Self::config_file() {
            Ok(p) => p,
            Err(e) => {
                eprintln!(
                    "Warning: could not determine config path: {}. Using defaults.",
                    e
                );
                return Self::default();
            }
        };

        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(settings) => settings,
                Err(e) => {
                    eprintln!(
                        "Warning: failed to parse {}: {}. Using defaults.",
                        path.display(),
                        e
                    );
                    Self::default()
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: failed to read {}: {}. Using defaults.",
                    path.display(),
                    e
                );
                Self::default()
            }
        }
    }

    /// Get the config file path (~/.vizgres/config.toml)
    pub fn config_file() -> ConfigResult<PathBuf> {
        Ok(super::connections::ConnectionConfig::config_dir()?.join("config.toml"))
    }

    /// Write a default config template with commented-out settings.
    /// Creates the parent directory if needed.
    pub fn write_defaults(path: &std::path::Path) -> ConfigResult<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, DEFAULT_CONFIG_TEMPLATE)?;
        Ok(())
    }
}

/// Commented-out default template for `config edit`
const DEFAULT_CONFIG_TEMPLATE: &str = r#"# vizgres configuration
# https://github.com/simpllyf/vizgres

[settings]
# preview_rows = 100
# max_tabs = 5
# history_size = 500
# query_timeout_ms = 30000  # 30 seconds client-side timeout, 0 = disabled
# max_result_rows = 1000    # row limit for query results, 0 = unlimited
# tree_category_limit = 500 # items per category before pagination, 0 = unlimited
# statement_timeout_ms = 60000  # 60 seconds server-side timeout, 0 = disabled
# confirm_destructive = true    # prompt before DROP, TRUNCATE, DELETE without WHERE
# read_only = false             # default read-only mode for all connections
# explain_visual = true         # visual tree for EXPLAIN, false = raw text
# theme = "dark"                # color theme: dark, light, midnight, ember

[keybindings.global]
# "ctrl+q" = "quit"
# "ctrl+p" = "command_bar"
# "f1" = "show_help"
# "tab" = "cycle_focus"
# "shift+tab" = "cycle_focus_reverse"
# "ctrl+t" = "new_tab"
# "ctrl+w" = "close_tab"
# "ctrl+n" = "next_tab"

[keybindings.editor]
# "f5" = "execute_query"
# "ctrl+enter" = "execute_query"
# "ctrl+e" = "explain_query"
# "ctrl+l" = "clear_editor"
# "ctrl+z" = "undo"
# "ctrl+shift+z" = "redo"
# "ctrl+alt+f" = "format_query"
# "ctrl+up" = "history_back"
# "ctrl+down" = "history_forward"
# "esc" = "cancel_query"

[keybindings.results]
# "enter" = "open_inspector"
# "v" = "toggle_view_mode"
# "y" = "copy_cell"
# "shift+y" = "copy_row"
# "ctrl+s" = "export_csv"
# "ctrl+j" = "export_json"
# "n" = "next_page"
# "p" = "prev_page"
# "esc" = "cancel_query"

[keybindings.tree]
# "enter" = "expand"
# "space" = "toggle_expand"
# "h" = "collapse"
# "x" = "delete_saved_query"
# "esc" = "cancel_query"
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let settings = Settings::default();
        assert_eq!(settings.settings.preview_rows, 100);
        assert_eq!(settings.settings.max_tabs, 5);
        assert_eq!(settings.settings.history_size, 500);
        assert_eq!(settings.settings.query_timeout_ms, 30000);
        assert_eq!(settings.settings.max_result_rows, 1000);
        assert_eq!(settings.settings.tree_category_limit, 500);
        assert_eq!(settings.settings.statement_timeout_ms, 60000);
        assert!(settings.settings.confirm_destructive);
        assert!(settings.settings.explain_visual);
        assert_eq!(settings.settings.theme, "dark");
        assert!(settings.keybindings.global.is_empty());
        assert!(settings.keybindings.editor.is_empty());
        assert!(settings.keybindings.results.is_empty());
        assert!(settings.keybindings.tree.is_empty());
    }

    #[test]
    fn test_partial_toml_fills_defaults() {
        let toml_str = r#"
[settings]
preview_rows = 50
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.settings.preview_rows, 50);
        assert_eq!(settings.settings.max_tabs, 5); // default
        assert_eq!(settings.settings.history_size, 500); // default
    }

    #[test]
    fn test_empty_file_returns_defaults() {
        let settings: Settings = toml::from_str("").unwrap();
        assert_eq!(settings.settings.preview_rows, 100);
        assert_eq!(settings.settings.max_tabs, 5);
        assert_eq!(settings.settings.history_size, 500);
        assert_eq!(settings.settings.query_timeout_ms, 30000);
        assert_eq!(settings.settings.max_result_rows, 1000);
        assert_eq!(settings.settings.tree_category_limit, 500);
        assert_eq!(settings.settings.statement_timeout_ms, 60000);
    }

    #[test]
    fn test_keybinding_overrides_deserialize() {
        let toml_str = r#"
[keybindings.editor]
"f6" = "execute_query"
"ctrl+enter" = "execute_query"
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.keybindings.editor.len(), 2);
        assert_eq!(
            settings.keybindings.editor.get("f6"),
            Some(&"execute_query".to_string())
        );
    }

    #[test]
    fn test_full_config_roundtrip() {
        let toml_str = r#"
[settings]
preview_rows = 200
max_tabs = 3
history_size = 1000

[keybindings.global]
"ctrl+q" = "quit"

[keybindings.editor]
"f5" = "execute_query"
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.settings.preview_rows, 200);
        assert_eq!(settings.settings.max_tabs, 3);
        assert_eq!(settings.settings.history_size, 1000);
        assert_eq!(settings.keybindings.global.len(), 1);
        assert_eq!(settings.keybindings.editor.len(), 1);
    }

    #[test]
    fn test_default_config_template_is_valid_toml() {
        // The template with comments should be parseable (comments are ignored)
        let result: Result<Settings, _> = toml::from_str(DEFAULT_CONFIG_TEMPLATE);
        assert!(result.is_ok(), "Template should be valid TOML");
    }

    #[test]
    fn test_custom_query_timeout() {
        let toml_str = r#"
[settings]
query_timeout_ms = 5000
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.settings.query_timeout_ms, 5000);
    }

    #[test]
    fn test_zero_query_timeout_disables() {
        let toml_str = r#"
[settings]
query_timeout_ms = 0
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.settings.query_timeout_ms, 0);
    }

    #[test]
    fn test_custom_max_result_rows() {
        let toml_str = r#"
[settings]
max_result_rows = 5000
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.settings.max_result_rows, 5000);
    }

    #[test]
    fn test_zero_max_result_rows_unlimited() {
        let toml_str = r#"
[settings]
max_result_rows = 0
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.settings.max_result_rows, 0);
    }

    #[test]
    fn test_custom_tree_category_limit() {
        let toml_str = r#"
[settings]
tree_category_limit = 200
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.settings.tree_category_limit, 200);
    }

    #[test]
    fn test_zero_tree_category_limit_unlimited() {
        let toml_str = r#"
[settings]
tree_category_limit = 0
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.settings.tree_category_limit, 0);
    }

    #[test]
    fn test_custom_statement_timeout() {
        let toml_str = r#"
[settings]
statement_timeout_ms = 120000
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.settings.statement_timeout_ms, 120000);
    }

    #[test]
    fn test_zero_statement_timeout_disables() {
        let toml_str = r#"
[settings]
statement_timeout_ms = 0
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.settings.statement_timeout_ms, 0);
    }

    #[test]
    fn test_confirm_destructive_default_true() {
        let settings: Settings = toml::from_str("").unwrap();
        assert!(settings.settings.confirm_destructive);
    }

    #[test]
    fn test_confirm_destructive_can_disable() {
        let toml_str = r#"
[settings]
confirm_destructive = false
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert!(!settings.settings.confirm_destructive);
    }

    #[test]
    fn test_read_only_defaults_false() {
        let settings: Settings = toml::from_str("").unwrap();
        assert!(!settings.settings.read_only);
    }

    #[test]
    fn test_read_only_can_enable() {
        let toml_str = r#"
[settings]
read_only = true
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert!(settings.settings.read_only);
    }
}

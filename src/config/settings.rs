//! User settings and preferences
//!
//! Manages application settings stored in ~/.vizgres/config.toml

use crate::config::ConnectionConfig;
use crate::error::ConfigResult;
use serde::{Deserialize, Serialize};

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Query history size limit
    #[serde(default = "default_history_size")]
    pub history_size: usize,

    /// Auto-save query history
    #[serde(default = "default_true")]
    pub save_history: bool,

    /// Default query row limit
    #[serde(default = "default_row_limit")]
    pub default_row_limit: usize,

    /// Enable syntax highlighting
    #[serde(default = "default_true")]
    pub syntax_highlighting: bool,

    /// Auto-format SQL on execute
    #[serde(default)]
    pub auto_format: bool,

    /// Theme name
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_history_size() -> usize {
    1000
}

fn default_row_limit() -> usize {
    1000
}

fn default_true() -> bool {
    true
}

fn default_theme() -> String {
    "default".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            history_size: default_history_size(),
            save_history: default_true(),
            default_row_limit: default_row_limit(),
            syntax_highlighting: default_true(),
            auto_format: false,
            theme: default_theme(),
        }
    }
}

/// Load settings from config file
pub fn load_settings() -> ConfigResult<Settings> {
    let path = ConnectionConfig::config_dir()?.join("config.toml");

    if !path.exists() {
        return Ok(Settings::default());
    }

    let content = std::fs::read_to_string(&path)?;
    let settings: Settings = toml::from_str(&content)?;
    Ok(settings)
}

/// Save settings to config file
pub fn save_settings(_settings: &Settings) -> ConfigResult<()> {
    // TODO: Phase 6 - Implement settings saving
    todo!("Saving settings not yet implemented")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_defaults() {
        let settings = Settings::default();
        assert_eq!(settings.history_size, 1000);
        assert_eq!(settings.default_row_limit, 1000);
        assert!(settings.save_history);
        assert!(settings.syntax_highlighting);
        assert!(!settings.auto_format);
    }
}

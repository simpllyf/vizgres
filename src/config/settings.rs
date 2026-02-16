//! User settings and preferences
//!
//! Manages application settings stored in ~/.vizgres/config.toml

use crate::config::ConnectionConfig;
use crate::error::ConfigResult;
use serde::{Deserialize, Serialize};

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_history_size")]
    pub history_size: usize,

    #[serde(default = "default_true")]
    pub save_history: bool,

    #[serde(default = "default_row_limit")]
    pub default_row_limit: usize,

    #[serde(default = "default_true")]
    pub syntax_highlighting: bool,

    #[serde(default)]
    pub auto_format: bool,

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

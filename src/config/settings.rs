//! User settings and preferences
//!
//! Manages application settings stored in ~/.vizgres/config.toml

use crate::config::ConnectionConfig;
use crate::error::ConfigResult;
use serde::{Deserialize, Serialize};

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_row_limit")]
    pub default_row_limit: usize,

    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_row_limit() -> usize {
    1000
}

fn default_theme() -> String {
    "default".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_row_limit: default_row_limit(),
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

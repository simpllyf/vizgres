//! Connection configuration
//!
//! Manages database connection profiles stored in ~/.vizgres/connections.toml

use crate::error::{ConfigError, ConfigResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Database connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Connection profile name
    pub name: String,

    /// Database host
    pub host: String,

    /// Database port
    #[serde(default = "default_port")]
    pub port: u16,

    /// Database name
    pub database: String,

    /// Username
    pub username: String,

    /// Password (not serialized to file - use keychain instead)
    #[serde(skip_serializing)]
    pub password: Option<String>,

    /// SSL mode
    #[serde(default)]
    pub ssl_mode: SslMode,
}

/// SSL connection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SslMode {
    Disable,
    Prefer,
    Require,
}

/// Container for multiple connections in TOML file
#[derive(Debug, Serialize, Deserialize)]
struct ConnectionsFile {
    #[serde(default)]
    connections: Vec<ConnectionConfig>,
}

fn default_port() -> u16 {
    5432
}

impl Default for SslMode {
    fn default() -> Self {
        SslMode::Prefer
    }
}

impl ConnectionConfig {
    /// Build a PostgreSQL connection string
    pub fn connection_string(&self) -> String {
        format!(
            "host={} port={} dbname={} user={}",
            self.host, self.port, self.database, self.username
        )
    }

    /// Get the config directory path (~/.vizgres/)
    pub fn config_dir() -> ConfigResult<PathBuf> {
        let home = dirs::home_dir().ok_or(ConfigError::NoHomeDir)?;
        Ok(home.join(".vizgres"))
    }

    /// Get the connections file path
    pub fn connections_file() -> ConfigResult<PathBuf> {
        Ok(Self::config_dir()?.join("connections.toml"))
    }
}

/// Load all connection profiles from config file
pub fn load_connections() -> ConfigResult<Vec<ConnectionConfig>> {
    let path = ConnectionConfig::connections_file()?;

    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&path).map_err(|e| {
        ConfigError::NotFound(format!("Failed to read connections file: {}", e))
    })?;

    let file: ConnectionsFile = toml::from_str(&content)?;
    Ok(file.connections)
}

/// Save a connection profile to config file
pub fn save_connection(config: &ConnectionConfig) -> ConfigResult<()> {
    // TODO: Phase 8 - Implement connection saving
    // 1. Load existing connections
    // 2. Update or append new connection
    // 3. Write back to file
    // 4. Handle password storage in keychain
    todo!("Saving connections not yet implemented")
}

/// Find a connection by name
pub fn find_connection(name: &str) -> ConfigResult<ConnectionConfig> {
    let connections = load_connections()?;
    connections
        .into_iter()
        .find(|c| c.name == name)
        .ok_or_else(|| ConfigError::ProfileNotFound(name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_string() {
        let config = ConnectionConfig {
            name: "test".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "mydb".to_string(),
            username: "user".to_string(),
            password: None,
            ssl_mode: SslMode::Disable,
        };

        assert_eq!(
            config.connection_string(),
            "host=localhost port=5432 dbname=mydb user=user"
        );
    }

    #[test]
    fn test_default_port() {
        assert_eq!(default_port(), 5432);
    }

    #[test]
    fn test_ssl_mode_default() {
        assert_eq!(SslMode::default(), SslMode::Prefer);
    }
}

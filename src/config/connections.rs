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

    /// Password
    #[serde(skip_serializing)]
    pub password: Option<String>,

    /// SSL mode
    #[serde(default)]
    pub ssl_mode: SslMode,
}

/// SSL connection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SslMode {
    Disable,
    #[default]
    Prefer,
    Require,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConnectionsFile {
    #[serde(default)]
    connections: Vec<ConnectionConfig>,
}

fn default_port() -> u16 {
    5432
}

impl ConnectionConfig {
    /// Parse a postgres:// URL into a ConnectionConfig
    pub fn from_url(url: &str) -> ConfigResult<Self> {
        // postgres://user:pass@host:port/dbname
        let url = url.trim();
        let rest = url
            .strip_prefix("postgres://")
            .or_else(|| url.strip_prefix("postgresql://"))
            .ok_or_else(|| ConfigError::Invalid("URL must start with postgres://".into()))?;

        // Split at @ to get credentials and host info
        let (creds, host_part) = rest
            .split_once('@')
            .ok_or_else(|| ConfigError::Invalid("URL must contain @".into()))?;

        // Parse credentials
        let (username, password) = if let Some((u, p)) = creds.split_once(':') {
            (u.to_string(), Some(p.to_string()))
        } else {
            (creds.to_string(), None)
        };

        // Split host:port/dbname
        let (host_port, database) = host_part
            .split_once('/')
            .ok_or_else(|| ConfigError::Invalid("URL must contain /dbname".into()))?;

        // Split database name from query params and parse sslmode
        let (database, ssl_mode) = if let Some((db, query)) = database.split_once('?') {
            let ssl = parse_sslmode_param(query);
            (db.to_string(), ssl)
        } else {
            (database.to_string(), SslMode::Prefer)
        };

        let (host, port) = if let Some((h, p)) = host_port.split_once(':') {
            let port = p
                .parse::<u16>()
                .map_err(|_| ConfigError::Invalid(format!("Invalid port: {}", p)))?;
            (h.to_string(), port)
        } else {
            (host_port.to_string(), 5432)
        };

        Ok(Self {
            name: format!("{}@{}", database, host),
            host,
            port,
            database,
            username,
            password,
            ssl_mode,
        })
    }

    /// Build a PostgreSQL connection string (without password)
    pub fn connection_string(&self) -> String {
        format!(
            "host={} port={} dbname={} user={}",
            self.host, self.port, self.database, self.username
        )
    }

    /// Build a full connection string including password
    pub fn connection_string_with_password(&self) -> String {
        let base = self.connection_string();
        let with_ssl = format!(
            "{} sslmode={}",
            base,
            match self.ssl_mode {
                SslMode::Disable => "disable",
                SslMode::Prefer => "prefer",
                SslMode::Require => "require",
            }
        );
        if let Some(ref pw) = self.password {
            format!("{} password={}", with_ssl, pw)
        } else {
            with_ssl
        }
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

/// Parse the `sslmode` value from a URL query string
fn parse_sslmode_param(query: &str) -> SslMode {
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("sslmode=") {
            return match value {
                "disable" => SslMode::Disable,
                "require" => SslMode::Require,
                _ => SslMode::Prefer,
            };
        }
    }
    SslMode::Prefer
}

/// Load all connection profiles from config file
pub fn load_connections() -> ConfigResult<Vec<ConnectionConfig>> {
    let path = ConnectionConfig::connections_file()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| ConfigError::NotFound(format!("Failed to read connections file: {}", e)))?;
    let file: ConnectionsFile = toml::from_str(&content)?;
    Ok(file.connections)
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
    fn test_from_url() {
        let config =
            ConnectionConfig::from_url("postgres://user:pass@localhost:5432/mydb").unwrap();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.database, "mydb");
        assert_eq!(config.username, "user");
        assert_eq!(config.password, Some("pass".to_string()));
        assert_eq!(config.ssl_mode, SslMode::Prefer);
    }

    #[test]
    fn test_from_url_default_port() {
        let config = ConnectionConfig::from_url("postgres://user:pass@localhost/mydb").unwrap();
        assert_eq!(config.port, 5432);
    }

    #[test]
    fn test_from_url_sslmode_require() {
        let config =
            ConnectionConfig::from_url("postgres://user:pass@host/db?sslmode=require").unwrap();
        assert_eq!(config.ssl_mode, SslMode::Require);
        assert_eq!(config.database, "db");
    }

    #[test]
    fn test_from_url_sslmode_disable() {
        let config =
            ConnectionConfig::from_url("postgres://user:pass@host/db?sslmode=disable").unwrap();
        assert_eq!(config.ssl_mode, SslMode::Disable);
    }

    #[test]
    fn test_connection_string_with_password() {
        let config = ConnectionConfig {
            name: "test".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "mydb".to_string(),
            username: "user".to_string(),
            password: Some("secret".to_string()),
            ssl_mode: SslMode::Disable,
        };
        assert_eq!(
            config.connection_string_with_password(),
            "host=localhost port=5432 dbname=mydb user=user sslmode=disable password=secret"
        );
    }
}

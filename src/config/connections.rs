//! Connection configuration
//!
//! Manages database connection profiles stored in ~/.vizgres/connections.toml

use crate::error::{ConfigError, ConfigResult};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Database connection configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    /// Password (stored in plaintext in config file)
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    /// Parse a postgres:// URL into a ConnectionConfig.
    ///
    /// Handles percent-encoded credentials (e.g. `p%40ss` → `p@ss`),
    /// IPv6 hosts in brackets (`[::1]`), and special characters in
    /// usernames and passwords.
    pub fn from_url(url: &str) -> ConfigResult<Self> {
        // postgres://user:pass@host:port/dbname?sslmode=...
        let url = url.trim();
        let rest = url
            .strip_prefix("postgres://")
            .or_else(|| url.strip_prefix("postgresql://"))
            .ok_or_else(|| ConfigError::Invalid("URL must start with postgres://".into()))?;

        // Split at LAST @ — so that unencoded @ in passwords still works
        let (creds, host_part) = rest
            .rsplit_once('@')
            .ok_or_else(|| ConfigError::Invalid("URL must contain @".into()))?;

        // Parse credentials (split at first : only — rest is password)
        let (username, password) = if let Some((u, p)) = creds.split_once(':') {
            (decode_component(u)?, Some(decode_component(p)?))
        } else {
            (decode_component(creds)?, None)
        };

        if username.is_empty() {
            return Err(ConfigError::Invalid("URL must contain a username".into()));
        }

        // Split host:port/dbname
        let (host_port, database) = host_part
            .split_once('/')
            .ok_or_else(|| ConfigError::Invalid("URL must contain /dbname".into()))?;

        // Split database name from query params and parse sslmode
        let (database, ssl_mode) = if let Some((db, query)) = database.split_once('?') {
            let ssl = parse_sslmode_param(query)?;
            (decode_component(db)?, ssl)
        } else {
            (decode_component(database)?, SslMode::Prefer)
        };

        if database.is_empty() {
            return Err(ConfigError::Invalid("URL must contain /dbname".into()));
        }

        // Parse host and port, handling IPv6 brackets: [::1]:5432
        let (host, port) = parse_host_port(host_port)?;

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

    /// Build a full connection string including password.
    ///
    /// Passwords are single-quoted per libpq conventions so that
    /// special characters (spaces, quotes, backslashes) are handled correctly.
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
            // Single-quote the password, escaping internal quotes and backslashes
            let escaped = pw.replace('\\', "\\\\").replace('\'', "\\'");
            format!("{} password='{}'", with_ssl, escaped)
        } else {
            with_ssl
        }
    }

    /// Build a postgres:// URL from this config.
    ///
    /// Percent-encodes username and password so special characters
    /// (`@`, `:`, `/`, etc.) round-trip safely through `from_url()`.
    pub fn to_url(&self) -> String {
        let user = utf8_percent_encode(&self.username, NON_ALPHANUMERIC);
        let host_port = if self.port == 5432 {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        };
        // Wrap IPv6 addresses in brackets
        let host_port = if self.host.contains(':') {
            if self.port == 5432 {
                format!("[{}]", self.host)
            } else {
                format!("[{}]:{}", self.host, self.port)
            }
        } else {
            host_port
        };
        let ssl_param = match self.ssl_mode {
            SslMode::Prefer => String::new(),
            SslMode::Disable => "?sslmode=disable".to_string(),
            SslMode::Require => "?sslmode=require".to_string(),
        };
        if let Some(ref pw) = self.password {
            let pass = utf8_percent_encode(pw, NON_ALPHANUMERIC);
            format!(
                "postgres://{}:{}@{}/{}{}",
                user, pass, host_port, self.database, ssl_param
            )
        } else {
            format!(
                "postgres://{}@{}/{}{}",
                user, host_port, self.database, ssl_param
            )
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

/// Percent-decode a URL component, returning a ConfigError on invalid UTF-8.
fn decode_component(s: &str) -> ConfigResult<String> {
    percent_decode_str(s)
        .decode_utf8()
        .map(|s| s.into_owned())
        .map_err(|_| ConfigError::Invalid("Invalid UTF-8 in URL".into()))
}

/// Parse host and port from `host:port`, handling IPv6 brackets.
fn parse_host_port(s: &str) -> ConfigResult<(String, u16)> {
    if let Some(rest) = s.strip_prefix('[') {
        // IPv6: [::1]:5432 or [::1]
        let (addr, after_bracket) = rest
            .split_once(']')
            .ok_or_else(|| ConfigError::Invalid("Unclosed '[' in IPv6 host".into()))?;
        let port = if let Some(port_str) = after_bracket.strip_prefix(':') {
            port_str
                .parse::<u16>()
                .map_err(|_| ConfigError::Invalid(format!("Invalid port: {}", port_str)))?
        } else {
            5432
        };
        Ok((addr.to_string(), port))
    } else if let Some((h, p)) = s.rsplit_once(':') {
        // Regular host:port — rsplit_once handles IPv6 without brackets
        // (e.g. "::1" has no port, won't match since p wouldn't parse)
        match p.parse::<u16>() {
            Ok(port) => Ok((h.to_string(), port)),
            Err(_) => Ok((s.to_string(), 5432)), // treat whole thing as host
        }
    } else {
        Ok((s.to_string(), 5432))
    }
}

/// Parse the `sslmode` value from a URL query string.
///
/// Rejects unknown or unsupported values instead of silently falling back,
/// so that security-sensitive modes like `verify-full` aren't quietly downgraded.
fn parse_sslmode_param(query: &str) -> ConfigResult<SslMode> {
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("sslmode=") {
            return match value {
                "disable" => Ok(SslMode::Disable),
                "prefer" => Ok(SslMode::Prefer),
                "require" => Ok(SslMode::Require),
                "allow" | "verify-ca" | "verify-full" => Err(ConfigError::Invalid(format!(
                    "sslmode '{}' is not yet supported (use disable, prefer, or require)",
                    value
                ))),
                other => Err(ConfigError::Invalid(format!(
                    "unknown sslmode: '{}'",
                    other
                ))),
            };
        }
    }
    Ok(SslMode::Prefer)
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

/// Save connection profiles to config file
pub fn save_connections(connections: &[ConnectionConfig]) -> ConfigResult<()> {
    let file = ConnectionsFile {
        connections: connections.to_vec(),
    };
    let content = toml::to_string_pretty(&file)?;
    let path = ConnectionConfig::connections_file()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    Ok(())
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
    fn test_from_url_sslmode_prefer_explicit() {
        let config =
            ConnectionConfig::from_url("postgres://user:pass@host/db?sslmode=prefer").unwrap();
        assert_eq!(config.ssl_mode, SslMode::Prefer);
    }

    #[test]
    fn test_from_url_sslmode_unknown_rejected() {
        let result = ConnectionConfig::from_url("postgres://user:pass@host/db?sslmode=bogus");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown sslmode"), "got: {}", err);
    }

    #[test]
    fn test_from_url_sslmode_unsupported_rejected() {
        let result = ConnectionConfig::from_url("postgres://user:pass@host/db?sslmode=verify-full");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not yet supported"), "got: {}", err);
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
            "host=localhost port=5432 dbname=mydb user=user sslmode=disable password='secret'"
        );
    }

    #[test]
    fn test_connection_string_password_with_special_chars() {
        let config = ConnectionConfig {
            name: "test".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "mydb".to_string(),
            username: "user".to_string(),
            password: Some("it's a p@ss\\word".to_string()),
            ssl_mode: SslMode::Disable,
        };
        assert_eq!(
            config.connection_string_with_password(),
            r"host=localhost port=5432 dbname=mydb user=user sslmode=disable password='it\'s a p@ss\\word'"
        );
    }

    // ── URL parsing edge cases ──────────────────────────────────

    #[test]
    fn test_from_url_password_with_at_sign_encoded() {
        let config = ConnectionConfig::from_url("postgres://user:p%40ss@localhost/mydb").unwrap();
        assert_eq!(config.username, "user");
        assert_eq!(config.password, Some("p@ss".to_string()));
        assert_eq!(config.host, "localhost");
    }

    #[test]
    fn test_from_url_password_with_at_sign_raw() {
        // Raw @ in password — parser splits at LAST @, so this works
        let config = ConnectionConfig::from_url("postgres://user:p@ss@localhost/mydb").unwrap();
        assert_eq!(config.username, "user");
        assert_eq!(config.password, Some("p@ss".to_string()));
        assert_eq!(config.host, "localhost");
    }

    #[test]
    fn test_from_url_password_with_colon() {
        let config =
            ConnectionConfig::from_url("postgres://user:pa:ss:word@localhost/mydb").unwrap();
        assert_eq!(config.username, "user");
        assert_eq!(config.password, Some("pa:ss:word".to_string()));
    }

    #[test]
    fn test_from_url_percent_encoded_credentials() {
        // Username and password with encoded special chars
        let config =
            ConnectionConfig::from_url("postgres://us%65r:p%40ss%3Aw%6Frd@localhost/mydb").unwrap();
        assert_eq!(config.username, "user"); // %65 = 'e'
        assert_eq!(config.password, Some("p@ss:word".to_string()));
    }

    #[test]
    fn test_from_url_no_password() {
        let config = ConnectionConfig::from_url("postgres://justuser@localhost/mydb").unwrap();
        assert_eq!(config.username, "justuser");
        assert!(config.password.is_none());
    }

    #[test]
    fn test_from_url_ipv6_with_brackets() {
        let config = ConnectionConfig::from_url("postgres://user:pass@[::1]:5432/mydb").unwrap();
        assert_eq!(config.host, "::1");
        assert_eq!(config.port, 5432);
    }

    #[test]
    fn test_from_url_ipv6_brackets_default_port() {
        let config = ConnectionConfig::from_url("postgres://user:pass@[::1]/mydb").unwrap();
        assert_eq!(config.host, "::1");
        assert_eq!(config.port, 5432);
    }

    #[test]
    fn test_from_url_ipv6_full_address() {
        let config =
            ConnectionConfig::from_url("postgres://user:pass@[2001:db8::1]:5433/mydb").unwrap();
        assert_eq!(config.host, "2001:db8::1");
        assert_eq!(config.port, 5433);
    }

    #[test]
    fn test_from_url_postgresql_prefix() {
        let config = ConnectionConfig::from_url("postgresql://user:pass@localhost/mydb").unwrap();
        assert_eq!(config.host, "localhost");
    }

    #[test]
    fn test_from_url_encoded_database_name() {
        let config = ConnectionConfig::from_url("postgres://user:pass@localhost/my%20db").unwrap();
        assert_eq!(config.database, "my db");
    }

    #[test]
    fn test_from_url_multiple_query_params() {
        let config = ConnectionConfig::from_url(
            "postgres://user:pass@localhost/db?sslmode=require&connect_timeout=10",
        )
        .unwrap();
        assert_eq!(config.ssl_mode, SslMode::Require);
        assert_eq!(config.database, "db");
    }

    #[test]
    fn test_from_url_empty_database_rejected() {
        let result = ConnectionConfig::from_url("postgres://user:pass@localhost/");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_url_missing_at_sign() {
        let result = ConnectionConfig::from_url("postgres://localhost/mydb");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_url_bad_scheme() {
        let result = ConnectionConfig::from_url("mysql://user:pass@localhost/mydb");
        assert!(result.is_err());
    }

    // ── to_url tests ──────────────────────────────────────────────

    #[test]
    fn test_to_url_roundtrip() {
        let original = ConnectionConfig {
            name: "test".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "mydb".to_string(),
            username: "user".to_string(),
            password: Some("pass".to_string()),
            ssl_mode: SslMode::Prefer,
        };
        let url = original.to_url();
        let parsed = ConnectionConfig::from_url(&url).unwrap();
        assert_eq!(parsed.host, original.host);
        assert_eq!(parsed.port, original.port);
        assert_eq!(parsed.database, original.database);
        assert_eq!(parsed.username, original.username);
        assert_eq!(parsed.password, original.password);
        assert_eq!(parsed.ssl_mode, original.ssl_mode);
    }

    #[test]
    fn test_to_url_special_chars() {
        let config = ConnectionConfig {
            name: "test".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "mydb".to_string(),
            username: "user".to_string(),
            password: Some("p@ss:w/rd".to_string()),
            ssl_mode: SslMode::Prefer,
        };
        let url = config.to_url();
        // Special chars should be percent-encoded
        assert!(!url.contains("p@ss"));
        // Round-trip should decode correctly
        let parsed = ConnectionConfig::from_url(&url).unwrap();
        assert_eq!(parsed.password, Some("p@ss:w/rd".to_string()));
    }

    #[test]
    fn test_to_url_no_password() {
        let config = ConnectionConfig {
            name: "test".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "mydb".to_string(),
            username: "user".to_string(),
            password: None,
            ssl_mode: SslMode::Prefer,
        };
        let url = config.to_url();
        assert_eq!(url, "postgres://user@localhost/mydb");
    }

    #[test]
    fn test_to_url_with_sslmode() {
        let config = ConnectionConfig {
            name: "test".to_string(),
            host: "host".to_string(),
            port: 5432,
            database: "db".to_string(),
            username: "user".to_string(),
            password: Some("pass".to_string()),
            ssl_mode: SslMode::Require,
        };
        let url = config.to_url();
        assert!(url.ends_with("?sslmode=require"));
    }

    #[test]
    fn test_to_url_non_default_port() {
        let config = ConnectionConfig {
            name: "test".to_string(),
            host: "localhost".to_string(),
            port: 5433,
            database: "mydb".to_string(),
            username: "user".to_string(),
            password: None,
            ssl_mode: SslMode::Prefer,
        };
        let url = config.to_url();
        assert_eq!(url, "postgres://user@localhost:5433/mydb");
    }

    #[test]
    fn test_to_url_ipv6() {
        let config = ConnectionConfig {
            name: "test".to_string(),
            host: "::1".to_string(),
            port: 5432,
            database: "mydb".to_string(),
            username: "user".to_string(),
            password: Some("pass".to_string()),
            ssl_mode: SslMode::Prefer,
        };
        let url = config.to_url();
        let parsed = ConnectionConfig::from_url(&url).unwrap();
        assert_eq!(parsed.host, "::1");
    }

    // ── password serialization ────────────────────────────────────

    #[test]
    fn test_password_serializes_to_toml() {
        let config = ConnectionConfig {
            name: "test".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "mydb".to_string(),
            username: "user".to_string(),
            password: Some("secret".to_string()),
            ssl_mode: SslMode::Prefer,
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(
            toml_str.contains("password"),
            "password should be serialized"
        );
        assert!(toml_str.contains("secret"));
    }

    #[test]
    fn test_no_password_omitted_from_toml() {
        let config = ConnectionConfig {
            name: "test".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "mydb".to_string(),
            username: "user".to_string(),
            password: None,
            ssl_mode: SslMode::Prefer,
        };
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(
            !toml_str.contains("password"),
            "None password should not appear in TOML"
        );
    }
}

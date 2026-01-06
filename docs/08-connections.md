# 08 - Connections

> Connection profiles, configuration format, credential storage, and management.

---

## Overview

Vizgres stores connection profiles and settings in `~/.vizgres/`. This directory contains configuration files for saved connections, user preferences, and query history.

---

## Directory Structure

```
~/.vizgres/
├── config.toml          # Global settings
├── connections.toml     # Saved connection profiles
├── history.sql          # Query history (optional)
└── themes/              # Custom color themes (optional)
    └── custom.toml
```

---

## Connection Profiles

### File: `~/.vizgres/connections.toml`

```toml
# Vizgres Connection Profiles

[[connections]]
name = "local"
host = "localhost"
port = 5432
database = "myapp_dev"
username = "postgres"
# password stored in system keychain or prompted

[[connections]]
name = "production"
host = "db.example.com"
port = 5432
database = "myapp_prod"
username = "readonly_user"
ssl_mode = "require"
ssl_cert = "/path/to/client-cert.pem"
ssl_key = "/path/to/client-key.pem"
ssl_root_cert = "/path/to/ca-cert.pem"

[[connections]]
name = "staging"
host = "staging-db.internal"
port = 5432
database = "myapp_staging"
username = "admin"
ssh_tunnel = true
ssh_host = "bastion.example.com"
ssh_user = "ubuntu"
ssh_key = "~/.ssh/id_rsa"
```

### Connection Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | String | Yes | Unique identifier for the connection |
| `host` | String | Yes | Database server hostname or IP |
| `port` | Integer | No | Port number (default: 5432) |
| `database` | String | Yes | Database name |
| `username` | String | Yes | Database username |
| `password` | String | No | Password (prefer keychain) |
| `ssl_mode` | String | No | SSL mode (see below) |
| `ssl_cert` | String | No | Path to client certificate |
| `ssl_key` | String | No | Path to client key |
| `ssl_root_cert` | String | No | Path to CA certificate |
| `ssh_tunnel` | Boolean | No | Use SSH tunnel |
| `ssh_host` | String | No | SSH bastion host |
| `ssh_user` | String | No | SSH username |
| `ssh_port` | Integer | No | SSH port (default: 22) |
| `ssh_key` | String | No | Path to SSH private key |
| `connect_timeout` | Integer | No | Connection timeout in seconds (default: 10) |
| `application_name` | String | No | Application name for pg_stat_activity |

### SSL Modes

| Mode | Description |
|------|-------------|
| `disable` | No SSL |
| `allow` | Try SSL, fall back to non-SSL |
| `prefer` | Try SSL first (default) |
| `require` | Require SSL, don't verify server |
| `verify-ca` | Require SSL, verify server certificate |
| `verify-full` | Require SSL, verify server certificate and hostname |

---

## Data Model

### Connection Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub database: String,
    pub username: String,
    #[serde(skip_serializing)]  // Never persist password to file
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub ssl_mode: SslMode,
    #[serde(default)]
    pub ssl_cert: Option<PathBuf>,
    #[serde(default)]
    pub ssl_key: Option<PathBuf>,
    #[serde(default)]
    pub ssl_root_cert: Option<PathBuf>,
    #[serde(default)]
    pub ssh_tunnel: Option<SshTunnelConfig>,
    #[serde(default = "default_timeout")]
    pub connect_timeout: u64,
    #[serde(default)]
    pub application_name: Option<String>,
}

fn default_port() -> u16 { 5432 }
fn default_timeout() -> u64 { 10 }

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SslMode {
    Disable,
    Allow,
    #[default]
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshTunnelConfig {
    pub host: String,
    pub user: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub key: Option<PathBuf>,
}

fn default_ssh_port() -> u16 { 22 }
```

### Connection String Generation

```rust
impl ConnectionConfig {
    pub fn connection_string(&self) -> String {
        let mut params = vec![
            format!("host={}", self.host),
            format!("port={}", self.port),
            format!("dbname={}", self.database),
            format!("user={}", self.username),
            format!("connect_timeout={}", self.connect_timeout),
        ];

        if let Some(app_name) = &self.application_name {
            params.push(format!("application_name={}", app_name));
        }

        params.push(format!("sslmode={}", self.ssl_mode.as_str()));

        if let Some(cert) = &self.ssl_cert {
            params.push(format!("sslcert={}", cert.display()));
        }
        if let Some(key) = &self.ssl_key {
            params.push(format!("sslkey={}", key.display()));
        }
        if let Some(root) = &self.ssl_root_cert {
            params.push(format!("sslrootcert={}", root.display()));
        }

        params.join(" ")
    }
}

impl SslMode {
    fn as_str(&self) -> &'static str {
        match self {
            SslMode::Disable => "disable",
            SslMode::Allow => "allow",
            SslMode::Prefer => "prefer",
            SslMode::Require => "require",
            SslMode::VerifyCa => "verify-ca",
            SslMode::VerifyFull => "verify-full",
        }
    }
}
```

---

## Credential Storage

### Strategy

Passwords should **never** be stored in plain text in config files. Vizgres uses a layered approach:

1. **System Keychain** (preferred): macOS Keychain, Linux Secret Service, Windows Credential Manager
2. **Environment Variables**: `PGPASSWORD` or `VIZGRES_<NAME>_PASSWORD`
3. **Interactive Prompt**: Ask user at connection time

### Keychain Integration

```rust
use keyring::Entry;

pub struct CredentialStore {
    service_name: String,
}

impl CredentialStore {
    pub fn new() -> Self {
        Self {
            service_name: "vizgres".to_string(),
        }
    }

    pub fn get_password(&self, connection_name: &str) -> Option<String> {
        let entry = Entry::new(&self.service_name, connection_name).ok()?;
        entry.get_password().ok()
    }

    pub fn set_password(&self, connection_name: &str, password: &str) -> Result<(), Error> {
        let entry = Entry::new(&self.service_name, connection_name)?;
        entry.set_password(password)?;
        Ok(())
    }

    pub fn delete_password(&self, connection_name: &str) -> Result<(), Error> {
        let entry = Entry::new(&self.service_name, connection_name)?;
        entry.delete_password()?;
        Ok(())
    }
}
```

### Password Resolution

```rust
impl ConnectionConfig {
    pub async fn resolve_password(&self, store: &CredentialStore) -> Result<String, Error> {
        // 1. Check if already set (from previous prompt)
        if let Some(pwd) = &self.password {
            return Ok(pwd.clone());
        }

        // 2. Check environment variable
        let env_var = format!("VIZGRES_{}_PASSWORD", self.name.to_uppercase());
        if let Ok(pwd) = std::env::var(&env_var) {
            return Ok(pwd);
        }

        // 3. Check PGPASSWORD
        if let Ok(pwd) = std::env::var("PGPASSWORD") {
            return Ok(pwd);
        }

        // 4. Check system keychain
        if let Some(pwd) = store.get_password(&self.name) {
            return Ok(pwd);
        }

        // 5. Prompt user
        Err(Error::PasswordRequired)
    }
}
```

---

## Global Settings

### File: `~/.vizgres/config.toml`

```toml
# Vizgres Global Configuration

[ui]
theme = "dark"
show_line_numbers = true
confirm_quit = true
results_limit_warning = 1000

[editor]
tab_size = 2
auto_format_on_paste = false
highlight_current_line = true

[tree]
show_system_schemas = false
show_row_counts = true
lazy_load = true

[results]
max_column_width = 50
null_display = "null"
date_format = "%Y-%m-%d"
timestamp_format = "%Y-%m-%d %H:%M:%S"
truncate_text_at = 100

[history]
enabled = true
max_entries = 1000
save_on_execute = true

[connections]
default = "local"
connect_on_startup = true
auto_reconnect = true
reconnect_delay_ms = 1000
max_reconnect_attempts = 3
```

### Settings Data Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub ui: UiSettings,
    #[serde(default)]
    pub editor: EditorSettings,
    #[serde(default)]
    pub tree: TreeSettings,
    #[serde(default)]
    pub results: ResultsSettings,
    #[serde(default)]
    pub history: HistorySettings,
    #[serde(default)]
    pub connections: ConnectionSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_true")]
    pub show_line_numbers: bool,
    #[serde(default = "default_true")]
    pub confirm_quit: bool,
    #[serde(default = "default_limit_warning")]
    pub results_limit_warning: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSettings {
    #[serde(default = "default_tab_size")]
    pub tab_size: usize,
    #[serde(default)]
    pub auto_format_on_paste: bool,
    #[serde(default = "default_true")]
    pub highlight_current_line: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeSettings {
    #[serde(default)]
    pub show_system_schemas: bool,
    #[serde(default = "default_true")]
    pub show_row_counts: bool,
    #[serde(default = "default_true")]
    pub lazy_load: bool,
}

fn default_theme() -> String { "dark".to_string() }
fn default_true() -> bool { true }
fn default_tab_size() -> usize { 2 }
fn default_limit_warning() -> usize { 1000 }
```

---

## SSH Tunnel Support

### Architecture

```
┌─────────┐      SSH Tunnel       ┌──────────┐       ┌────────────┐
│ Vizgres │ ──────────────────▶   │ Bastion  │ ────▶ │ PostgreSQL │
│ Client  │   localhost:54321     │  Server  │       │   Server   │
└─────────┘                       └──────────┘       └────────────┘
```

### Implementation

```rust
use std::process::{Command, Child};

pub struct SshTunnel {
    process: Child,
    local_port: u16,
}

impl SshTunnel {
    pub fn establish(config: &SshTunnelConfig, remote_host: &str, remote_port: u16) -> Result<Self, Error> {
        // Find available local port
        let local_port = find_available_port()?;

        let mut cmd = Command::new("ssh");
        cmd.arg("-N")  // Don't execute remote command
           .arg("-L").arg(format!("{}:{}:{}", local_port, remote_host, remote_port))
           .arg("-p").arg(config.port.to_string())
           .arg(format!("{}@{}", config.user, config.host));

        if let Some(key) = &config.key {
            cmd.arg("-i").arg(key);
        }

        let process = cmd.spawn()?;

        // Wait for tunnel to establish
        std::thread::sleep(std::time::Duration::from_millis(500));

        Ok(Self { process, local_port })
    }

    pub fn local_port(&self) -> u16 {
        self.local_port
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}
```

---

## Connection Management

### Connection Dialog (New Connection)

```
╔═══════════════════════════════════════════════════════════════════════════╗
║ New Connection                                                            ║
╠═══════════════════════════════════════════════════════════════════════════╣
║                                                                           ║
║  Name:       [local_dev                    ]                              ║
║                                                                           ║
║  Host:       [localhost                    ]  Port: [5432  ]              ║
║                                                                           ║
║  Database:   [myapp_development            ]                              ║
║                                                                           ║
║  Username:   [postgres                     ]                              ║
║                                                                           ║
║  Password:   [••••••••                     ]  [ ] Save to keychain        ║
║                                                                           ║
║  SSL Mode:   [prefer ▼]                                                   ║
║                                                                           ║
║  [ ] Use SSH Tunnel                                                       ║
║                                                                           ║
║                                    [ Test Connection ]  [ Save ]  [Cancel]║
╚═══════════════════════════════════════════════════════════════════════════╝
```

### Connection Flow

```rust
pub struct ConnectionManager {
    configs: Vec<ConnectionConfig>,
    credential_store: CredentialStore,
    active_connection: Option<ActiveConnection>,
}

pub struct ActiveConnection {
    config: ConnectionConfig,
    provider: Box<dyn DatabaseProvider>,
    tunnel: Option<SshTunnel>,
    connected_at: DateTime<Utc>,
}

impl ConnectionManager {
    pub async fn connect(&mut self, name: &str) -> Result<(), Error> {
        // Find configuration
        let config = self.configs.iter()
            .find(|c| c.name == name)
            .ok_or(Error::ConnectionNotFound(name.to_string()))?
            .clone();

        // Disconnect existing
        if self.active_connection.is_some() {
            self.disconnect().await?;
        }

        // Resolve password
        let password = config.resolve_password(&self.credential_store).await?;
        let mut config = config;
        config.password = Some(password);

        // Establish SSH tunnel if needed
        let (connect_host, connect_port, tunnel) = if let Some(ssh) = &config.ssh_tunnel {
            let tunnel = SshTunnel::establish(ssh, &config.host, config.port)?;
            let port = tunnel.local_port();
            ("localhost".to_string(), port, Some(tunnel))
        } else {
            (config.host.clone(), config.port, None)
        };

        // Connect to database
        let mut connect_config = config.clone();
        connect_config.host = connect_host;
        connect_config.port = connect_port;

        let provider = PostgresProvider::connect(&connect_config).await?;

        self.active_connection = Some(ActiveConnection {
            config,
            provider: Box::new(provider),
            tunnel,
            connected_at: Utc::now(),
        });

        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<(), Error> {
        if let Some(mut conn) = self.active_connection.take() {
            conn.provider.disconnect().await?;
            // Tunnel dropped automatically
        }
        Ok(())
    }
}
```

---

## File Operations

### Loading Configuration

```rust
pub fn load_connections() -> Result<Vec<ConnectionConfig>, ConfigError> {
    let path = config_dir()?.join("connections.toml");

    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&path)?;

    #[derive(Deserialize)]
    struct ConnectionsFile {
        connections: Vec<ConnectionConfig>,
    }

    let file: ConnectionsFile = toml::from_str(&content)?;
    Ok(file.connections)
}

pub fn save_connections(connections: &[ConnectionConfig]) -> Result<(), ConfigError> {
    let path = config_dir()?.join("connections.toml");

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    #[derive(Serialize)]
    struct ConnectionsFile<'a> {
        connections: &'a [ConnectionConfig],
    }

    let content = toml::to_string_pretty(&ConnectionsFile { connections })?;
    std::fs::write(&path, content)?;

    Ok(())
}

fn config_dir() -> Result<PathBuf, ConfigError> {
    dirs::home_dir()
        .map(|h| h.join(".vizgres"))
        .ok_or(ConfigError::NoHomeDir)
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_string_basic() {
        let config = ConnectionConfig {
            name: "test".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            username: "postgres".to_string(),
            password: None,
            ssl_mode: SslMode::Prefer,
            ..Default::default()
        };

        let conn_str = config.connection_string();

        assert!(conn_str.contains("host=localhost"));
        assert!(conn_str.contains("port=5432"));
        assert!(conn_str.contains("dbname=testdb"));
        assert!(conn_str.contains("user=postgres"));
        assert!(conn_str.contains("sslmode=prefer"));
    }

    #[test]
    fn test_connection_string_with_ssl() {
        let config = ConnectionConfig {
            name: "test".to_string(),
            host: "db.example.com".to_string(),
            port: 5432,
            database: "prod".to_string(),
            username: "admin".to_string(),
            password: None,
            ssl_mode: SslMode::VerifyFull,
            ssl_cert: Some(PathBuf::from("/path/to/cert.pem")),
            ssl_key: Some(PathBuf::from("/path/to/key.pem")),
            ssl_root_cert: Some(PathBuf::from("/path/to/ca.pem")),
            ..Default::default()
        };

        let conn_str = config.connection_string();

        assert!(conn_str.contains("sslmode=verify-full"));
        assert!(conn_str.contains("sslcert=/path/to/cert.pem"));
        assert!(conn_str.contains("sslkey=/path/to/key.pem"));
        assert!(conn_str.contains("sslrootcert=/path/to/ca.pem"));
    }

    #[test]
    fn test_parse_connections_toml() {
        let toml = r#"
            [[connections]]
            name = "local"
            host = "localhost"
            port = 5432
            database = "mydb"
            username = "user"

            [[connections]]
            name = "remote"
            host = "db.example.com"
            database = "prod"
            username = "admin"
            ssl_mode = "require"
        "#;

        let file: ConnectionsFile = toml::from_str(toml).unwrap();
        assert_eq!(file.connections.len(), 2);
        assert_eq!(file.connections[0].name, "local");
        assert_eq!(file.connections[1].ssl_mode, SslMode::Require);
    }

    #[test]
    fn test_default_values() {
        let toml = r#"
            [[connections]]
            name = "minimal"
            host = "localhost"
            database = "test"
            username = "user"
        "#;

        let file: ConnectionsFile = toml::from_str(toml).unwrap();
        let conn = &file.connections[0];

        assert_eq!(conn.port, 5432);  // Default
        assert_eq!(conn.ssl_mode, SslMode::Prefer);  // Default
        assert_eq!(conn.connect_timeout, 10);  // Default
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_connect_to_local_postgres() {
    let config = ConnectionConfig {
        name: "test".to_string(),
        host: "localhost".to_string(),
        port: 5432,
        database: "postgres".to_string(),
        username: "postgres".to_string(),
        password: Some("postgres".to_string()),
        ..Default::default()
    };

    let provider = PostgresProvider::connect(&config).await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert!(provider.is_connected().await);
}

#[tokio::test]
async fn test_connection_failure_bad_host() {
    let config = ConnectionConfig {
        name: "bad".to_string(),
        host: "nonexistent.invalid".to_string(),
        port: 5432,
        database: "test".to_string(),
        username: "user".to_string(),
        password: None,
        connect_timeout: 1,  // Quick timeout
        ..Default::default()
    };

    let result = PostgresProvider::connect(&config).await;
    assert!(result.is_err());
}
```

### Credential Store Tests

```rust
#[test]
fn test_password_resolution_from_env() {
    std::env::set_var("VIZGRES_PROD_PASSWORD", "secret123");

    let config = ConnectionConfig {
        name: "prod".to_string(),
        ..Default::default()
    };

    let store = CredentialStore::new();
    let result = tokio_test::block_on(config.resolve_password(&store));

    assert_eq!(result.unwrap(), "secret123");

    std::env::remove_var("VIZGRES_PROD_PASSWORD");
}
```

---

## Security Considerations

1. **Never log passwords**: Ensure passwords are excluded from debug output
2. **Secure file permissions**: Config files should be `0600` (user read/write only)
3. **Keychain preferred**: Always prefer system keychain over file storage
4. **SSL by default**: Default to `ssl_mode = prefer`
5. **Timeout protection**: Always set connection timeouts

```rust
pub fn ensure_secure_permissions(path: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(path, perms)?;
    }
    Ok(())
}
```

---

## Next Steps

See [09-roadmap.md](./09-roadmap.md) for implementation phases.

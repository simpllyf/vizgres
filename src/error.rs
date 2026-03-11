//! Error types for vizgres
//!
//! This module defines the error hierarchy used throughout the application.
//! We use `thiserror` for library-style errors with clear error chains.

use std::io;

/// Main error type for the vizgres application
#[derive(Debug, thiserror::Error)]
pub enum VizgresError {
    /// Database-related errors
    #[error("Database error: {0}")]
    Database(#[from] DbError),

    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Command parsing errors
    #[error("Command error: {0}")]
    Command(#[from] CommandError),
}

/// Database operation errors
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// Failed to establish connection
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Query execution failed
    #[error("Query execution failed: {message}")]
    QueryFailed {
        message: String,
        position: Option<u32>, // byte offset in query
    },

    /// Schema introspection failed
    #[error("Schema loading failed: {0}")]
    SchemaLoadFailed(String),

    /// Query timed out after configured duration (stores milliseconds)
    #[error("Query timed out after {0}ms")]
    Timeout(u64),
}

/// Configuration loading/parsing errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Home directory not found
    #[error("Could not determine home directory")]
    NoHomeDir,

    /// Config file not found
    #[error("Configuration file not found: {0}")]
    NotFound(String),

    /// Failed to parse TOML
    #[error("Failed to parse configuration: {0}")]
    ParseError(#[from] toml::de::Error),

    /// Failed to serialize TOML
    #[error("Failed to serialize configuration: {0}")]
    SerializeError(#[from] toml::ser::Error),

    /// IO error when reading/writing config
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid configuration value
    #[error("Invalid configuration: {0}")]
    Invalid(String),

    /// Connection profile not found
    #[error("Connection profile '{0}' not found")]
    ProfileNotFound(String),
}

/// Command parsing and execution errors
#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    /// Unknown command
    #[error("Unknown command: {0}")]
    Unknown(String),
}

/// Return a user-friendly hint for a connection error message.
/// Matches common PostgreSQL/network error patterns and suggests actionable fixes.
pub fn connection_hint(error: &str) -> Option<&'static str> {
    let lower = error.to_lowercase();
    if lower.contains("connection refused") {
        return Some("Is PostgreSQL running? Check host/port or try: pg_isready");
    }
    if lower.contains("password authentication failed") {
        return Some("Check your username and password");
    }
    if lower.contains("does not exist") && lower.contains("database") {
        return Some("Database not found — verify the database name");
    }
    if lower.contains("role") && lower.contains("does not exist") {
        return Some("User/role not found — verify the username");
    }
    if lower.contains("could not translate host name")
        || lower.contains("name or service not known")
    {
        return Some("Hostname not found — check the host address");
    }
    if lower.contains("timeout") || lower.contains("timed out") {
        return Some("Connection timed out — server may be unreachable or firewalled");
    }
    if lower.contains("ssl") || lower.contains("tls") {
        return Some("SSL/TLS error — try ssl_mode = \"disable\" or check certificates");
    }
    if lower.contains("too many connections") || lower.contains("remaining connection slots") {
        return Some(
            "Server connection limit reached — try again later or increase max_connections",
        );
    }
    if lower.contains("starting up") {
        return Some("Server is starting up — wait a moment and retry");
    }
    None
}

/// Specialized Result type for vizgres operations
pub type Result<T> = std::result::Result<T, VizgresError>;

/// Specialized Result type for database operations
pub type DbResult<T> = std::result::Result<T, DbError>;

/// Specialized Result type for config operations
pub type ConfigResult<T> = std::result::Result<T, ConfigError>;

/// Specialized Result type for command operations
pub type CommandResult<T> = std::result::Result<T, CommandError>;

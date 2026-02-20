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

    /// Terminal/UI errors (reserved for future use)
    #[allow(dead_code)]
    #[error("Terminal error: {0}")]
    Terminal(String),

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
    #[error("Query execution failed: {0}")]
    QueryFailed(String),

    /// Schema introspection failed
    #[error("Schema loading failed: {0}")]
    SchemaLoadFailed(String),

    /// Not connected to a database (reserved for future use)
    #[allow(dead_code)]
    #[error("Not connected to database")]
    NotConnected,

    /// Operation timed out (reserved for future use)
    #[allow(dead_code)]
    #[error("Operation timed out")]
    Timeout,

    /// Type conversion error (reserved for future use)
    #[allow(dead_code)]
    #[error("Type conversion error: {0}")]
    TypeConversion(String),
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

    /// Missing required argument
    #[allow(dead_code)]
    #[error("Missing required argument for command")]
    MissingArgument,

    /// Invalid argument (reserved for future use)
    #[allow(dead_code)]
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Command execution failed (reserved for future use)
    #[allow(dead_code)]
    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),
}

/// Specialized Result type for vizgres operations
pub type Result<T> = std::result::Result<T, VizgresError>;

/// Specialized Result type for database operations
pub type DbResult<T> = std::result::Result<T, DbError>;

/// Specialized Result type for config operations
pub type ConfigResult<T> = std::result::Result<T, ConfigError>;

/// Specialized Result type for command operations
pub type CommandResult<T> = std::result::Result<T, CommandError>;

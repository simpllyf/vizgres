//! vizgres - A fast, keyboard-driven PostgreSQL client for the terminal
//!
//! vizgres provides a terminal-based interface for working with PostgreSQL databases,
//! designed for daily use by developers who prefer keyboard navigation over mouse-based GUIs.
//!
//! # Features
//!
//! - **Schema Browser**: Navigate databases, schemas, tables, and columns
//! - **Query Editor**: Write and execute SQL queries with multi-line support
//! - **Results Viewer**: Browse query results with cell-level navigation
//! - **Inspector**: View full cell contents with JSON pretty-printing
//! - **Keyboard-First**: All operations accessible via keyboard shortcuts
//!
//! # Architecture
//!
//! The library is organized into several modules:
//!
//! - [`config`]: Connection profiles and application settings
//! - [`db`]: Database connectivity and schema introspection
//! - [`ui`]: Terminal user interface components
//! - [`commands`]: Command parsing for the command bar
//! - [`error`]: Error types and result aliases
//! - [`app`]: Application state and event handling
//!
//! # Example
//!
//! ```no_run
//! use vizgres::config::ConnectionConfig;
//! use vizgres::db::Database;
//! use vizgres::db::postgres::PostgresProvider;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Parse connection URL
//! let config = ConnectionConfig::from_url("postgres://user:pass@localhost/mydb")?;
//!
//! // Connect to database
//! let (provider, _conn_err_rx) = PostgresProvider::connect(&config).await?;
//!
//! // Execute a query
//! let results = provider.execute_query("SELECT * FROM users").await?;
//! println!("Got {} rows", results.row_count);
//!
//! // Load schema
//! let schema = provider.get_schema().await?;
//! for s in &schema.schemas {
//!     println!("Schema: {} ({} tables)", s.name, s.tables.len());
//! }
//! # Ok(())
//! # }
//! ```

pub mod app;
pub mod commands;
pub mod completer;
pub mod config;
pub mod db;
pub mod error;
pub mod history;
pub mod keymap;
pub mod ui;

pub use error::{CommandError, ConfigError, DbError, Result, VizgresError};

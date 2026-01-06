//! vizgres library
//!
//! This library contains the core functionality of vizgres, separated from the
//! main binary for better testability and potential reuse.

pub mod app;
pub mod commands;
pub mod config;
pub mod db;
pub mod error;
pub mod sql;
pub mod ui;

// Re-export commonly used types
pub use error::{CommandError, ConfigError, DbError, Result, VizgresError};

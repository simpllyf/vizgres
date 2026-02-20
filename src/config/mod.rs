//! Configuration management
//!
//! Handles loading connection profiles and application settings.

pub mod connections;
pub mod settings;

pub use connections::{ConnectionConfig, find_connection, load_connections, save_connections};
pub use settings::Settings;

//! Configuration management
//!
//! Handles loading connection profiles and application settings.

pub mod connections;
pub mod saved_queries;
pub mod settings;

pub use connections::{ConnectionConfig, find_connection, load_connections, save_connections};
pub use saved_queries::SavedQuery;
pub use settings::Settings;

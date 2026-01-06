//! Configuration management
//!
//! Handles loading and saving connection profiles and user settings.

pub mod connections;
pub mod settings;

pub use connections::{ConnectionConfig, SslMode, load_connections, save_connection};
pub use settings::{Settings, load_settings};

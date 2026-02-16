//! Configuration management
//!
//! Handles loading connection profiles and user settings.

pub mod connections;
pub mod settings;

pub use connections::{ConnectionConfig, find_connection};

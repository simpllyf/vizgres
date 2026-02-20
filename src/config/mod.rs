//! Configuration management
//!
//! Handles loading connection profiles.

pub mod connections;

pub use connections::{ConnectionConfig, load_connections, save_connections};

//! Configuration management
//!
//! Handles loading connection profiles.

pub mod connections;

pub use connections::{ConnectionConfig, find_connection};

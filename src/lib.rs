//! vizgres library
//!
//! Core functionality of vizgres - a keyboard-driven PostgreSQL TUI client.

#![allow(dead_code)]

pub mod app;
pub mod commands;
pub mod config;
pub mod db;
pub mod error;
pub mod sql;
pub mod ui;

pub use error::{CommandError, ConfigError, DbError, Result, VizgresError};

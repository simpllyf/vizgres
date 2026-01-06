//! Database abstraction layer
//!
//! This module provides a trait-based abstraction over database operations,
//! allowing for multiple database backends and easy testing with mocks.

pub mod postgres;
pub mod provider;
pub mod schema;
pub mod types;

// Re-export main types
pub use provider::DatabaseProvider;
pub use schema::{Column, Schema, SchemaTree, Table};
pub use types::{CellValue, ColumnDef, DataType, QueryResults, Row};

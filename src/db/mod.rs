//! Database layer
//!
//! PostgreSQL connection, query execution, and schema introspection.

pub mod postgres;
pub mod schema;
pub mod sql_limit;
pub mod types;

pub use postgres::PostgresProvider;
pub use types::QueryResults;

use crate::db::schema::{Function, Index, SchemaTree, Table};
use crate::error::DbResult;

/// Trait abstracting database operations for testability.
/// Send + Sync required for Arc sharing across tokio::spawn tasks.
pub trait Database: Send + Sync {
    /// Execute a query with client-side timeout protection.
    ///
    /// - `timeout_ms`: Client-side timeout (tokio::time::timeout). 0 = disabled.
    /// - `max_rows`: Maximum rows to return. 0 = unlimited.
    ///
    /// Server-side `statement_timeout` is set at connection level.
    fn execute_query(
        &self,
        sql: &str,
        timeout_ms: u64,
        max_rows: usize,
    ) -> impl std::future::Future<Output = DbResult<QueryResults>> + Send;

    /// Load schema with optional limit per category. Pass 0 for unlimited.
    fn get_schema(
        &self,
        limit: usize,
    ) -> impl std::future::Future<Output = DbResult<SchemaTree>> + Send;

    /// Search schema objects by name pattern (case-insensitive substring match).
    /// Returns a SchemaTree containing only matching objects and their containers.
    fn search_schema(
        &self,
        pattern: &str,
    ) -> impl std::future::Future<Output = DbResult<SchemaTree>> + Send;

    /// Load more tables for a schema (for pagination).
    fn load_more_tables(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> impl std::future::Future<Output = DbResult<Vec<Table>>> + Send;

    /// Load more views for a schema (for pagination).
    fn load_more_views(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> impl std::future::Future<Output = DbResult<Vec<Table>>> + Send;

    /// Load more functions for a schema (for pagination).
    fn load_more_functions(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> impl std::future::Future<Output = DbResult<Vec<Function>>> + Send;

    /// Load more indexes for a schema (for pagination).
    fn load_more_indexes(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> impl std::future::Future<Output = DbResult<Vec<Index>>> + Send;
}

// Compile-time assertion: PostgresProvider must implement Database + Send + Sync
const _: fn() = || {
    fn assert_impl<T: Database + Send + Sync>() {}
    assert_impl::<PostgresProvider>();
};

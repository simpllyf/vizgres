//! Database layer
//!
//! PostgreSQL connection, query execution, and schema introspection.

pub mod postgres;
pub mod schema;
pub mod types;

pub use postgres::PostgresProvider;
pub use types::QueryResults;

use crate::db::schema::SchemaTree;
use crate::error::DbResult;

/// Trait abstracting database operations for testability.
/// Send + Sync required for Arc sharing across tokio::spawn tasks.
pub trait Database: Send + Sync {
    fn execute_query(
        &self,
        sql: &str,
    ) -> impl std::future::Future<Output = DbResult<QueryResults>> + Send;
    fn get_schema(&self) -> impl std::future::Future<Output = DbResult<SchemaTree>> + Send;
}

// Compile-time assertion: PostgresProvider must implement Database + Send + Sync
const _: fn() = || {
    fn assert_impl<T: Database + Send + Sync>() {}
    assert_impl::<PostgresProvider>();
};

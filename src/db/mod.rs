//! Database layer
//!
//! PostgreSQL connection, query execution, and schema introspection.

pub mod postgres;
pub mod schema;
pub mod types;

pub use postgres::PostgresProvider;
pub use types::QueryResults;

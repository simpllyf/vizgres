//! Database provider trait
//!
//! Defines the interface that all database backends must implement.
//! This abstraction allows for:
//! - Multiple database support (PostgreSQL, MySQL, SQLite, etc.)
//! - Easy testing with mock implementations
//! - Consistent error handling

use crate::config::ConnectionConfig;
use crate::db::schema::SchemaTree;
use crate::db::types::{ColumnInfo, QueryResults};
use crate::error::DbResult;
use async_trait::async_trait;

/// Main database provider trait
///
/// All database implementations must implement this trait to provide
/// a consistent interface for the application.
#[async_trait]
pub trait DatabaseProvider: Send + Sync {
    /// Establish connection to the database
    ///
    /// # Errors
    /// Returns `DbError::ConnectionFailed` if connection cannot be established
    async fn connect(config: &ConnectionConfig) -> DbResult<Self>
    where
        Self: Sized;

    /// Close the database connection
    ///
    /// # Errors
    /// Returns error if disconnection fails (though this is rare)
    async fn disconnect(&mut self) -> DbResult<()>;

    /// Check if the connection is still alive
    ///
    /// This should be a lightweight check (e.g., SELECT 1)
    async fn is_connected(&self) -> bool;

    /// Execute a SQL query and return results
    ///
    /// # Arguments
    /// * `sql` - The SQL query to execute
    ///
    /// # Errors
    /// Returns `DbError::QueryFailed` if query execution fails
    /// Returns `DbError::NotConnected` if not connected
    async fn execute_query(&self, sql: &str) -> DbResult<QueryResults>;

    /// Get the complete database schema tree
    ///
    /// This includes schemas, tables, views, functions, etc.
    ///
    /// # Errors
    /// Returns `DbError::SchemaLoadFailed` if schema introspection fails
    async fn get_schema(&self) -> DbResult<SchemaTree>;

    /// Get detailed information about a specific table
    ///
    /// # Arguments
    /// * `schema` - Schema name
    /// * `table` - Table name
    ///
    /// # Errors
    /// Returns error if table doesn't exist or query fails
    async fn get_table_columns(&self, schema: &str, table: &str) -> DbResult<Vec<ColumnInfo>>;

    /// Get EXPLAIN output for a query
    ///
    /// # Arguments
    /// * `sql` - The SQL query to explain
    ///
    /// # Errors
    /// Returns error if EXPLAIN fails
    async fn explain_query(&self, sql: &str) -> DbResult<ExplainPlan>;

    /// Get autocomplete suggestions for current context
    ///
    /// # Arguments
    /// * `context` - Current editing context (cursor position, partial query, etc.)
    ///
    /// # Errors
    /// Returns error if completion generation fails
    async fn get_completions(&self, context: &CompletionContext) -> DbResult<Vec<Completion>>;
}

/// EXPLAIN plan representation
#[derive(Debug, Clone)]
pub struct ExplainPlan {
    /// Raw EXPLAIN output
    pub raw: String,
    /// Parsed plan nodes (if available)
    pub nodes: Vec<PlanNode>,
}

/// A node in the query execution plan
#[derive(Debug, Clone)]
pub struct PlanNode {
    /// Node type (e.g., "Seq Scan", "Index Scan")
    pub node_type: String,
    /// Estimated cost
    pub cost: Option<f64>,
    /// Estimated rows
    pub rows: Option<u64>,
    /// Additional details
    pub details: String,
}

/// Context for autocomplete suggestions
#[derive(Debug, Clone)]
pub struct CompletionContext {
    /// The full query text
    pub text: String,
    /// Cursor position within the text
    pub cursor_position: usize,
    /// Tables referenced in the query so far
    pub tables_in_query: Vec<TableRef>,
}

/// Reference to a table in a query (with optional alias)
#[derive(Debug, Clone)]
pub struct TableRef {
    /// Schema name
    pub schema: String,
    /// Table name
    pub table: String,
    /// Alias (if any)
    pub alias: Option<String>,
}

/// Autocomplete suggestion
#[derive(Debug, Clone)]
pub struct Completion {
    /// The text to insert
    pub text: String,
    /// Type of completion (table, column, keyword, function)
    pub kind: CompletionKind,
    /// Additional information to display
    pub detail: Option<String>,
}

/// Type of autocomplete suggestion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Table,
    Column,
    Keyword,
    Function,
    Schema,
}

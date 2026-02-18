//! Database schema introspection
//!
//! Structures for representing database schema hierarchies.

use crate::db::types::DataType;

/// Complete database schema tree
#[derive(Debug, Clone)]
pub struct SchemaTree {
    /// All schemas in the database
    pub schemas: Vec<Schema>,
}

/// A database schema (namespace)
#[derive(Debug, Clone)]
pub struct Schema {
    /// Schema name
    pub name: String,
    /// Tables in this schema
    pub tables: Vec<Table>,
    /// Views in this schema (same shape as tables)
    pub views: Vec<Table>,
    /// Indexes in this schema
    pub indexes: Vec<Index>,
    /// Functions and procedures in this schema
    pub functions: Vec<Function>,
}

/// A database table
#[derive(Debug, Clone)]
pub struct Table {
    /// Table name
    pub name: String,
    /// Columns in this table
    pub columns: Vec<Column>,
}

/// A table column
#[derive(Debug, Clone)]
pub struct Column {
    /// Column name
    pub name: String,
    /// Data type
    pub data_type: DataType,
    /// Whether this column is part of the primary key
    pub is_primary_key: bool,
    /// Foreign key reference, if any
    pub foreign_key: Option<ForeignKey>,
}

/// A foreign key reference from a column to another table's column
#[derive(Debug, Clone)]
pub struct ForeignKey {
    /// Target table (e.g. "users" or "other_schema.users")
    pub target_table: String,
    /// Target column (e.g. "id")
    pub target_column: String,
}

/// A database index
#[derive(Debug, Clone)]
pub struct Index {
    /// Index name (e.g. "users_pkey")
    pub name: String,
    /// Columns covered by this index
    pub columns: Vec<String>,
    /// Whether this is a unique index
    #[allow(dead_code)]
    pub is_unique: bool,
    /// Whether this is the primary key index
    #[allow(dead_code)]
    pub is_primary: bool,
    /// Table this index belongs to
    #[allow(dead_code)]
    pub table_name: String,
}

/// A stored function or procedure
#[derive(Debug, Clone)]
pub struct Function {
    /// Function name
    pub name: String,
    /// Formatted argument list (e.g. "integer, text")
    pub args: String,
    /// Return type (e.g. "void", "integer", "SETOF record")
    pub return_type: String,
}

impl SchemaTree {
    /// Create a new empty schema tree
    pub fn new() -> Self {
        Self {
            schemas: Vec::new(),
        }
    }
}

impl Default for SchemaTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_tree_new() {
        let tree = SchemaTree::new();
        assert!(tree.schemas.is_empty());
    }
}

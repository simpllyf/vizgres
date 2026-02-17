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

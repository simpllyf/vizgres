//! Database schema introspection
//!
//! Structures for representing database schema hierarchies and queries
//! for loading schema information from PostgreSQL.

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
    /// Views in this schema
    pub views: Vec<View>,
    /// Functions/procedures in this schema
    pub functions: Vec<Function>,
    /// Sequences in this schema
    pub sequences: Vec<Sequence>,
}

/// A database table
#[derive(Debug, Clone)]
pub struct Table {
    /// Table name
    pub name: String,
    /// Schema name
    pub schema: String,
    /// Columns in this table
    pub columns: Vec<Column>,
    /// Indexes on this table
    pub indexes: Vec<Index>,
    /// Constraints on this table
    pub constraints: Vec<Constraint>,
    /// Estimated row count
    pub row_estimate: u64,
}

/// A database view
#[derive(Debug, Clone)]
pub struct View {
    /// View name
    pub name: String,
    /// Schema name
    pub schema: String,
    /// View definition (SQL)
    pub definition: String,
}

/// A database function/procedure
#[derive(Debug, Clone)]
pub struct Function {
    /// Function name
    pub name: String,
    /// Schema name
    pub schema: String,
    /// Function signature
    pub signature: String,
    /// Return type
    pub return_type: String,
}

/// A database sequence
#[derive(Debug, Clone)]
pub struct Sequence {
    /// Sequence name
    pub name: String,
    /// Schema name
    pub schema: String,
}

/// A table column
#[derive(Debug, Clone)]
pub struct Column {
    /// Column name
    pub name: String,
    /// Data type
    pub data_type: DataType,
    /// Is nullable?
    pub nullable: bool,
    /// Default value expression
    pub default: Option<String>,
    /// Is this a primary key?
    pub is_primary_key: bool,
    /// Ordinal position
    pub ordinal_position: i32,
}

/// A table index
#[derive(Debug, Clone)]
pub struct Index {
    /// Index name
    pub name: String,
    /// Columns in the index
    pub columns: Vec<String>,
    /// Is this a unique index?
    pub is_unique: bool,
    /// Is this a primary key index?
    pub is_primary: bool,
}

/// A table constraint
#[derive(Debug, Clone)]
pub struct Constraint {
    /// Constraint name
    pub name: String,
    /// Constraint type
    pub constraint_type: ConstraintType,
    /// Constraint definition
    pub definition: String,
}

/// Types of table constraints
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintType {
    PrimaryKey,
    ForeignKey,
    Unique,
    Check,
}

impl SchemaTree {
    /// Create a new empty schema tree
    pub fn new() -> Self {
        Self {
            schemas: Vec::new(),
        }
    }

    /// Find a schema by name
    pub fn find_schema(&self, name: &str) -> Option<&Schema> {
        self.schemas.iter().find(|s| s.name == name)
    }

    /// Get all table names across all schemas
    pub fn all_tables(&self) -> Vec<(&str, &str)> {
        self.schemas
            .iter()
            .flat_map(|schema| {
                schema
                    .tables
                    .iter()
                    .map(move |table| (schema.name.as_str(), table.name.as_str()))
            })
            .collect()
    }
}

impl Default for SchemaTree {
    fn default() -> Self {
        Self::new()
    }
}

impl Schema {
    /// Find a table by name in this schema
    pub fn find_table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name == name)
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

    #[test]
    fn test_find_schema() {
        let tree = SchemaTree {
            schemas: vec![Schema {
                name: "public".to_string(),
                tables: vec![],
                views: vec![],
                functions: vec![],
                sequences: vec![],
            }],
        };
        assert!(tree.find_schema("public").is_some());
        assert!(tree.find_schema("nonexistent").is_none());
    }
}

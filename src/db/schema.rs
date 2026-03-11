//! Database schema introspection
//!
//! Structures for representing database schema hierarchies.

use crate::db::types::DataType;

/// A collection with pagination metadata
#[derive(Debug, Clone)]
pub struct PaginatedVec<T> {
    /// Items in the current page
    pub items: Vec<T>,
    /// Total count of items (may be greater than items.len())
    pub total_count: usize,
}

impl<T> PaginatedVec<T> {
    /// Create a new PaginatedVec with items and total count
    pub fn new(items: Vec<T>, total_count: usize) -> Self {
        Self { items, total_count }
    }

    /// Create a PaginatedVec from items where total equals items length
    pub fn from_vec(items: Vec<T>) -> Self {
        let total_count = items.len();
        Self { items, total_count }
    }

    /// Whether there are more items available beyond what's loaded
    pub fn is_truncated(&self) -> bool {
        self.items.len() < self.total_count
    }

    /// Number of items currently loaded
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether no items are loaded
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Iterator over items
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    /// Extend items with more loaded items
    pub fn extend(&mut self, more: Vec<T>) {
        self.items.extend(more);
    }

    /// Get a reference to the first item, if any
    pub fn first(&self) -> Option<&T> {
        self.items.first()
    }

    /// Get a reference to the item at the given index
    pub fn get(&self, index: usize) -> Option<&T> {
        self.items.get(index)
    }
}

impl<T> Default for PaginatedVec<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            total_count: 0,
        }
    }
}

impl<'a, T> IntoIterator for &'a PaginatedVec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

/// Complete database schema tree
#[derive(Debug, Clone)]
pub struct SchemaTree {
    /// All schemas in the database (with pagination metadata)
    pub schemas: PaginatedVec<Schema>,
}

/// A database schema (namespace)
#[derive(Debug, Clone)]
pub struct Schema {
    /// Schema name
    pub name: String,
    /// Tables in this schema (with pagination metadata)
    pub tables: PaginatedVec<Table>,
    /// Views in this schema (same shape as tables, with pagination metadata)
    pub views: PaginatedVec<Table>,
    /// Indexes in this schema (with pagination metadata)
    pub indexes: PaginatedVec<Index>,
    /// Functions and procedures in this schema (with pagination metadata)
    pub functions: PaginatedVec<Function>,
}

/// A database table
#[derive(Debug, Clone)]
pub struct Table {
    /// Table name
    pub name: String,
    /// Columns in this table
    pub columns: Vec<Column>,
    /// Estimated row count from pg_stat_user_tables (None for views)
    pub row_count: Option<i64>,
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
            schemas: PaginatedVec::default(),
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

    #[test]
    fn test_paginated_vec_new() {
        let pv: PaginatedVec<i32> = PaginatedVec::new(vec![1, 2, 3], 10);
        assert_eq!(pv.len(), 3);
        assert_eq!(pv.total_count, 10);
        assert!(pv.is_truncated());
    }

    #[test]
    fn test_paginated_vec_from_vec() {
        let pv = PaginatedVec::from_vec(vec![1, 2, 3]);
        assert_eq!(pv.len(), 3);
        assert_eq!(pv.total_count, 3);
        assert!(!pv.is_truncated());
    }

    #[test]
    fn test_paginated_vec_empty() {
        let pv: PaginatedVec<i32> = PaginatedVec::default();
        assert!(pv.is_empty());
        assert_eq!(pv.total_count, 0);
        assert!(!pv.is_truncated());
    }

    #[test]
    fn test_paginated_vec_extend() {
        let mut pv = PaginatedVec::new(vec![1, 2], 5);
        assert_eq!(pv.len(), 2);
        pv.extend(vec![3, 4]);
        assert_eq!(pv.len(), 4);
        assert!(pv.is_truncated()); // still 4 < 5
    }

    #[test]
    fn test_paginated_vec_iter() {
        let pv = PaginatedVec::from_vec(vec![1, 2, 3]);
        let sum: i32 = pv.iter().sum();
        assert_eq!(sum, 6);
    }
}

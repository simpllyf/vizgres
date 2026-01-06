//! SQL autocomplete
//!
//! Provides context-aware SQL autocomplete suggestions.

use crate::db::provider::CompletionContext;
use crate::db::schema::SchemaTree;

/// SQL autocomplete engine
pub struct SqlCompleter {
    /// Cached database schema for completions
    schema: Option<SchemaTree>,
}

impl SqlCompleter {
    /// Create a new SQL completer
    pub fn new() -> Self {
        Self { schema: None }
    }

    /// Update the schema cache
    pub fn update_schema(&mut self, schema: SchemaTree) {
        self.schema = Some(schema);
    }

    /// Get completion suggestions for the current context
    pub fn complete(&self, _context: &CompletionContext) -> Vec<String> {
        // TODO: Phase 7 - Implement smart autocomplete
        // 1. Tokenize query up to cursor
        // 2. Determine context (after SELECT, FROM, WHERE, etc.)
        // 3. Return relevant suggestions:
        //    - Table names after FROM
        //    - Column names after SELECT or in WHERE
        //    - Keywords in appropriate positions
        //    - Function names
        todo!("SQL autocomplete not yet implemented")
    }

    /// Get table name suggestions
    #[allow(dead_code)]
    fn complete_tables(&self) -> Vec<String> {
        // TODO: Phase 7 - Return table names from schema
        Vec::new()
    }

    /// Get column name suggestions for a table
    #[allow(dead_code)]
    fn complete_columns(&self, _table: &str) -> Vec<String> {
        // TODO: Phase 7 - Return column names for a specific table
        Vec::new()
    }

    /// Get SQL keyword suggestions
    #[allow(dead_code)]
    fn complete_keywords(&self, _prefix: &str) -> Vec<String> {
        // TODO: Phase 7 - Return matching SQL keywords
        const KEYWORDS: &[&str] = &[
            "SELECT", "FROM", "WHERE", "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "ON", "GROUP",
            "BY", "ORDER", "HAVING", "LIMIT", "OFFSET", "INSERT", "UPDATE", "DELETE", "CREATE",
            "ALTER", "DROP", "AND", "OR", "NOT", "IN", "EXISTS", "BETWEEN", "LIKE", "IS", "NULL",
        ];
        KEYWORDS.iter().map(|s| s.to_string()).collect()
    }
}

impl Default for SqlCompleter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_completer_new() {
        let completer = SqlCompleter::new();
        assert!(completer.schema.is_none());
    }

    #[test]
    fn test_keywords_contains_select() {
        let completer = SqlCompleter::new();
        let keywords = completer.complete_keywords("");
        assert!(keywords.contains(&"SELECT".to_string()));
    }
}

//! SQL autocomplete
//!
//! Provides context-aware SQL autocomplete suggestions.
//! Not implemented for MVP.

use crate::db::schema::SchemaTree;

/// SQL autocomplete engine
pub struct SqlCompleter {
    schema: Option<SchemaTree>,
}

impl SqlCompleter {
    pub fn new() -> Self {
        Self { schema: None }
    }

    pub fn update_schema(&mut self, schema: SchemaTree) {
        self.schema = Some(schema);
    }

    pub fn complete(&self, _prefix: &str) -> Vec<String> {
        Vec::new()
    }
}

impl Default for SqlCompleter {
    fn default() -> Self {
        Self::new()
    }
}

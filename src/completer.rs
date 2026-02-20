//! Inline ghost-text auto-complete engine
//!
//! Provides SQL keyword and schema-aware completion candidates for the query
//! editor. The completer is a stateful struct owned by `App`; the editor only
//! stores the ghost-text suffix to render.

use crate::db::schema::SchemaTree;
use crate::ui::highlight;

const MAX_CANDIDATES: usize = 5;

/// SQL clause context — controls which schema objects to suggest.
pub enum SqlContext<'a> {
    /// No schema objects, keywords only (default/unknown position)
    Keyword,
    /// Tables and views (after FROM, JOIN, INTO, UPDATE, TABLE, TRUNCATE)
    Table,
    /// Columns and functions (after SELECT, WHERE, AND, OR, ON, SET, HAVING, etc.)
    ColumnOrFunction,
    /// Columns only (after ORDER BY, GROUP BY)
    Column,
    /// Columns of a specific table (after "tablename.")
    TableColumns(&'a str),
    /// Tables in a specific schema (after "schema.")
    SchemaTables(&'a str),
}

/// Completion engine — tracks filtered candidates and cycling index.
pub struct Completer {
    candidates: Vec<String>,
    index: usize,
    prefix: String,
}

impl Completer {
    pub fn new() -> Self {
        Self {
            candidates: Vec::new(),
            index: 0,
            prefix: String::new(),
        }
    }

    /// Rebuild candidates from `prefix` filtered by SQL context.
    ///
    /// Sources: schema objects first (filtered by `context`),
    /// then SQL keywords as fallback. Returns the ghost-text suffix for the
    /// first match or `None` if no matches.
    pub fn recompute(
        &mut self,
        prefix: &str,
        context: SqlContext<'_>,
        schema: Option<&SchemaTree>,
    ) -> Option<String> {
        self.candidates.clear();
        self.index = 0;
        self.prefix = prefix.to_string();

        // Allow empty prefix for dot-qualified contexts (e.g., "users.")
        let allow_empty = matches!(
            context,
            SqlContext::TableColumns(_) | SqlContext::SchemaTables(_)
        );
        if prefix.is_empty() && !allow_empty {
            return None;
        }

        let prefix_lower = prefix.to_ascii_lowercase();

        // Schema objects — filtered by context
        if let Some(tree) = schema {
            match context {
                SqlContext::Keyword => { /* skip schema objects entirely */ }

                SqlContext::Table => {
                    for s in &tree.schemas {
                        for table in &s.tables {
                            self.try_push(&table.name, &prefix_lower, prefix);
                        }
                        for view in &s.views {
                            self.try_push(&view.name, &prefix_lower, prefix);
                        }
                    }
                }

                SqlContext::ColumnOrFunction => {
                    for s in &tree.schemas {
                        for table in &s.tables {
                            for col in &table.columns {
                                self.try_push(&col.name, &prefix_lower, prefix);
                            }
                        }
                        for view in &s.views {
                            for col in &view.columns {
                                self.try_push(&col.name, &prefix_lower, prefix);
                            }
                        }
                        for func in &s.functions {
                            self.try_push(&func.name, &prefix_lower, prefix);
                        }
                    }
                }

                SqlContext::Column => {
                    for s in &tree.schemas {
                        for table in &s.tables {
                            for col in &table.columns {
                                self.try_push(&col.name, &prefix_lower, prefix);
                            }
                        }
                        for view in &s.views {
                            for col in &view.columns {
                                self.try_push(&col.name, &prefix_lower, prefix);
                            }
                        }
                    }
                }

                SqlContext::TableColumns(table_name) => {
                    let table_lower = table_name.to_ascii_lowercase();
                    for s in &tree.schemas {
                        for table in s.tables.iter().chain(s.views.iter()) {
                            if table.name.to_ascii_lowercase() == table_lower {
                                for col in &table.columns {
                                    self.try_push_dot(&col.name, &prefix_lower);
                                }
                            }
                        }
                    }
                }

                SqlContext::SchemaTables(schema_name) => {
                    let schema_lower = schema_name.to_ascii_lowercase();
                    for s in &tree.schemas {
                        if s.name.to_ascii_lowercase() == schema_lower {
                            for table in &s.tables {
                                self.try_push_dot(&table.name, &prefix_lower);
                            }
                            for view in &s.views {
                                self.try_push_dot(&view.name, &prefix_lower);
                            }
                        }
                    }
                }
            }
        }

        // SQL keywords (always available as fallback)
        if !prefix.is_empty() && self.candidates.len() < MAX_CANDIDATES {
            let keywords = highlight::sql_keywords();
            let mut kw_matches: Vec<&str> = keywords
                .iter()
                .filter(|kw| {
                    kw.len() > prefix.len() && kw.to_ascii_lowercase().starts_with(&prefix_lower)
                })
                .copied()
                .collect();
            kw_matches.sort_unstable();
            for kw in kw_matches {
                if self.candidates.len() >= MAX_CANDIDATES {
                    break;
                }
                if !self.candidates.iter().any(|c| c.eq_ignore_ascii_case(kw)) {
                    self.candidates.push(kw.to_string());
                }
            }
        }

        self.suffix()
    }

    /// Advance to the next candidate (wraps around). Returns suffix.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<String> {
        if self.candidates.is_empty() {
            return None;
        }
        self.index = (self.index + 1) % self.candidates.len();
        self.suffix()
    }

    /// Move to the previous candidate (wraps around). Returns suffix.
    pub fn prev(&mut self) -> Option<String> {
        if self.candidates.is_empty() {
            return None;
        }
        self.index = if self.index == 0 {
            self.candidates.len() - 1
        } else {
            self.index - 1
        };
        self.suffix()
    }

    /// Clear all completion state.
    pub fn clear(&mut self) {
        self.candidates.clear();
        self.index = 0;
        self.prefix.clear();
    }

    /// Whether the completer has active candidates.
    pub fn is_active(&self) -> bool {
        !self.candidates.is_empty()
    }

    /// Ghost-text suffix: the current candidate minus the typed prefix.
    fn suffix(&self) -> Option<String> {
        self.candidates
            .get(self.index)
            .and_then(|c| c.get(self.prefix.len()..).map(|s| s.to_string()))
    }

    /// Push candidate if it's a case-insensitive prefix match and not exact.
    fn try_push(&mut self, name: &str, prefix_lower: &str, prefix: &str) {
        if self.candidates.len() >= MAX_CANDIDATES {
            return;
        }
        if name.len() > prefix.len()
            && name.to_ascii_lowercase().starts_with(prefix_lower)
            && !self.candidates.iter().any(|c| c == name)
        {
            self.candidates.push(name.to_string());
        }
    }

    /// Push candidate for dot-qualified completion (allows empty prefix).
    fn try_push_dot(&mut self, name: &str, prefix_lower: &str) {
        if self.candidates.len() >= MAX_CANDIDATES {
            return;
        }
        if (prefix_lower.is_empty() || name.to_ascii_lowercase().starts_with(prefix_lower))
            && !self.candidates.iter().any(|c| c == name)
        {
            self.candidates.push(name.to_string());
        }
    }
}

impl Default for Completer {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the word immediately before the cursor position.
///
/// Scans backward from `col` to find the word start. Word delimiters are
/// whitespace and `().,;=<>!+-*/'\"`.
pub fn word_before_cursor(line: &str, col: usize) -> &str {
    let col = col.min(line.len());
    if col == 0 || !line.is_char_boundary(col) {
        return "";
    }
    let mut start = col;
    while start > 0 && line.is_char_boundary(start - 1) {
        let b = line.as_bytes()[start - 1];
        if b.is_ascii_whitespace() || b"().,;=<>!+-*/\'\"".contains(&b) {
            break;
        }
        start -= 1;
    }
    &line[start..col]
}

/// Check for a dot-qualifier before the current prefix.
///
/// If the character at `prefix_start - 1` is `.`, returns the word before the
/// dot (e.g., for `"users.na"` with prefix_start=6, returns `Some("users")`).
pub fn dot_qualifier(line: &str, prefix_start: usize) -> Option<&str> {
    if prefix_start == 0 || !line.is_char_boundary(prefix_start) {
        return None;
    }
    if line.as_bytes()[prefix_start - 1] != b'.' {
        return None;
    }
    let dot_pos = prefix_start - 1;
    if dot_pos == 0 {
        return None;
    }
    // Scan backward from the dot to find the qualifier word
    let word = word_before_cursor(line, dot_pos);
    if word.is_empty() { None } else { Some(word) }
}

/// Detect the SQL context by scanning backward through tokens before the cursor.
///
/// Uses the most recent SQL clause keyword to determine what kind of completion
/// is appropriate. With a `dot_qualifier`, checks against schema/table names instead.
pub fn detect_context<'a>(
    text_before_prefix: &str,
    dot_qual: Option<&'a str>,
    schema: Option<&SchemaTree>,
) -> SqlContext<'a> {
    // Dot-qualified: check if qualifier is a schema name or table name
    if let Some(qualifier) = dot_qual {
        if let Some(tree) = schema {
            let q_lower = qualifier.to_ascii_lowercase();
            // Check schemas first
            for s in &tree.schemas {
                if s.name.to_ascii_lowercase() == q_lower {
                    return SqlContext::SchemaTables(qualifier);
                }
            }
            // Then tables/views
            for s in &tree.schemas {
                for table in s.tables.iter().chain(s.views.iter()) {
                    if table.name.to_ascii_lowercase() == q_lower {
                        return SqlContext::TableColumns(qualifier);
                    }
                }
            }
        }
        return SqlContext::Keyword;
    }

    // Tokenize by splitting on whitespace and punctuation, scan backward
    let tokens: Vec<&str> = text_before_prefix
        .split(|c: char| c.is_ascii_whitespace() || "(),;=<>!+-*/'\"".contains(c))
        .filter(|t| !t.is_empty())
        .collect();

    for i in (0..tokens.len()).rev() {
        let upper = tokens[i].to_ascii_uppercase();
        match upper.as_str() {
            "FROM" | "JOIN" | "INTO" | "UPDATE" | "TABLE" | "TRUNCATE" => {
                return SqlContext::Table;
            }
            "SELECT" | "WHERE" | "AND" | "OR" | "ON" | "SET" | "HAVING" | "CASE" | "WHEN"
            | "THEN" | "ELSE" | "RETURNING" => {
                return SqlContext::ColumnOrFunction;
            }
            "BY" => {
                // Look for ORDER/GROUP before BY
                if i > 0 {
                    let prev = tokens[i - 1].to_ascii_uppercase();
                    if prev == "ORDER" || prev == "GROUP" {
                        return SqlContext::Column;
                    }
                }
            }
            _ => continue,
        }
    }

    SqlContext::Keyword
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::{Column, Function, Schema, Table};
    use crate::db::types::DataType;

    // ── word_before_cursor ──────────────────────────────────

    #[test]
    fn word_at_end_of_line() {
        assert_eq!(word_before_cursor("SELECT", 6), "SELECT");
    }

    #[test]
    fn word_after_space() {
        assert_eq!(word_before_cursor("SELECT us", 9), "us");
    }

    #[test]
    fn word_after_paren() {
        assert_eq!(word_before_cursor("COUNT(di", 8), "di");
    }

    #[test]
    fn word_after_dot() {
        assert_eq!(word_before_cursor("public.us", 9), "us");
    }

    #[test]
    fn empty_at_start() {
        assert_eq!(word_before_cursor("hello", 0), "");
    }

    #[test]
    fn empty_after_space() {
        assert_eq!(word_before_cursor("SELECT ", 7), "");
    }

    #[test]
    fn empty_line() {
        assert_eq!(word_before_cursor("", 0), "");
    }

    #[test]
    fn col_beyond_line_length() {
        assert_eq!(word_before_cursor("abc", 10), "abc");
    }

    // ── dot_qualifier ───────────────────────────────────────

    #[test]
    fn dot_qualifier_present() {
        // "users.na" with prefix "na" starting at 6
        assert_eq!(dot_qualifier("users.na", 6), Some("users"));
    }

    #[test]
    fn dot_qualifier_empty_prefix() {
        // "users." with prefix "" starting at 6
        assert_eq!(dot_qualifier("users.", 6), Some("users"));
    }

    #[test]
    fn dot_qualifier_no_dot() {
        assert_eq!(dot_qualifier("SELECT us", 7), None);
    }

    #[test]
    fn dot_qualifier_at_line_start() {
        assert_eq!(dot_qualifier(".foo", 1), None);
    }

    #[test]
    fn dot_qualifier_nothing_before_dot() {
        // Space before dot: "( .foo"
        assert_eq!(dot_qualifier(" .foo", 2), None);
    }

    // ── detect_context ──────────────────────────────────────

    #[test]
    fn context_after_from() {
        assert!(matches!(
            detect_context("SELECT * FROM ", None, None),
            SqlContext::Table
        ));
    }

    #[test]
    fn context_after_select() {
        assert!(matches!(
            detect_context("SELECT ", None, None),
            SqlContext::ColumnOrFunction
        ));
    }

    #[test]
    fn context_after_join() {
        assert!(matches!(
            detect_context("FROM users JOIN ", None, None),
            SqlContext::Table
        ));
    }

    #[test]
    fn context_after_where() {
        assert!(matches!(
            detect_context("SELECT * FROM users WHERE ", None, None),
            SqlContext::ColumnOrFunction
        ));
    }

    #[test]
    fn context_order_by() {
        assert!(matches!(
            detect_context("SELECT * FROM users ORDER BY ", None, None),
            SqlContext::Column
        ));
    }

    #[test]
    fn context_group_by() {
        assert!(matches!(
            detect_context("SELECT count(*) FROM users GROUP BY ", None, None),
            SqlContext::Column
        ));
    }

    #[test]
    fn context_order_by_after_columns() {
        // ORDER and BY are always adjacent tokens; identifiers after BY don't break detection
        assert!(matches!(
            detect_context("SELECT * FROM t ORDER BY col1, ", None, None),
            SqlContext::Column
        ));
    }

    #[test]
    fn context_comma_list_select() {
        // "SELECT col1, col2, " — scanner skips identifiers, finds SELECT
        assert!(matches!(
            detect_context("SELECT col1, col2, ", None, None),
            SqlContext::ColumnOrFunction
        ));
    }

    #[test]
    fn context_multiline() {
        assert!(matches!(
            detect_context("SELECT *\nFROM ", None, None),
            SqlContext::Table
        ));
    }

    #[test]
    fn context_empty_text() {
        assert!(matches!(
            detect_context("", None, None),
            SqlContext::Keyword
        ));
    }

    #[test]
    fn context_unknown_defaults_to_keyword() {
        assert!(matches!(
            detect_context("FOOBAR ", None, None),
            SqlContext::Keyword
        ));
    }

    #[test]
    fn context_dot_schema() {
        let schema = sample_schema();
        assert!(matches!(
            detect_context("", Some("public"), Some(&schema)),
            SqlContext::SchemaTables("public")
        ));
    }

    #[test]
    fn context_dot_table() {
        let schema = sample_schema();
        assert!(matches!(
            detect_context("", Some("users"), Some(&schema)),
            SqlContext::TableColumns("users")
        ));
    }

    #[test]
    fn context_dot_unknown() {
        let schema = sample_schema();
        assert!(matches!(
            detect_context("", Some("nonexistent"), Some(&schema)),
            SqlContext::Keyword
        ));
    }

    #[test]
    fn context_subquery_from() {
        // Subquery: inner FROM should be detected
        assert!(matches!(
            detect_context("WHERE id IN (SELECT name FROM ", None, None),
            SqlContext::Table
        ));
    }

    #[test]
    fn context_after_and() {
        assert!(matches!(
            detect_context("WHERE id = 1 AND ", None, None),
            SqlContext::ColumnOrFunction
        ));
    }

    #[test]
    fn context_case_insensitive() {
        assert!(matches!(
            detect_context("select * from ", None, None),
            SqlContext::Table
        ));
    }

    // ── Completer basics ────────────────────────────────────

    #[test]
    fn recompute_keywords() {
        let mut c = Completer::new();
        let result = c.recompute("SEL", SqlContext::Keyword, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "ECT");
    }

    #[test]
    fn recompute_case_insensitive() {
        let mut c = Completer::new();
        let result = c.recompute("sel", SqlContext::Keyword, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "ECT");
    }

    #[test]
    fn exact_match_excluded() {
        let mut c = Completer::new();
        let result = c.recompute("SELECT", SqlContext::Keyword, None);
        if let Some(suffix) = result {
            assert!(!suffix.is_empty());
        }
    }

    #[test]
    fn empty_prefix_returns_none() {
        let mut c = Completer::new();
        assert!(c.recompute("", SqlContext::Keyword, None).is_none());
        assert!(!c.is_active());
    }

    #[test]
    fn no_match_returns_none() {
        let mut c = Completer::new();
        assert!(c.recompute("zzzzzzz", SqlContext::Keyword, None).is_none());
    }

    // ── Schema objects with context filtering ────────────────

    fn sample_schema() -> SchemaTree {
        SchemaTree {
            schemas: vec![Schema {
                name: "public".to_string(),
                tables: vec![Table {
                    name: "users".to_string(),
                    columns: vec![Column {
                        name: "username".to_string(),
                        data_type: DataType::Text,
                        is_primary_key: false,
                        foreign_key: None,
                    }],
                }],
                views: vec![],
                indexes: vec![],
                functions: vec![Function {
                    name: "update_stats".to_string(),
                    args: "".to_string(),
                    return_type: "void".to_string(),
                }],
            }],
        }
    }

    #[test]
    fn table_context_only_tables() {
        let mut c = Completer::new();
        let schema = sample_schema();
        let result = c.recompute("us", SqlContext::Table, Some(&schema));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "ers"); // "users" table
        // Should NOT include "username" column
        assert!(!c.candidates.iter().any(|c| c == "username"));
    }

    #[test]
    fn column_or_function_context() {
        let mut c = Completer::new();
        let schema = sample_schema();
        let result = c.recompute("us", SqlContext::ColumnOrFunction, Some(&schema));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "ername"); // "username" column
        // Should NOT include "users" table
        assert!(!c.candidates.iter().any(|c| c == "users"));
    }

    #[test]
    fn column_context_no_functions() {
        let mut c = Completer::new();
        let schema = sample_schema();
        c.recompute("update", SqlContext::Column, Some(&schema));
        // "update_stats" is a function — Column context must not include it
        assert!(!c.candidates.iter().any(|c| c == "update_stats"));
    }

    #[test]
    fn keyword_context_skips_schema() {
        let mut c = Completer::new();
        let schema = sample_schema();
        let result = c.recompute("us", SqlContext::Keyword, Some(&schema));
        // Should NOT include "users" (table) or "username" (column)
        assert!(!c.candidates.iter().any(|c| c == "users"));
        assert!(!c.candidates.iter().any(|c| c == "username"));
        // But should still match keywords like USING
        if let Some(suffix) = result {
            assert!(!suffix.is_empty());
        }
    }

    #[test]
    fn dot_table_columns() {
        let mut c = Completer::new();
        let schema = sample_schema();
        // "users." → empty prefix, TableColumns context
        let result = c.recompute("", SqlContext::TableColumns("users"), Some(&schema));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "username");
        assert!(c.candidates.iter().any(|c| c == "username"));
    }

    #[test]
    fn dot_table_columns_with_prefix() {
        let mut c = Completer::new();
        let schema = sample_schema();
        let result = c.recompute("user", SqlContext::TableColumns("users"), Some(&schema));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "name");
    }

    #[test]
    fn dot_schema_tables() {
        let mut c = Completer::new();
        let schema = sample_schema();
        // "public." → empty prefix, SchemaTables context
        let result = c.recompute("", SqlContext::SchemaTables("public"), Some(&schema));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "users");
    }

    #[test]
    fn dot_schema_tables_with_prefix() {
        let mut c = Completer::new();
        let schema = sample_schema();
        let result = c.recompute("us", SqlContext::SchemaTables("public"), Some(&schema));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "ers");
    }

    // ── Cycling ─────────────────────────────────────────────

    #[test]
    fn cycling_wraps_around() {
        let mut c = Completer::new();
        c.recompute("SEL", SqlContext::Keyword, None);
        assert!(c.is_active());
        let first = c.suffix().unwrap();
        for _ in 0..c.candidates.len() {
            c.next();
        }
        assert_eq!(c.suffix().unwrap(), first);
    }

    #[test]
    fn prev_wraps_from_zero() {
        let mut c = Completer::new();
        c.recompute("SEL", SqlContext::Keyword, None);
        assert!(c.is_active());
        let prev_result = c.prev();
        assert!(prev_result.is_some());
        assert_eq!(c.index, c.candidates.len() - 1);
    }

    #[test]
    fn next_on_empty_returns_none() {
        let mut c = Completer::new();
        assert!(c.next().is_none());
    }

    #[test]
    fn prev_on_empty_returns_none() {
        let mut c = Completer::new();
        assert!(c.prev().is_none());
    }

    // ── Max candidates cap ──────────────────────────────────

    #[test]
    fn max_five_candidates() {
        let mut c = Completer::new();
        c.recompute("A", SqlContext::Keyword, None);
        assert!(c.candidates.len() <= MAX_CANDIDATES);
    }

    // ── Clear ───────────────────────────────────────────────

    #[test]
    fn clear_resets_state() {
        let mut c = Completer::new();
        c.recompute("SEL", SqlContext::Keyword, None);
        assert!(c.is_active());
        c.clear();
        assert!(!c.is_active());
        assert!(c.prefix.is_empty());
    }
}

//! Inline ghost-text auto-complete engine
//!
//! Provides SQL keyword and schema-aware completion candidates for the query
//! editor. The completer is a stateful struct owned by `App`; the editor only
//! stores the ghost-text suffix to render.

use crate::db::schema::SchemaTree;
use crate::ui::highlight;

const MAX_CANDIDATES: usize = 5;

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

    /// Rebuild candidates from `prefix` (case-insensitive prefix match).
    ///
    /// Sources: schema objects first (tables, views, columns, functions),
    /// then SQL keywords. Returns the ghost-text suffix for the first match
    /// or `None` if no matches.
    pub fn recompute(&mut self, prefix: &str, schema: Option<&SchemaTree>) -> Option<String> {
        self.candidates.clear();
        self.index = 0;
        self.prefix = prefix.to_string();

        if prefix.is_empty() {
            return None;
        }

        let prefix_lower = prefix.to_ascii_lowercase();

        // Schema objects first
        if let Some(tree) = schema {
            for s in &tree.schemas {
                for table in &s.tables {
                    self.try_push(&table.name, &prefix_lower, prefix);
                    for col in &table.columns {
                        self.try_push(&col.name, &prefix_lower, prefix);
                    }
                }
                for view in &s.views {
                    self.try_push(&view.name, &prefix_lower, prefix);
                    for col in &view.columns {
                        self.try_push(&col.name, &prefix_lower, prefix);
                    }
                }
                for func in &s.functions {
                    self.try_push(&func.name, &prefix_lower, prefix);
                }
            }
        }

        // SQL keywords
        if self.candidates.len() < MAX_CANDIDATES {
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
                // Avoid duplicating schema objects that happen to match
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
        if name.len() > prefix.len() && name.to_ascii_lowercase().starts_with(prefix_lower) {
            // Avoid duplicates
            if !self.candidates.iter().any(|c| c == name) {
                self.candidates.push(name.to_string());
            }
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

    // ── Completer basics ────────────────────────────────────

    #[test]
    fn recompute_keywords() {
        let mut c = Completer::new();
        let result = c.recompute("SEL", None);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "ECT");
    }

    #[test]
    fn recompute_case_insensitive() {
        let mut c = Completer::new();
        let result = c.recompute("sel", None);
        assert!(result.is_some());
        // Should match the keyword "SELECT" and return the suffix preserving its casing
        assert_eq!(result.unwrap(), "ECT");
    }

    #[test]
    fn exact_match_excluded() {
        let mut c = Completer::new();
        // "SELECT" exactly should not suggest itself
        let result = c.recompute("SELECT", None);
        // Should have other matches starting with SELECT but not SELECT itself
        // (or none if no longer keywords start with SELECT)
        if let Some(suffix) = result {
            assert!(!suffix.is_empty());
        }
    }

    #[test]
    fn empty_prefix_returns_none() {
        let mut c = Completer::new();
        assert!(c.recompute("", None).is_none());
        assert!(!c.is_active());
    }

    #[test]
    fn no_match_returns_none() {
        let mut c = Completer::new();
        assert!(c.recompute("zzzzzzz", None).is_none());
    }

    // ── Schema objects rank before keywords ──────────────────

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
    fn schema_objects_before_keywords() {
        let mut c = Completer::new();
        let schema = sample_schema();
        let result = c.recompute("us", Some(&schema));
        assert!(result.is_some());
        // First candidate should be the table "users", not keyword "USING"
        assert_eq!(result.unwrap(), "ers");
    }

    #[test]
    fn schema_function_match() {
        let mut c = Completer::new();
        let schema = sample_schema();
        let result = c.recompute("update", Some(&schema));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "_stats");
    }

    #[test]
    fn schema_column_match() {
        let mut c = Completer::new();
        let schema = sample_schema();
        let result = c.recompute("usern", Some(&schema));
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "ame");
    }

    // ── Cycling ─────────────────────────────────────────────

    #[test]
    fn cycling_wraps_around() {
        let mut c = Completer::new();
        c.recompute("SEL", None);
        assert!(c.is_active());
        let first = c.suffix().unwrap();
        // Cycle through all candidates and come back to the first
        for _ in 0..c.candidates.len() {
            c.next();
        }
        assert_eq!(c.suffix().unwrap(), first);
    }

    #[test]
    fn prev_wraps_from_zero() {
        let mut c = Completer::new();
        c.recompute("SEL", None);
        assert!(c.is_active());
        // prev from index 0 wraps to last
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
        // "A" should match many keywords — ensure capped at 5
        c.recompute("A", None);
        assert!(c.candidates.len() <= MAX_CANDIDATES);
    }

    // ── Clear ───────────────────────────────────────────────

    #[test]
    fn clear_resets_state() {
        let mut c = Completer::new();
        c.recompute("SEL", None);
        assert!(c.is_active());
        c.clear();
        assert!(!c.is_active());
        assert!(c.prefix.is_empty());
    }
}

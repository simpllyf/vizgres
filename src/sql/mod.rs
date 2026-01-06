//! SQL utilities
//!
//! Formatting, syntax highlighting, and autocomplete for SQL.

pub mod completer;
pub mod formatter;

pub use completer::SqlCompleter;
pub use formatter::format_sql;

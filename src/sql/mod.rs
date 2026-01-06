//! SQL utilities
//!
//! Formatting, syntax highlighting, and autocomplete for SQL.

pub mod completer;
pub mod formatter;

#[allow(unused_imports)]
pub use completer::SqlCompleter;
#[allow(unused_imports)]
pub use formatter::format_sql;

//! SQL formatting
//!
//! Formats SQL queries using the sqlformat crate.

use sqlformat::{format, FormatOptions, Indent, QueryParams};

/// Format a SQL query string
///
/// # Examples
/// ```ignore
/// let formatted = format_sql("select * from users where id=1");
/// // Returns nicely formatted SQL with proper indentation
/// ```
pub fn format_sql(sql: &str) -> String {
    let options = FormatOptions {
        indent: Indent::Spaces(2),
        uppercase: true,
        lines_between_queries: 2,
    };

    format(sql, &QueryParams::None, options)
}

/// Format SQL with custom indentation
pub fn format_sql_with_indent(sql: &str, indent_size: u8) -> String {
    let options = FormatOptions {
        indent: Indent::Spaces(indent_size),
        uppercase: true,
        lines_between_queries: 2,
    };

    format(sql, &QueryParams::None, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_simple_query() {
        let input = "select * from users";
        let formatted = format_sql(input);
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("FROM"));
    }

    #[test]
    fn test_format_with_where() {
        let input = "select id,name from users where active=true";
        let formatted = format_sql(input);
        assert!(formatted.contains("WHERE"));
    }

    #[test]
    fn test_format_with_custom_indent() {
        let input = "select * from users";
        let formatted = format_sql_with_indent(input, 4);
        assert!(formatted.contains("SELECT"));
    }
}

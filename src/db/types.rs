//! Database type definitions
//!
//! Core data structures for representing database query results,
//! data types, and values.

use std::time::Duration;

/// Query execution results
#[derive(Debug, Clone)]
pub struct QueryResults {
    /// Column definitions
    pub columns: Vec<ColumnDef>,
    /// Result rows
    pub rows: Vec<Row>,
    /// Query execution time
    pub execution_time: Duration,
    /// Total row count (may differ from rows.len() if limited)
    pub row_count: usize,
    /// Whether results were truncated due to row limit
    pub truncated: bool,
}

impl QueryResults {
    /// Create a new QueryResults, validating column-row alignment in debug builds.
    pub fn new(
        columns: Vec<ColumnDef>,
        rows: Vec<Row>,
        execution_time: Duration,
        row_count: usize,
    ) -> Self {
        debug_assert!(
            rows.iter().all(|r| r.values.len() == columns.len()),
            "every row must have exactly as many values as there are columns ({} columns)",
            columns.len(),
        );
        Self {
            columns,
            rows,
            execution_time,
            row_count,
            truncated: false,
        }
    }

    /// Create a new QueryResults with truncation flag.
    pub fn new_truncated(
        columns: Vec<ColumnDef>,
        rows: Vec<Row>,
        execution_time: Duration,
        row_count: usize,
        truncated: bool,
    ) -> Self {
        debug_assert!(
            rows.iter().all(|r| r.values.len() == columns.len()),
            "every row must have exactly as many values as there are columns ({} columns)",
            columns.len(),
        );
        Self {
            columns,
            rows,
            execution_time,
            row_count,
            truncated,
        }
    }
}

/// Column definition in query results
#[derive(Debug, Clone)]
pub struct ColumnDef {
    /// Column name
    pub name: String,
    /// Data type
    pub data_type: DataType,
    /// Whether column can contain NULL
    pub nullable: bool,
}

/// Database data types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataType {
    // Integer types
    SmallInt,
    Integer,
    BigInt,

    // Floating point
    Real,
    Double,
    Numeric,

    // Text types
    Text,
    Varchar(Option<usize>),
    Char(Option<usize>),

    // Boolean
    Boolean,

    // Date/time types
    Date,
    Time,
    Timestamp,
    TimestampTz,
    Interval,

    // JSON types
    Json,
    Jsonb,

    // Binary data
    Bytea,

    // UUID
    Uuid,

    // Array type
    Array(Box<DataType>),

    // Other/unknown types
    Unknown(String),
}

/// A single row of query results
#[derive(Debug, Clone)]
pub struct Row {
    /// Cell values in column order
    pub values: Vec<CellValue>,
}

/// A cell value (single column value in a row)
#[derive(Debug, Clone)]
pub enum CellValue {
    /// NULL value
    Null,

    /// Integer value
    Integer(i64),

    /// Floating point value
    Float(f64),

    /// Text/string value
    Text(String),

    /// Boolean value
    Boolean(bool),

    /// JSON value (stored as compact JSON string for efficient display)
    Json(String),

    /// Binary data
    Binary(Vec<u8>),

    /// Date/time value (stored as string for now)
    DateTime(String),

    /// UUID value
    Uuid(String),

    /// Array value
    Array(Vec<CellValue>),
}

impl DataType {
    /// Get a human-readable display name for this type
    pub fn display_name(&self) -> String {
        match self {
            DataType::SmallInt => "smallint".to_string(),
            DataType::Integer => "integer".to_string(),
            DataType::BigInt => "bigint".to_string(),
            DataType::Real => "real".to_string(),
            DataType::Double => "double precision".to_string(),
            DataType::Numeric => "numeric".to_string(),
            DataType::Text => "text".to_string(),
            DataType::Varchar(Some(n)) => format!("varchar({})", n),
            DataType::Varchar(None) => "varchar".to_string(),
            DataType::Char(Some(n)) => format!("char({})", n),
            DataType::Char(None) => "char".to_string(),
            DataType::Boolean => "boolean".to_string(),
            DataType::Date => "date".to_string(),
            DataType::Time => "time".to_string(),
            DataType::Timestamp => "timestamp".to_string(),
            DataType::TimestampTz => "timestamptz".to_string(),
            DataType::Interval => "interval".to_string(),
            DataType::Json => "json".to_string(),
            DataType::Jsonb => "jsonb".to_string(),
            DataType::Bytea => "bytea".to_string(),
            DataType::Uuid => "uuid".to_string(),
            DataType::Array(inner) => format!("{}[]", inner.display_name()),
            DataType::Unknown(s) => s.clone(),
        }
    }
}

/// Truncate a string with "..." suffix, by display width (not bytes/chars).
fn truncate_with_ellipsis(s: &str, max_cols: usize) -> String {
    crate::ui::unicode::truncate_to_width(s, max_cols)
}

impl CellValue {
    /// Get a display string for this cell value (truncated if needed).
    ///
    /// String-backed types (Text, Json, DateTime, Uuid) avoid full cloning
    /// when the value exceeds `max_len` — only the needed prefix is copied.
    pub fn display_string(&self, max_len: usize) -> String {
        let full = match self {
            CellValue::Null => return "NULL".to_string(),
            CellValue::Integer(i) => i.to_string(),
            CellValue::Float(f) => f.to_string(),
            // String-backed types: avoid cloning the full string when truncating
            CellValue::Text(s)
            | CellValue::Json(s)
            | CellValue::DateTime(s)
            | CellValue::Uuid(s) => {
                if crate::ui::unicode::display_width(s) <= max_len {
                    return s.clone();
                }
                return truncate_with_ellipsis(s, max_len);
            }
            CellValue::Boolean(b) => b.to_string(),
            CellValue::Binary(b) => format!("<binary {} bytes>", b.len()),
            CellValue::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| v.display_string(max_len)).collect();
                format!("{{{}}}", items.join(","))
            }
        };

        if crate::ui::unicode::display_width(&full) > max_len {
            truncate_with_ellipsis(&full, max_len)
        } else {
            full
        }
    }

    /// Check if this is a NULL value
    pub fn is_null(&self) -> bool {
        matches!(self, CellValue::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datatype_display_name() {
        assert_eq!(DataType::Integer.display_name(), "integer");
        assert_eq!(DataType::Varchar(Some(255)).display_name(), "varchar(255)");
        assert_eq!(
            DataType::Array(Box::new(DataType::Integer)).display_name(),
            "integer[]"
        );
    }

    #[test]
    fn test_cell_value_display_string() {
        let val = CellValue::Text("Hello, world!".to_string());
        assert_eq!(val.display_string(5), "He...");
        assert_eq!(val.display_string(100), "Hello, world!");
    }

    #[test]
    fn test_cell_value_is_null() {
        assert!(CellValue::Null.is_null());
        assert!(!CellValue::Integer(42).is_null());
    }

    #[test]
    fn test_array_display_string() {
        let arr = CellValue::Array(vec![
            CellValue::Text("a".to_string()),
            CellValue::Text("b".to_string()),
        ]);
        assert_eq!(arr.display_string(100), "{a,b}");
    }

    #[test]
    fn test_array_display_truncates() {
        let arr = CellValue::Array(vec![
            CellValue::Text("hello".to_string()),
            CellValue::Text("world".to_string()),
        ]);
        let display = arr.display_string(10);
        assert!(display.len() <= 13); // some overshoot from joining is ok
    }

    #[test]
    fn test_query_results_new_valid() {
        let results = QueryResults::new(
            vec![ColumnDef {
                name: "x".to_string(),
                data_type: DataType::Integer,
                nullable: false,
            }],
            vec![Row {
                values: vec![CellValue::Integer(1)],
            }],
            Duration::from_millis(1),
            1,
        );
        assert_eq!(results.columns.len(), 1);
        assert_eq!(results.rows.len(), 1);
    }

    #[test]
    #[should_panic(expected = "every row must have exactly as many values")]
    #[cfg(debug_assertions)]
    fn test_query_results_new_misaligned_panics() {
        QueryResults::new(
            vec![
                ColumnDef {
                    name: "a".to_string(),
                    data_type: DataType::Integer,
                    nullable: false,
                },
                ColumnDef {
                    name: "b".to_string(),
                    data_type: DataType::Text,
                    nullable: false,
                },
            ],
            vec![Row {
                values: vec![CellValue::Integer(1)], // only 1 value for 2 columns
            }],
            Duration::from_millis(1),
            1,
        );
    }

    #[test]
    fn test_json_display_string_no_truncation() {
        let val = CellValue::Json(r#"{"key":"value"}"#.to_string());
        assert_eq!(val.display_string(100), r#"{"key":"value"}"#);
    }

    #[test]
    fn test_json_display_string_truncated() {
        let val = CellValue::Json(serde_json::json!({"key": "value", "other": "data"}).to_string());
        let display = val.display_string(15);
        assert!(
            display.ends_with("..."),
            "should end with ellipsis: {display}"
        );
        assert!(display.len() <= 15, "should respect max_len: {display}");
    }

    #[test]
    fn test_json_display_large_value_is_efficient() {
        // Build a large JSON string (~100KB) — display_string is pure truncation, no parsing
        let mut obj = serde_json::Map::new();
        for i in 0..1000 {
            obj.insert(
                format!("key_{i}"),
                serde_json::Value::String("x".repeat(100)),
            );
        }
        let val = CellValue::Json(serde_json::Value::Object(obj).to_string());

        let display = val.display_string(40);
        assert!(display.len() <= 40);
        assert!(display.ends_with("..."));
    }

    #[test]
    fn test_json_display_empty_object() {
        let val = CellValue::Json("{}".to_string());
        assert_eq!(val.display_string(100), "{}");
    }

    #[test]
    fn test_json_display_nested() {
        let val = CellValue::Json(serde_json::json!({"a":{"b":{"c":1}}}).to_string());
        let full = val.display_string(100);
        assert_eq!(full, r#"{"a":{"b":{"c":1}}}"#);
    }

    #[test]
    fn test_text_display_utf8_truncation() {
        // Multi-byte chars should not panic on truncation
        let val = CellValue::Text("café au lait".to_string());
        let display = val.display_string(7);
        assert!(display.ends_with("..."));
        // Should not panic or produce invalid UTF-8
        assert!(display.len() <= 10); // may be fewer bytes due to char boundary
    }
}

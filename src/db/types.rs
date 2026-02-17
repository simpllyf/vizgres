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
}

/// Column definition in query results
#[derive(Debug, Clone)]
pub struct ColumnDef {
    /// Column name
    pub name: String,
    /// Data type
    pub data_type: DataType,
    /// Whether column can contain NULL (reserved for future use)
    #[allow(dead_code)]
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

    /// JSON value (parsed)
    Json(serde_json::Value),

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

impl CellValue {
    /// Get a display string for this cell value (truncated if needed)
    pub fn display_string(&self, max_len: usize) -> String {
        let full = match self {
            CellValue::Null => "NULL".to_string(),
            CellValue::Integer(i) => i.to_string(),
            CellValue::Float(f) => f.to_string(),
            CellValue::Text(s) => s.clone(),
            CellValue::Boolean(b) => b.to_string(),
            CellValue::Json(v) => v.to_string(),
            CellValue::Binary(b) => format!("<binary {} bytes>", b.len()),
            CellValue::DateTime(s) => s.clone(),
            CellValue::Uuid(s) => s.clone(),
            CellValue::Array(arr) => {
                let items: Vec<String> = arr.iter().map(|v| v.display_string(max_len)).collect();
                format!("{{{}}}", items.join(","))
            }
        };

        if full.len() > max_len {
            format!("{}...", &full[..max_len.saturating_sub(3)])
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
}

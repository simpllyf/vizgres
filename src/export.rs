//! Query results export (CSV / JSON)
//!
//! Pure serialization functions — no filesystem I/O. The caller writes the
//! returned string to disk.

use crate::db::types::{CellValue, QueryResults};

/// Export format selector
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
}

impl ExportFormat {
    /// File extension for this format (without leading dot)
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Json => "json",
        }
    }
}

/// Serialize query results as RFC 4180 CSV.
pub fn to_csv(results: &QueryResults) -> String {
    let mut out = String::new();

    // Header row
    for (i, col) in results.columns.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        csv_escape_into(&mut out, &col.name);
    }
    out.push('\n');

    // Data rows
    for row in &results.rows {
        for (i, cell) in row.values.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let text = cell_to_export_string(cell);
            csv_escape_into(&mut out, &text);
        }
        out.push('\n');
    }

    out
}

/// Serialize query results as a JSON array of objects with typed values.
pub fn to_json(results: &QueryResults) -> String {
    let col_names: Vec<&str> = results.columns.iter().map(|c| c.name.as_str()).collect();

    let rows: Vec<serde_json::Value> = results
        .rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for (i, cell) in row.values.iter().enumerate() {
                let key = col_names.get(i).copied().unwrap_or("?");
                obj.insert(key.to_string(), cell_to_json(cell));
            }
            serde_json::Value::Object(obj)
        })
        .collect();

    serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".to_string())
}

/// Full untruncated value string for CSV export (NULL → empty string).
fn cell_to_export_string(cell: &CellValue) -> String {
    match cell {
        CellValue::Null => String::new(),
        CellValue::Integer(i) => i.to_string(),
        CellValue::Float(f) => f.to_string(),
        CellValue::Text(s) => s.clone(),
        CellValue::Boolean(b) => b.to_string(),
        CellValue::Json(v) => v.to_string(),
        CellValue::Binary(b) => hex_encode(b),
        CellValue::DateTime(s) => s.clone(),
        CellValue::Uuid(s) => s.clone(),
        CellValue::Array(arr) => {
            let items: Vec<String> = arr.iter().map(cell_to_export_string).collect();
            format!("{{{}}}", items.join(","))
        }
    }
}

/// Convert a CellValue to a serde_json::Value with type preservation.
fn cell_to_json(cell: &CellValue) -> serde_json::Value {
    match cell {
        CellValue::Null => serde_json::Value::Null,
        CellValue::Integer(i) => serde_json::json!(*i),
        CellValue::Float(f) => {
            if f.is_finite() {
                serde_json::json!(*f)
            } else {
                // NaN / Infinity aren't valid JSON numbers
                serde_json::Value::String(f.to_string())
            }
        }
        CellValue::Text(s) => serde_json::Value::String(s.clone()),
        CellValue::Boolean(b) => serde_json::Value::Bool(*b),
        CellValue::Json(v) => v.clone(),
        CellValue::Binary(b) => serde_json::Value::String(hex_encode(b)),
        CellValue::DateTime(s) => serde_json::Value::String(s.clone()),
        CellValue::Uuid(s) => serde_json::Value::String(s.clone()),
        CellValue::Array(arr) => serde_json::Value::Array(arr.iter().map(cell_to_json).collect()),
    }
}

/// Quote a field if it contains `,` `"` or a newline (RFC 4180).
fn csv_escape_into(out: &mut String, field: &str) {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        out.push('"');
        for c in field.chars() {
            if c == '"' {
                out.push_str("\"\"");
            } else {
                out.push(c);
            }
        }
        out.push('"');
    } else {
        out.push_str(field);
    }
}

/// Hex-encode binary data (e.g. `\xdeadbeef`).
fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(2 + bytes.len() * 2);
    s.push_str("\\x");
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::types::{ColumnDef, DataType, Row};
    use std::time::Duration;

    fn sample_results() -> QueryResults {
        QueryResults::new(
            vec![
                ColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    nullable: false,
                },
                ColumnDef {
                    name: "name".to_string(),
                    data_type: DataType::Text,
                    nullable: true,
                },
            ],
            vec![
                Row {
                    values: vec![CellValue::Integer(1), CellValue::Text("Alice".to_string())],
                },
                Row {
                    values: vec![CellValue::Integer(2), CellValue::Text("Bob".to_string())],
                },
            ],
            Duration::from_millis(42),
            2,
        )
    }

    #[test]
    fn test_format_extension() {
        assert_eq!(ExportFormat::Csv.extension(), "csv");
        assert_eq!(ExportFormat::Json.extension(), "json");
    }

    #[test]
    fn test_basic_csv() {
        let csv = to_csv(&sample_results());
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "id,name");
        assert_eq!(lines[1], "1,Alice");
        assert_eq!(lines[2], "2,Bob");
    }

    #[test]
    fn test_csv_escaping_commas() {
        let results = QueryResults::new(
            vec![ColumnDef {
                name: "val".to_string(),
                data_type: DataType::Text,
                nullable: false,
            }],
            vec![Row {
                values: vec![CellValue::Text("a,b".to_string())],
            }],
            Duration::from_millis(1),
            1,
        );
        let csv = to_csv(&results);
        assert!(csv.contains("\"a,b\""));
    }

    #[test]
    fn test_csv_escaping_quotes() {
        let results = QueryResults::new(
            vec![ColumnDef {
                name: "val".to_string(),
                data_type: DataType::Text,
                nullable: false,
            }],
            vec![Row {
                values: vec![CellValue::Text("say \"hi\"".to_string())],
            }],
            Duration::from_millis(1),
            1,
        );
        let csv = to_csv(&results);
        assert!(csv.contains("\"say \"\"hi\"\"\""));
    }

    #[test]
    fn test_csv_escaping_newlines() {
        let results = QueryResults::new(
            vec![ColumnDef {
                name: "val".to_string(),
                data_type: DataType::Text,
                nullable: false,
            }],
            vec![Row {
                values: vec![CellValue::Text("line1\nline2".to_string())],
            }],
            Duration::from_millis(1),
            1,
        );
        let csv = to_csv(&results);
        assert!(csv.contains("\"line1\nline2\""));
    }

    #[test]
    fn test_csv_null_is_empty() {
        let results = QueryResults::new(
            vec![ColumnDef {
                name: "val".to_string(),
                data_type: DataType::Text,
                nullable: true,
            }],
            vec![Row {
                values: vec![CellValue::Null],
            }],
            Duration::from_millis(1),
            1,
        );
        let csv = to_csv(&results);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[1], "");
    }

    #[test]
    fn test_csv_empty_results() {
        let results = QueryResults::new(
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
            vec![],
            Duration::from_millis(1),
            0,
        );
        let csv = to_csv(&results);
        assert_eq!(csv, "a,b\n");
    }

    #[test]
    fn test_json_typed_values() {
        let json_str = to_json(&sample_results());
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["id"], 1);
        assert_eq!(parsed[0]["name"], "Alice");
        assert_eq!(parsed[1]["id"], 2);
        assert_eq!(parsed[1]["name"], "Bob");
    }

    #[test]
    fn test_json_null_and_bool() {
        let results = QueryResults::new(
            vec![
                ColumnDef {
                    name: "flag".to_string(),
                    data_type: DataType::Boolean,
                    nullable: false,
                },
                ColumnDef {
                    name: "missing".to_string(),
                    data_type: DataType::Text,
                    nullable: true,
                },
            ],
            vec![Row {
                values: vec![CellValue::Boolean(true), CellValue::Null],
            }],
            Duration::from_millis(1),
            1,
        );
        let json_str = to_json(&results);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed[0]["flag"], true);
        assert!(parsed[0]["missing"].is_null());
    }

    #[test]
    fn test_json_passthrough() {
        let inner = serde_json::json!({"nested": [1, 2, 3]});
        let results = QueryResults::new(
            vec![ColumnDef {
                name: "data".to_string(),
                data_type: DataType::Jsonb,
                nullable: false,
            }],
            vec![Row {
                values: vec![CellValue::Json(inner.clone())],
            }],
            Duration::from_millis(1),
            1,
        );
        let json_str = to_json(&results);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed[0]["data"], inner);
    }

    #[test]
    fn test_json_arrays() {
        let results = QueryResults::new(
            vec![ColumnDef {
                name: "tags".to_string(),
                data_type: DataType::Array(Box::new(DataType::Text)),
                nullable: false,
            }],
            vec![Row {
                values: vec![CellValue::Array(vec![
                    CellValue::Text("a".to_string()),
                    CellValue::Text("b".to_string()),
                ])],
            }],
            Duration::from_millis(1),
            1,
        );
        let json_str = to_json(&results);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed[0]["tags"], serde_json::json!(["a", "b"]));
    }

    #[test]
    fn test_json_empty_results() {
        let results = QueryResults::new(
            vec![ColumnDef {
                name: "x".to_string(),
                data_type: DataType::Integer,
                nullable: false,
            }],
            vec![],
            Duration::from_millis(1),
            0,
        );
        let json_str = to_json(&results);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_csv_binary_hex_encoded() {
        let results = QueryResults::new(
            vec![ColumnDef {
                name: "data".to_string(),
                data_type: DataType::Bytea,
                nullable: false,
            }],
            vec![Row {
                values: vec![CellValue::Binary(vec![0xde, 0xad, 0xbe, 0xef])],
            }],
            Duration::from_millis(1),
            1,
        );
        let csv = to_csv(&results);
        assert!(csv.contains("\\xdeadbeef"));
    }

    #[test]
    fn test_json_nan_infinity_as_strings() {
        let results = QueryResults::new(
            vec![
                ColumnDef {
                    name: "a".to_string(),
                    data_type: DataType::Double,
                    nullable: false,
                },
                ColumnDef {
                    name: "b".to_string(),
                    data_type: DataType::Double,
                    nullable: false,
                },
                ColumnDef {
                    name: "c".to_string(),
                    data_type: DataType::Double,
                    nullable: false,
                },
            ],
            vec![Row {
                values: vec![
                    CellValue::Float(f64::NAN),
                    CellValue::Float(f64::INFINITY),
                    CellValue::Float(f64::NEG_INFINITY),
                ],
            }],
            Duration::from_millis(1),
            1,
        );
        // Should not panic and should produce valid JSON
        let json_str = to_json(&results);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed[0]["a"], "NaN");
        assert_eq!(parsed[0]["b"], "inf");
        assert_eq!(parsed[0]["c"], "-inf");
    }

    #[test]
    fn test_csv_nan_infinity() {
        let results = QueryResults::new(
            vec![ColumnDef {
                name: "x".to_string(),
                data_type: DataType::Double,
                nullable: false,
            }],
            vec![Row {
                values: vec![CellValue::Float(f64::NAN)],
            }],
            Duration::from_millis(1),
            1,
        );
        let csv = to_csv(&results);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[1], "NaN");
    }

    #[test]
    fn test_csv_header_needs_escaping() {
        let results = QueryResults::new(
            vec![ColumnDef {
                name: "col,name".to_string(),
                data_type: DataType::Text,
                nullable: false,
            }],
            vec![],
            Duration::from_millis(1),
            0,
        );
        let csv = to_csv(&results);
        assert_eq!(csv, "\"col,name\"\n");
    }
}

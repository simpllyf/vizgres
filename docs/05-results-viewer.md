# 05 - Results Viewer

> Table rendering, scrolling, cell selection, JSONB display, and data export.

---

## Overview

The Results Viewer displays query results in a tabular format with support for scrolling, column resizing, cell selection, and detailed inspection of complex values like JSONB.

---

## Visual Layout

### Standard View

```
╔═══════════════════════════════════════════════════════════════════════════╗
║ Results                                             [42 rows, 23ms] ▼ ║
╠════════╤════════════╤══════════════════════════╤═════════════════════════╣
║ id     │ name       │ email                    │ metadata                ║
╠════════╪════════════╪══════════════════════════╪═════════════════════════╣
║ 1      │ Alice      │ alice@example.com        │ {"role": "admin", ...   ║
║ 2      │ Bob        │ bob@test.org             │ {"role": "user", "...   ║
║ 3      │ Charlie    │ charlie@demo.io          │ null                    ║
║ 4      │ Diana      │ diana@sample.net         │ {"role": "moderato...   ║
║ █5█    │ █Edward █  │ █edward@example.com█     │ █{"role": "user"}█      ║
║ 6      │ Fiona      │ fiona@test.com           │ {"role": "user", "...   ║
╚════════╧════════════╧══════════════════════════╧══════════════════[1-6/42]╝
                                                                     ▲
                                             Row range / total ──────┘
```

### Components

| Element | Description |
|---------|-------------|
| Title bar | "Results" with row count and execution time |
| Column headers | Column names, sortable (click or key) |
| Data rows | Query result data |
| Selected row | Highlighted background |
| Selected cell | Inverse colors (for cell inspector) |
| Footer | Current row range and total count |

---

## Data Model

### Results State

```rust
pub struct ResultsViewer {
    // Data
    results: Option<QueryResults>,

    // Selection
    selected_row: usize,
    selected_col: usize,
    selection_mode: SelectionMode,

    // Scrolling
    scroll_row: usize,
    scroll_col: usize,
    viewport_rows: usize,
    viewport_cols: usize,

    // Column sizing
    column_widths: Vec<usize>,
    auto_resize: bool,

    // Sorting
    sort_column: Option<usize>,
    sort_direction: SortDirection,
}

pub enum SelectionMode {
    Row,   // Entire row selected
    Cell,  // Single cell selected (for inspection)
}

pub enum SortDirection {
    Ascending,
    Descending,
}

pub struct QueryResults {
    pub columns: Vec<ColumnDef>,
    pub rows: Vec<Row>,
    pub execution_time: Duration,
    pub affected_rows: Option<u64>,  // For INSERT/UPDATE/DELETE
}
```

---

## Navigation

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `←` / `h` | Move selection left (cell mode) / Scroll left (row mode) |
| `→` / `l` | Move selection right (cell mode) / Scroll right (row mode) |
| `Home` | Jump to first row |
| `End` | Jump to last row |
| `Ctrl+Home` | Jump to first cell (top-left) |
| `Ctrl+End` | Jump to last cell (bottom-right) |
| `PageUp` | Scroll up one page |
| `PageDown` | Scroll down one page |
| `Tab` | Toggle between row/cell selection mode |
| `Enter` | Open cell inspector popup |
| `c` | Copy cell value |
| `y` | Copy entire row (as CSV) |
| `s` | Sort by current column |
| `S` | Reverse sort |

### Navigation Logic

```rust
impl ResultsViewer {
    pub fn move_down(&mut self) {
        if let Some(results) = &self.results {
            if self.selected_row < results.rows.len() - 1 {
                self.selected_row += 1;
                self.ensure_row_visible();
            }
        }
    }

    pub fn move_right(&mut self) {
        if let Some(results) = &self.results {
            match self.selection_mode {
                SelectionMode::Cell => {
                    if self.selected_col < results.columns.len() - 1 {
                        self.selected_col += 1;
                        self.ensure_col_visible();
                    }
                }
                SelectionMode::Row => {
                    // Horizontal scroll
                    self.scroll_col = (self.scroll_col + 1)
                        .min(results.columns.len().saturating_sub(1));
                }
            }
        }
    }

    fn ensure_row_visible(&mut self) {
        if self.selected_row < self.scroll_row {
            self.scroll_row = self.selected_row;
        } else if self.selected_row >= self.scroll_row + self.viewport_rows {
            self.scroll_row = self.selected_row - self.viewport_rows + 1;
        }
    }
}
```

---

## Column Sizing

### Auto-Size Algorithm

```rust
impl ResultsViewer {
    fn calculate_column_widths(&mut self, available_width: u16) {
        if let Some(results) = &self.results {
            let col_count = results.columns.len();

            // Calculate content-based widths
            let mut widths: Vec<usize> = results.columns.iter().enumerate()
                .map(|(i, col)| {
                    let header_width = col.name.len();
                    let max_content = results.rows.iter()
                        .map(|row| self.display_width(&row.values[i]))
                        .max()
                        .unwrap_or(0);
                    header_width.max(max_content).max(4).min(50)
                })
                .collect();

            // Distribute remaining space proportionally
            let total_content: usize = widths.iter().sum();
            let separators = col_count + 1;  // │ between columns
            let available = available_width as usize - separators;

            if total_content < available {
                // Expand columns proportionally
                let extra = available - total_content;
                for (i, width) in widths.iter_mut().enumerate() {
                    let share = extra * (*width) / total_content;
                    *width += share;
                }
            }

            self.column_widths = widths;
        }
    }

    fn display_width(&self, value: &CellValue) -> usize {
        match value {
            CellValue::Null => 4,  // "null"
            CellValue::Text(s) => s.chars().count().min(50),
            CellValue::Integer(n) => n.to_string().len(),
            CellValue::Float(f) => format!("{:.2}", f).len(),
            CellValue::Boolean(b) => if *b { 4 } else { 5 },
            CellValue::Json(_) => 20,  // Truncated preview
            CellValue::Binary(b) => format!("({} bytes)", b.len()).len(),
        }
    }
}
```

### Manual Resize

```rust
// Future: Mouse drag on column separator
// For now: Commands to adjust width

impl ResultsViewer {
    pub fn resize_column(&mut self, col: usize, delta: i16) {
        if col < self.column_widths.len() {
            let new_width = (self.column_widths[col] as i16 + delta).max(4) as usize;
            self.column_widths[col] = new_width.min(100);
        }
    }
}
```

---

## Cell Value Display

### Type-Specific Rendering

| Type | Display | Style |
|------|---------|-------|
| NULL | `null` | Dim gray, italic |
| Integer | Right-aligned | Default |
| Float | Right-aligned, 2 decimals | Default |
| Boolean | `true` / `false` | Green / Red |
| Text | Left-aligned, truncated | Default |
| JSON/JSONB | Compact preview | Cyan |
| Timestamp | ISO format | Default |
| Binary | `(N bytes)` | Dim |
| UUID | Full value | Default |
| Array | `[a, b, c]` | Default |

### Truncation

```rust
fn render_cell(&self, value: &CellValue, width: usize) -> String {
    let text = self.format_value(value);

    if text.chars().count() <= width {
        format!("{:width$}", text, width = width)
    } else {
        let truncated: String = text.chars().take(width - 1).collect();
        format!("{}…", truncated)
    }
}

fn format_value(&self, value: &CellValue) -> String {
    match value {
        CellValue::Null => "null".to_string(),
        CellValue::Integer(n) => n.to_string(),
        CellValue::Float(f) => format!("{:.2}", f),
        CellValue::Boolean(b) => b.to_string(),
        CellValue::Text(s) => s.replace('\n', "↵"),
        CellValue::Json(v) => {
            // Compact single-line preview
            serde_json::to_string(v).unwrap_or_default()
        }
        CellValue::Binary(b) => format!("({} bytes)", b.len()),
    }
}
```

---

## Cell Inspector Popup

### Purpose
Display full content of selected cell, with special formatting for JSONB and long text.

### Visual Layout

```
┌───────────────────────┬─────────────────────────────────────────────────┐
│ Results               │ Cell: metadata (jsonb)                          │
│ ─────────────────────│─────────────────────────────────────────────────│
│ id │ name │ metadata │ {                                               │
│  1 │ Ali… │ {"role…  │   "role": "admin",                              │
│  2 │ Bob… │ {"role…  │   "permissions": [                              │
│ █3█│█Cha…█│█{"role…█ │     "read",                                     │
│  4 │ Dia… │ {"role…  │     "write",                                    │
│                       │     "delete"                                    │
│                       │   ],                                            │
│                       │   "settings": {                                 │
│                       │     "theme": "dark",                            │
│                       │     "notifications": true                       │
│                       │   }                                             │
│                       │ }                                               │
│                       │                                          [Esc] │
└───────────────────────┴─────────────────────────────────────────────────┘
```

### Popup Behavior

```rust
pub struct CellInspector {
    visible: bool,
    column_name: String,
    column_type: String,
    content: String,
    content_type: ContentType,
    scroll_offset: usize,
    total_lines: usize,
}

pub enum ContentType {
    PlainText,
    Json,
    Xml,
    Binary,
}

impl CellInspector {
    pub fn open(&mut self, col: &ColumnDef, value: &CellValue) {
        self.visible = true;
        self.column_name = col.name.clone();
        self.column_type = format!("{:?}", col.data_type).to_lowercase();
        self.scroll_offset = 0;

        match value {
            CellValue::Json(v) => {
                self.content_type = ContentType::Json;
                self.content = serde_json::to_string_pretty(v)
                    .unwrap_or_else(|_| v.to_string());
            }
            CellValue::Text(s) => {
                self.content_type = if s.trim_start().starts_with('{') || s.trim_start().starts_with('[') {
                    // Try to parse as JSON
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
                        self.content = serde_json::to_string_pretty(&v).unwrap();
                        ContentType::Json
                    } else {
                        self.content = s.clone();
                        ContentType::PlainText
                    }
                } else {
                    self.content = s.clone();
                    ContentType::PlainText
                };
            }
            CellValue::Binary(b) => {
                self.content_type = ContentType::Binary;
                self.content = hex_dump(b);
            }
            other => {
                self.content_type = ContentType::PlainText;
                self.content = format!("{:?}", other);
            }
        }

        self.total_lines = self.content.lines().count();
    }

    pub fn close(&mut self) {
        self.visible = false;
    }
}
```

### JSON Syntax Highlighting

```rust
fn highlight_json(content: &str) -> Vec<Line> {
    content.lines().map(|line| {
        let mut spans = vec![];
        // Tokenize JSON:
        // - Keys: cyan
        // - Strings: orange
        // - Numbers: green
        // - Booleans/null: magenta
        // - Brackets/braces: white
        Line::from(spans)
    }).collect()
}
```

### Popup Keyboard Controls

| Key | Action |
|-----|--------|
| `Escape` / `Enter` / `q` | Close popup |
| `↑` / `k` | Scroll up |
| `↓` / `j` | Scroll down |
| `PageUp` | Scroll up one page |
| `PageDown` | Scroll down one page |
| `Home` | Scroll to top |
| `End` | Scroll to bottom |
| `c` / `y` | Copy content to clipboard |

---

## Sorting

### Visual Indicator

```
╔════════╤════════════▼╤══════════════════════════╗
║ id     │ name        │ email                    ║
╠════════╪════════════╪══════════════════════════╣
         ▲
         └── Sort indicator (▲ ascending, ▼ descending)
```

### Sorting Logic

```rust
impl ResultsViewer {
    pub fn sort_by_column(&mut self, col: usize) {
        if let Some(results) = &mut self.results {
            if self.sort_column == Some(col) {
                // Toggle direction
                self.sort_direction = match self.sort_direction {
                    SortDirection::Ascending => SortDirection::Descending,
                    SortDirection::Descending => SortDirection::Ascending,
                };
            } else {
                self.sort_column = Some(col);
                self.sort_direction = SortDirection::Ascending;
            }

            results.rows.sort_by(|a, b| {
                let cmp = self.compare_values(&a.values[col], &b.values[col]);
                match self.sort_direction {
                    SortDirection::Ascending => cmp,
                    SortDirection::Descending => cmp.reverse(),
                }
            });
        }
    }

    fn compare_values(&self, a: &CellValue, b: &CellValue) -> std::cmp::Ordering {
        match (a, b) {
            (CellValue::Null, CellValue::Null) => std::cmp::Ordering::Equal,
            (CellValue::Null, _) => std::cmp::Ordering::Less,  // Nulls first
            (_, CellValue::Null) => std::cmp::Ordering::Greater,
            (CellValue::Integer(a), CellValue::Integer(b)) => a.cmp(b),
            (CellValue::Float(a), CellValue::Float(b)) => {
                a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
            }
            (CellValue::Text(a), CellValue::Text(b)) => a.cmp(b),
            // ... other type comparisons
        }
    }
}
```

---

## Export

### Export Formats

| Format | Command | Extension |
|--------|---------|-----------|
| CSV | `:export csv` | `.csv` |
| JSON | `:export json` | `.json` |
| SQL INSERT | `:export sql` | `.sql` |
| Markdown | `:export md` | `.md` |

### Export Implementation

```rust
pub enum ExportFormat {
    Csv,
    Json,
    SqlInsert,
    Markdown,
}

impl ResultsViewer {
    pub fn export(&self, format: ExportFormat) -> String {
        let results = match &self.results {
            Some(r) => r,
            None => return String::new(),
        };

        match format {
            ExportFormat::Csv => self.export_csv(results),
            ExportFormat::Json => self.export_json(results),
            ExportFormat::SqlInsert => self.export_sql(results),
            ExportFormat::Markdown => self.export_markdown(results),
        }
    }

    fn export_csv(&self, results: &QueryResults) -> String {
        let mut output = String::new();

        // Header
        let headers: Vec<_> = results.columns.iter()
            .map(|c| escape_csv(&c.name))
            .collect();
        output.push_str(&headers.join(","));
        output.push('\n');

        // Rows
        for row in &results.rows {
            let values: Vec<_> = row.values.iter()
                .map(|v| escape_csv(&format_value_for_export(v)))
                .collect();
            output.push_str(&values.join(","));
            output.push('\n');
        }

        output
    }

    fn export_json(&self, results: &QueryResults) -> String {
        let rows: Vec<_> = results.rows.iter().map(|row| {
            let obj: serde_json::Map<String, serde_json::Value> = results.columns.iter()
                .zip(row.values.iter())
                .map(|(col, val)| (col.name.clone(), cell_to_json(val)))
                .collect();
            serde_json::Value::Object(obj)
        }).collect();

        serde_json::to_string_pretty(&rows).unwrap_or_default()
    }
}
```

---

## EXPLAIN Plan Viewer

### Special Results Mode

When `Ctrl+E` (EXPLAIN) is executed, results are displayed as a tree:

```
╔═══════════════════════════════════════════════════════════════════════════╗
║ EXPLAIN ANALYZE                                              [Total: 45ms]║
╠═══════════════════════════════════════════════════════════════════════════╣
║ ▼ Limit (cost=0.29..8.31 rows=10 width=72) (actual time=0.03..0.05 rows=10║
║   ▼ Index Scan using users_created_at_idx on users                        ║
║       (cost=0.29..802.30 rows=10000 width=72)                             ║
║       (actual time=0.03..0.04 rows=10 loops=1)                            ║
║       Index Cond: (created_at > '2024-01-01')                             ║
║       Rows Removed by Filter: 0                                           ║
║                                                                           ║
║ Planning Time: 0.15 ms                                                    ║
║ Execution Time: 0.08 ms                                                   ║
╚═══════════════════════════════════════════════════════════════════════════╝
```

### EXPLAIN Parsing

```rust
pub struct ExplainNode {
    pub node_type: String,
    pub relation: Option<String>,
    pub alias: Option<String>,
    pub startup_cost: f64,
    pub total_cost: f64,
    pub plan_rows: u64,
    pub plan_width: u64,
    pub actual_time: Option<(f64, f64)>,
    pub actual_rows: Option<u64>,
    pub actual_loops: Option<u64>,
    pub children: Vec<ExplainNode>,
    pub extra_info: Vec<String>,
}

fn parse_explain_json(json: &serde_json::Value) -> ExplainNode {
    // Parse PostgreSQL EXPLAIN (FORMAT JSON) output
    // Recursively build tree structure
}
```

---

## Empty/Error States

### No Results

```
╔═══════════════════════════════════════════════════════════════════════════╗
║ Results                                                                   ║
╠═══════════════════════════════════════════════════════════════════════════╣
║                                                                           ║
║                         No results to display                             ║
║                                                                           ║
║               Execute a query with Ctrl+Enter                             ║
║                                                                           ║
╚═══════════════════════════════════════════════════════════════════════════╝
```

### Query Error

```
╔═══════════════════════════════════════════════════════════════════════════╗
║ Results                                                           [ERROR] ║
╠═══════════════════════════════════════════════════════════════════════════╣
║                                                                           ║
║  ERROR:  relation "userz" does not exist                                  ║
║  LINE 1: SELECT * FROM userz                                              ║
║                        ^                                                  ║
║                                                                           ║
║  HINT: Perhaps you meant "users"?                                         ║
║                                                                           ║
╚═══════════════════════════════════════════════════════════════════════════╝
```

### Affected Rows (Non-SELECT)

```
╔═══════════════════════════════════════════════════════════════════════════╗
║ Results                                                          [15ms]  ║
╠═══════════════════════════════════════════════════════════════════════════╣
║                                                                           ║
║                     UPDATE completed successfully                         ║
║                                                                           ║
║                         42 rows affected                                  ║
║                                                                           ║
╚═══════════════════════════════════════════════════════════════════════════╝
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_move_down_increments_row() {
        let mut viewer = ResultsViewer::new();
        viewer.results = Some(test_results(10, 3));
        viewer.selected_row = 0;

        viewer.move_down();

        assert_eq!(viewer.selected_row, 1);
    }

    #[test]
    fn test_navigation_move_down_stops_at_last_row() {
        let mut viewer = ResultsViewer::new();
        viewer.results = Some(test_results(10, 3));
        viewer.selected_row = 9;

        viewer.move_down();

        assert_eq!(viewer.selected_row, 9);
    }

    #[test]
    fn test_column_width_calculation_respects_min_width() {
        let mut viewer = ResultsViewer::new();
        viewer.results = Some(QueryResults {
            columns: vec![ColumnDef { name: "id".to_string(), .. }],
            rows: vec![Row { values: vec![CellValue::Integer(1)] }],
            ..
        });

        viewer.calculate_column_widths(100);

        assert!(viewer.column_widths[0] >= 4);  // Minimum width
    }

    #[test]
    fn test_sort_by_column_toggles_direction() {
        let mut viewer = ResultsViewer::new();
        viewer.results = Some(test_results(5, 2));

        viewer.sort_by_column(0);
        assert_eq!(viewer.sort_direction, SortDirection::Ascending);

        viewer.sort_by_column(0);
        assert_eq!(viewer.sort_direction, SortDirection::Descending);
    }

    #[test]
    fn test_cell_display_truncates_long_text() {
        let viewer = ResultsViewer::new();
        let value = CellValue::Text("This is a very long text value".to_string());

        let display = viewer.render_cell(&value, 10);

        assert_eq!(display, "This is a…");
    }

    #[test]
    fn test_null_display_shows_null_text() {
        let viewer = ResultsViewer::new();
        let display = viewer.render_cell(&CellValue::Null, 10);

        assert_eq!(display.trim(), "null");
    }
}
```

### Export Tests

```rust
#[test]
fn test_export_csv_includes_headers() {
    let viewer = ResultsViewer::new();
    viewer.results = Some(test_results_with_columns(&["id", "name"]));

    let csv = viewer.export(ExportFormat::Csv);

    assert!(csv.starts_with("id,name\n"));
}

#[test]
fn test_export_csv_escapes_commas() {
    let mut viewer = ResultsViewer::new();
    viewer.results = Some(QueryResults {
        columns: vec![ColumnDef { name: "text".to_string(), .. }],
        rows: vec![Row { values: vec![CellValue::Text("hello, world".to_string())] }],
        ..
    });

    let csv = viewer.export(ExportFormat::Csv);

    assert!(csv.contains("\"hello, world\""));
}

#[test]
fn test_export_json_creates_valid_json() {
    let viewer = ResultsViewer::new();
    viewer.results = Some(test_results(3, 2));

    let json = viewer.export(ExportFormat::Json);

    assert!(serde_json::from_str::<serde_json::Value>(&json).is_ok());
}
```

### Cell Inspector Tests

```rust
#[test]
fn test_cell_inspector_formats_json_pretty() {
    let mut inspector = CellInspector::new();
    let value = CellValue::Json(serde_json::json!({"key": "value"}));

    inspector.open(&test_column("data"), &value);

    assert!(inspector.content.contains('\n'));  // Multi-line
    assert!(inspector.content.contains("  "));  // Indented
}

#[test]
fn test_cell_inspector_detects_json_in_text() {
    let mut inspector = CellInspector::new();
    let value = CellValue::Text(r#"{"key": "value"}"#.to_string());

    inspector.open(&test_column("data"), &value);

    assert_eq!(inspector.content_type, ContentType::Json);
}
```

### Snapshot Tests

```rust
#[test]
fn test_results_render_with_data() {
    let viewer = ResultsViewer::new();
    viewer.results = Some(test_results(5, 3));

    let output = render_results(&viewer, Rect::new(0, 0, 60, 10));
    insta::assert_snapshot!(output);
}

#[test]
fn test_results_render_with_selection() {
    let mut viewer = ResultsViewer::new();
    viewer.results = Some(test_results(5, 3));
    viewer.selected_row = 2;
    viewer.selection_mode = SelectionMode::Row;

    let output = render_results(&viewer, Rect::new(0, 0, 60, 10));
    insta::assert_snapshot!(output);
}

#[test]
fn test_results_render_with_null_values() {
    let mut viewer = ResultsViewer::new();
    viewer.results = Some(results_with_nulls());

    let output = render_results(&viewer, Rect::new(0, 0, 60, 10));
    insta::assert_snapshot!(output);
}
```

---

## Performance Considerations

1. **Virtual Scrolling**: Only render visible rows
2. **Lazy Column Width**: Calculate on first render, cache
3. **Streaming Results**: For large result sets, fetch in chunks
4. **Limit Default**: Always suggest LIMIT if not present

```rust
// Virtual row rendering
fn render_visible_rows(&self, frame: &mut Frame, area: Rect) {
    let start_row = self.scroll_row;
    let end_row = (self.scroll_row + self.viewport_rows)
        .min(self.results.as_ref().map(|r| r.rows.len()).unwrap_or(0));

    for (i, row_idx) in (start_row..end_row).enumerate() {
        self.render_row(frame, area, row_idx, i);
    }
}
```

---

## Next Steps

See [06-command-bar.md](./06-command-bar.md) for command palette details.

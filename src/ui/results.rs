//! Query results viewer widget
//!
//! Displays query results in a scrollable table with cell-level selection.

use crate::db::types::{CellValue, QueryResults};
use crate::ui::Component;
use crate::ui::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::cell::Cell;

/// Display mode for query results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Standard horizontal table layout
    Table,
    /// Vertical key-value layout showing one row at a time
    Vertical,
}

/// Pagination display info passed from App to ResultsViewer
#[derive(Debug, Clone)]
pub struct PaginationInfo {
    /// Row offset of the current page (0 for first page)
    pub page_offset: usize,
    /// Whether more rows exist beyond this page
    pub has_more: bool,
    /// Whether we can go to a previous page
    pub has_prev: bool,
}

/// Results table viewer
pub struct ResultsViewer {
    results: Option<QueryResults>,
    selected_row: usize,
    selected_col: usize,
    scroll_offset: usize,
    h_scroll_offset: usize,
    /// Computed column widths
    col_widths: Vec<u16>,
    /// Last query error (shown in results area)
    error: Option<String>,
    /// Current display mode
    view_mode: ViewMode,
    /// Pagination info for footer display
    pagination: Option<PaginationInfo>,
    /// Visible height for adaptive page jumps (updated during render)
    page_height: Cell<usize>,
}

impl ResultsViewer {
    pub fn new() -> Self {
        Self {
            results: None,
            selected_row: 0,
            selected_col: 0,
            scroll_offset: 0,
            h_scroll_offset: 0,
            col_widths: Vec::new(),
            error: None,
            view_mode: ViewMode::Table,
            pagination: None,
            page_height: Cell::new(20),
        }
    }

    pub fn set_results(&mut self, results: QueryResults) {
        self.col_widths = compute_column_widths(&results);
        self.results = Some(results);
        self.error = None;
        self.selected_row = 0;
        self.selected_col = 0;
        self.scroll_offset = 0;
        self.h_scroll_offset = 0;
    }

    /// Set an error to display in the results area
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.results = None;
    }

    /// Clear results (reserved for future use)
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.results = None;
        self.selected_row = 0;
        self.selected_col = 0;
        self.col_widths.clear();
    }

    /// Access the underlying query results (for export)
    pub fn results(&self) -> Option<&QueryResults> {
        self.results.as_ref()
    }

    /// Get text of the selected cell
    pub fn selected_cell_text(&self) -> Option<String> {
        let results = self.results.as_ref()?;
        let row = results.rows.get(self.selected_row)?;
        let cell = row.values.get(self.selected_col)?;
        Some(cell.display_string(10000))
    }

    /// Get full cell info (value string, column name, data type display) for the inspector
    pub fn selected_cell_info(&self) -> Option<(String, String, String)> {
        let results = self.results.as_ref()?;
        let row = results.rows.get(self.selected_row)?;
        let cell = row.values.get(self.selected_col)?;
        let col_def = results.columns.get(self.selected_col)?;

        let value = match cell {
            CellValue::Json(s) => {
                // Parse compact JSON string and pretty-print for the inspector
                serde_json::from_str::<serde_json::Value>(s)
                    .and_then(|v| serde_json::to_string_pretty(&v))
                    .unwrap_or_else(|_| s.clone())
            }
            other => other.display_string(100000),
        };

        Some((
            value,
            col_def.name.clone(),
            col_def.data_type.display_name(),
        ))
    }

    /// Get tab-separated values of the selected row
    pub fn selected_row_text(&self) -> Option<String> {
        let results = self.results.as_ref()?;
        let row = results.rows.get(self.selected_row)?;
        let parts: Vec<String> = row.values.iter().map(|v| v.display_string(10000)).collect();
        Some(parts.join("\t"))
    }

    /// Toggle between table and vertical view modes
    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Table => ViewMode::Vertical,
            ViewMode::Vertical => ViewMode::Table,
        };
    }

    /// Current view mode
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    pub fn move_up(&mut self) {
        if self.selected_row > 0 {
            self.selected_row -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let count = self.row_count();
        if count > 0 && self.selected_row < count - 1 {
            self.selected_row += 1;
        }
    }

    pub fn move_left(&mut self) {
        if self.selected_col > 0 {
            self.selected_col -= 1;
        }
    }

    pub fn move_right(&mut self) {
        let count = self.col_count();
        if self.selected_col < count.saturating_sub(1) {
            self.selected_col += 1;
        }
    }

    /// Set pagination info for footer display
    pub fn set_pagination(&mut self, info: Option<PaginationInfo>) {
        self.pagination = info;
    }

    /// Current pagination info
    pub fn pagination(&self) -> Option<&PaginationInfo> {
        self.pagination.as_ref()
    }

    pub fn page_up(&mut self) {
        let height = self.page_height.get();
        self.selected_row = self.selected_row.saturating_sub(height);
    }

    pub fn page_down(&mut self) {
        let height = self.page_height.get();
        let count = self.row_count();
        self.selected_row = (self.selected_row + height).min(count.saturating_sub(1));
    }

    pub fn go_to_top(&mut self) {
        self.selected_row = 0;
    }

    pub fn go_to_bottom(&mut self) {
        let count = self.row_count();
        self.selected_row = count.saturating_sub(1);
    }

    pub fn go_to_home(&mut self) {
        self.selected_col = 0;
        self.h_scroll_offset = 0;
    }

    pub fn go_to_end(&mut self) {
        let count = self.col_count();
        self.selected_col = count.saturating_sub(1);
    }

    /// Widen the currently selected column by a fixed step
    pub fn widen_column(&mut self) {
        if self.selected_col < self.col_widths.len() {
            let w = &mut self.col_widths[self.selected_col];
            *w = (*w + 4).min(200);
        }
    }

    /// Narrow the currently selected column by a fixed step
    pub fn narrow_column(&mut self) {
        if self.selected_col < self.col_widths.len() {
            let w = &mut self.col_widths[self.selected_col];
            *w = (*w).saturating_sub(4).max(4);
        }
    }

    /// Reset all column widths to auto-computed values
    pub fn reset_column_widths(&mut self) {
        if let Some(ref results) = self.results {
            self.col_widths = compute_column_widths(results);
        }
    }

    fn row_count(&self) -> usize {
        self.results.as_ref().map_or(0, |r| r.rows.len())
    }

    fn col_count(&self) -> usize {
        self.results.as_ref().map_or(0, |r| r.columns.len())
    }

    #[allow(dead_code)]
    fn ensure_visible(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        if self.selected_row < self.scroll_offset {
            self.scroll_offset = self.selected_row;
        } else if self.selected_row >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected_row - visible_height + 1;
        }
    }
}

impl Default for ResultsViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ResultsViewer {
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, theme: &Theme) {
        // Show error if present
        if let Some(ref error) = self.error {
            let lines: Vec<Line> = vec![
                Line::from(Span::styled("Query Error", theme.results_error_title)),
                Line::from(""),
                Line::from(Span::styled(error.as_str(), theme.results_error_text)),
            ];
            let p = Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false });
            frame.render_widget(p, area);
            return;
        }

        let results = match &self.results {
            Some(r) if !r.columns.is_empty() => r,
            _ => {
                let msg = if self.results.is_some() {
                    "Query returned no columns"
                } else {
                    "No results yet. Write a query and press F5 to execute."
                };
                let p = Paragraph::new(msg).style(theme.results_empty);
                frame.render_widget(p, area);
                return;
            }
        };

        if area.height < 2 || area.width < 5 {
            return;
        }

        if self.view_mode == ViewMode::Vertical {
            render_vertical(frame, area, self, results, focused, theme);
            return;
        }

        let visible_height = (area.height as usize).saturating_sub(2); // header + footer
        self.page_height.set(visible_height.max(1));
        let viewer = self;

        // Ensure selected row is visible
        let scroll_offset = if viewer.selected_row < viewer.scroll_offset {
            viewer.selected_row
        } else if viewer.selected_row >= viewer.scroll_offset + visible_height {
            viewer.selected_row - visible_height + 1
        } else {
            viewer.scroll_offset
        };

        let col_widths = &self.col_widths;

        // Auto-adjust h_scroll to keep selected_col visible
        let h_scroll = {
            let mut hs = viewer.h_scroll_offset;
            // If selected column is before the scroll offset, snap left
            if viewer.selected_col < hs {
                hs = viewer.selected_col;
            } else {
                // Check if selected_col fits in viewport from current offset
                let mut x: u16 = 0;
                let mut visible = false;
                for ci in hs..col_widths.len() {
                    let w = col_widths.get(ci).copied().unwrap_or(10);
                    if ci == viewer.selected_col {
                        visible = x + w <= area.width;
                        break;
                    }
                    x += w + 1;
                    if x >= area.width {
                        break;
                    }
                }
                if !visible {
                    // Scroll right: find min offset that shows selected_col
                    let mut new_hs = viewer.selected_col;
                    let mut total: u16 = col_widths.get(viewer.selected_col).copied().unwrap_or(10);
                    while new_hs > 0 {
                        let prev_w = col_widths.get(new_hs - 1).copied().unwrap_or(10);
                        if total + prev_w + 1 > area.width {
                            break;
                        }
                        total += prev_w + 1;
                        new_hs -= 1;
                    }
                    hs = new_hs;
                }
            }
            hs
        };

        // Render header row
        let header_y = area.y;
        let mut x = area.x;
        for (col_idx, col_def) in results.columns.iter().enumerate().skip(h_scroll) {
            if x >= area.x + area.width {
                break;
            }
            let w = col_widths
                .get(col_idx)
                .copied()
                .unwrap_or(10)
                .min(area.x + area.width - x);
            let style = if focused && col_idx == viewer.selected_col {
                theme.results_header_selected
            } else {
                theme.results_header
            };
            // Show "name: type" in header for better context
            let header_text = format!("{}: {}", col_def.name, col_def.data_type.display_name());
            let header = truncate_str(&header_text, w as usize);
            let padded = format!("{:<width$}", header, width = w as usize);
            frame.render_widget(
                Paragraph::new(padded).style(style),
                Rect::new(x, header_y, w, 1),
            );
            x += w + 1; // +1 for column separator
        }

        // Render rows
        for vis_row in 0..visible_height {
            let row_idx = scroll_offset + vis_row;
            let y = area.y + 1 + vis_row as u16;
            if y >= area.y + area.height - 1 {
                break;
            }

            if row_idx >= results.rows.len() {
                break;
            }

            let row = &results.rows[row_idx];
            let is_selected_row = row_idx == viewer.selected_row;
            let row_base_style = if vis_row % 2 == 0 {
                theme.results_row_even
            } else {
                theme.results_row_odd
            };

            let mut x = area.x;
            for (col_idx, cell) in row.values.iter().enumerate().skip(h_scroll) {
                if x >= area.x + area.width {
                    break;
                }
                let w = col_widths
                    .get(col_idx)
                    .copied()
                    .unwrap_or(10)
                    .min(area.x + area.width - x);

                let style = if focused && is_selected_row && col_idx == viewer.selected_col {
                    theme.results_selected
                } else if cell.is_null() {
                    theme.results_null
                } else {
                    row_base_style
                };

                let text = cell.display_string(w as usize);
                let padded = format!("{:<width$}", text, width = w as usize);
                frame.render_widget(Paragraph::new(padded).style(style), Rect::new(x, y, w, 1));
                x += w + 1;
            }
        }

        // Footer with row count, pagination, and timing
        let footer_y = area.y + area.height - 1;
        let footer = build_footer(viewer, results);
        let footer_style = theme.results_footer;
        frame.render_widget(
            Paragraph::new(footer).style(footer_style),
            Rect::new(area.x, footer_y, area.width, 1),
        );
    }
}

/// Build footer text with pagination-aware row display
fn build_footer(viewer: &ResultsViewer, results: &QueryResults) -> String {
    let time_ms = results.execution_time.as_secs_f64() * 1000.0;
    let col_info = format!("Col {}/{}", viewer.selected_col + 1, results.columns.len());

    let row_info = if let Some(ref pg) = viewer.pagination {
        if results.rows.is_empty() {
            "0 rows".to_string()
        } else {
            let start = pg.page_offset + 1;
            let end = pg.page_offset + results.rows.len();
            let more = if pg.has_more { "+" } else { "" };
            let mut hints = Vec::new();
            if pg.has_more {
                hints.push("n=more");
            }
            if pg.has_prev {
                hints.push("p=prev");
            }
            let hint_str = if hints.is_empty() {
                String::new()
            } else {
                format!(" | {}", hints.join(" "))
            };
            format!("Rows {}-{} of {}{}", start, end, end, more) + &hint_str
        }
    } else {
        let truncated_suffix = if results.truncated { "+" } else { "" };
        format!(
            "Row {}/{}{}",
            viewer.selected_row + 1,
            results.row_count,
            truncated_suffix,
        )
    };

    format!("{} | {} | {:.1}ms", row_info, col_info, time_ms)
}

/// Compute column widths based on header names and data
fn compute_column_widths(results: &QueryResults) -> Vec<u16> {
    let mut widths: Vec<u16> = results
        .columns
        .iter()
        .map(|c| c.name.len() as u16 + 1)
        .collect();

    // Sample first 100 rows to determine widths
    for row in results.rows.iter().take(100) {
        for (i, cell) in row.values.iter().enumerate() {
            if i < widths.len() {
                let cell_width = cell.display_string(50).len() as u16 + 1;
                widths[i] = widths[i].max(cell_width);
            }
        }
    }

    // Cap widths
    for w in &mut widths {
        *w = (*w).clamp(4, 40);
    }

    widths
}

/// Truncate a string to max characters (not bytes), adding "..." if truncated.
fn truncate_str(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else if max > 3 {
        let truncated: String = s.chars().take(max - 3).collect();
        format!("{}...", truncated)
    } else {
        s.chars().take(max).collect()
    }
}

/// Render results in vertical mode: one row at a time as column_name │ value pairs.
fn render_vertical(
    frame: &mut Frame,
    area: Rect,
    viewer: &ResultsViewer,
    results: &QueryResults,
    focused: bool,
    theme: &Theme,
) {
    // Layout: 1 line header, N field lines, 1 line footer
    let visible_fields = (area.height as usize).saturating_sub(2);
    if visible_fields == 0 {
        return;
    }

    let row = match results.rows.get(viewer.selected_row) {
        Some(r) => r,
        None => {
            let p = Paragraph::new("No rows").style(theme.results_empty);
            frame.render_widget(p, area);
            return;
        }
    };

    // Header: "Row N/M (vertical)"
    let truncated_suffix = if results.truncated { "+" } else { "" };
    let header = format!(
        " Row {}/{}{} \u{2500} vertical view (v to toggle)",
        viewer.selected_row + 1,
        results.row_count,
        truncated_suffix,
    );
    let header_style = if focused {
        theme.results_header
    } else {
        theme.results_footer
    };
    frame.render_widget(
        Paragraph::new(header).style(header_style),
        Rect::new(area.x, area.y, area.width, 1),
    );

    // Compute label width (longest column name)
    let label_width = results
        .columns
        .iter()
        .map(|c| c.name.len())
        .max()
        .unwrap_or(0)
        .min(area.width as usize / 3);

    // Compute field scroll offset to keep selected_col visible
    let field_offset = if viewer.selected_col >= visible_fields {
        viewer.selected_col - visible_fields + 1
    } else {
        0
    };

    let separator = "\u{2502}"; // │
    let value_width = (area.width as usize).saturating_sub(label_width + 3); // " │ "

    for vis in 0..visible_fields {
        let col_idx = field_offset + vis;
        let y = area.y + 1 + vis as u16;
        if y >= area.y + area.height - 1 {
            break;
        }
        if col_idx >= results.columns.len() {
            break;
        }

        let col_def = &results.columns[col_idx];
        let cell = match row.values.get(col_idx) {
            Some(c) => c,
            None => break,
        };

        let is_selected = focused && col_idx == viewer.selected_col;
        let label = format!("{:>width$}", col_def.name, width = label_width);
        let value = cell.display_string(value_width);
        let padded_value = format!("{:<width$}", value, width = value_width);

        let label_style = if is_selected {
            theme.results_selected
        } else {
            theme.results_header
        };
        let sep_style = theme.results_footer;
        let value_style = if is_selected {
            theme.results_selected
        } else if cell.is_null() {
            theme.results_null
        } else if col_idx % 2 == 0 {
            theme.results_row_even
        } else {
            theme.results_row_odd
        };

        // Render: label │ value
        let label_w = label_width as u16;
        let sep_w = 3u16; // " │ "
        let val_w = area.width.saturating_sub(label_w + sep_w);

        if label_w > 0 {
            frame.render_widget(
                Paragraph::new(label).style(label_style),
                Rect::new(area.x, y, label_w.min(area.width), 1),
            );
        }
        if label_w + sep_w <= area.width {
            frame.render_widget(
                Paragraph::new(format!(" {} ", separator)).style(sep_style),
                Rect::new(
                    area.x + label_w,
                    y,
                    sep_w.min(area.width.saturating_sub(label_w)),
                    1,
                ),
            );
        }
        if val_w > 0 {
            frame.render_widget(
                Paragraph::new(padded_value).style(value_style),
                Rect::new(area.x + label_w + sep_w, y, val_w, 1),
            );
        }
    }

    // Footer
    let footer_y = area.y + area.height - 1;
    let footer = format!(
        "Field {}/{} | {:.1}ms | \u{2191}\u{2193}=rows \u{2190}\u{2192}=fields",
        viewer.selected_col + 1,
        results.columns.len(),
        results.execution_time.as_secs_f64() * 1000.0,
    );
    frame.render_widget(
        Paragraph::new(footer).style(theme.results_footer),
        Rect::new(area.x, footer_y, area.width, 1),
    );
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
    fn test_results_viewer_new() {
        let viewer = ResultsViewer::new();
        assert!(viewer.results.is_none());
        assert_eq!(viewer.selected_row, 0);
    }

    #[test]
    fn test_set_results() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        assert!(viewer.results.is_some());
        assert_eq!(viewer.row_count(), 2);
        assert_eq!(viewer.col_count(), 2);
    }

    #[test]
    fn test_selected_cell_text() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        assert_eq!(viewer.selected_cell_text(), Some("1".to_string()));
        viewer.selected_col = 1;
        assert_eq!(viewer.selected_cell_text(), Some("Alice".to_string()));
    }

    #[test]
    fn test_selected_row_text() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        assert_eq!(viewer.selected_row_text(), Some("1\tAlice".to_string()));
    }

    fn json_results() -> QueryResults {
        QueryResults::new(
            vec![
                ColumnDef {
                    name: "id".to_string(),
                    data_type: DataType::Integer,
                    nullable: false,
                },
                ColumnDef {
                    name: "data".to_string(),
                    data_type: DataType::Jsonb,
                    nullable: false,
                },
            ],
            vec![Row {
                values: vec![
                    CellValue::Integer(1),
                    CellValue::Json(r#"{"name":"Alice","scores":[10,20]}"#.to_string()),
                ],
            }],
            Duration::from_millis(5),
            1,
        )
    }

    #[test]
    fn test_selected_cell_info_json_pretty_prints() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(json_results());
        viewer.selected_col = 1; // JSON column
        let (value, col_name, data_type) = viewer.selected_cell_info().unwrap();
        assert_eq!(col_name, "data");
        assert_eq!(data_type, "jsonb");
        // Should be pretty-printed (contains newlines), not compact
        assert!(
            value.contains('\n'),
            "JSON should be pretty-printed: {value}"
        );
        // Should be valid JSON when re-parsed
        let parsed: serde_json::Value = serde_json::from_str(&value).unwrap();
        assert_eq!(parsed["name"], "Alice");
    }

    #[test]
    fn test_selected_cell_text_json_compact() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(json_results());
        viewer.selected_col = 1;
        let text = viewer.selected_cell_text().unwrap();
        // selected_cell_text uses display_string, which returns compact JSON
        assert!(!text.contains('\n'), "should be compact: {text}");
        assert!(text.contains("Alice"));
    }

    #[test]
    fn test_set_error_clears_results() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        assert!(viewer.results.is_some());

        viewer.set_error("relation \"foo\" does not exist".to_string());
        assert!(viewer.results.is_none());
        assert!(viewer.error.is_some());
    }

    #[test]
    fn test_set_results_clears_error() {
        let mut viewer = ResultsViewer::new();
        viewer.set_error("some error".to_string());
        assert!(viewer.error.is_some());

        viewer.set_results(sample_results());
        assert!(viewer.error.is_none());
        assert!(viewer.results.is_some());
    }

    #[test]
    fn test_navigation_on_empty_results() {
        let mut viewer = ResultsViewer::new();
        // All navigation should be safe on empty state
        viewer.move_up();
        viewer.move_down();
        viewer.move_left();
        viewer.move_right();
        viewer.page_up();
        viewer.page_down();
        viewer.go_to_top();
        viewer.go_to_bottom();
        viewer.go_to_home();
        viewer.go_to_end();
        assert_eq!(viewer.selected_row, 0);
        assert_eq!(viewer.selected_col, 0);
    }

    #[test]
    fn test_navigation_boundary_clamping() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results()); // 2 rows, 2 cols

        // move_down stops at last row
        viewer.move_down();
        assert_eq!(viewer.selected_row, 1);
        viewer.move_down();
        assert_eq!(viewer.selected_row, 1); // stays at 1

        // move_right stops at last column
        viewer.move_right();
        assert_eq!(viewer.selected_col, 1);
        viewer.move_right();
        assert_eq!(viewer.selected_col, 1); // stays at 1

        // move_up stops at 0
        viewer.selected_row = 0;
        viewer.move_up();
        assert_eq!(viewer.selected_row, 0);

        // move_left stops at 0
        viewer.selected_col = 0;
        viewer.move_left();
        assert_eq!(viewer.selected_col, 0);
    }

    #[test]
    fn test_go_to_top_bottom() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        viewer.go_to_bottom();
        assert_eq!(viewer.selected_row, 1); // last row (index 1 of 2)
        viewer.go_to_top();
        assert_eq!(viewer.selected_row, 0);
    }

    #[test]
    fn test_go_to_home_end() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        viewer.go_to_end();
        assert_eq!(viewer.selected_col, 1); // last col (index 1 of 2)
        viewer.go_to_home();
        assert_eq!(viewer.selected_col, 0);
    }

    #[test]
    fn test_h_scroll_resets_on_set_results() {
        let mut viewer = ResultsViewer::new();
        viewer.h_scroll_offset = 5;
        viewer.set_results(sample_results());
        assert_eq!(viewer.h_scroll_offset, 0);
    }

    #[test]
    fn test_go_to_home_resets_h_scroll() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        viewer.h_scroll_offset = 3;
        viewer.selected_col = 1;
        viewer.go_to_home();
        assert_eq!(viewer.selected_col, 0);
        assert_eq!(viewer.h_scroll_offset, 0);
    }

    // UTF-8 truncation tests
    #[test]
    fn test_truncate_str_ascii_no_truncation() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_str_ascii_truncation() {
        assert_eq!(truncate_str("hello world", 8), "hello...");
        assert_eq!(truncate_str("hello world", 5), "he...");
    }

    #[test]
    fn test_truncate_str_utf8_no_truncation() {
        // Multi-byte characters should count as 1 each
        assert_eq!(truncate_str("café", 10), "café");
        assert_eq!(truncate_str("café", 4), "café");
        assert_eq!(truncate_str("日本語", 5), "日本語");
        assert_eq!(truncate_str("日本語", 3), "日本語");
    }

    #[test]
    fn test_truncate_str_utf8_truncation() {
        // Should not panic on multi-byte chars
        // "café au lait" = 12 chars, max=7, take 4 chars + "..." = "café..."
        assert_eq!(truncate_str("café au lait", 7), "café...");
        // "日本語テスト" = 6 chars, max=5, take 2 chars + "..." = "日本..."
        assert_eq!(truncate_str("日本語テスト", 5), "日本...");
        // Mixed emoji and text: "hello☕world" = 11 chars, max=8, take 5 chars = "hello..."
        assert_eq!(truncate_str("hello☕world", 8), "hello...");
    }

    #[test]
    fn test_truncate_str_emoji() {
        // Emoji are typically 3-4 bytes but 1 character
        assert_eq!(truncate_str("☕☕☕", 3), "☕☕☕"); // exactly fits
        // max=3 but 4 chars: max is not > 3, so take first 3 chars (no room for ellipsis)
        assert_eq!(truncate_str("☕☕☕☕", 3), "☕☕☕");
        // "test☕☕" = 6 chars, max=5, take 2 chars + "..." = "te..."
        assert_eq!(truncate_str("test☕☕", 5), "te...");
    }

    #[test]
    fn test_truncate_str_edge_cases() {
        // Empty string
        assert_eq!(truncate_str("", 5), "");
        // Single character
        assert_eq!(truncate_str("x", 5), "x");
        // Max <= 3 (no room for ellipsis)
        assert_eq!(truncate_str("hello", 3), "hel");
        assert_eq!(truncate_str("hello", 2), "he");
        assert_eq!(truncate_str("日本語", 2), "日本");
    }

    // ── View mode tests ──────────────────────────────────
    #[test]
    fn test_default_view_mode_is_table() {
        let viewer = ResultsViewer::new();
        assert_eq!(viewer.view_mode(), ViewMode::Table);
    }

    #[test]
    fn test_toggle_view_mode() {
        let mut viewer = ResultsViewer::new();
        viewer.toggle_view_mode();
        assert_eq!(viewer.view_mode(), ViewMode::Vertical);
        viewer.toggle_view_mode();
        assert_eq!(viewer.view_mode(), ViewMode::Table);
    }

    #[test]
    fn test_set_results_preserves_view_mode() {
        let mut viewer = ResultsViewer::new();
        viewer.toggle_view_mode();
        assert_eq!(viewer.view_mode(), ViewMode::Vertical);
        viewer.set_results(sample_results());
        // View mode should persist across new results
        assert_eq!(viewer.view_mode(), ViewMode::Vertical);
    }

    #[test]
    fn test_navigation_works_in_vertical_mode() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results()); // 2 rows, 2 cols
        viewer.toggle_view_mode();

        // up/down still navigates rows
        viewer.move_down();
        assert_eq!(viewer.selected_row, 1);
        viewer.move_up();
        assert_eq!(viewer.selected_row, 0);

        // left/right navigates fields (columns)
        viewer.move_right();
        assert_eq!(viewer.selected_col, 1);
        viewer.move_left();
        assert_eq!(viewer.selected_col, 0);
    }

    #[test]
    fn test_widen_column_increases_by_step() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        let original = viewer.col_widths[0];
        viewer.widen_column();
        assert_eq!(viewer.col_widths[0], original + 4);
    }

    #[test]
    fn test_widen_column_caps_at_max() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        viewer.col_widths[0] = 198;
        viewer.widen_column();
        assert_eq!(viewer.col_widths[0], 200);
        // Already at max — stays at 200
        viewer.widen_column();
        assert_eq!(viewer.col_widths[0], 200);
    }

    #[test]
    fn test_narrow_column_decreases_by_step() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        let original = viewer.col_widths[0];
        viewer.narrow_column();
        assert_eq!(viewer.col_widths[0], original.saturating_sub(4).max(4));
    }

    #[test]
    fn test_narrow_column_floors_at_min() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        viewer.col_widths[0] = 6;
        viewer.narrow_column();
        assert_eq!(viewer.col_widths[0], 4);
        // Already at min — stays at 4
        viewer.narrow_column();
        assert_eq!(viewer.col_widths[0], 4);
    }

    #[test]
    fn test_resize_targets_selected_column() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        let col0_before = viewer.col_widths[0];
        let col1_before = viewer.col_widths[1];
        // Widen col 0 — col 1 unchanged
        viewer.widen_column();
        assert_eq!(viewer.col_widths[0], col0_before + 4);
        assert_eq!(viewer.col_widths[1], col1_before);
        // Move to col 1 and narrow — col 0 unchanged
        viewer.move_right();
        viewer.narrow_column();
        assert_eq!(viewer.col_widths[0], col0_before + 4);
        assert_eq!(viewer.col_widths[1], col1_before.saturating_sub(4).max(4));
    }

    #[test]
    fn test_reset_column_widths_restores_auto() {
        let mut viewer = ResultsViewer::new();
        viewer.set_results(sample_results());
        let auto_widths = viewer.col_widths.clone();
        // Mutate widths
        viewer.widen_column();
        viewer.move_right();
        viewer.narrow_column();
        assert_ne!(viewer.col_widths, auto_widths);
        // Reset restores original auto-computed widths
        viewer.reset_column_widths();
        assert_eq!(viewer.col_widths, auto_widths);
    }

    #[test]
    fn test_resize_noop_without_results() {
        let mut viewer = ResultsViewer::new();
        // No results — col_widths is empty, should not panic
        viewer.widen_column();
        viewer.narrow_column();
        viewer.reset_column_widths();
        assert!(viewer.col_widths.is_empty());
    }
}

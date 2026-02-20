//! Query results viewer widget
//!
//! Displays query results in a scrollable table with cell-level selection.

use crate::db::types::{CellValue, QueryResults};
use crate::ui::Component;
use crate::ui::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

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
            CellValue::Json(v) => serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
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

    pub fn page_up(&mut self) {
        self.selected_row = self.selected_row.saturating_sub(20);
    }

    pub fn page_down(&mut self) {
        let count = self.row_count();
        self.selected_row = (self.selected_row + 20).min(count.saturating_sub(1));
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

        let visible_height = (area.height as usize).saturating_sub(2); // header + footer
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
            let name = truncate_str(&col_def.name, w as usize);
            let padded = format!("{:<width$}", name, width = w as usize);
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

        // Footer with row count and timing
        let footer_y = area.y + area.height - 1;
        let footer = format!(
            "Row {}/{} | Col {}/{} | {:.1}ms",
            viewer.selected_row + 1,
            results.row_count,
            viewer.selected_col + 1,
            results.columns.len(),
            results.execution_time.as_secs_f64() * 1000.0,
        );
        let footer_style = theme.results_footer;
        frame.render_widget(
            Paragraph::new(footer).style(footer_style),
            Rect::new(area.x, footer_y, area.width, 1),
        );
    }
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

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else if max > 3 {
        format!("{}...", &s[..max - 3])
    } else {
        s[..max].to_string()
    }
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
}

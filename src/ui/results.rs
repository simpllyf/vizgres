//! Query results viewer widget
//!
//! Displays query results in a scrollable table with cell-level selection.

use crate::db::types::{CellValue, QueryResults};
use crate::ui::Component;
use crossterm::event::{KeyCode, KeyEvent};
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
        }
    }

    pub fn set_results(&mut self, results: QueryResults) {
        self.col_widths = compute_column_widths(&results);
        self.results = Some(results);
        self.selected_row = 0;
        self.selected_col = 0;
        self.scroll_offset = 0;
        self.h_scroll_offset = 0;
    }

    /// Clear results (reserved for future use)
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.results = None;
        self.selected_row = 0;
        self.selected_col = 0;
        self.col_widths.clear();
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
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        let row_count = self.row_count();
        let col_count = self.col_count();
        if row_count == 0 {
            return false;
        }

        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_row < row_count - 1 {
                    self.selected_row += 1;
                }
                true
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_row > 0 {
                    self.selected_row -= 1;
                }
                true
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.selected_col < col_count.saturating_sub(1) {
                    self.selected_col += 1;
                }
                true
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if self.selected_col > 0 {
                    self.selected_col -= 1;
                }
                true
            }
            KeyCode::Home => {
                self.selected_col = 0;
                true
            }
            KeyCode::End => {
                self.selected_col = col_count.saturating_sub(1);
                true
            }
            KeyCode::PageDown => {
                self.selected_row = (self.selected_row + 20).min(row_count.saturating_sub(1));
                true
            }
            KeyCode::PageUp => {
                self.selected_row = self.selected_row.saturating_sub(20);
                true
            }
            KeyCode::Char('g') => {
                self.selected_row = 0;
                true
            }
            KeyCode::Char('G') => {
                self.selected_row = row_count.saturating_sub(1);
                true
            }
            _ => false,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let results = match &self.results {
            Some(r) if !r.columns.is_empty() => r,
            _ => {
                let msg = if self.results.is_some() {
                    "Query returned no columns"
                } else {
                    "No results. Execute a query with Ctrl+Enter."
                };
                let p = Paragraph::new(msg).style(Style::default().fg(Color::DarkGray));
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

        // Determine which columns to show based on horizontal scroll
        let col_widths = &self.col_widths;

        // Render header row
        let header_y = area.y;
        let mut x = area.x;
        for (col_idx, col_def) in results.columns.iter().enumerate() {
            if x >= area.x + area.width {
                break;
            }
            let w = col_widths
                .get(col_idx)
                .copied()
                .unwrap_or(10)
                .min(area.x + area.width - x);
            let style = if focused && col_idx == viewer.selected_col {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
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
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };

            let mut x = area.x;
            for (col_idx, cell) in row.values.iter().enumerate() {
                if x >= area.x + area.width {
                    break;
                }
                let w = col_widths
                    .get(col_idx)
                    .copied()
                    .unwrap_or(10)
                    .min(area.x + area.width - x);

                let style = if focused && is_selected_row && col_idx == viewer.selected_col {
                    Style::default().fg(Color::Black).bg(Color::Yellow)
                } else if cell.is_null() {
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC)
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
        let footer_style = Style::default().fg(Color::DarkGray);
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
        QueryResults {
            columns: vec![
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
            rows: vec![
                Row {
                    values: vec![CellValue::Integer(1), CellValue::Text("Alice".to_string())],
                },
                Row {
                    values: vec![CellValue::Integer(2), CellValue::Text("Bob".to_string())],
                },
            ],
            execution_time: Duration::from_millis(42),
            row_count: 2,
        }
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
}

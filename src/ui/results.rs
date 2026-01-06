//! Query results viewer widget
//!
//! Displays query results in a scrollable table.

use crate::db::types::QueryResults;
use crate::ui::Component;
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

/// Results table viewer
pub struct ResultsViewer {
    /// Current query results
    results: Option<QueryResults>,

    /// Selected row index
    selected_row: usize,

    /// Selected column index
    selected_col: usize,

    /// Vertical scroll offset
    scroll_offset: usize,

    /// Horizontal scroll offset
    horizontal_offset: usize,
}

impl ResultsViewer {
    /// Create a new results viewer
    pub fn new() -> Self {
        Self {
            results: None,
            selected_row: 0,
            selected_col: 0,
            scroll_offset: 0,
            horizontal_offset: 0,
        }
    }

    /// Set the query results to display
    pub fn set_results(&mut self, results: QueryResults) {
        self.results = Some(results);
        self.selected_row = 0;
        self.selected_col = 0;
        self.scroll_offset = 0;
        self.horizontal_offset = 0;
    }

    /// Clear the results
    pub fn clear(&mut self) {
        self.results = None;
        self.selected_row = 0;
        self.selected_col = 0;
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        // TODO: Phase 5 - Implement navigation
        if self.selected_row > 0 {
            self.selected_row -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        // TODO: Phase 5 - Implement navigation with bounds checking
        self.selected_row += 1;
    }

    /// Move selection left
    pub fn move_left(&mut self) {
        // TODO: Phase 5 - Implement navigation
        if self.selected_col > 0 {
            self.selected_col -= 1;
        }
    }

    /// Move selection right
    pub fn move_right(&mut self) {
        // TODO: Phase 5 - Implement navigation with bounds checking
        self.selected_col += 1;
    }

    /// Get the currently selected cell value
    pub fn selected_cell(&self) -> Option<String> {
        // TODO: Phase 5 - Return selected cell value
        None
    }
}

impl Default for ResultsViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ResultsViewer {
    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        // TODO: Phase 5 - Handle navigation and cell inspection
        // - Arrow keys for navigation
        // - Enter to open cell inspector
        // - Page up/down for scrolling
        // - Home/end for column navigation
        false
    }

    fn render(&self, _frame: &mut Frame, _area: Rect, _focused: bool) {
        // TODO: Phase 5 - Render results table
        // - Show column headers
        // - Display rows with proper alignment
        // - Truncate long values
        // - Highlight selected cell
        // - Show scrollbars if needed
        // - Display row count and timing info
    }

    fn min_size(&self) -> (u16, u16) {
        (40, 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_results_viewer_new() {
        let viewer = ResultsViewer::new();
        assert!(viewer.results.is_none());
        assert_eq!(viewer.selected_row, 0);
    }

    #[test]
    fn test_move_up_at_top() {
        let mut viewer = ResultsViewer::new();
        viewer.move_up();
        assert_eq!(viewer.selected_row, 0);
    }
}

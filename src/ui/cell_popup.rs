//! Cell value inspector popup
//!
//! Displays full cell content in an overlay, with special handling for JSONB.

use crate::ui::Component;
use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

/// Cell value popup/inspector
pub struct CellPopup {
    /// The cell value to display
    content: Option<String>,

    /// Whether this is JSON content
    is_json: bool,

    /// Scroll offset for large content
    scroll_offset: usize,
}

impl CellPopup {
    /// Create a new cell popup
    pub fn new() -> Self {
        Self {
            content: None,
            is_json: false,
            scroll_offset: 0,
        }
    }

    /// Show cell content
    pub fn show(&mut self, content: String, is_json: bool) {
        self.content = Some(content);
        self.is_json = is_json;
        self.scroll_offset = 0;
    }

    /// Hide the popup
    pub fn hide(&mut self) {
        self.content = None;
        self.scroll_offset = 0;
    }

    /// Check if popup is visible
    pub fn is_visible(&self) -> bool {
        self.content.is_some()
    }

    /// Scroll down
    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    /// Scroll up
    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }
}

impl Default for CellPopup {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for CellPopup {
    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        // TODO: Phase 5 - Handle popup keys
        // - Escape to close
        // - Arrow up/down for scrolling
        // - Page up/down for scrolling
        // - Copy to clipboard (Ctrl+C)
        false
    }

    fn render(&self, _frame: &mut Frame, _area: Rect, _focused: bool) {
        // TODO: Phase 5 - Render popup overlay
        // - Center the popup on screen
        // - Show content with scrolling
        // - Pretty-print JSON if is_json
        // - Show scrollbar for large content
        // - Border with title
    }

    fn min_size(&self) -> (u16, u16) {
        (40, 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_popup_new() {
        let popup = CellPopup::new();
        assert!(!popup.is_visible());
    }

    #[test]
    fn test_show_hide() {
        let mut popup = CellPopup::new();
        popup.show("test content".to_string(), false);
        assert!(popup.is_visible());
        popup.hide();
        assert!(!popup.is_visible());
    }

    #[test]
    fn test_scroll_up_at_top() {
        let mut popup = CellPopup::new();
        popup.show("test".to_string(), false);
        popup.scroll_up();
        assert_eq!(popup.scroll_offset, 0);
    }
}

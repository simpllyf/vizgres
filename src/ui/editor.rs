//! Query editor widget
//!
//! Multi-line SQL editor with syntax highlighting and formatting.

use crate::ui::Component;
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

/// Query editor component
pub struct QueryEditor {
    /// The query text buffer
    content: String,

    /// Cursor position (line, column)
    cursor: (usize, usize),

    /// Scroll offset (first visible line)
    _scroll_offset: usize,
}

impl QueryEditor {
    /// Create a new query editor
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor: (0, 0),
            _scroll_offset: 0,
        }
    }

    /// Get the current query text
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Set the query text
    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.cursor = (0, 0);
    }

    /// Insert character at cursor
    pub fn insert_char(&mut self, _c: char) {
        // TODO: Phase 4 - Implement text insertion
        todo!("Text insertion not yet implemented")
    }

    /// Delete character before cursor (backspace)
    pub fn delete_char(&mut self) {
        // TODO: Phase 4 - Implement deletion
        todo!("Character deletion not yet implemented")
    }

    /// Move cursor up
    pub fn move_up(&mut self) {
        // TODO: Phase 4 - Implement cursor movement
        if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
        }
    }

    /// Move cursor down
    pub fn move_down(&mut self) {
        // TODO: Phase 4 - Implement cursor movement
        self.cursor.0 += 1;
    }

    /// Move cursor left
    pub fn move_left(&mut self) {
        // TODO: Phase 4 - Implement cursor movement
        if self.cursor.1 > 0 {
            self.cursor.1 -= 1;
        }
    }

    /// Move cursor right
    pub fn move_right(&mut self) {
        // TODO: Phase 4 - Implement cursor movement
        self.cursor.1 += 1;
    }
}

impl Default for QueryEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for QueryEditor {
    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        // TODO: Phase 4 - Handle all editor keys
        // - Character insertion
        // - Cursor movement (arrows, home, end, page up/down)
        // - Backspace/delete
        // - Enter for new line
        // - Tab for indentation
        false
    }

    fn render(&self, _frame: &mut Frame, _area: Rect, _focused: bool) {
        // TODO: Phase 4 - Render editor with syntax highlighting
        // - Show line numbers
        // - Highlight SQL keywords
        // - Show cursor position
        // - Handle scrolling for large queries
    }

    fn min_size(&self) -> (u16, u16) {
        (40, 10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_new() {
        let editor = QueryEditor::new();
        assert_eq!(editor.content(), "");
        assert_eq!(editor.cursor, (0, 0));
    }

    #[test]
    fn test_set_content() {
        let mut editor = QueryEditor::new();
        editor.set_content("SELECT * FROM users".to_string());
        assert_eq!(editor.content(), "SELECT * FROM users");
    }

    #[test]
    fn test_move_up_at_top() {
        let mut editor = QueryEditor::new();
        editor.move_up();
        assert_eq!(editor.cursor.0, 0);
    }
}

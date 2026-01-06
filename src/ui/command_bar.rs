//! Command bar widget
//!
//! Input bar for entering commands (starting with :)

use crate::ui::Component;
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

/// Command bar component
pub struct CommandBar {
    /// Input buffer
    input: String,

    /// Cursor position
    cursor: usize,

    /// Whether the command bar is visible/active
    active: bool,

    /// Autocomplete suggestions
    _suggestions: Vec<String>,
}

impl CommandBar {
    /// Create a new command bar
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            active: false,
            _suggestions: Vec::new(),
        }
    }

    /// Activate the command bar
    pub fn activate(&mut self) {
        self.active = true;
        self.input.clear();
        self.cursor = 0;
    }

    /// Deactivate the command bar
    pub fn deactivate(&mut self) {
        self.active = false;
        self.input.clear();
        self.cursor = 0;
    }

    /// Check if command bar is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the current input
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Insert character at cursor
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += 1;
        // TODO: Phase 6 - Update autocomplete suggestions
    }

    /// Delete character before cursor
    pub fn delete_char(&mut self) {
        if self.cursor > 0 {
            self.input.remove(self.cursor - 1);
            self.cursor -= 1;
            // TODO: Phase 6 - Update autocomplete suggestions
        }
    }

    /// Move cursor left
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right
    pub fn move_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }
}

impl Default for CommandBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for CommandBar {
    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        // TODO: Phase 2 - Handle command bar input
        // - Character insertion
        // - Backspace/delete
        // - Left/right arrows
        // - Enter to submit
        // - Escape to cancel
        // - Tab for autocomplete
        false
    }

    fn render(&self, _frame: &mut Frame, _area: Rect, _focused: bool) {
        // TODO: Phase 2 - Render command bar
        // - Show prompt (>)
        // - Show input with cursor
        // - Show autocomplete suggestions if available
    }

    fn min_size(&self) -> (u16, u16) {
        (10, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_bar_new() {
        let bar = CommandBar::new();
        assert!(!bar.is_active());
        assert_eq!(bar.input(), "");
    }

    #[test]
    fn test_activate_deactivate() {
        let mut bar = CommandBar::new();
        bar.activate();
        assert!(bar.is_active());
        bar.deactivate();
        assert!(!bar.is_active());
    }

    #[test]
    fn test_insert_char() {
        let mut bar = CommandBar::new();
        bar.insert_char('q');
        bar.insert_char('u');
        bar.insert_char('i');
        bar.insert_char('t');
        assert_eq!(bar.input(), "quit");
        assert_eq!(bar.cursor, 4);
    }

    #[test]
    fn test_delete_char() {
        let mut bar = CommandBar::new();
        bar.insert_char('a');
        bar.insert_char('b');
        bar.delete_char();
        assert_eq!(bar.input(), "a");
        assert_eq!(bar.cursor, 1);
    }
}

//! Query editor widget
//!
//! Multi-line SQL editor with line numbers and cursor.

use crate::ui::Component;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Query editor component
pub struct QueryEditor {
    /// Lines of text
    lines: Vec<String>,

    /// Cursor position (line, column)
    cursor: (usize, usize),

    /// Scroll offset (first visible line)
    scroll_offset: usize,
}

impl QueryEditor {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: (0, 0),
            scroll_offset: 0,
        }
    }

    /// Get the full content as a single string
    pub fn get_content(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    /// Clear all editor content
    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor = (0, 0);
        self.scroll_offset = 0;
    }

    /// Set the editor content (used for testing and future features like query history)
    #[allow(dead_code)]
    pub fn set_content(&mut self, content: String) {
        self.lines = content.lines().map(String::from).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor = (0, 0);
        self.scroll_offset = 0;
    }

    fn insert_char(&mut self, c: char) {
        let line = &mut self.lines[self.cursor.0];
        // Handle cursor beyond line length
        let col = self.cursor.1.min(line.len());
        line.insert(col, c);
        self.cursor.1 = col + 1;
    }

    fn backspace(&mut self) {
        if self.cursor.1 > 0 {
            let col = self.cursor.1.min(self.lines[self.cursor.0].len());
            if col > 0 {
                self.lines[self.cursor.0].remove(col - 1);
                self.cursor.1 = col - 1;
            }
        } else if self.cursor.0 > 0 {
            // Join with previous line
            let current_line = self.lines.remove(self.cursor.0);
            self.cursor.0 -= 1;
            self.cursor.1 = self.lines[self.cursor.0].len();
            self.lines[self.cursor.0].push_str(&current_line);
        }
    }

    fn delete_forward(&mut self) {
        let line_len = self.lines[self.cursor.0].len();
        let col = self.cursor.1.min(line_len);
        if col < line_len {
            self.lines[self.cursor.0].remove(col);
        } else if self.cursor.0 < self.lines.len() - 1 {
            // Join next line into current
            let next_line = self.lines.remove(self.cursor.0 + 1);
            self.lines[self.cursor.0].push_str(&next_line);
        }
    }

    fn new_line(&mut self) {
        let col = self.cursor.1.min(self.lines[self.cursor.0].len());
        let rest = self.lines[self.cursor.0][col..].to_string();
        self.lines[self.cursor.0].truncate(col);
        self.cursor.0 += 1;
        self.cursor.1 = 0;
        self.lines.insert(self.cursor.0, rest);
    }

    fn move_up(&mut self) {
        if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
            self.cursor.1 = self.cursor.1.min(self.lines[self.cursor.0].len());
        }
    }

    fn move_down(&mut self) {
        if self.cursor.0 < self.lines.len() - 1 {
            self.cursor.0 += 1;
            self.cursor.1 = self.cursor.1.min(self.lines[self.cursor.0].len());
        }
    }

    fn move_left(&mut self) {
        if self.cursor.1 > 0 {
            self.cursor.1 -= 1;
        } else if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
            self.cursor.1 = self.lines[self.cursor.0].len();
        }
    }

    fn move_right(&mut self) {
        let line_len = self.lines[self.cursor.0].len();
        if self.cursor.1 < line_len {
            self.cursor.1 += 1;
        } else if self.cursor.0 < self.lines.len() - 1 {
            self.cursor.0 += 1;
            self.cursor.1 = 0;
        }
    }

    fn move_home(&mut self) {
        self.cursor.1 = 0;
    }

    fn move_end(&mut self) {
        self.cursor.1 = self.lines[self.cursor.0].len();
    }

    #[allow(dead_code)]
    fn ensure_cursor_visible(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }
        if self.cursor.0 < self.scroll_offset {
            self.scroll_offset = self.cursor.0;
        } else if self.cursor.0 >= self.scroll_offset + visible_height {
            self.scroll_offset = self.cursor.0 - visible_height + 1;
        }
    }
}

impl Default for QueryEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for QueryEditor {
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    return false; // Let parent handle Ctrl combos
                }
                self.insert_char(c);
                true
            }
            KeyCode::Backspace => {
                self.backspace();
                true
            }
            KeyCode::Delete => {
                self.delete_forward();
                true
            }
            KeyCode::Enter => {
                self.new_line();
                true
            }
            KeyCode::Up => {
                self.move_up();
                true
            }
            KeyCode::Down => {
                self.move_down();
                true
            }
            KeyCode::Left => {
                self.move_left();
                true
            }
            KeyCode::Right => {
                self.move_right();
                true
            }
            KeyCode::Home => {
                self.move_home();
                true
            }
            KeyCode::End => {
                self.move_end();
                true
            }
            _ => false,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        if area.width < 2 || area.height == 0 {
            return;
        }

        let visible_height = area.height as usize;
        let line_num_width = format!("{}", self.lines.len()).len().max(2) as u16;
        let content_x = area.x + line_num_width + 1; // +1 for space after line number
        let content_width = area.width.saturating_sub(line_num_width + 1);

        for i in 0..visible_height {
            let line_idx = self.scroll_offset + i;
            let y = area.y + i as u16;

            if line_idx < self.lines.len() {
                // Line number
                let line_num = format!("{:>width$}", line_idx + 1, width = line_num_width as usize);
                let num_style = Style::default().fg(Color::DarkGray);
                frame.render_widget(
                    Paragraph::new(line_num).style(num_style),
                    Rect::new(area.x, y, line_num_width, 1),
                );

                // Line content
                let line = &self.lines[line_idx];
                let display_line = if line.len() > content_width as usize {
                    &line[..content_width as usize]
                } else {
                    line.as_str()
                };

                let style = Style::default().fg(Color::White);
                frame.render_widget(
                    Paragraph::new(display_line).style(style),
                    Rect::new(content_x, y, content_width, 1),
                );

                // Cursor
                if focused && line_idx == self.cursor.0 {
                    let cursor_col = self.cursor.1.min(line.len());
                    let cursor_x = content_x + cursor_col as u16;
                    if cursor_x < area.x + area.width {
                        frame.set_cursor_position(Position::new(cursor_x, y));
                    }
                }
            } else {
                // Empty line indicator
                let tilde = Paragraph::new("~").style(Style::default().fg(Color::DarkGray));
                frame.render_widget(tilde, Rect::new(area.x, y, 1, 1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_new() {
        let editor = QueryEditor::new();
        assert!(editor.is_empty());
        assert_eq!(editor.get_content(), "");
    }

    #[test]
    fn test_insert_chars() {
        let mut editor = QueryEditor::new();
        editor.insert_char('S');
        editor.insert_char('E');
        editor.insert_char('L');
        assert_eq!(editor.get_content(), "SEL");
    }

    #[test]
    fn test_new_line() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.new_line();
        editor.insert_char('b');
        assert_eq!(editor.get_content(), "a\nb");
    }

    #[test]
    fn test_backspace_joins_lines() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.new_line();
        editor.insert_char('b');
        editor.cursor = (1, 0);
        editor.backspace();
        assert_eq!(editor.get_content(), "ab");
    }

    #[test]
    fn test_delete_forward() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('c');
        editor.cursor = (0, 1);
        editor.delete_forward();
        assert_eq!(editor.get_content(), "ac");
    }

    #[test]
    fn test_delete_forward_joins_lines() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.new_line();
        editor.insert_char('b');
        editor.cursor = (0, 1);
        editor.delete_forward();
        assert_eq!(editor.get_content(), "ab");
    }

    #[test]
    fn test_set_content() {
        let mut editor = QueryEditor::new();
        editor.set_content("SELECT *\nFROM users".to_string());
        assert_eq!(editor.get_content(), "SELECT *\nFROM users");
        assert_eq!(editor.lines.len(), 2);
    }

    #[test]
    fn test_clear() {
        let mut editor = QueryEditor::new();
        editor.set_content("SELECT * FROM users".to_string());
        assert!(!editor.is_empty());
        editor.clear();
        assert!(editor.is_empty());
        assert_eq!(editor.get_content(), "");
    }
}

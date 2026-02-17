//! Command bar widget
//!
//! Input bar for entering commands (starting with /)

use crate::ui::Component;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Command bar component
pub struct CommandBar {
    input: String,
    cursor: usize,
    active: bool,
}

impl CommandBar {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            active: false,
        }
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.input.clear();
        self.cursor = 0;
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.input.clear();
        self.cursor = 0;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn input(&self) -> &str {
        &self.input
    }
}

impl Default for CommandBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for CommandBar {
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                self.input.insert(self.cursor, c);
                self.cursor += 1;
                true
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.input.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
                true
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                true
            }
            KeyCode::Right => {
                if self.cursor < self.input.len() {
                    self.cursor += 1;
                }
                true
            }
            KeyCode::Home => {
                self.cursor = 0;
                true
            }
            KeyCode::End => {
                self.cursor = self.input.len();
                true
            }
            _ => false,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, _focused: bool) {
        if !self.active {
            return;
        }

        let prompt = "/";
        let display = format!("{}{}", prompt, self.input);
        let style = Style::default().fg(Color::White);
        let paragraph = Paragraph::new(display).style(style);
        frame.render_widget(paragraph, area);

        // Show cursor
        let cursor_x = area.x + prompt.len() as u16 + self.cursor as u16;
        if cursor_x < area.x + area.width {
            frame.set_cursor_position(Position::new(cursor_x, area.y));
        }
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
}

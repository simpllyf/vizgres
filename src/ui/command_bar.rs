//! Command bar widget
//!
//! Input bar for entering commands (starting with /)

use crate::ui::Component;
use crate::ui::ComponentAction;
use crate::ui::theme::Theme;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Command bar component
pub struct CommandBar {
    input: String,
    cursor: usize,
    active: bool,
    /// Custom prompt prefix (e.g. "Save as: "). When None, uses "/".
    prompt: Option<String>,
}

impl CommandBar {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor: 0,
            active: false,
            prompt: None,
        }
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.input.clear();
        self.cursor = 0;
        self.prompt = None;
    }

    /// Activate with a custom prompt prefix and pre-filled input text.
    pub fn activate_with_prompt(&mut self, prompt: String, prefill: String) {
        self.active = true;
        self.cursor = prefill.len();
        self.input = prefill;
        self.prompt = Some(prompt);
    }

    pub fn deactivate(&mut self) {
        self.active = false;
        self.input.clear();
        self.cursor = 0;
        self.prompt = None;
    }

    /// Whether the command bar is in prompt mode (vs command mode).
    pub fn is_prompt_mode(&self) -> bool {
        self.prompt.is_some()
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn input_text(&self) -> &str {
        &self.input
    }
}

impl Default for CommandBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for CommandBar {
    fn handle_key(&mut self, key: KeyEvent) -> ComponentAction {
        // Submit (Enter) and Dismiss (Esc) are handled by KeyMap.
        // Only free-form text input is handled here.
        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    return ComponentAction::Ignored;
                }
                self.input.insert(self.cursor, c);
                self.cursor += 1;
                ComponentAction::Consumed
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.input.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
                ComponentAction::Consumed
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                ComponentAction::Consumed
            }
            KeyCode::Right => {
                if self.cursor < self.input.len() {
                    self.cursor += 1;
                }
                ComponentAction::Consumed
            }
            KeyCode::Home => {
                self.cursor = 0;
                ComponentAction::Consumed
            }
            KeyCode::End => {
                self.cursor = self.input.len();
                ComponentAction::Consumed
            }
            _ => ComponentAction::Ignored,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, _focused: bool, theme: &Theme) {
        if !self.active {
            return;
        }

        let prompt = self.prompt.as_deref().unwrap_or("/");
        let display = format!("{}{}", prompt, self.input);
        let paragraph = Paragraph::new(display).style(theme.command_text);
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
        assert!(bar.input.is_empty());
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
    fn test_prompt_mode() {
        let mut bar = CommandBar::new();
        assert!(!bar.is_prompt_mode());

        bar.activate_with_prompt("Save as: ".to_string(), "file.csv".to_string());
        assert!(bar.is_active());
        assert!(bar.is_prompt_mode());
        assert_eq!(bar.input_text(), "file.csv");
        assert_eq!(bar.cursor, 8);
    }

    #[test]
    fn test_prompt_mode_cleared_on_deactivate() {
        let mut bar = CommandBar::new();
        bar.activate_with_prompt("Save as: ".to_string(), "file.csv".to_string());
        assert!(bar.is_prompt_mode());

        bar.deactivate();
        assert!(!bar.is_prompt_mode());
        assert!(!bar.is_active());
    }

    #[test]
    fn test_activate_clears_prompt() {
        let mut bar = CommandBar::new();
        bar.activate_with_prompt("Save as: ".to_string(), "file.csv".to_string());
        assert!(bar.is_prompt_mode());

        bar.activate();
        assert!(!bar.is_prompt_mode());
        assert_eq!(bar.input_text(), "");
    }
}

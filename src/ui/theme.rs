//! UI theme and styling
//!
//! Defines colors, styles, and visual appearance for all UI components.

use ratatui::style::{Color, Modifier, Style};

/// Application theme
#[derive(Debug, Clone)]
pub struct Theme {
    // Panel borders
    pub border_focused: Style,
    pub border_unfocused: Style,

    // Tree browser
    pub tree_schema: Style,
    pub tree_table: Style,
    pub tree_column: Style,
    pub tree_selected: Style,

    // Query editor
    pub editor_text: Style,
    pub editor_keyword: Style,
    pub editor_string: Style,
    pub editor_cursor: Style,

    // Results table
    pub results_header: Style,
    pub results_row_even: Style,
    pub results_row_odd: Style,
    pub results_selected: Style,
    pub results_null: Style,

    // Command bar
    pub command_prompt: Style,
    pub command_input: Style,
    pub command_autocomplete: Style,

    // Status messages
    pub status_success: Style,
    pub status_error: Style,
    pub status_info: Style,
    pub status_warning: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            // Borders
            border_focused: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            border_unfocused: Style::default().fg(Color::DarkGray),

            // Tree browser
            tree_schema: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            tree_table: Style::default().fg(Color::Green),
            tree_column: Style::default().fg(Color::Gray),
            tree_selected: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),

            // Query editor
            editor_text: Style::default().fg(Color::White),
            editor_keyword: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            editor_string: Style::default().fg(Color::Green),
            editor_cursor: Style::default()
                .bg(Color::White)
                .fg(Color::Black),

            // Results table
            results_header: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            results_row_even: Style::default().fg(Color::White),
            results_row_odd: Style::default().fg(Color::Gray),
            results_selected: Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow),
            results_null: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),

            // Command bar
            command_prompt: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            command_input: Style::default().fg(Color::White),
            command_autocomplete: Style::default().fg(Color::DarkGray),

            // Status messages
            status_success: Style::default().fg(Color::Green),
            status_error: Style::default().fg(Color::Red),
            status_info: Style::default().fg(Color::Blue),
            status_warning: Style::default().fg(Color::Yellow),
        }
    }
}

impl Theme {
    /// Create a new theme with default colors
    pub fn new() -> Self {
        Self::default()
    }

    /// Get border style based on focus
    pub fn border_style(&self, focused: bool) -> Style {
        if focused {
            self.border_focused
        } else {
            self.border_unfocused
        }
    }
}

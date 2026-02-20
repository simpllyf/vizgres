//! UI theme and styling
//!
//! Defines colors, styles, and visual appearance for all UI components.
//! Every color in the application is defined here, so changing the palette
//! only requires editing this file.

use ratatui::style::{Color, Modifier, Style};

/// Application theme — single source of truth for all colors and styles
#[derive(Debug, Clone)]
pub struct Theme {
    // Panel borders
    pub border_focused: Style,
    pub border_unfocused: Style,

    // Panel titles
    pub panel_title_focused: Style,
    pub panel_title_unfocused: Style,

    // Inspector popup chrome
    pub popup_title: Style,
    pub popup_border: Style,
    pub shadow: Style,

    // Tree browser
    pub tree_schema: Style,
    pub tree_category: Style,
    pub tree_table: Style,
    pub tree_view: Style,
    pub tree_column: Style,
    pub tree_function: Style,
    pub tree_index: Style,
    pub tree_selected: Style,
    pub tree_empty: Style,

    // Query editor
    pub editor_text: Style,
    pub editor_keyword: Style,
    pub editor_string: Style,
    pub editor_number: Style,
    pub editor_comment: Style,
    pub editor_ghost: Style,
    #[allow(dead_code)] // reserved for cursor-shape rendering
    pub editor_cursor: Style,
    pub editor_line_number: Style,
    pub editor_tilde: Style,

    // Results table
    pub results_header: Style,
    pub results_header_selected: Style,
    pub results_row_even: Style,
    pub results_row_odd: Style,
    pub results_selected: Style,
    pub results_null: Style,
    pub results_empty: Style,
    pub results_error_title: Style,
    pub results_error_text: Style,
    pub results_footer: Style,

    // Inspector
    pub inspector_header: Style,
    pub inspector_text: Style,

    // Help overlay
    pub help_section: Style,
    pub help_key: Style,
    pub help_desc: Style,

    // Command bar
    #[allow(dead_code)] // reserved for styled prompt prefix
    pub command_prompt: Style,
    #[allow(dead_code)] // reserved for input vs autocomplete split
    pub command_input: Style,
    pub command_text: Style,
    #[allow(dead_code)] // reserved for autocomplete suggestions
    pub command_autocomplete: Style,

    // Tab bar
    pub tab_active: Style,
    pub tab_inactive: Style,
    pub tab_separator: Style,

    // Connection dialog
    pub dialog_label: Style,
    pub dialog_input: Style,
    pub dialog_input_focused: Style,
    pub dialog_selected: Style,
    pub dialog_hint: Style,
    pub dialog_warning: Style,

    // Status bar
    pub status_success: Style,
    pub status_error: Style,
    pub status_info: Style,
    pub status_warning: Style,
    pub status_conn_info: Style,
    pub status_help_hint: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            // Borders
            border_focused: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            border_unfocused: Style::default().fg(Color::DarkGray),

            // Panel titles
            panel_title_focused: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            panel_title_unfocused: Style::default().fg(Color::DarkGray),

            // Inspector popup chrome
            popup_title: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            popup_border: Style::default().fg(Color::Yellow),
            shadow: Style::default().bg(Color::DarkGray).fg(Color::DarkGray),

            // Tree browser
            tree_schema: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            tree_category: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            tree_table: Style::default().fg(Color::Green),
            tree_view: Style::default().fg(Color::Magenta),
            tree_column: Style::default().fg(Color::Gray),
            tree_function: Style::default().fg(Color::Cyan),
            tree_index: Style::default().fg(Color::DarkGray),
            tree_selected: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            tree_empty: Style::default().fg(Color::DarkGray),

            // Query editor
            editor_text: Style::default().fg(Color::White),
            editor_keyword: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            editor_string: Style::default().fg(Color::Green),
            editor_number: Style::default().fg(Color::Cyan),
            editor_comment: Style::default().fg(Color::DarkGray),
            editor_ghost: Style::default().fg(Color::DarkGray),
            editor_cursor: Style::default().bg(Color::White).fg(Color::Black),
            editor_line_number: Style::default().fg(Color::DarkGray),
            editor_tilde: Style::default().fg(Color::DarkGray),

            // Results table
            results_header: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            results_header_selected: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            results_row_even: Style::default().fg(Color::White),
            results_row_odd: Style::default().fg(Color::Gray),
            results_selected: Style::default().fg(Color::Black).bg(Color::Yellow),
            results_null: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            results_empty: Style::default().fg(Color::DarkGray),
            results_error_title: Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            results_error_text: Style::default().fg(Color::Red),
            results_footer: Style::default().fg(Color::DarkGray),

            // Inspector
            inspector_header: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            inspector_text: Style::default().fg(Color::White),

            // Help overlay
            help_section: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            help_key: Style::default().fg(Color::Cyan),
            help_desc: Style::default().fg(Color::White),

            // Command bar
            command_prompt: Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
            command_input: Style::default().fg(Color::White),
            command_text: Style::default().fg(Color::White),
            command_autocomplete: Style::default().fg(Color::DarkGray),

            // Connection dialog
            dialog_label: Style::default().fg(Color::Cyan),
            dialog_input: Style::default().fg(Color::White),
            dialog_input_focused: Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            dialog_selected: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            dialog_hint: Style::default().fg(Color::DarkGray),
            dialog_warning: Style::default().fg(Color::Yellow),

            // Tab bar
            tab_active: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            tab_inactive: Style::default().fg(Color::DarkGray),
            tab_separator: Style::default().fg(Color::DarkGray),

            // Status bar
            status_success: Style::default().fg(Color::Green),
            status_error: Style::default().fg(Color::Red),
            status_info: Style::default().fg(Color::Blue),
            status_warning: Style::default().fg(Color::Yellow),
            status_conn_info: Style::default().fg(Color::DarkGray),
            status_help_hint: Style::default().fg(Color::DarkGray),
        }
    }
}

impl Theme {
    /// Get border style based on focus
    pub fn border_style(&self, focused: bool) -> Style {
        if focused {
            self.border_focused
        } else {
            self.border_unfocused
        }
    }
}

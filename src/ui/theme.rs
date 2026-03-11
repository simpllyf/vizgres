//! UI theme and styling
//!
//! Defines colors, styles, and visual appearance for all UI components.
//! Every color in the application is defined here, so changing the palette
//! only requires editing this file.
//!
//! To add a new theme:
//! 1. Add a variant to `ThemeName`
//! 2. Add a constructor method (e.g. `Theme::mytheme()`)
//! 3. Wire it in `ThemeName::parse()` and `Theme::by_name()`

use ratatui::style::{Color, Modifier, Style};

/// Available theme names
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeName {
    Dark,
    Light,
    Midnight,
    Ember,
}

impl ThemeName {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "dark" => Some(Self::Dark),
            "light" => Some(Self::Light),
            "midnight" => Some(Self::Midnight),
            "ember" => Some(Self::Ember),
            _ => None,
        }
    }

    pub fn all() -> &'static [&'static str] {
        &["dark", "light", "midnight", "ember"]
    }
}

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
    pub tree_load_more: Style,
    pub tree_selected: Style,
    pub tree_empty: Style,
    pub tree_filter_bar: Style,
    pub tree_filter_text: Style,
    pub tree_filter_match: Style,

    // Query editor
    pub editor_text: Style,
    pub editor_keyword: Style,
    pub editor_string: Style,
    pub editor_number: Style,
    pub editor_comment: Style,
    pub editor_ghost: Style,
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
    pub command_prompt: Style,
    pub command_input: Style,
    pub command_text: Style,
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
    pub status_txn_active: Style,
    pub status_txn_failed: Style,
    pub status_read_only: Style,
    pub status_confirm: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
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

    /// Look up a theme by name. Returns None for unrecognized names.
    pub fn by_name(name: &str) -> Option<Self> {
        match ThemeName::parse(name)? {
            ThemeName::Dark => Some(Self::dark()),
            ThemeName::Light => Some(Self::light()),
            ThemeName::Midnight => Some(Self::midnight()),
            ThemeName::Ember => Some(Self::ember()),
        }
    }

    // ── Dark (default) ───────────────────────────────────────────
    // Cyan accents, yellow headers, green strings. Classic dark terminal.

    pub fn dark() -> Self {
        let bold = Modifier::BOLD;
        Self {
            border_focused: Style::default().fg(Color::Cyan).add_modifier(bold),
            border_unfocused: Style::default().fg(Color::DarkGray),
            panel_title_focused: Style::default().fg(Color::Cyan).add_modifier(bold),
            panel_title_unfocused: Style::default().fg(Color::DarkGray),
            popup_title: Style::default().fg(Color::Yellow).add_modifier(bold),
            popup_border: Style::default().fg(Color::Yellow),
            shadow: Style::default().bg(Color::DarkGray).fg(Color::DarkGray),
            tree_schema: Style::default().fg(Color::Blue).add_modifier(bold),
            tree_category: Style::default().fg(Color::Yellow).add_modifier(bold),
            tree_table: Style::default().fg(Color::Green),
            tree_view: Style::default().fg(Color::Magenta),
            tree_column: Style::default().fg(Color::Gray),
            tree_function: Style::default().fg(Color::Cyan),
            tree_index: Style::default().fg(Color::DarkGray),
            tree_load_more: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            tree_selected: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(bold),
            tree_empty: Style::default().fg(Color::DarkGray),
            tree_filter_bar: Style::default().fg(Color::White).bg(Color::Blue),
            tree_filter_text: Style::default().fg(Color::White).add_modifier(bold),
            tree_filter_match: Style::default().fg(Color::Yellow).add_modifier(bold),
            editor_text: Style::default().fg(Color::White),
            editor_keyword: Style::default().fg(Color::Blue).add_modifier(bold),
            editor_string: Style::default().fg(Color::Green),
            editor_number: Style::default().fg(Color::Cyan),
            editor_comment: Style::default().fg(Color::DarkGray),
            editor_ghost: Style::default().fg(Color::DarkGray),
            editor_cursor: Style::default().bg(Color::White).fg(Color::Black),
            editor_line_number: Style::default().fg(Color::DarkGray),
            editor_tilde: Style::default().fg(Color::DarkGray),
            results_header: Style::default().fg(Color::Yellow).add_modifier(bold),
            results_header_selected: Style::default()
                .fg(Color::Yellow)
                .add_modifier(bold | Modifier::UNDERLINED),
            results_row_even: Style::default().fg(Color::White),
            results_row_odd: Style::default().fg(Color::Gray),
            results_selected: Style::default().fg(Color::Black).bg(Color::Yellow),
            results_null: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
            results_empty: Style::default().fg(Color::DarkGray),
            results_error_title: Style::default().fg(Color::Red).add_modifier(bold),
            results_error_text: Style::default().fg(Color::Red),
            results_footer: Style::default().fg(Color::DarkGray),
            inspector_header: Style::default().fg(Color::Cyan).add_modifier(bold),
            inspector_text: Style::default().fg(Color::White),
            help_section: Style::default().fg(Color::Yellow).add_modifier(bold),
            help_key: Style::default().fg(Color::Cyan),
            help_desc: Style::default().fg(Color::White),
            command_prompt: Style::default().fg(Color::Magenta).add_modifier(bold),
            command_input: Style::default().fg(Color::White),
            command_text: Style::default().fg(Color::White),
            command_autocomplete: Style::default().fg(Color::DarkGray),
            dialog_label: Style::default().fg(Color::Cyan),
            dialog_input: Style::default().fg(Color::White),
            dialog_input_focused: Style::default().fg(Color::White).add_modifier(bold),
            dialog_selected: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(bold),
            dialog_hint: Style::default().fg(Color::DarkGray),
            dialog_warning: Style::default().fg(Color::Yellow),
            tab_active: Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(bold),
            tab_inactive: Style::default().fg(Color::DarkGray),
            tab_separator: Style::default().fg(Color::DarkGray),
            status_success: Style::default().fg(Color::Green),
            status_error: Style::default().fg(Color::Red),
            status_info: Style::default().fg(Color::Blue),
            status_warning: Style::default().fg(Color::Yellow),
            status_conn_info: Style::default().fg(Color::DarkGray),
            status_help_hint: Style::default().fg(Color::DarkGray),
            status_txn_active: Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(bold),
            status_txn_failed: Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(bold),
            status_read_only: Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(bold),
            status_confirm: Style::default().fg(Color::Yellow).add_modifier(bold),
        }
    }

    // ── Light ────────────────────────────────────────────────────
    // Dark text on light backgrounds. Blue accents, readable in daylight.

    pub fn light() -> Self {
        let bold = Modifier::BOLD;
        Self {
            border_focused: Style::default().fg(Color::Blue).add_modifier(bold),
            border_unfocused: Style::default().fg(Color::Gray),
            panel_title_focused: Style::default().fg(Color::Blue).add_modifier(bold),
            panel_title_unfocused: Style::default().fg(Color::Gray),
            popup_title: Style::default()
                .fg(Color::Rgb(140, 80, 0))
                .add_modifier(bold),
            popup_border: Style::default().fg(Color::Rgb(140, 80, 0)),
            shadow: Style::default()
                .bg(Color::Rgb(200, 200, 200))
                .fg(Color::Rgb(200, 200, 200)),
            tree_schema: Style::default()
                .fg(Color::Rgb(0, 0, 180))
                .add_modifier(bold),
            tree_category: Style::default()
                .fg(Color::Rgb(140, 80, 0))
                .add_modifier(bold),
            tree_table: Style::default().fg(Color::Rgb(0, 130, 0)),
            tree_view: Style::default().fg(Color::Rgb(150, 0, 150)),
            tree_column: Style::default().fg(Color::Rgb(80, 80, 80)),
            tree_function: Style::default().fg(Color::Rgb(0, 120, 150)),
            tree_index: Style::default().fg(Color::Gray),
            tree_load_more: Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
            tree_selected: Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(bold),
            tree_empty: Style::default().fg(Color::Gray),
            tree_filter_bar: Style::default().fg(Color::White).bg(Color::Rgb(0, 0, 180)),
            tree_filter_text: Style::default().fg(Color::White).add_modifier(bold),
            tree_filter_match: Style::default()
                .fg(Color::Rgb(200, 120, 0))
                .add_modifier(bold),
            editor_text: Style::default().fg(Color::Rgb(30, 30, 30)),
            editor_keyword: Style::default()
                .fg(Color::Rgb(0, 0, 180))
                .add_modifier(bold),
            editor_string: Style::default().fg(Color::Rgb(0, 130, 0)),
            editor_number: Style::default().fg(Color::Rgb(0, 120, 150)),
            editor_comment: Style::default().fg(Color::Gray),
            editor_ghost: Style::default().fg(Color::Gray),
            editor_cursor: Style::default().bg(Color::Rgb(30, 30, 30)).fg(Color::White),
            editor_line_number: Style::default().fg(Color::Gray),
            editor_tilde: Style::default().fg(Color::Gray),
            results_header: Style::default()
                .fg(Color::Rgb(0, 0, 180))
                .add_modifier(bold),
            results_header_selected: Style::default()
                .fg(Color::Rgb(0, 0, 180))
                .add_modifier(bold | Modifier::UNDERLINED),
            results_row_even: Style::default().fg(Color::Rgb(30, 30, 30)),
            results_row_odd: Style::default().fg(Color::Rgb(60, 60, 60)),
            results_selected: Style::default().fg(Color::White).bg(Color::Blue),
            results_null: Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
            results_empty: Style::default().fg(Color::Gray),
            results_error_title: Style::default()
                .fg(Color::Rgb(180, 0, 0))
                .add_modifier(bold),
            results_error_text: Style::default().fg(Color::Rgb(180, 0, 0)),
            results_footer: Style::default().fg(Color::Gray),
            inspector_header: Style::default().fg(Color::Blue).add_modifier(bold),
            inspector_text: Style::default().fg(Color::Rgb(30, 30, 30)),
            help_section: Style::default()
                .fg(Color::Rgb(140, 80, 0))
                .add_modifier(bold),
            help_key: Style::default().fg(Color::Blue),
            help_desc: Style::default().fg(Color::Rgb(30, 30, 30)),
            command_prompt: Style::default()
                .fg(Color::Rgb(150, 0, 150))
                .add_modifier(bold),
            command_input: Style::default().fg(Color::Rgb(30, 30, 30)),
            command_text: Style::default().fg(Color::Rgb(30, 30, 30)),
            command_autocomplete: Style::default().fg(Color::Gray),
            dialog_label: Style::default().fg(Color::Blue),
            dialog_input: Style::default().fg(Color::Rgb(30, 30, 30)),
            dialog_input_focused: Style::default()
                .fg(Color::Rgb(30, 30, 30))
                .add_modifier(bold),
            dialog_selected: Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(bold),
            dialog_hint: Style::default().fg(Color::Gray),
            dialog_warning: Style::default().fg(Color::Rgb(200, 120, 0)),
            tab_active: Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(bold),
            tab_inactive: Style::default().fg(Color::Gray),
            tab_separator: Style::default().fg(Color::Gray),
            status_success: Style::default().fg(Color::Rgb(0, 130, 0)),
            status_error: Style::default().fg(Color::Rgb(180, 0, 0)),
            status_info: Style::default().fg(Color::Blue),
            status_warning: Style::default().fg(Color::Rgb(200, 120, 0)),
            status_conn_info: Style::default().fg(Color::Gray),
            status_help_hint: Style::default().fg(Color::Gray),
            status_txn_active: Style::default()
                .fg(Color::Rgb(30, 30, 30))
                .bg(Color::Rgb(255, 200, 50))
                .add_modifier(bold),
            status_txn_failed: Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(180, 0, 0))
                .add_modifier(bold),
            status_read_only: Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(bold),
            status_confirm: Style::default()
                .fg(Color::Rgb(200, 120, 0))
                .add_modifier(bold),
        }
    }

    // ── Midnight ─────────────────────────────────────────────────
    // Deep blues and soft purples. Lavender accents, gentle on the eyes.

    pub fn midnight() -> Self {
        let bold = Modifier::BOLD;
        // Palette: deep navy bg assumed, lavender/periwinkle accents
        let lavender = Color::Rgb(180, 160, 255);
        let soft_blue = Color::Rgb(120, 150, 255);
        let pale_pink = Color::Rgb(255, 160, 200);
        let mint = Color::Rgb(130, 230, 180);
        let peach = Color::Rgb(255, 190, 140);
        let muted = Color::Rgb(100, 110, 140);
        let text = Color::Rgb(210, 215, 230);
        let dim = Color::Rgb(70, 80, 110);

        Self {
            border_focused: Style::default().fg(lavender).add_modifier(bold),
            border_unfocused: Style::default().fg(dim),
            panel_title_focused: Style::default().fg(lavender).add_modifier(bold),
            panel_title_unfocused: Style::default().fg(dim),
            popup_title: Style::default().fg(peach).add_modifier(bold),
            popup_border: Style::default().fg(peach),
            shadow: Style::default()
                .bg(Color::Rgb(20, 20, 40))
                .fg(Color::Rgb(20, 20, 40)),
            tree_schema: Style::default().fg(soft_blue).add_modifier(bold),
            tree_category: Style::default().fg(peach).add_modifier(bold),
            tree_table: Style::default().fg(mint),
            tree_view: Style::default().fg(pale_pink),
            tree_column: Style::default().fg(muted),
            tree_function: Style::default().fg(lavender),
            tree_index: Style::default().fg(dim),
            tree_load_more: Style::default().fg(dim).add_modifier(Modifier::ITALIC),
            tree_selected: Style::default()
                .fg(Color::Rgb(20, 20, 40))
                .bg(lavender)
                .add_modifier(bold),
            tree_empty: Style::default().fg(dim),
            tree_filter_bar: Style::default().fg(Color::White).bg(soft_blue),
            tree_filter_text: Style::default().fg(Color::White).add_modifier(bold),
            tree_filter_match: Style::default().fg(peach).add_modifier(bold),
            editor_text: Style::default().fg(text),
            editor_keyword: Style::default().fg(soft_blue).add_modifier(bold),
            editor_string: Style::default().fg(mint),
            editor_number: Style::default().fg(peach),
            editor_comment: Style::default().fg(dim),
            editor_ghost: Style::default().fg(dim),
            editor_cursor: Style::default()
                .bg(Color::Rgb(210, 215, 230))
                .fg(Color::Rgb(20, 20, 40)),
            editor_line_number: Style::default().fg(dim),
            editor_tilde: Style::default().fg(dim),
            results_header: Style::default().fg(lavender).add_modifier(bold),
            results_header_selected: Style::default()
                .fg(lavender)
                .add_modifier(bold | Modifier::UNDERLINED),
            results_row_even: Style::default().fg(text),
            results_row_odd: Style::default().fg(muted),
            results_selected: Style::default().fg(Color::Rgb(20, 20, 40)).bg(lavender),
            results_null: Style::default().fg(dim).add_modifier(Modifier::ITALIC),
            results_empty: Style::default().fg(dim),
            results_error_title: Style::default()
                .fg(Color::Rgb(255, 100, 100))
                .add_modifier(bold),
            results_error_text: Style::default().fg(Color::Rgb(255, 100, 100)),
            results_footer: Style::default().fg(dim),
            inspector_header: Style::default().fg(lavender).add_modifier(bold),
            inspector_text: Style::default().fg(text),
            help_section: Style::default().fg(peach).add_modifier(bold),
            help_key: Style::default().fg(lavender),
            help_desc: Style::default().fg(text),
            command_prompt: Style::default().fg(pale_pink).add_modifier(bold),
            command_input: Style::default().fg(text),
            command_text: Style::default().fg(text),
            command_autocomplete: Style::default().fg(dim),
            dialog_label: Style::default().fg(lavender),
            dialog_input: Style::default().fg(text),
            dialog_input_focused: Style::default().fg(text).add_modifier(bold),
            dialog_selected: Style::default()
                .fg(Color::Rgb(20, 20, 40))
                .bg(lavender)
                .add_modifier(bold),
            dialog_hint: Style::default().fg(dim),
            dialog_warning: Style::default().fg(peach),
            tab_active: Style::default()
                .fg(Color::Rgb(20, 20, 40))
                .bg(lavender)
                .add_modifier(bold),
            tab_inactive: Style::default().fg(dim),
            tab_separator: Style::default().fg(dim),
            status_success: Style::default().fg(mint),
            status_error: Style::default().fg(Color::Rgb(255, 100, 100)),
            status_info: Style::default().fg(soft_blue),
            status_warning: Style::default().fg(peach),
            status_conn_info: Style::default().fg(dim),
            status_help_hint: Style::default().fg(dim),
            status_txn_active: Style::default()
                .fg(Color::Rgb(20, 20, 40))
                .bg(peach)
                .add_modifier(bold),
            status_txn_failed: Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(255, 100, 100))
                .add_modifier(bold),
            status_read_only: Style::default()
                .fg(Color::White)
                .bg(soft_blue)
                .add_modifier(bold),
            status_confirm: Style::default().fg(peach).add_modifier(bold),
        }
    }

    // ── Ember ────────────────────────────────────────────────────
    // Warm ambers, oranges, and muted reds. Cozy campfire terminal.

    pub fn ember() -> Self {
        let bold = Modifier::BOLD;
        let amber = Color::Rgb(255, 180, 50);
        let orange = Color::Rgb(230, 130, 50);
        let warm_red = Color::Rgb(220, 80, 60);
        let sage = Color::Rgb(140, 190, 120);
        let sand = Color::Rgb(220, 200, 170);
        let muted = Color::Rgb(140, 120, 100);
        let dim = Color::Rgb(90, 80, 70);
        let coal = Color::Rgb(30, 25, 20);

        Self {
            border_focused: Style::default().fg(amber).add_modifier(bold),
            border_unfocused: Style::default().fg(dim),
            panel_title_focused: Style::default().fg(amber).add_modifier(bold),
            panel_title_unfocused: Style::default().fg(dim),
            popup_title: Style::default().fg(orange).add_modifier(bold),
            popup_border: Style::default().fg(orange),
            shadow: Style::default()
                .bg(Color::Rgb(20, 15, 10))
                .fg(Color::Rgb(20, 15, 10)),
            tree_schema: Style::default().fg(orange).add_modifier(bold),
            tree_category: Style::default().fg(amber).add_modifier(bold),
            tree_table: Style::default().fg(sage),
            tree_view: Style::default().fg(Color::Rgb(200, 150, 180)),
            tree_column: Style::default().fg(muted),
            tree_function: Style::default().fg(Color::Rgb(180, 160, 120)),
            tree_index: Style::default().fg(dim),
            tree_load_more: Style::default().fg(dim).add_modifier(Modifier::ITALIC),
            tree_selected: Style::default().fg(coal).bg(amber).add_modifier(bold),
            tree_empty: Style::default().fg(dim),
            tree_filter_bar: Style::default().fg(coal).bg(orange),
            tree_filter_text: Style::default().fg(coal).add_modifier(bold),
            tree_filter_match: Style::default().fg(amber).add_modifier(bold),
            editor_text: Style::default().fg(sand),
            editor_keyword: Style::default().fg(orange).add_modifier(bold),
            editor_string: Style::default().fg(sage),
            editor_number: Style::default().fg(amber),
            editor_comment: Style::default().fg(dim),
            editor_ghost: Style::default().fg(dim),
            editor_cursor: Style::default().bg(sand).fg(coal),
            editor_line_number: Style::default().fg(dim),
            editor_tilde: Style::default().fg(dim),
            results_header: Style::default().fg(amber).add_modifier(bold),
            results_header_selected: Style::default()
                .fg(amber)
                .add_modifier(bold | Modifier::UNDERLINED),
            results_row_even: Style::default().fg(sand),
            results_row_odd: Style::default().fg(muted),
            results_selected: Style::default().fg(coal).bg(amber),
            results_null: Style::default().fg(dim).add_modifier(Modifier::ITALIC),
            results_empty: Style::default().fg(dim),
            results_error_title: Style::default().fg(warm_red).add_modifier(bold),
            results_error_text: Style::default().fg(warm_red),
            results_footer: Style::default().fg(dim),
            inspector_header: Style::default().fg(amber).add_modifier(bold),
            inspector_text: Style::default().fg(sand),
            help_section: Style::default().fg(orange).add_modifier(bold),
            help_key: Style::default().fg(amber),
            help_desc: Style::default().fg(sand),
            command_prompt: Style::default().fg(orange).add_modifier(bold),
            command_input: Style::default().fg(sand),
            command_text: Style::default().fg(sand),
            command_autocomplete: Style::default().fg(dim),
            dialog_label: Style::default().fg(amber),
            dialog_input: Style::default().fg(sand),
            dialog_input_focused: Style::default().fg(sand).add_modifier(bold),
            dialog_selected: Style::default().fg(coal).bg(amber).add_modifier(bold),
            dialog_hint: Style::default().fg(dim),
            dialog_warning: Style::default().fg(orange),
            tab_active: Style::default().fg(coal).bg(amber).add_modifier(bold),
            tab_inactive: Style::default().fg(dim),
            tab_separator: Style::default().fg(dim),
            status_success: Style::default().fg(sage),
            status_error: Style::default().fg(warm_red),
            status_info: Style::default().fg(orange),
            status_warning: Style::default().fg(amber),
            status_conn_info: Style::default().fg(dim),
            status_help_hint: Style::default().fg(dim),
            status_txn_active: Style::default().fg(coal).bg(amber).add_modifier(bold),
            status_txn_failed: Style::default()
                .fg(Color::White)
                .bg(warm_red)
                .add_modifier(bold),
            status_read_only: Style::default().fg(coal).bg(orange).add_modifier(bold),
            status_confirm: Style::default().fg(amber).add_modifier(bold),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_dark() {
        let default = Theme::default();
        let dark = Theme::dark();
        // Check a distinctive style to verify they match
        assert_eq!(
            format!("{:?}", default.border_focused),
            format!("{:?}", dark.border_focused)
        );
    }

    #[test]
    fn test_by_name_all_themes() {
        for name in ThemeName::all() {
            assert!(
                Theme::by_name(name).is_some(),
                "Theme '{}' should be loadable",
                name
            );
        }
    }

    #[test]
    fn test_by_name_case_insensitive() {
        assert!(Theme::by_name("Dark").is_some());
        assert!(Theme::by_name("MIDNIGHT").is_some());
        assert!(Theme::by_name("Ember").is_some());
    }

    #[test]
    fn test_by_name_unknown_returns_none() {
        assert!(Theme::by_name("nonexistent").is_none());
        assert!(Theme::by_name("").is_none());
    }

    #[test]
    fn test_theme_name_from_str() {
        assert_eq!(ThemeName::parse("dark"), Some(ThemeName::Dark));
        assert_eq!(ThemeName::parse("light"), Some(ThemeName::Light));
        assert_eq!(ThemeName::parse("midnight"), Some(ThemeName::Midnight));
        assert_eq!(ThemeName::parse("ember"), Some(ThemeName::Ember));
        assert_eq!(ThemeName::parse("nope"), None);
    }

    #[test]
    fn test_border_style_helper() {
        let theme = Theme::dark();
        assert_eq!(
            format!("{:?}", theme.border_style(true)),
            format!("{:?}", theme.border_focused)
        );
        assert_eq!(
            format!("{:?}", theme.border_style(false)),
            format!("{:?}", theme.border_unfocused)
        );
    }

    #[test]
    fn test_all_themes_have_distinct_accents() {
        let dark = Theme::dark();
        let light = Theme::light();
        let midnight = Theme::midnight();
        let ember = Theme::ember();
        // Each theme should have a different focused border color
        let colors: Vec<_> = [dark, light, midnight, ember]
            .iter()
            .map(|t| format!("{:?}", t.border_focused))
            .collect();
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(colors[i], colors[j], "Themes {} and {} should differ", i, j);
            }
        }
    }
}

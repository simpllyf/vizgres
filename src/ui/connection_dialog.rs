//! Connection picker dialog
//!
//! A modal dialog for connecting to PostgreSQL databases. Shows a URL input
//! field, an optional name field for saving connections, and a list of
//! previously saved connections. Follows the Inspector/Help popup pattern.

use crate::config::connections::{ConnectionConfig, load_connections, save_connections};
use crate::ui::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Actions returned by the dialog to the parent
pub enum DialogAction {
    /// User submitted a valid connection
    Connect(ConnectionConfig),
    /// User dismissed the dialog (Esc)
    Dismissed,
    /// Key was consumed by the dialog (no further handling needed)
    Consumed,
}

/// Which field currently has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogFocus {
    UrlInput,
    NameInput,
    SavedList,
}

/// Connection picker dialog state
pub struct ConnectionDialog {
    visible: bool,
    url_input: String,
    url_cursor: usize,
    name_input: String,
    name_cursor: usize,
    connections: Vec<ConnectionConfig>,
    selected: usize,
    focus: DialogFocus,
    error: Option<String>,
}

impl ConnectionDialog {
    pub fn new() -> Self {
        Self {
            visible: false,
            url_input: String::new(),
            url_cursor: 0,
            name_input: String::new(),
            name_cursor: 0,
            connections: Vec::new(),
            selected: 0,
            focus: DialogFocus::UrlInput,
            error: None,
        }
    }

    /// Show the dialog, loading saved connections from disk
    pub fn show(&mut self) {
        self.visible = true;
        self.url_input.clear();
        self.url_cursor = 0;
        self.name_input.clear();
        self.name_cursor = 0;
        self.error = None;
        self.focus = DialogFocus::UrlInput;
        self.connections = load_connections().unwrap_or_default();
        self.selected = 0;
    }

    /// Hide and reset the dialog
    pub fn hide(&mut self) {
        self.visible = false;
        self.url_input.clear();
        self.url_cursor = 0;
        self.name_input.clear();
        self.name_cursor = 0;
        self.error = None;
        self.connections.clear();
        self.selected = 0;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Handle a key event, returning a DialogAction
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> DialogAction {
        use crossterm::event::{KeyCode, KeyModifiers};

        match key.code {
            KeyCode::Esc => return DialogAction::Dismissed,
            KeyCode::Tab if key.modifiers == KeyModifiers::NONE => {
                self.focus = match self.focus {
                    DialogFocus::UrlInput => DialogFocus::NameInput,
                    DialogFocus::NameInput => {
                        if self.connections.is_empty() {
                            DialogFocus::UrlInput
                        } else {
                            DialogFocus::SavedList
                        }
                    }
                    DialogFocus::SavedList => DialogFocus::UrlInput,
                };
                self.error = None;
                return DialogAction::Consumed;
            }
            KeyCode::BackTab => {
                self.focus = match self.focus {
                    DialogFocus::UrlInput => {
                        if self.connections.is_empty() {
                            DialogFocus::NameInput
                        } else {
                            DialogFocus::SavedList
                        }
                    }
                    DialogFocus::NameInput => DialogFocus::UrlInput,
                    DialogFocus::SavedList => DialogFocus::NameInput,
                };
                self.error = None;
                return DialogAction::Consumed;
            }
            KeyCode::Enter => {
                return self.handle_enter();
            }
            _ => {}
        }

        // Dispatch to focused field
        match self.focus {
            DialogFocus::UrlInput => self.handle_text_input_key(key, true),
            DialogFocus::NameInput => self.handle_text_input_key(key, false),
            DialogFocus::SavedList => self.handle_list_key(key),
        }
    }

    fn handle_enter(&mut self) -> DialogAction {
        match self.focus {
            DialogFocus::UrlInput | DialogFocus::NameInput => {
                if self.url_input.trim().is_empty() {
                    self.error = Some("URL is required".to_string());
                    return DialogAction::Consumed;
                }
                match ConnectionConfig::from_url(&self.url_input) {
                    Ok(mut config) => {
                        // If name is provided, save with that name
                        let name = self.name_input.trim().to_string();
                        if !name.is_empty() {
                            config.name = name;
                            self.save_connection(&config);
                        }
                        DialogAction::Connect(config)
                    }
                    Err(e) => {
                        self.error = Some(e.to_string());
                        DialogAction::Consumed
                    }
                }
            }
            DialogFocus::SavedList => {
                // Load selected connection into URL input for editing
                if let Some(conn) = self.connections.get(self.selected) {
                    self.url_input = conn.to_url();
                    self.url_cursor = self.url_input.len();
                    self.name_input = conn.name.clone();
                    self.name_cursor = self.name_input.len();
                    self.focus = DialogFocus::UrlInput;
                    self.error = None;
                }
                DialogAction::Consumed
            }
        }
    }

    fn handle_text_input_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        is_url: bool,
    ) -> DialogAction {
        use crossterm::event::KeyCode;

        let (input, cursor) = if is_url {
            (&mut self.url_input, &mut self.url_cursor)
        } else {
            (&mut self.name_input, &mut self.name_cursor)
        };

        match key.code {
            KeyCode::Char(c) => {
                input.insert(*cursor, c);
                *cursor += c.len_utf8();
                self.error = None;
            }
            KeyCode::Backspace => {
                if *cursor > 0 {
                    // Find the byte position of the previous char boundary
                    let prev = input[..*cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    input.drain(prev..*cursor);
                    *cursor = prev;
                    self.error = None;
                }
            }
            KeyCode::Delete => {
                if *cursor < input.len() {
                    let next = *cursor
                        + input[*cursor..]
                            .chars()
                            .next()
                            .map(|c| c.len_utf8())
                            .unwrap_or(0);
                    input.drain(*cursor..next);
                    self.error = None;
                }
            }
            KeyCode::Left => {
                if *cursor > 0 {
                    *cursor = input[..*cursor]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                }
            }
            KeyCode::Right => {
                if *cursor < input.len() {
                    *cursor += input[*cursor..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                }
            }
            KeyCode::Home => {
                *cursor = 0;
            }
            KeyCode::End => {
                *cursor = input.len();
            }
            _ => {}
        }

        DialogAction::Consumed
    }

    fn handle_list_key(&mut self, key: crossterm::event::KeyEvent) -> DialogAction {
        use crossterm::event::KeyCode;

        if self.connections.is_empty() {
            return DialogAction::Consumed;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.connections.len() {
                    self.selected += 1;
                }
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                self.delete_selected();
            }
            _ => {}
        }

        DialogAction::Consumed
    }

    /// Delete the currently selected connection from the list and disk
    fn delete_selected(&mut self) {
        if self.selected < self.connections.len() {
            self.connections.remove(self.selected);
            if self.selected >= self.connections.len() && self.selected > 0 {
                self.selected -= 1;
            }
            // Persist deletion
            let _ = save_connections(&self.connections);

            // If list is now empty and focus was on list, move to URL input
            if self.connections.is_empty() && self.focus == DialogFocus::SavedList {
                self.focus = DialogFocus::UrlInput;
            }
        }
    }

    /// Save a connection, replacing any existing one with the same name
    fn save_connection(&mut self, config: &ConnectionConfig) {
        self.connections.retain(|c| c.name != config.name);
        self.connections.push(config.clone());
        let _ = save_connections(&self.connections);
    }

    /// Render the dialog content into the provided inner area
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if area.height < 4 || area.width < 20 {
            return;
        }

        let mut y = area.y;
        let inner_width = area.width.saturating_sub(2);
        let x = area.x + 1;

        // URL label + input
        let url_label = "  URL: ";
        let url_input_width = inner_width.saturating_sub(url_label.len() as u16);
        let url_style = if self.focus == DialogFocus::UrlInput {
            theme.dialog_input_focused
        } else {
            theme.dialog_input
        };

        let visible_url = visible_slice(&self.url_input, self.url_cursor, url_input_width as usize);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(url_label, theme.dialog_label),
                Span::styled(visible_url.text, url_style),
            ])),
            Rect::new(x, y, inner_width, 1),
        );

        // Show cursor for URL input
        if self.focus == DialogFocus::UrlInput {
            let cursor_x = x + url_label.len() as u16 + visible_url.cursor_offset as u16;
            frame.set_cursor_position((cursor_x.min(x + inner_width - 1), y));
        }

        y += 1;

        // Name label + input
        let name_label = "  Save as: ";
        let name_input_width = inner_width.saturating_sub(name_label.len() as u16);
        let name_style = if self.focus == DialogFocus::NameInput {
            theme.dialog_input_focused
        } else {
            theme.dialog_input
        };

        let visible_name = visible_slice(
            &self.name_input,
            self.name_cursor,
            name_input_width as usize,
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(name_label, theme.dialog_label),
                Span::styled(visible_name.text, name_style),
            ])),
            Rect::new(x, y, inner_width, 1),
        );

        if self.focus == DialogFocus::NameInput {
            let cursor_x = x + name_label.len() as u16 + visible_name.cursor_offset as u16;
            frame.set_cursor_position((cursor_x.min(x + inner_width - 1), y));
        }

        y += 1;

        // Error message (if any)
        if let Some(ref err) = self.error {
            y += 1;
            let msg = if err.len() > inner_width as usize {
                format!("{}...", &err[..inner_width as usize - 3])
            } else {
                err.clone()
            };
            frame.render_widget(
                Paragraph::new(Span::styled(format!("  {}", msg), theme.dialog_warning)),
                Rect::new(x, y, inner_width, 1),
            );
        }

        y += 1;

        // Separator
        if y < area.y + area.height {
            let separator = format!(
                "  {}",
                "\u{2500}".repeat((inner_width as usize).saturating_sub(4))
            );
            frame.render_widget(
                Paragraph::new(Span::styled(
                    format!(
                        "{} Saved connections {}",
                        &separator[..2.min(separator.len())],
                        &"\u{2500}".repeat((inner_width as usize).saturating_sub(22).max(0))
                    ),
                    theme.dialog_label,
                )),
                Rect::new(x, y, inner_width, 1),
            );
            y += 1;
        }

        // Saved connections list
        let list_height = (area.y + area.height).saturating_sub(y + 2); // reserve 2 for hint + warning
        if !self.connections.is_empty() {
            for (i, conn) in self.connections.iter().enumerate() {
                if i as u16 >= list_height {
                    break;
                }
                let prefix = if i == self.selected && self.focus == DialogFocus::SavedList {
                    "  \u{25b8} "
                } else {
                    "    "
                };
                let url_preview = conn.to_url();
                let name_width = 16.min(inner_width as usize / 3);
                let name_display = if conn.name.len() > name_width {
                    format!("{:.width$}", conn.name, width = name_width)
                } else {
                    format!("{:<width$}", conn.name, width = name_width)
                };
                let remaining =
                    (inner_width as usize).saturating_sub(prefix.len() + name_width + 2);
                let url_display = if url_preview.len() > remaining {
                    format!(
                        "{:.width$}...",
                        url_preview,
                        width = remaining.saturating_sub(3)
                    )
                } else {
                    url_preview
                };

                let style = if i == self.selected && self.focus == DialogFocus::SavedList {
                    theme.dialog_selected
                } else {
                    theme.dialog_input
                };

                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(prefix, style),
                        Span::styled(name_display, style),
                        Span::styled("  ", style),
                        Span::styled(url_display, theme.dialog_hint),
                    ])),
                    Rect::new(x, y + i as u16, inner_width, 1),
                );
            }
            y += self.connections.len().min(list_height as usize) as u16;
        } else {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "    (no saved connections)",
                    theme.dialog_hint,
                )),
                Rect::new(x, y, inner_width, 1),
            );
            y += 1;
        }

        // Bottom area: hints + warning
        let bottom_y = area.y + area.height - 2;
        if y < bottom_y {
            y = bottom_y;
        }

        // Hint line
        if y < area.y + area.height {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "  Enter=connect  Tab=next  d=delete  Esc=cancel",
                    theme.dialog_hint,
                )),
                Rect::new(x, y, inner_width, 1),
            );
            y += 1;
        }

        // Warning line
        if y < area.y + area.height {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    "  \u{26a0} Passwords are stored in plaintext",
                    theme.dialog_warning,
                )),
                Rect::new(x, y, inner_width, 1),
            );
        }
    }
}

impl Default for ConnectionDialog {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper for showing a visible slice of a string with cursor position
struct VisibleSlice {
    text: String,
    cursor_offset: usize,
}

/// Get the visible portion of a string that fits within `width`, keeping
/// the cursor visible. Returns the display text and cursor offset within it.
fn visible_slice(input: &str, cursor: usize, width: usize) -> VisibleSlice {
    if input.len() <= width {
        return VisibleSlice {
            text: input.to_string(),
            cursor_offset: cursor,
        };
    }

    // Scroll to keep cursor visible
    let start = if cursor > width.saturating_sub(1) {
        cursor - width + 1
    } else {
        0
    };
    let end = (start + width).min(input.len());
    VisibleSlice {
        text: input[start..end].to_string(),
        cursor_offset: cursor - start,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn char_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn test_dialog_show_hide() {
        let mut dialog = ConnectionDialog::new();
        assert!(!dialog.is_visible());

        dialog.show();
        assert!(dialog.is_visible());
        assert_eq!(dialog.focus, DialogFocus::UrlInput);
        assert!(dialog.url_input.is_empty());

        dialog.hide();
        assert!(!dialog.is_visible());
        assert!(dialog.url_input.is_empty());
    }

    #[test]
    fn test_dialog_tab_cycles_focus() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();
        dialog.connections.clear(); // ensure empty for this test
        assert_eq!(dialog.focus, DialogFocus::UrlInput);

        dialog.handle_key(key(KeyCode::Tab));
        assert_eq!(dialog.focus, DialogFocus::NameInput);

        // With no connections, Tab from Name goes back to URL
        dialog.handle_key(key(KeyCode::Tab));
        assert_eq!(dialog.focus, DialogFocus::UrlInput);
    }

    #[test]
    fn test_dialog_tab_includes_list_when_connections_exist() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();
        dialog.connections = vec![ConnectionConfig {
            name: "test".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "mydb".to_string(),
            username: "user".to_string(),
            password: None,
            ssl_mode: crate::config::connections::SslMode::Prefer,
        }];

        dialog.handle_key(key(KeyCode::Tab)); // URL → Name
        assert_eq!(dialog.focus, DialogFocus::NameInput);

        dialog.handle_key(key(KeyCode::Tab)); // Name → List
        assert_eq!(dialog.focus, DialogFocus::SavedList);

        dialog.handle_key(key(KeyCode::Tab)); // List → URL
        assert_eq!(dialog.focus, DialogFocus::UrlInput);
    }

    #[test]
    fn test_dialog_text_input() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();

        // Type some characters
        dialog.handle_key(char_key('a'));
        dialog.handle_key(char_key('b'));
        dialog.handle_key(char_key('c'));
        assert_eq!(dialog.url_input, "abc");
        assert_eq!(dialog.url_cursor, 3);

        // Backspace
        dialog.handle_key(key(KeyCode::Backspace));
        assert_eq!(dialog.url_input, "ab");
        assert_eq!(dialog.url_cursor, 2);

        // Move left, insert
        dialog.handle_key(key(KeyCode::Left));
        assert_eq!(dialog.url_cursor, 1);
        dialog.handle_key(char_key('x'));
        assert_eq!(dialog.url_input, "axb");
        assert_eq!(dialog.url_cursor, 2);
    }

    #[test]
    fn test_dialog_enter_parses_url() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();

        // Type a valid URL
        let url = "postgres://user:pass@localhost/mydb";
        for c in url.chars() {
            dialog.handle_key(char_key(c));
        }

        let action = dialog.handle_key(key(KeyCode::Enter));
        match action {
            DialogAction::Connect(config) => {
                assert_eq!(config.host, "localhost");
                assert_eq!(config.username, "user");
                assert_eq!(config.database, "mydb");
            }
            _ => panic!("Expected Connect action"),
        }
    }

    #[test]
    fn test_dialog_invalid_url_shows_error() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();

        // Type an invalid URL
        for c in "not-a-url".chars() {
            dialog.handle_key(char_key(c));
        }

        let action = dialog.handle_key(key(KeyCode::Enter));
        assert!(matches!(action, DialogAction::Consumed));
        assert!(dialog.error.is_some());
    }

    #[test]
    fn test_dialog_esc_dismisses() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();

        let action = dialog.handle_key(key(KeyCode::Esc));
        assert!(matches!(action, DialogAction::Dismissed));
    }

    #[test]
    fn test_dialog_enter_on_saved_loads_url() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();
        dialog.connections = vec![ConnectionConfig {
            name: "my-db".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            database: "mydb".to_string(),
            username: "user".to_string(),
            password: Some("pass".to_string()),
            ssl_mode: crate::config::connections::SslMode::Prefer,
        }];

        // Switch to saved list
        dialog.focus = DialogFocus::SavedList;
        dialog.selected = 0;

        let action = dialog.handle_key(key(KeyCode::Enter));
        assert!(matches!(action, DialogAction::Consumed));

        // URL should be populated from saved connection
        assert!(!dialog.url_input.is_empty());
        assert!(dialog.url_input.contains("localhost"));
        // Name should be loaded
        assert_eq!(dialog.name_input, "my-db");
        // Focus should switch to URL input for editing
        assert_eq!(dialog.focus, DialogFocus::UrlInput);
    }

    #[test]
    fn test_dialog_empty_url_shows_error() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();

        let action = dialog.handle_key(key(KeyCode::Enter));
        assert!(matches!(action, DialogAction::Consumed));
        assert_eq!(dialog.error.as_deref(), Some("URL is required"));
    }

    #[test]
    fn test_dialog_home_end_in_text_input() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();

        for c in "hello".chars() {
            dialog.handle_key(char_key(c));
        }
        assert_eq!(dialog.url_cursor, 5);

        dialog.handle_key(key(KeyCode::Home));
        assert_eq!(dialog.url_cursor, 0);

        dialog.handle_key(key(KeyCode::End));
        assert_eq!(dialog.url_cursor, 5);
    }

    #[test]
    fn test_dialog_delete_key() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();

        for c in "abc".chars() {
            dialog.handle_key(char_key(c));
        }
        dialog.handle_key(key(KeyCode::Home)); // cursor at 0
        dialog.handle_key(key(KeyCode::Delete));
        assert_eq!(dialog.url_input, "bc");
        assert_eq!(dialog.url_cursor, 0);
    }

    #[test]
    fn test_dialog_list_navigation() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();
        dialog.connections = vec![
            ConnectionConfig {
                name: "db1".to_string(),
                host: "h1".to_string(),
                port: 5432,
                database: "d1".to_string(),
                username: "u1".to_string(),
                password: None,
                ssl_mode: crate::config::connections::SslMode::Prefer,
            },
            ConnectionConfig {
                name: "db2".to_string(),
                host: "h2".to_string(),
                port: 5432,
                database: "d2".to_string(),
                username: "u2".to_string(),
                password: None,
                ssl_mode: crate::config::connections::SslMode::Prefer,
            },
        ];
        dialog.focus = DialogFocus::SavedList;
        assert_eq!(dialog.selected, 0);

        dialog.handle_key(key(KeyCode::Down));
        assert_eq!(dialog.selected, 1);

        // Can't go past end
        dialog.handle_key(key(KeyCode::Down));
        assert_eq!(dialog.selected, 1);

        dialog.handle_key(key(KeyCode::Up));
        assert_eq!(dialog.selected, 0);

        // Can't go before start
        dialog.handle_key(key(KeyCode::Up));
        assert_eq!(dialog.selected, 0);
    }

    #[test]
    fn test_visible_slice_short_input() {
        let result = visible_slice("hello", 3, 20);
        assert_eq!(result.text, "hello");
        assert_eq!(result.cursor_offset, 3);
    }

    #[test]
    fn test_visible_slice_scrolled() {
        let result = visible_slice("abcdefghij", 8, 5);
        // Cursor at 8, width 5: should show chars 4..9
        assert_eq!(result.text.len(), 5);
        assert!(result.cursor_offset < 5);
    }

    #[test]
    fn test_backtab_reverse_cycles() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();
        dialog.connections.clear(); // ensure empty for this test

        assert_eq!(dialog.focus, DialogFocus::UrlInput);

        dialog.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(dialog.focus, DialogFocus::NameInput);

        dialog.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(dialog.focus, DialogFocus::UrlInput);
    }

    #[test]
    fn test_enter_with_name_sets_config_name() {
        let mut dialog = ConnectionDialog::new();
        dialog.show();

        // Type URL
        for c in "postgres://user:pass@localhost/mydb".chars() {
            dialog.handle_key(char_key(c));
        }

        // Switch to name field and type a name
        dialog.handle_key(key(KeyCode::Tab));
        for c in "my-local".chars() {
            dialog.handle_key(char_key(c));
        }

        // Press Enter (from name field)
        let action = dialog.handle_key(key(KeyCode::Enter));
        match action {
            DialogAction::Connect(config) => {
                assert_eq!(config.name, "my-local");
                assert_eq!(config.host, "localhost");
            }
            _ => panic!("Expected Connect action"),
        }
    }
}

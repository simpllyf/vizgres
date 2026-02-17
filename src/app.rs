//! Application state and event handling
//!
//! Central state machine: events come in, state updates, actions go out.

use crate::commands::{Command, parse_command};
use crate::db::{PostgresProvider, QueryResults};
use crate::error::Result;
use crate::ui::Component;
use crate::ui::command_bar::CommandBar;
use crate::ui::editor::QueryEditor;
use crate::ui::inspector::Inspector;
use crate::ui::results::ResultsViewer;
use crate::ui::tree::TreeBrowser;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Main application state
pub struct App {
    /// Current database connection
    pub connection: Option<PostgresProvider>,

    /// Name of current connection profile
    pub connection_name: Option<String>,

    /// Which panel currently has focus
    pub focus: PanelFocus,

    /// Focus before command bar was opened (to restore on Escape)
    pub previous_focus: PanelFocus,

    /// UI Components
    pub tree_browser: TreeBrowser,
    pub editor: QueryEditor,
    pub results_viewer: ResultsViewer,
    pub command_bar: CommandBar,
    pub inspector: Inspector,

    /// Status message to display
    pub status_message: Option<StatusMessage>,

    /// Persistent clipboard handle (kept alive to avoid Linux clipboard drop race)
    clipboard: Option<arboard::Clipboard>,

    /// Whether the application is running
    pub running: bool,
}

/// Panel focus state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    TreeBrowser,
    QueryEditor,
    ResultsViewer,
    CommandBar,
    Inspector,
}

/// Status message with severity level
pub struct StatusMessage {
    pub message: String,
    pub level: StatusLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// Application events from the event loop
pub enum AppEvent {
    /// Keyboard input event
    Key(KeyEvent),
    /// Terminal resize event
    Resize,
    /// Query execution completed successfully
    QueryCompleted(QueryResults),
    /// Query execution failed
    QueryFailed(String),
}

/// Actions returned by event handlers for the main loop to execute
pub enum Action {
    ExecuteQuery(String),
    LoadSchema,
    Quit,
    None,
}

impl App {
    pub fn new() -> Self {
        Self {
            connection: None,
            connection_name: None,
            focus: PanelFocus::QueryEditor,
            previous_focus: PanelFocus::QueryEditor,
            tree_browser: TreeBrowser::new(),
            editor: QueryEditor::new(),
            results_viewer: ResultsViewer::new(),
            command_bar: CommandBar::new(),
            inspector: Inspector::new(),
            status_message: None,
            clipboard: arboard::Clipboard::new().ok(),
            running: true,
        }
    }

    /// Handle an application event and return resulting action
    pub fn handle_event(&mut self, event: AppEvent) -> Result<Action> {
        match event {
            AppEvent::Key(key) => Ok(self.handle_key(key)),
            AppEvent::Resize => Ok(Action::None),
            AppEvent::QueryCompleted(results) => {
                let count = results.row_count;
                let time = results.execution_time;
                self.results_viewer.set_results(results);
                self.set_status(
                    format!("{} rows in {:.1}ms", count, time.as_secs_f64() * 1000.0),
                    StatusLevel::Success,
                );
                self.focus = PanelFocus::ResultsViewer;
                Ok(Action::None)
            }
            AppEvent::QueryFailed(err) => {
                self.results_viewer.set_error(err.clone());
                self.set_status("Query failed".to_string(), StatusLevel::Error);
                self.focus = PanelFocus::ResultsViewer;
                Ok(Action::None)
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Action {
        // Clear toast on any user action
        self.status_message = None;

        // Global keybindings (always active)
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
            return Action::Quit;
        }

        // Ctrl+P opens command bar from anywhere (except when already in it)
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('p')
            && self.focus != PanelFocus::CommandBar
        {
            self.open_command_bar();
            return Action::None;
        }

        // Tab cycles focus (except when in command bar or inspector)
        if key.code == KeyCode::Tab
            && self.focus != PanelFocus::CommandBar
            && self.focus != PanelFocus::Inspector
        {
            self.cycle_focus();
            return Action::None;
        }

        // Backtab (Shift+Tab) cycles focus backwards
        if key.code == KeyCode::BackTab
            && self.focus != PanelFocus::CommandBar
            && self.focus != PanelFocus::Inspector
        {
            self.cycle_focus_reverse();
            return Action::None;
        }

        match self.focus {
            PanelFocus::QueryEditor => self.handle_editor_key(key),
            PanelFocus::ResultsViewer => self.handle_results_key(key),
            PanelFocus::TreeBrowser => self.handle_tree_key(key),
            PanelFocus::CommandBar => self.handle_command_bar_key(key),
            PanelFocus::Inspector => self.handle_inspector_key(key),
        }
    }

    fn handle_editor_key(&mut self, key: KeyEvent) -> Action {
        // Execute query: Ctrl+Enter or F5
        let is_execute = key.code == KeyCode::F(5)
            || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Enter);
        if is_execute {
            let sql = self.editor.get_content();
            if !sql.trim().is_empty() {
                self.set_status("Executing query...".to_string(), StatusLevel::Info);
                return Action::ExecuteQuery(sql);
            }
            return Action::None;
        }

        // Ctrl+L = clear editor
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('l') {
            self.editor.clear();
            return Action::None;
        }

        self.editor.handle_key(key);
        Action::None
    }

    fn handle_results_key(&mut self, key: KeyEvent) -> Action {
        // Enter opens inspector
        if key.code == KeyCode::Enter {
            if let Some((value, col_name, data_type)) = self.results_viewer.selected_cell_info() {
                self.inspector.show(value, col_name, data_type);
                self.previous_focus = self.focus;
                self.focus = PanelFocus::Inspector;
            }
            return Action::None;
        }

        // y = copy cell, Y = copy row
        if key.code == KeyCode::Char('y') && !key.modifiers.contains(KeyModifiers::SHIFT) {
            if let Some(text) = self.results_viewer.selected_cell_text() {
                self.copy_to_clipboard(&text);
            }
            return Action::None;
        }
        if key.code == KeyCode::Char('Y') {
            if let Some(text) = self.results_viewer.selected_row_text() {
                self.copy_to_clipboard(&text);
            }
            return Action::None;
        }

        self.results_viewer.handle_key(key);
        Action::None
    }

    fn handle_tree_key(&mut self, key: KeyEvent) -> Action {
        self.tree_browser.handle_key(key);
        Action::None
    }

    fn handle_command_bar_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter => {
                let input = self.command_bar.input().to_string();
                self.command_bar.deactivate();
                self.focus = self.previous_focus;

                if input.is_empty() {
                    return Action::None;
                }

                match parse_command(&input) {
                    Ok(cmd) => self.execute_command(cmd),
                    Err(e) => {
                        self.set_status(e.to_string(), StatusLevel::Error);
                        Action::None
                    }
                }
            }
            KeyCode::Esc => {
                self.command_bar.deactivate();
                self.focus = self.previous_focus;
                Action::None
            }
            _ => {
                self.command_bar.handle_key(key);
                Action::None
            }
        }
    }

    fn handle_inspector_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => {
                self.inspector.hide();
                self.focus = self.previous_focus;
                Action::None
            }
            KeyCode::Char('y') => {
                if let Some(text) = self.inspector.content_text() {
                    self.copy_to_clipboard(&text);
                }
                Action::None
            }
            _ => {
                self.inspector.handle_key(key);
                Action::None
            }
        }
    }

    fn execute_command(&mut self, command: Command) -> Action {
        match command {
            Command::Refresh => {
                self.set_status("Refreshing schema...".to_string(), StatusLevel::Info);
                Action::LoadSchema
            }
            Command::Clear => {
                self.editor.clear();
                Action::None
            }
            Command::Help => {
                self.set_status(
                    "Tab=cycle | Ctrl+Q=quit | F5=run | Ctrl+P=commands | /help".to_string(),
                    StatusLevel::Info,
                );
                Action::None
            }
            Command::Quit => Action::Quit,
        }
    }

    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            PanelFocus::TreeBrowser => PanelFocus::QueryEditor,
            PanelFocus::QueryEditor => PanelFocus::ResultsViewer,
            PanelFocus::ResultsViewer => PanelFocus::TreeBrowser,
            other => other,
        };
    }

    fn cycle_focus_reverse(&mut self) {
        self.focus = match self.focus {
            PanelFocus::TreeBrowser => PanelFocus::ResultsViewer,
            PanelFocus::QueryEditor => PanelFocus::TreeBrowser,
            PanelFocus::ResultsViewer => PanelFocus::QueryEditor,
            other => other,
        };
    }

    fn open_command_bar(&mut self) {
        self.previous_focus = self.focus;
        self.focus = PanelFocus::CommandBar;
        self.command_bar.activate();
    }

    pub fn set_status(&mut self, message: String, level: StatusLevel) {
        self.status_message = Some(StatusMessage { message, level });
    }

    fn copy_to_clipboard(&mut self, text: &str) {
        if let Some(clipboard) = self.clipboard.as_mut() {
            match clipboard.set_text(text) {
                Ok(()) => self.set_status("Copied to clipboard".to_string(), StatusLevel::Success),
                Err(e) => {
                    self.set_status(format!("Clipboard error: {}", e), StatusLevel::Warning);
                }
            }
        } else {
            self.set_status("Clipboard unavailable".to_string(), StatusLevel::Warning);
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_new_has_correct_defaults() {
        let app = App::new();
        assert!(app.connection.is_none());
        assert_eq!(app.focus, PanelFocus::QueryEditor);
        assert!(app.running);
    }

    #[test]
    fn test_cycle_focus() {
        let mut app = App::new();
        assert_eq!(app.focus, PanelFocus::QueryEditor);
        app.cycle_focus();
        assert_eq!(app.focus, PanelFocus::ResultsViewer);
        app.cycle_focus();
        assert_eq!(app.focus, PanelFocus::TreeBrowser);
        app.cycle_focus();
        assert_eq!(app.focus, PanelFocus::QueryEditor);
    }

    #[test]
    fn test_status_cleared_on_set() {
        let mut app = App::new();
        assert!(app.status_message.is_none());

        app.set_status("test".to_string(), StatusLevel::Info);
        assert!(app.status_message.is_some());
        assert_eq!(app.status_message.as_ref().unwrap().message, "test");
    }
}

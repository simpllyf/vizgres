//! Application state and event handling
//!
//! Central state machine: events come in, state updates, actions go out.

use crate::commands::{Command, parse_command};
use crate::db::{PostgresProvider, QueryResults};
use crate::error::Result;
use crate::keymap::{KeyAction, KeyMap};
use crate::ui::Component;
use crate::ui::ComponentAction;
use crate::ui::command_bar::CommandBar;
use crate::ui::editor::QueryEditor;
use crate::ui::inspector::Inspector;
use crate::ui::results::ResultsViewer;
use crate::ui::tree::TreeBrowser;
use crossterm::event::KeyEvent;

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

    /// Data-driven keybinding configuration
    keymap: KeyMap,

    /// Status message to display
    pub status_message: Option<StatusMessage>,

    /// Persistent clipboard handle (kept alive to avoid Linux clipboard drop race)
    clipboard: Option<arboard::Clipboard>,

    /// Whether the application is running
    pub running: bool,
}

/// Panel focus state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
            keymap: KeyMap::default(),
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
        self.status_message = None;

        // Try KeyMap first — global bindings, then panel-specific
        if let Some(key_action) = self.keymap.resolve(self.focus, key) {
            // Global actions that shouldn't fire from certain panels
            match key_action {
                KeyAction::OpenCommandBar if self.focus == PanelFocus::CommandBar => {}
                KeyAction::CycleFocus | KeyAction::CycleFocusReverse
                    if self.focus == PanelFocus::CommandBar
                        || self.focus == PanelFocus::Inspector => {}
                _ => return self.execute_key_action(key_action),
            }
        }

        // Fall through to component for free-form text input (editor, command bar)
        let component_action = match self.focus {
            PanelFocus::QueryEditor => self.editor.handle_key(key),
            PanelFocus::CommandBar => self.command_bar.handle_key(key),
            _ => ComponentAction::Ignored,
        };
        self.process_component_action(component_action)
    }

    fn execute_key_action(&mut self, action: KeyAction) -> Action {
        match action {
            // ── Global ───────────────────────────────────────
            KeyAction::Quit => Action::Quit,
            KeyAction::OpenCommandBar => {
                self.open_command_bar();
                Action::None
            }
            KeyAction::CycleFocus => {
                self.cycle_focus();
                Action::None
            }
            KeyAction::CycleFocusReverse => {
                self.cycle_focus_reverse();
                Action::None
            }

            // ── Navigation ───────────────────────────────────
            KeyAction::MoveUp => {
                match self.focus {
                    PanelFocus::ResultsViewer => self.results_viewer.move_up(),
                    PanelFocus::TreeBrowser => self.tree_browser.move_up(),
                    PanelFocus::Inspector => self.inspector.scroll_up(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::MoveDown => {
                match self.focus {
                    PanelFocus::ResultsViewer => self.results_viewer.move_down(),
                    PanelFocus::TreeBrowser => self.tree_browser.move_down(),
                    PanelFocus::Inspector => self.inspector.scroll_down(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::MoveLeft => {
                if self.focus == PanelFocus::ResultsViewer {
                    self.results_viewer.move_left();
                }
                Action::None
            }
            KeyAction::MoveRight => {
                if self.focus == PanelFocus::ResultsViewer {
                    self.results_viewer.move_right();
                }
                Action::None
            }
            KeyAction::PageUp => {
                match self.focus {
                    PanelFocus::ResultsViewer => self.results_viewer.page_up(),
                    PanelFocus::Inspector => self.inspector.page_up(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::PageDown => {
                match self.focus {
                    PanelFocus::ResultsViewer => self.results_viewer.page_down(),
                    PanelFocus::Inspector => self.inspector.page_down(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::GoToTop => {
                match self.focus {
                    PanelFocus::ResultsViewer => self.results_viewer.go_to_top(),
                    PanelFocus::Inspector => self.inspector.scroll_to_top(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::GoToBottom => {
                match self.focus {
                    PanelFocus::ResultsViewer => self.results_viewer.go_to_bottom(),
                    PanelFocus::Inspector => self.inspector.scroll_to_bottom(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::Home => {
                if self.focus == PanelFocus::ResultsViewer {
                    self.results_viewer.go_to_home();
                }
                Action::None
            }
            KeyAction::End => {
                if self.focus == PanelFocus::ResultsViewer {
                    self.results_viewer.go_to_end();
                }
                Action::None
            }

            // ── Editor ───────────────────────────────────────
            KeyAction::ExecuteQuery => {
                let sql = self.editor.get_content();
                if !sql.trim().is_empty() {
                    self.set_status("Executing query...".to_string(), StatusLevel::Info);
                    Action::ExecuteQuery(sql)
                } else {
                    Action::None
                }
            }
            KeyAction::ClearEditor => {
                self.editor.clear();
                Action::None
            }

            // ── Results ──────────────────────────────────────
            KeyAction::OpenInspector => {
                if let Some((value, col_name, data_type)) =
                    self.results_viewer.selected_cell_info()
                {
                    self.inspector.show(value, col_name, data_type);
                    self.previous_focus = self.focus;
                    self.focus = PanelFocus::Inspector;
                }
                Action::None
            }
            KeyAction::CopyCell => {
                if let Some(text) = self.results_viewer.selected_cell_text() {
                    self.copy_to_clipboard(&text);
                }
                Action::None
            }
            KeyAction::CopyRow => {
                if let Some(text) = self.results_viewer.selected_row_text() {
                    self.copy_to_clipboard(&text);
                }
                Action::None
            }

            // ── Inspector ────────────────────────────────────
            KeyAction::CopyContent => {
                if let Some(text) = self.inspector.content_text() {
                    self.copy_to_clipboard(&text);
                }
                Action::None
            }

            // ── Tree ─────────────────────────────────────────
            KeyAction::ToggleExpand => {
                self.tree_browser.toggle_expand();
                Action::None
            }
            KeyAction::Expand => {
                self.tree_browser.expand_current();
                Action::None
            }
            KeyAction::Collapse => {
                self.tree_browser.collapse_current();
                Action::None
            }

            // ── Modal (inspector, command bar) ───────────────
            KeyAction::Dismiss => {
                match self.focus {
                    PanelFocus::Inspector => {
                        self.inspector.hide();
                        self.focus = self.previous_focus;
                    }
                    PanelFocus::CommandBar => {
                        self.command_bar.deactivate();
                        self.focus = self.previous_focus;
                    }
                    _ => {}
                }
                Action::None
            }
            KeyAction::Submit => {
                if self.focus == PanelFocus::CommandBar {
                    let input = self.command_bar.input_text().to_string();
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
                } else {
                    Action::None
                }
            }
        }
    }

    fn process_component_action(&mut self, _action: ComponentAction) -> Action {
        // Components only return Consumed/Ignored for text input.
        // All meaningful actions are handled by KeyMap → execute_key_action.
        Action::None
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

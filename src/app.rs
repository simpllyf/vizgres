//! Application state and event handling
//!
//! Central state machine: events come in, state updates, actions go out.

use crate::commands::{Command, parse_command};
use crate::db::QueryResults;
use crate::db::schema::SchemaTree;
use crate::error::Result;
use crate::history::QueryHistory;
use crate::keymap::{KeyAction, KeyMap};
use crate::ui::Component;
use crate::ui::ComponentAction;
use crate::ui::command_bar::CommandBar;
use crate::ui::editor::QueryEditor;
use crate::ui::inspector::Inspector;
use crate::ui::results::ResultsViewer;
use crate::ui::theme::Theme;
use crate::ui::tree::TreeBrowser;
use crossterm::event::KeyEvent;

/// Main application state
pub struct App {
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

    /// Query history for Ctrl+Up/Down navigation
    history: QueryHistory,

    /// Data-driven keybinding configuration
    keymap: KeyMap,

    /// UI theme (created once, reused every frame)
    pub theme: Theme,

    /// Status message to display
    pub status_message: Option<StatusMessage>,

    /// Persistent clipboard handle (kept alive to avoid Linux clipboard drop race)
    clipboard: Option<arboard::Clipboard>,

    /// Error from clipboard initialization (preserved for diagnostics)
    clipboard_error: Option<String>,

    /// Whether a query is currently in flight (for cancel support)
    pub query_running: bool,

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
    /// Schema loaded successfully
    SchemaLoaded(SchemaTree),
    /// Schema loading failed
    SchemaFailed(String),
    /// Background database connection lost
    ConnectionLost(String),
}

/// Actions returned by event handlers for the main loop to execute
pub enum Action {
    ExecuteQuery(String),
    CancelQuery,
    LoadSchema,
    Quit,
    None,
}

impl App {
    pub fn new() -> Self {
        let (clipboard, clipboard_error) = match arboard::Clipboard::new() {
            Ok(c) => (Some(c), None),
            Err(e) => (None, Some(e.to_string())),
        };
        Self {
            connection_name: None,
            focus: PanelFocus::QueryEditor,
            previous_focus: PanelFocus::QueryEditor,
            tree_browser: TreeBrowser::new(),
            editor: QueryEditor::new(),
            results_viewer: ResultsViewer::new(),
            command_bar: CommandBar::new(),
            inspector: Inspector::new(),
            history: QueryHistory::load(500),
            keymap: KeyMap::default(),
            theme: Theme::default(),
            status_message: None,
            clipboard,
            clipboard_error,
            query_running: false,
            running: true,
        }
    }

    /// Create an app pre-loaded with a connection name and schema
    pub fn with_connection(name: String, schema: SchemaTree) -> Self {
        let mut app = Self::new();
        app.connection_name = Some(name);
        app.tree_browser.set_schema(schema);
        app
    }

    /// Handle an application event and return resulting action
    pub fn handle_event(&mut self, event: AppEvent) -> Result<Action> {
        match event {
            AppEvent::Key(key) => Ok(self.handle_key(key)),
            AppEvent::Resize => Ok(Action::None),
            AppEvent::QueryCompleted(results) => {
                self.query_running = false;
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
                self.query_running = false;
                let cancelled = err.contains("canceling statement due to user request");
                self.results_viewer.set_error(err);
                self.set_status(
                    if cancelled {
                        "Query cancelled".to_string()
                    } else {
                        "Query failed".to_string()
                    },
                    if cancelled {
                        StatusLevel::Warning
                    } else {
                        StatusLevel::Error
                    },
                );
                self.focus = PanelFocus::ResultsViewer;
                Ok(Action::None)
            }
            AppEvent::SchemaLoaded(schema) => {
                self.tree_browser.set_schema(schema);
                self.set_status("Schema refreshed".to_string(), StatusLevel::Info);
                Ok(Action::None)
            }
            AppEvent::SchemaFailed(err) => {
                self.set_status(
                    format!("Schema refresh failed: {}", err),
                    StatusLevel::Error,
                );
                Ok(Action::None)
            }
            AppEvent::ConnectionLost(msg) => {
                self.set_status(msg, StatusLevel::Error);
                Ok(Action::None)
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Action {
        self.status_message = None;

        // Try KeyMap first — global bindings, then panel-specific
        if let Some(key_action) = self.keymap.resolve(self.focus, key) {
            // Suppress certain global actions in modal panels to avoid
            // the key falling through to the component (e.g., Ctrl+P
            // inserting 'p' in the command bar).
            match key_action {
                KeyAction::OpenCommandBar if self.focus == PanelFocus::CommandBar => {
                    return Action::None;
                }
                KeyAction::CycleFocus | KeyAction::CycleFocusReverse
                    if self.focus == PanelFocus::CommandBar
                        || self.focus == PanelFocus::Inspector =>
                {
                    return Action::None;
                }
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
                    self.history.push(&sql);
                    self.set_status("Executing query...".to_string(), StatusLevel::Info);
                    Action::ExecuteQuery(sql)
                } else {
                    Action::None
                }
            }
            KeyAction::ExplainQuery => {
                let sql = self.editor.get_content();
                if !sql.trim().is_empty() {
                    let explain = format!("EXPLAIN ANALYZE {}", sql.trim());
                    self.history.push(&sql);
                    self.set_status("Running EXPLAIN ANALYZE...".to_string(), StatusLevel::Info);
                    Action::ExecuteQuery(explain)
                } else {
                    Action::None
                }
            }
            KeyAction::CancelQuery => {
                if self.query_running {
                    self.set_status("Cancelling query...".to_string(), StatusLevel::Warning);
                    Action::CancelQuery
                } else {
                    Action::None
                }
            }
            KeyAction::ClearEditor => {
                self.editor.clear();
                Action::None
            }
            KeyAction::Undo => {
                self.editor.undo();
                Action::None
            }
            KeyAction::Redo => {
                self.editor.redo();
                Action::None
            }
            KeyAction::HistoryBack => {
                let current = self.editor.get_content();
                if let Some(entry) = self.history.back(&current) {
                    self.editor.set_content(entry.to_string());
                }
                Action::None
            }
            KeyAction::HistoryForward => {
                if let Some(entry) = self.history.forward() {
                    self.editor.set_content(entry.to_string());
                }
                Action::None
            }

            // ── Results ──────────────────────────────────────
            KeyAction::OpenInspector => {
                if let Some((value, col_name, data_type)) = self.results_viewer.selected_cell_info()
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

    fn process_component_action(&mut self, action: ComponentAction) -> Action {
        // Components only return Consumed/Ignored for text input.
        // All meaningful actions are handled by KeyMap → execute_key_action.
        match action {
            ComponentAction::Consumed | ComponentAction::Ignored => Action::None,
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
            let reason = self.clipboard_error.as_deref().unwrap_or("unknown reason");
            self.set_status(
                format!("Clipboard unavailable: {}", reason),
                StatusLevel::Warning,
            );
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
        assert!(app.connection_name.is_none());
        assert_eq!(app.focus, PanelFocus::QueryEditor);
        assert!(app.running);
    }

    #[test]
    fn test_with_connection_constructor() {
        use crate::db::schema::{Schema, SchemaTree, Table};
        let schema = SchemaTree {
            schemas: vec![Schema {
                name: "public".to_string(),
                tables: vec![Table {
                    name: "users".to_string(),
                    columns: vec![],
                }],
                views: vec![],
                indexes: vec![],
                functions: vec![],
            }],
        };
        let app = App::with_connection("test-db".to_string(), schema);
        assert_eq!(app.connection_name.as_deref(), Some("test-db"));
    }

    #[test]
    fn test_schema_loaded_event() {
        use crate::db::schema::SchemaTree;
        let mut app = App::new();
        let schema = SchemaTree::new();
        let action = app.handle_event(AppEvent::SchemaLoaded(schema)).unwrap();
        assert!(matches!(action, Action::None));
        assert_eq!(
            app.status_message.as_ref().unwrap().message,
            "Schema refreshed"
        );
    }

    #[test]
    fn test_schema_failed_event() {
        let mut app = App::new();
        let action = app
            .handle_event(AppEvent::SchemaFailed("connection lost".to_string()))
            .unwrap();
        assert!(matches!(action, Action::None));
        assert!(
            app.status_message
                .as_ref()
                .unwrap()
                .message
                .contains("Schema refresh failed")
        );
    }

    #[test]
    fn test_connection_lost_event() {
        let mut app = App::new();
        let action = app
            .handle_event(AppEvent::ConnectionLost(
                "Connection lost: server closed".to_string(),
            ))
            .unwrap();
        assert!(matches!(action, Action::None));
        let msg = &app.status_message.as_ref().unwrap();
        assert!(msg.message.contains("Connection lost"));
        assert_eq!(msg.level, StatusLevel::Error);
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

    #[test]
    fn test_suppressed_global_keys_dont_fall_through() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();

        // Ctrl+P in command bar should be suppressed (not insert 'p')
        app.focus = PanelFocus::CommandBar;
        app.command_bar.activate();
        let ctrl_p = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        app.handle_key(ctrl_p);
        assert_eq!(app.command_bar.input_text(), "");

        // Tab in inspector should be suppressed (not cycle focus)
        app.focus = PanelFocus::Inspector;
        let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        app.handle_key(tab);
        assert_eq!(app.focus, PanelFocus::Inspector);
    }

    #[test]
    fn test_cycle_focus_reverse() {
        let mut app = App::new();
        assert_eq!(app.focus, PanelFocus::QueryEditor);
        app.cycle_focus_reverse();
        assert_eq!(app.focus, PanelFocus::TreeBrowser);
        app.cycle_focus_reverse();
        assert_eq!(app.focus, PanelFocus::ResultsViewer);
        app.cycle_focus_reverse();
        assert_eq!(app.focus, PanelFocus::QueryEditor);
    }

    #[test]
    fn test_cycle_focus_noop_in_modal() {
        let mut app = App::new();
        app.focus = PanelFocus::Inspector;
        app.cycle_focus();
        assert_eq!(app.focus, PanelFocus::Inspector);

        app.focus = PanelFocus::CommandBar;
        app.cycle_focus();
        assert_eq!(app.focus, PanelFocus::CommandBar);
    }

    #[test]
    fn test_execute_query_ignores_empty() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;
        // F5 with empty editor should return None
        let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
        let action = app.handle_key(f5);
        assert!(matches!(action, Action::None));
    }

    #[test]
    fn test_explain_query_prefixes_sql() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;
        app.editor.set_content("SELECT * FROM users".to_string());

        let ctrl_e = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
        let action = app.handle_key(ctrl_e);
        match action {
            Action::ExecuteQuery(sql) => {
                assert_eq!(sql, "EXPLAIN ANALYZE SELECT * FROM users");
            }
            other => panic!(
                "Expected ExecuteQuery, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn test_explain_ignores_empty_editor() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;

        let ctrl_e = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
        let action = app.handle_key(ctrl_e);
        assert!(matches!(action, Action::None));
    }

    #[test]
    fn test_cancel_query_when_running() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;
        app.query_running = true;

        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let action = app.handle_key(esc);
        assert!(matches!(action, Action::CancelQuery));
    }

    #[test]
    fn test_cancel_query_noop_when_idle() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;
        // query_running is false by default

        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let action = app.handle_key(esc);
        assert!(matches!(action, Action::None));
    }

    #[test]
    fn test_query_completed_clears_running() {
        let mut app = App::new();
        app.query_running = true;

        let results =
            crate::db::QueryResults::new(vec![], vec![], std::time::Duration::from_millis(10), 0);
        app.handle_event(AppEvent::QueryCompleted(results)).unwrap();
        assert!(!app.query_running);
    }

    #[test]
    fn test_query_failed_clears_running() {
        let mut app = App::new();
        app.query_running = true;

        app.handle_event(AppEvent::QueryFailed("some error".to_string()))
            .unwrap();
        assert!(!app.query_running);
    }

    #[test]
    fn test_query_cancelled_shows_warning() {
        let mut app = App::new();
        app.query_running = true;

        app.handle_event(AppEvent::QueryFailed(
            "ERROR: canceling statement due to user request".to_string(),
        ))
        .unwrap();
        assert!(!app.query_running);
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.message, "Query cancelled");
        assert_eq!(msg.level, StatusLevel::Warning);
    }

    #[test]
    fn test_history_back_populates_editor() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;

        // Type and execute a query
        app.editor.set_content("SELECT 1".to_string());
        let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
        let action = app.handle_key(f5);
        assert!(matches!(action, Action::ExecuteQuery(_)));

        // Clear editor (simulating user clearing it)
        app.editor.clear();
        assert_eq!(app.editor.get_content(), "");

        // Ctrl+Up should recall the query
        let ctrl_up = KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL);
        app.handle_key(ctrl_up);
        assert_eq!(app.editor.get_content(), "SELECT 1");

        // Ctrl+Down should restore the draft (empty)
        let ctrl_down = KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL);
        app.handle_key(ctrl_down);
        assert_eq!(app.editor.get_content(), "");
    }
}

//! Application state and event handling
//!
//! Central state machine: events come in, state updates, actions go out.

use crate::commands::{Command, parse_command};
use crate::completer::{self, Completer};
use crate::config::ConnectionConfig;
use crate::config::settings::Settings;
use crate::db::QueryResults;
use crate::db::schema::SchemaTree;
use crate::error::Result;
use crate::export::ExportFormat;
use crate::history::QueryHistory;
use crate::keymap::{KeyAction, KeyMap};
use crate::ui::Component;
use crate::ui::ComponentAction;
use crate::ui::command_bar::CommandBar;
use crate::ui::connection_dialog::{ConnectionDialog, DialogAction};
use crate::ui::editor::QueryEditor;
use crate::ui::help::HelpOverlay;
use crate::ui::inspector::Inspector;
use crate::ui::results::ResultsViewer;
use crate::ui::theme::Theme;
use crate::ui::tree::TreeBrowser;
use crossterm::event::KeyEvent;

/// A single query tab containing its own editor, results, and completer
pub struct Tab {
    /// Stable identifier (monotonically increasing, never reused)
    pub id: usize,
    pub editor: QueryEditor,
    pub results_viewer: ResultsViewer,
    completer: Completer,
    /// Whether this tab has a query in flight
    pub query_running: bool,
}

impl Tab {
    fn new(id: usize) -> Self {
        Self {
            id,
            editor: QueryEditor::new(),
            results_viewer: ResultsViewer::new(),
            completer: Completer::new(),
            query_running: false,
        }
    }
}

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
    pub command_bar: CommandBar,
    pub inspector: Inspector,
    pub help: HelpOverlay,
    pub connection_dialog: ConnectionDialog,

    /// Query tabs (each has its own editor + results + completer)
    pub tabs: Vec<Tab>,
    /// Index into `tabs` for the currently active tab
    pub active_tab: usize,
    /// Next stable tab ID to assign
    next_tab_id: usize,

    /// Pending export format (set when Ctrl+S/Ctrl+J opens the filename prompt)
    pending_export: Option<ExportFormat>,

    /// Query history for Ctrl+Up/Down navigation
    history: QueryHistory,

    /// Maximum number of tabs allowed
    max_tabs: usize,

    /// Data-driven keybinding configuration
    pub keymap: KeyMap,

    /// UI theme (created once, reused every frame)
    pub theme: Theme,

    /// Status message to display
    pub status_message: Option<StatusMessage>,

    /// Persistent clipboard handle (kept alive to avoid Linux clipboard drop race)
    clipboard: Option<arboard::Clipboard>,

    /// Error from clipboard initialization (preserved for diagnostics)
    clipboard_error: Option<String>,

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
    Help,
    ConnectionDialog,
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
    QueryCompleted {
        results: QueryResults,
        tab_id: usize,
    },
    /// Query execution failed
    QueryFailed { error: String, tab_id: usize },
    /// Schema loaded successfully
    SchemaLoaded(SchemaTree),
    /// Schema loading failed
    SchemaFailed(String),
    /// Bracketed paste event
    Paste(String),
    /// Background database connection lost
    ConnectionLost(String),
}

/// Actions returned by event handlers for the main loop to execute
pub enum Action {
    ExecuteQuery { sql: String, tab_id: usize },
    CancelQuery,
    LoadSchema,
    Connect(ConnectionConfig),
    Quit,
    None,
}

impl App {
    pub fn new() -> Self {
        Self::new_with_settings(&Settings::default())
    }

    /// Create an app with custom settings (preview rows, max tabs, keybindings, etc.)
    pub fn new_with_settings(settings: &Settings) -> Self {
        let (clipboard, clipboard_error) = match arboard::Clipboard::new() {
            Ok(c) => (Some(c), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let (keymap, warnings) = KeyMap::from_config(&settings.keybindings);
        let mut app = Self {
            connection_name: None,
            focus: PanelFocus::QueryEditor,
            previous_focus: PanelFocus::QueryEditor,
            tree_browser: TreeBrowser::with_preview_rows(settings.settings.preview_rows),
            command_bar: CommandBar::new(),
            inspector: Inspector::new(),
            help: HelpOverlay::new(),
            connection_dialog: ConnectionDialog::new(),
            tabs: vec![Tab::new(0)],
            active_tab: 0,
            next_tab_id: 1,
            pending_export: None,
            history: QueryHistory::load(settings.settings.history_size),
            max_tabs: settings.settings.max_tabs,
            keymap,
            theme: Theme::default(),
            status_message: None,
            clipboard,
            clipboard_error,
            running: true,
        };
        if !warnings.is_empty() {
            app.set_status(
                format!("Config: {}", warnings.join("; ")),
                StatusLevel::Warning,
            );
        }
        app
    }

    /// Create an app pre-loaded with a connection name and schema
    pub fn with_connection(name: String, schema: SchemaTree, settings: &Settings) -> Self {
        let mut app = Self::new_with_settings(settings);
        app.connection_name = Some(name);
        app.tree_browser.set_schema(schema);
        app
    }

    /// Handle an application event and return resulting action
    pub fn handle_event(&mut self, event: AppEvent) -> Result<Action> {
        match event {
            AppEvent::Key(key) => Ok(self.handle_key(key)),
            AppEvent::Paste(data) => {
                if self.focus == PanelFocus::QueryEditor {
                    self.tab_mut().editor.insert_text(&data);
                    self.update_completions();
                }
                Ok(Action::None)
            }
            AppEvent::Resize => Ok(Action::None),
            AppEvent::QueryCompleted { results, tab_id } => {
                let count = results.row_count;
                let time = results.execution_time;
                if let Some(idx) = self.tab_index_by_id(tab_id) {
                    self.tabs[idx].query_running = false;
                    self.tabs[idx].results_viewer.set_results(results);
                    if idx == self.active_tab {
                        self.focus = PanelFocus::ResultsViewer;
                    }
                }
                self.set_status(
                    format!("{} rows in {:.1}ms", count, time.as_secs_f64() * 1000.0),
                    StatusLevel::Success,
                );
                Ok(Action::None)
            }
            AppEvent::QueryFailed { error, tab_id } => {
                let cancelled = error.contains("canceling statement due to user request");
                if let Some(idx) = self.tab_index_by_id(tab_id) {
                    self.tabs[idx].query_running = false;
                    self.tabs[idx].results_viewer.set_error(error);
                    if idx == self.active_tab {
                        self.focus = PanelFocus::ResultsViewer;
                    }
                }
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

        // Connection dialog intercepts all keys when visible
        if self.focus == PanelFocus::ConnectionDialog {
            return match self.connection_dialog.handle_key(key) {
                DialogAction::Connect(config) => {
                    self.connection_dialog.hide();
                    self.focus = self.previous_focus;
                    Action::Connect(config)
                }
                DialogAction::Dismissed => {
                    self.connection_dialog.hide();
                    self.focus = self.previous_focus;
                    Action::None
                }
                DialogAction::Consumed => Action::None,
            };
        }

        // Try KeyMap first — global bindings, then panel-specific
        if let Some(key_action) = self.keymap.resolve(self.focus, key) {
            // Suppress certain global actions in modal panels to avoid
            // the key falling through to the component (e.g., Ctrl+P
            // inserting 'p' in the command bar).
            match key_action {
                KeyAction::OpenCommandBar if self.focus == PanelFocus::CommandBar => {
                    return Action::None;
                }
                KeyAction::CycleFocus
                | KeyAction::CycleFocusReverse
                | KeyAction::NewTab
                | KeyAction::CloseTab
                | KeyAction::NextTab
                    if self.focus == PanelFocus::CommandBar
                        || self.focus == PanelFocus::Inspector
                        || self.focus == PanelFocus::Help
                        || self.focus == PanelFocus::ConnectionDialog =>
                {
                    return Action::None;
                }
                KeyAction::ShowHelp if self.focus == PanelFocus::Help => {
                    return Action::None;
                }
                _ => return self.execute_key_action(key_action),
            }
        }

        // Fall through to component for free-form text input (editor, command bar)
        let component_action = match self.focus {
            PanelFocus::QueryEditor => {
                let result = self.tab_mut().editor.handle_key(key);
                if matches!(result, ComponentAction::Consumed) {
                    self.update_completions();
                }
                result
            }
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

            KeyAction::ShowHelp => {
                self.previous_focus = self.focus;
                self.focus = PanelFocus::Help;
                self.help.show();
                Action::None
            }

            KeyAction::NewTab => {
                if !self.new_tab() {
                    self.set_status(
                        format!("Maximum {} tabs open", self.max_tabs),
                        StatusLevel::Warning,
                    );
                }
                Action::None
            }
            KeyAction::CloseTab => {
                if self.tab().query_running {
                    self.set_status(
                        "Cannot close tab while query is running".to_string(),
                        StatusLevel::Warning,
                    );
                } else if !self.close_tab() {
                    self.set_status(
                        "Cannot close the last tab".to_string(),
                        StatusLevel::Warning,
                    );
                }
                Action::None
            }
            KeyAction::NextTab => {
                self.next_tab();
                Action::None
            }

            // ── Navigation ───────────────────────────────────
            KeyAction::MoveUp => {
                match self.focus {
                    PanelFocus::ResultsViewer => self.tab_mut().results_viewer.move_up(),
                    PanelFocus::TreeBrowser => self.tree_browser.move_up(),
                    PanelFocus::Inspector => self.inspector.scroll_up(),
                    PanelFocus::Help => self.help.scroll_up(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::MoveDown => {
                match self.focus {
                    PanelFocus::ResultsViewer => self.tab_mut().results_viewer.move_down(),
                    PanelFocus::TreeBrowser => self.tree_browser.move_down(),
                    PanelFocus::Inspector => self.inspector.scroll_down(),
                    PanelFocus::Help => self.help.scroll_down(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::MoveLeft => {
                if self.focus == PanelFocus::ResultsViewer {
                    self.tab_mut().results_viewer.move_left();
                }
                Action::None
            }
            KeyAction::MoveRight => {
                if self.focus == PanelFocus::ResultsViewer {
                    self.tab_mut().results_viewer.move_right();
                }
                Action::None
            }
            KeyAction::PageUp => {
                match self.focus {
                    PanelFocus::ResultsViewer => self.tab_mut().results_viewer.page_up(),
                    PanelFocus::Inspector => self.inspector.page_up(),
                    PanelFocus::Help => self.help.page_up(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::PageDown => {
                match self.focus {
                    PanelFocus::ResultsViewer => self.tab_mut().results_viewer.page_down(),
                    PanelFocus::Inspector => self.inspector.page_down(),
                    PanelFocus::Help => self.help.page_down(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::GoToTop => {
                match self.focus {
                    PanelFocus::ResultsViewer => self.tab_mut().results_viewer.go_to_top(),
                    PanelFocus::Inspector => self.inspector.scroll_to_top(),
                    PanelFocus::Help => self.help.scroll_to_top(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::GoToBottom => {
                match self.focus {
                    PanelFocus::ResultsViewer => self.tab_mut().results_viewer.go_to_bottom(),
                    PanelFocus::Inspector => self.inspector.scroll_to_bottom(),
                    PanelFocus::Help => self.help.scroll_to_bottom(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::Home => {
                if self.focus == PanelFocus::ResultsViewer {
                    self.tab_mut().results_viewer.go_to_home();
                }
                Action::None
            }
            KeyAction::End => {
                if self.focus == PanelFocus::ResultsViewer {
                    self.tab_mut().results_viewer.go_to_end();
                }
                Action::None
            }

            // ── Editor ───────────────────────────────────────
            KeyAction::ExecuteQuery => {
                let sql = self.tab().editor.get_content();
                if !sql.trim().is_empty() {
                    let tab_id = self.tab().id;
                    self.tab_mut().query_running = true;
                    self.history.push(&sql);
                    self.set_status("Executing query...".to_string(), StatusLevel::Info);
                    Action::ExecuteQuery { sql, tab_id }
                } else {
                    Action::None
                }
            }
            KeyAction::ExplainQuery => {
                let sql = self.tab().editor.get_content();
                if !sql.trim().is_empty() {
                    let tab_id = self.tab().id;
                    let explain = format!("EXPLAIN ANALYZE {}", sql.trim());
                    self.tab_mut().query_running = true;
                    self.history.push(&sql);
                    self.set_status("Running EXPLAIN ANALYZE...".to_string(), StatusLevel::Info);
                    Action::ExecuteQuery {
                        sql: explain,
                        tab_id,
                    }
                } else {
                    Action::None
                }
            }
            KeyAction::CancelQuery => {
                if self.tabs.iter().any(|t| t.query_running) {
                    self.set_status("Cancelling query...".to_string(), StatusLevel::Warning);
                    Action::CancelQuery
                } else {
                    Action::None
                }
            }
            KeyAction::ClearEditor => {
                self.tab_mut().editor.clear();
                self.clear_completions();
                Action::None
            }
            KeyAction::Undo => {
                self.tab_mut().editor.undo();
                self.clear_completions();
                Action::None
            }
            KeyAction::Redo => {
                self.tab_mut().editor.redo();
                self.clear_completions();
                Action::None
            }
            KeyAction::FormatQuery => {
                let sql = self.tab().editor.get_content();
                if !sql.trim().is_empty() {
                    let formatted = sqlformat::format(
                        &sql,
                        &sqlformat::QueryParams::None,
                        &sqlformat::FormatOptions {
                            indent: sqlformat::Indent::Spaces(2),
                            uppercase: Some(true),
                            lines_between_queries: 1,
                            ..Default::default()
                        },
                    );
                    self.tab_mut().editor.replace_content(formatted);
                    self.clear_completions();
                    self.set_status("Query formatted".to_string(), StatusLevel::Info);
                }
                Action::None
            }
            KeyAction::NextCompletion => {
                let tab = &mut self.tabs[self.active_tab];
                if tab.completer.is_active() {
                    tab.editor.set_ghost_text(tab.completer.next());
                }
                Action::None
            }
            KeyAction::PrevCompletion => {
                let tab = &mut self.tabs[self.active_tab];
                if tab.completer.is_active() {
                    tab.editor.set_ghost_text(tab.completer.prev());
                }
                Action::None
            }
            KeyAction::HistoryBack => {
                let current = self.tab().editor.get_content();
                let entry = self.history.back(&current).map(|e| e.to_string());
                if let Some(text) = entry {
                    self.tab_mut().editor.set_content(text);
                }
                self.clear_completions();
                Action::None
            }
            KeyAction::HistoryForward => {
                let entry = self.history.forward().map(|e| e.to_string());
                if let Some(text) = entry {
                    self.tab_mut().editor.set_content(text);
                }
                self.clear_completions();
                Action::None
            }

            // ── Results ──────────────────────────────────────
            KeyAction::OpenInspector => {
                if let Some((value, col_name, data_type)) =
                    self.tab().results_viewer.selected_cell_info()
                {
                    self.inspector.show(value, col_name, data_type);
                    self.previous_focus = self.focus;
                    self.focus = PanelFocus::Inspector;
                }
                Action::None
            }
            KeyAction::CopyCell => {
                if let Some(text) = self.tab().results_viewer.selected_cell_text() {
                    self.copy_to_clipboard(&text);
                }
                Action::None
            }
            KeyAction::CopyRow => {
                if let Some(text) = self.tab().results_viewer.selected_row_text() {
                    self.copy_to_clipboard(&text);
                }
                Action::None
            }
            KeyAction::ExportCsv => {
                self.start_export(ExportFormat::Csv);
                Action::None
            }
            KeyAction::ExportJson => {
                self.start_export(ExportFormat::Json);
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
                if self.focus == PanelFocus::TreeBrowser
                    && let Some(sql) = self.tree_browser.preview_query()
                {
                    let tab_id = self.tab().id;
                    self.tab_mut().editor.set_content(sql.clone());
                    self.tab_mut().query_running = true;
                    self.set_status("Executing query...".to_string(), StatusLevel::Info);
                    return Action::ExecuteQuery { sql, tab_id };
                }
                self.tree_browser.expand_current();
                Action::None
            }
            KeyAction::Collapse => {
                self.tree_browser.collapse_current();
                Action::None
            }

            // ── Modal (inspector, command bar, help) ──────────
            KeyAction::Dismiss => {
                match self.focus {
                    PanelFocus::Inspector => {
                        self.inspector.hide();
                        self.focus = self.previous_focus;
                    }
                    PanelFocus::CommandBar => {
                        self.pending_export = None;
                        self.command_bar.deactivate();
                        self.focus = self.previous_focus;
                    }
                    PanelFocus::Help => {
                        self.help.hide();
                        self.focus = self.previous_focus;
                    }
                    _ => {}
                }
                Action::None
            }
            KeyAction::Submit => {
                if self.focus == PanelFocus::CommandBar {
                    let input = self.command_bar.input_text().to_string();
                    let is_prompt = self.command_bar.is_prompt_mode();
                    let format = self.pending_export.take();
                    self.command_bar.deactivate();
                    self.focus = self.previous_focus;

                    if input.is_empty() {
                        return Action::None;
                    }

                    if is_prompt {
                        if let Some(fmt) = format {
                            self.execute_export(fmt, &input);
                        }
                        Action::None
                    } else {
                        match parse_command(&input) {
                            Ok(cmd) => self.execute_command(cmd),
                            Err(e) => {
                                self.set_status(e.to_string(), StatusLevel::Error);
                                Action::None
                            }
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
                self.tab_mut().editor.clear();
                Action::None
            }
            Command::Help => {
                self.previous_focus = self.focus;
                self.focus = PanelFocus::Help;
                self.help.show();
                Action::None
            }
            Command::Quit => Action::Quit,
            Command::Connect => {
                self.show_connection_dialog();
                Action::None
            }
        }
    }

    /// Reference to the active tab
    pub fn tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }

    /// Mutable reference to the active tab
    pub fn tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab]
    }

    /// Number of open tabs
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Find the index of a tab by its stable ID
    fn tab_index_by_id(&self, id: usize) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    /// Open a new tab and switch to it. Returns false if at capacity.
    fn new_tab(&mut self) -> bool {
        if self.tabs.len() >= self.max_tabs {
            return false;
        }
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab::new(id));
        self.active_tab = self.tabs.len() - 1;
        self.focus = PanelFocus::QueryEditor;
        true
    }

    /// Close the active tab. Returns false if it's the last tab.
    fn close_tab(&mut self) -> bool {
        if self.tabs.len() <= 1 {
            return false;
        }
        self.tabs.remove(self.active_tab);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        true
    }

    /// Cycle to the next tab, wrapping around.
    fn next_tab(&mut self) {
        if self.tabs.len() > 1 {
            self.active_tab = (self.active_tab + 1) % self.tabs.len();
        }
    }

    /// Recompute completions based on current cursor context.
    fn update_completions(&mut self) {
        let idx = self.active_tab;
        let (line_idx, col) = self.tabs[idx].editor.cursor();
        let line = match self.tabs[idx].editor.line(line_idx) {
            Some(l) => l.to_string(),
            None => {
                self.clear_completions();
                return;
            }
        };

        // Only complete at end-of-word: next char (if any) should not be alphanumeric/underscore
        let bytes = line.as_bytes();
        if col < bytes.len() {
            let next = bytes[col];
            if next.is_ascii_alphanumeric() || next == b'_' {
                self.clear_completions();
                return;
            }
        }

        let prefix = completer::word_before_cursor(&line, col);
        let prefix_start = col - prefix.len();

        // Check for dot qualifier (e.g., "users." or "public.u")
        let dot_qual = completer::dot_qualifier(&line, prefix_start);

        // Allow empty prefix when dot-qualified (e.g., "users.")
        if prefix.is_empty() && dot_qual.is_none() {
            self.clear_completions();
            return;
        }

        let schema = self.tree_browser.schema();

        // Build text-before-prefix for context detection (skip if dot-qualified)
        let context = if dot_qual.is_some() {
            completer::detect_context("", dot_qual, schema)
        } else {
            let mut text_before = String::new();
            for i in 0..line_idx {
                if let Some(prev_line) = self.tabs[idx].editor.line(i) {
                    text_before.push_str(prev_line);
                    text_before.push('\n');
                }
            }
            text_before.push_str(&line[..prefix_start]);
            completer::detect_context(&text_before, None, schema)
        };

        let ghost = self.tabs[idx].completer.recompute(prefix, context, schema);
        self.tabs[idx].editor.set_ghost_text(ghost);
    }

    /// Clear completion state and editor ghost text.
    fn clear_completions(&mut self) {
        let idx = self.active_tab;
        self.tabs[idx].completer.clear();
        self.tabs[idx].editor.set_ghost_text(None);
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

    /// Show the connection picker dialog
    pub fn show_connection_dialog(&mut self) {
        self.previous_focus = self.focus;
        self.focus = PanelFocus::ConnectionDialog;
        self.connection_dialog.show();
    }

    /// Apply a new connection (after successful connect + schema load)
    pub fn apply_connection(&mut self, name: String, schema: crate::db::schema::SchemaTree) {
        self.connection_name = Some(name);
        self.tree_browser.set_schema(schema);
        // Reset all tabs to fresh state
        self.tabs = vec![Tab::new(0)];
        self.active_tab = 0;
        self.next_tab_id = 1;
        self.focus = PanelFocus::QueryEditor;
    }

    fn start_export(&mut self, format: ExportFormat) {
        if self.tab().results_viewer.results().is_none() {
            self.set_status("No results to export".to_string(), StatusLevel::Warning);
            return;
        }
        let now = chrono::Local::now();
        let filename = format!(
            "export_{}.{}",
            now.format("%Y-%m-%d_%H%M%S"),
            format.extension()
        );
        self.pending_export = Some(format);
        self.previous_focus = self.focus;
        self.focus = PanelFocus::CommandBar;
        self.command_bar
            .activate_with_prompt("Save as: ".to_string(), filename);
    }

    fn execute_export(&mut self, format: ExportFormat, path: &str) {
        let Some(results) = self.tab().results_viewer.results() else {
            self.set_status("No results to export".to_string(), StatusLevel::Warning);
            return;
        };

        let data = match format {
            ExportFormat::Csv => crate::export::to_csv(results),
            ExportFormat::Json => crate::export::to_json(results),
        };

        match std::fs::write(path, &data) {
            Ok(()) => {
                let ext = format.extension().to_uppercase();
                self.set_status(
                    format!("Exported {} as {} ({} bytes)", path, ext, data.len()),
                    StatusLevel::Success,
                );
            }
            Err(e) => {
                self.set_status(format!("Export failed: {}", e), StatusLevel::Error);
            }
        }
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
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active_tab, 0);
        assert_eq!(app.tabs[0].id, 0);
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
        let app = App::with_connection("test-db".to_string(), schema, &Settings::default());
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
        app.tabs[0]
            .editor
            .set_content("SELECT * FROM users".to_string());

        let ctrl_e = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
        let action = app.handle_key(ctrl_e);
        match action {
            Action::ExecuteQuery { sql, .. } => {
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
        app.tabs[0].query_running = true;

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
        app.tabs[0].query_running = true;

        let results =
            crate::db::QueryResults::new(vec![], vec![], std::time::Duration::from_millis(10), 0);
        app.handle_event(AppEvent::QueryCompleted { results, tab_id: 0 })
            .unwrap();
        assert!(!app.tabs[0].query_running);
    }

    #[test]
    fn test_query_failed_clears_running() {
        let mut app = App::new();
        app.tabs[0].query_running = true;

        app.handle_event(AppEvent::QueryFailed {
            error: "some error".to_string(),
            tab_id: 0,
        })
        .unwrap();
        assert!(!app.tabs[0].query_running);
    }

    #[test]
    fn test_query_cancelled_shows_warning() {
        let mut app = App::new();
        app.tabs[0].query_running = true;

        app.handle_event(AppEvent::QueryFailed {
            error: "ERROR: canceling statement due to user request".to_string(),
            tab_id: 0,
        })
        .unwrap();
        assert!(!app.tabs[0].query_running);
        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.message, "Query cancelled");
        assert_eq!(msg.level, StatusLevel::Warning);
    }

    #[test]
    fn test_enter_on_table_executes_preview_query() {
        use crate::db::schema::{Schema, SchemaTree, Table};
        use crossterm::event::{KeyCode, KeyModifiers};

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
        let mut app = App::with_connection("test".to_string(), schema, &Settings::default());
        app.focus = PanelFocus::TreeBrowser;

        // Navigate to the "users" table node via public API
        // Auto-expanded items: [0] public, [1] Tables, [2] users
        app.tree_browser.move_down(); // → Tables
        app.tree_browser.move_down(); // → users

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let action = app.handle_key(enter);

        match action {
            Action::ExecuteQuery { sql, .. } => {
                assert_eq!(sql, "SELECT * FROM \"public\".\"users\" LIMIT 100");
            }
            other => panic!(
                "Expected ExecuteQuery, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
        // Editor should contain the generated SQL
        assert_eq!(
            app.tabs[0].editor.get_content(),
            "SELECT * FROM \"public\".\"users\" LIMIT 100"
        );
    }

    #[test]
    fn test_enter_on_schema_node_expands() {
        use crate::db::schema::{Schema, SchemaTree, Table};
        use crossterm::event::{KeyCode, KeyModifiers};

        let schema = SchemaTree {
            schemas: vec![
                Schema {
                    name: "public".to_string(),
                    tables: vec![Table {
                        name: "t".to_string(),
                        columns: vec![],
                    }],
                    views: vec![],
                    indexes: vec![],
                    functions: vec![],
                },
                Schema {
                    name: "other".to_string(),
                    tables: vec![Table {
                        name: "x".to_string(),
                        columns: vec![],
                    }],
                    views: vec![],
                    indexes: vec![],
                    functions: vec![],
                },
            ],
        };
        let mut app = App::with_connection("test".to_string(), schema, &Settings::default());
        app.focus = PanelFocus::TreeBrowser;

        // Navigate to the collapsed "other" schema node
        // Items: [0] public, [1] Tables, [2] t, [3] other
        app.tree_browser.move_down(); // → Tables
        app.tree_browser.move_down(); // → t
        app.tree_browser.move_down(); // → other

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let action = app.handle_key(enter);

        // Should expand (not execute a query)
        assert!(matches!(action, Action::None));
        assert_eq!(app.tabs[0].editor.get_content(), "");
    }

    #[test]
    fn test_format_query_formats_content() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;
        app.tabs[0]
            .editor
            .set_content("select name, age from users where id > 10".to_string());

        let ctrl_alt_f = KeyEvent::new(
            KeyCode::Char('f'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        );
        let action = app.handle_key(ctrl_alt_f);
        assert!(matches!(action, Action::None));

        let content = app.tabs[0].editor.get_content();
        // Keywords should be uppercased
        assert!(content.contains("SELECT"));
        assert!(content.contains("FROM"));
        assert!(content.contains("WHERE"));
        // Status message should be set
        assert_eq!(
            app.status_message.as_ref().unwrap().message,
            "Query formatted"
        );
    }

    #[test]
    fn test_format_query_skips_empty() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;
        // Editor is empty by default

        let ctrl_alt_f = KeyEvent::new(
            KeyCode::Char('f'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        );
        let action = app.handle_key(ctrl_alt_f);
        assert!(matches!(action, Action::None));
        // No status message should be set (status is cleared on key press)
        assert!(app.status_message.is_none());
    }

    #[test]
    fn test_format_query_is_undoable() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;
        // replace_content (not set_content) so undo stack is preserved
        app.tabs[0].editor.replace_content("select 1".to_string());

        let ctrl_alt_f = KeyEvent::new(
            KeyCode::Char('f'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        );
        app.handle_key(ctrl_alt_f);
        let formatted = app.tabs[0].editor.get_content();
        assert!(formatted.contains("SELECT"));

        // Undo should restore pre-format content
        app.tabs[0].editor.undo();
        assert_eq!(app.tabs[0].editor.get_content(), "select 1");
    }

    #[test]
    fn test_history_back_populates_editor() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;

        // Type and execute a query
        app.tabs[0].editor.set_content("SELECT 1".to_string());
        let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
        let action = app.handle_key(f5);
        assert!(matches!(action, Action::ExecuteQuery { .. }));

        // Clear editor (simulating user clearing it)
        app.tabs[0].editor.clear();
        assert_eq!(app.tabs[0].editor.get_content(), "");

        // Ctrl+Up should recall the query
        let ctrl_up = KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL);
        app.handle_key(ctrl_up);
        assert_eq!(app.tabs[0].editor.get_content(), "SELECT 1");

        // Ctrl+Down should restore the draft (empty)
        let ctrl_down = KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL);
        app.handle_key(ctrl_down);
        assert_eq!(app.tabs[0].editor.get_content(), "");
    }

    #[test]
    fn test_paste_event_inserts_text() {
        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;
        app.handle_event(AppEvent::Paste("SELECT 1".to_string()))
            .unwrap();
        assert_eq!(app.tabs[0].editor.get_content(), "SELECT 1");
    }

    // ── Tab management tests ─────────────────────────────────

    #[test]
    fn test_new_tab() {
        let mut app = App::new();
        assert_eq!(app.tabs.len(), 1);
        assert!(app.new_tab());
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.active_tab, 1);
        assert_eq!(app.tabs[1].id, 1);
        assert_eq!(app.focus, PanelFocus::QueryEditor);
    }

    #[test]
    fn test_close_last_tab_denied() {
        let mut app = App::new();
        assert!(!app.close_tab());
        assert_eq!(app.tabs.len(), 1);
    }

    #[test]
    fn test_close_running_tab_denied() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.tabs[0].query_running = true;

        let ctrl_w = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        app.handle_key(ctrl_w);

        assert_eq!(app.tabs.len(), 1);
        assert!(
            app.status_message
                .as_ref()
                .unwrap()
                .message
                .contains("Cannot close tab while query is running")
        );
    }

    #[test]
    fn test_close_tab_adjusts_active() {
        let mut app = App::new();
        app.new_tab();
        app.new_tab();
        assert_eq!(app.tabs.len(), 3);

        // Active is tab index 2 (the last one)
        assert_eq!(app.active_tab, 2);
        assert!(app.close_tab());
        assert_eq!(app.tabs.len(), 2);
        // Active should clamp to last valid index
        assert_eq!(app.active_tab, 1);
    }

    #[test]
    fn test_next_tab_wraps() {
        let mut app = App::new();
        app.new_tab();
        app.new_tab();
        assert_eq!(app.active_tab, 2);

        app.next_tab();
        assert_eq!(app.active_tab, 0);

        app.next_tab();
        assert_eq!(app.active_tab, 1);

        app.next_tab();
        assert_eq!(app.active_tab, 2);
    }

    #[test]
    fn test_max_tabs() {
        let mut app = App::new();
        // Default max_tabs is 5, already have 1, add 4 more
        for _ in 0..4 {
            assert!(app.new_tab());
        }
        assert_eq!(app.tabs.len(), 5);
        assert!(!app.new_tab());
        assert_eq!(app.tabs.len(), 5);
    }

    #[test]
    fn test_configurable_max_tabs() {
        let mut settings = Settings::default();
        settings.settings.max_tabs = 3;
        let mut app = App::new_with_settings(&settings);
        assert!(app.new_tab());
        assert!(app.new_tab());
        assert!(!app.new_tab()); // 3rd tab fails (already have 3)
        assert_eq!(app.tabs.len(), 3);
    }

    #[test]
    fn test_query_routes_to_correct_tab() {
        let mut app = App::new();
        app.new_tab(); // tab id=1

        // Set up tab 0 (id=0) as running
        app.active_tab = 0;
        app.tabs[0].query_running = true;

        // Switch to tab 1
        app.active_tab = 1;

        // Complete query for tab id=0
        let results =
            crate::db::QueryResults::new(vec![], vec![], std::time::Duration::from_millis(10), 0);
        app.handle_event(AppEvent::QueryCompleted { results, tab_id: 0 })
            .unwrap();

        // Tab 0 should have cleared running flag
        assert!(!app.tabs[0].query_running);

        // Focus should NOT have switched to ResultsViewer (active tab is 1, result was for tab 0)
        assert_ne!(app.focus, PanelFocus::ResultsViewer);
    }

    #[test]
    fn test_stable_tab_ids() {
        let mut app = App::new();
        assert_eq!(app.tabs[0].id, 0);

        app.new_tab();
        assert_eq!(app.tabs[1].id, 1);

        app.new_tab();
        assert_eq!(app.tabs[2].id, 2);

        // Close tab at index 1 (id=1)
        app.active_tab = 1;
        app.close_tab();

        // IDs should be stable: [0, 2]
        assert_eq!(app.tabs[0].id, 0);
        assert_eq!(app.tabs[1].id, 2);

        // New tab gets id=3 (not reusing 1)
        app.new_tab();
        assert_eq!(app.tabs[2].id, 3);
    }

    #[test]
    fn test_query_completed_for_unknown_tab_id() {
        let mut app = App::new();
        let results =
            crate::db::QueryResults::new(vec![], vec![], std::time::Duration::from_millis(1), 0);
        // tab_id=99 does not exist — should not panic
        let action = app
            .handle_event(AppEvent::QueryCompleted {
                results,
                tab_id: 99,
            })
            .unwrap();
        assert!(matches!(action, Action::None));
        // Status is still set (success toast), no crash
        assert!(app.status_message.is_some());
    }

    #[test]
    fn test_close_tab_back_to_single() {
        let mut app = App::new();
        app.new_tab();
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.tab_count(), 2);

        // Close the second tab
        assert!(app.close_tab());
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tab_count(), 1);

        // tab_count() == 1 means tab bar should not show
        // (render.rs uses app.tab_count() > 1)
    }

    #[test]
    fn test_next_tab_noop_with_single_tab() {
        let mut app = App::new();
        assert_eq!(app.active_tab, 0);
        app.next_tab();
        assert_eq!(app.active_tab, 0);
    }

    #[test]
    fn test_tab_actions_suppressed_in_modals() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();

        // Tab actions should be suppressed in CommandBar, Inspector, Help
        for focus in [
            PanelFocus::CommandBar,
            PanelFocus::Inspector,
            PanelFocus::Help,
        ] {
            app.focus = focus;
            let ctrl_t = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
            app.handle_key(ctrl_t);
            assert_eq!(
                app.tabs.len(),
                1,
                "NewTab should be suppressed in {:?}",
                focus
            );
        }
    }

    #[test]
    fn test_export_no_results_warns() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::ResultsViewer;

        let ctrl_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        app.handle_key(ctrl_s);

        let msg = app.status_message.as_ref().unwrap();
        assert_eq!(msg.message, "No results to export");
        assert_eq!(msg.level, StatusLevel::Warning);
        assert!(app.pending_export.is_none());
    }

    #[test]
    fn test_export_opens_prompt() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        // Load some results
        let results =
            crate::db::QueryResults::new(vec![], vec![], std::time::Duration::from_millis(1), 0);
        app.tabs[0].results_viewer.set_results(results);
        app.focus = PanelFocus::ResultsViewer;

        let ctrl_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        app.handle_key(ctrl_s);

        assert!(app.pending_export.is_some());
        assert_eq!(app.pending_export, Some(ExportFormat::Csv));
        assert_eq!(app.focus, PanelFocus::CommandBar);
        assert!(app.command_bar.is_active());
        assert!(app.command_bar.is_prompt_mode());
        assert!(app.command_bar.input_text().ends_with(".csv"));
    }

    #[test]
    fn test_dismiss_clears_pending_export() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        let results =
            crate::db::QueryResults::new(vec![], vec![], std::time::Duration::from_millis(1), 0);
        app.tabs[0].results_viewer.set_results(results);
        app.focus = PanelFocus::ResultsViewer;

        // Start export flow
        let ctrl_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        app.handle_key(ctrl_s);
        assert!(app.pending_export.is_some());

        // Press Escape to dismiss
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        app.handle_key(esc);

        assert!(app.pending_export.is_none());
        assert!(!app.command_bar.is_active());
    }

    #[test]
    fn test_export_json_opens_prompt() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        let results =
            crate::db::QueryResults::new(vec![], vec![], std::time::Duration::from_millis(1), 0);
        app.tabs[0].results_viewer.set_results(results);
        app.focus = PanelFocus::ResultsViewer;

        let ctrl_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL);
        app.handle_key(ctrl_j);

        assert_eq!(app.pending_export, Some(ExportFormat::Json));
        assert!(app.command_bar.input_text().ends_with(".json"));
    }

    #[test]
    fn test_execute_query_sets_running_flag() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;
        app.tabs[0].editor.set_content("SELECT 1".to_string());

        let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
        let action = app.handle_key(f5);

        assert!(matches!(action, Action::ExecuteQuery { .. }));
        assert!(app.tabs[0].query_running);
    }

    // ── Connection dialog tests ─────────────────────────────────

    #[test]
    fn test_connection_dialog_opens_on_command() {
        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;

        // Open command bar, type /connect, submit
        let action = app.execute_command(crate::commands::Command::Connect);
        assert!(matches!(action, Action::None));
        assert_eq!(app.focus, PanelFocus::ConnectionDialog);
        assert!(app.connection_dialog.is_visible());
    }

    #[test]
    fn test_connection_dialog_dismiss_restores_focus() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.focus = PanelFocus::ResultsViewer;
        app.show_connection_dialog();

        assert_eq!(app.focus, PanelFocus::ConnectionDialog);
        assert_eq!(app.previous_focus, PanelFocus::ResultsViewer);

        // Press Esc to dismiss
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let action = app.handle_key(esc);
        assert!(matches!(action, Action::None));
        assert_eq!(app.focus, PanelFocus::ResultsViewer);
        assert!(!app.connection_dialog.is_visible());
    }

    #[test]
    fn test_connection_dialog_returns_connect_action() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.show_connection_dialog();

        // Type a valid URL
        for c in "postgres://user:pass@localhost/mydb".chars() {
            let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
            app.handle_key(key);
        }

        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let action = app.handle_key(enter);

        match action {
            Action::Connect(config) => {
                assert_eq!(config.host, "localhost");
                assert_eq!(config.username, "user");
            }
            other => panic!(
                "Expected Action::Connect, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
        // Dialog should be hidden after connect
        assert!(!app.connection_dialog.is_visible());
    }

    #[test]
    fn test_apply_connection_resets_state() {
        use crate::db::schema::{Schema, SchemaTree, Table};

        let mut app = App::new();
        // Simulate having multiple tabs
        app.new_tab();
        app.new_tab();
        assert_eq!(app.tabs.len(), 3);

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

        app.apply_connection("new-db".to_string(), schema);

        assert_eq!(app.connection_name.as_deref(), Some("new-db"));
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.active_tab, 0);
        assert_eq!(app.focus, PanelFocus::QueryEditor);
    }

    #[test]
    fn test_global_keys_suppressed_in_connection_dialog() {
        use crossterm::event::{KeyCode, KeyModifiers};

        let mut app = App::new();
        app.show_connection_dialog();

        // Tab should be consumed by dialog (cycling focus), not global CycleFocus
        let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        let action = app.handle_key(tab);
        assert!(matches!(action, Action::None));
        // Still in ConnectionDialog focus
        assert_eq!(app.focus, PanelFocus::ConnectionDialog);
    }
}

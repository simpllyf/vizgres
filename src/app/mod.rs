//! Application state and event handling
//!
//! Central state machine: events come in, state updates, actions go out.
//!
//! Split into submodules:
//! - `event_handler` — event dispatch (handle_event, handle_key)
//! - `actions` — key action execution (execute_key_action)
//! - `sql_utils` — SQL analysis helpers (transaction intent, meta-commands)

mod actions;
mod event_handler;
mod sql_utils;

use sql_utils::detect_transaction_intent;

use crate::commands::{Command, parse_command};
use crate::completer::{self, Completer};
use crate::config::ConnectionConfig;
use crate::config::settings::Settings;
use crate::db::QueryResults;
use crate::db::schema::{Function, Index, SchemaTree, Table};
use crate::db::sql_limit;
use crate::error::Result;
use crate::export::ExportFormat;
use crate::history::QueryHistory;
use crate::keymap::{KeyAction, KeyMap};
use crate::ui::Component;
use crate::ui::ComponentAction;
use crate::ui::command_bar::CommandBar;
use crate::ui::connection_dialog::{ConnectionDialog, DialogAction};
use crate::ui::editor::QueryEditor;
use crate::ui::explain::ExplainViewer;
use crate::ui::help::HelpOverlay;
use crate::ui::inspector::Inspector;
use crate::ui::results::ResultsViewer;
use crate::ui::theme::Theme;
use crate::ui::tree::TreeBrowser;
use crossterm::event::KeyEvent;

/// Server-side pagination state for a query
#[derive(Debug, Clone)]
pub struct PaginationState {
    /// Original SQL before LIMIT/OFFSET was added
    pub original_sql: String,
    /// Current page (0-based)
    pub current_page: usize,
    /// Rows per page
    pub page_size: usize,
    /// Whether more rows exist beyond this page
    pub has_more: bool,
    /// Whether the user's SQL already had LIMIT (no auto-pagination)
    pub user_has_limit: bool,
    /// Page before navigation (for rollback on query failure)
    previous_page: Option<usize>,
}

impl PaginationState {
    /// Row offset for the current page
    pub fn offset(&self) -> usize {
        self.current_page * self.page_size
    }

    /// Build the SQL for the current page (appends LIMIT/OFFSET)
    pub fn paged_sql(&self) -> String {
        if self.user_has_limit {
            self.original_sql.clone()
        } else {
            format!(
                "{} LIMIT {} OFFSET {}",
                self.original_sql.trim().trim_end_matches(';'),
                self.page_size + 1, // +1 to detect if more rows exist
                self.offset()
            )
        }
    }
}

/// A single query tab containing its own editor, results, and completer.
/// Each tab holds its own transaction state (independent per connection).
pub struct Tab {
    /// Stable identifier (monotonically increasing, never reused)
    pub id: usize,
    pub editor: QueryEditor,
    pub results_viewer: ResultsViewer,
    completer: Completer,
    /// Whether this tab has a query in flight
    pub query_running: bool,
    /// When the current query started (for elapsed time display)
    pub query_start: Option<std::time::Instant>,
    /// Client-side transaction state for this tab's connection
    pub transaction_state: TransactionState,
    /// Pagination state for the current result set
    pub pagination: Option<PaginationState>,
    /// Visual EXPLAIN tree viewer (replaces results panel when present)
    pub explain_viewer: Option<ExplainViewer>,
    /// Whether the last query was an EXPLAIN (for routing results)
    explain_pending: bool,
    /// Row count received during streaming (for progress display)
    pub rows_streaming: Option<usize>,
}

impl Tab {
    fn new(id: usize) -> Self {
        Self {
            id,
            editor: QueryEditor::new(),
            results_viewer: ResultsViewer::new(),
            completer: Completer::new(),
            query_running: false,
            query_start: None,
            transaction_state: TransactionState::Idle,
            pagination: None,
            explain_viewer: None,
            explain_pending: false,
            rows_streaming: None,
        }
    }
}

/// Main application state
pub struct App {
    /// Name of current connection profile
    pub connection_name: Option<String>,
    /// Whether the current connection is a saved profile from connections.toml
    pub is_saved_connection: bool,

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

    /// Pending save-query prompt (waiting for user to type a name)
    pending_save_query: bool,

    /// Query history for Ctrl+Up/Down navigation
    history: QueryHistory,

    /// Maximum number of tabs allowed
    max_tabs: usize,

    /// Data-driven keybinding configuration
    pub keymap: KeyMap,

    /// UI theme (created once, reused every frame)
    pub theme: Theme,

    /// Query timeout in milliseconds (0 = disabled)
    query_timeout_ms: u64,

    /// Maximum result rows (0 = unlimited)
    max_result_rows: usize,

    /// Server-side statement timeout in milliseconds (0 = disabled)
    /// Applied at connection time via the connection string
    pub statement_timeout_ms: u64,

    /// Whether to prompt before executing destructive queries (DROP, TRUNCATE, etc.)
    confirm_destructive: bool,

    /// Read-only mode — blocks write queries at client level
    pub read_only: bool,

    /// Global default for read-only mode (from settings)
    default_read_only: bool,

    /// Whether to show EXPLAIN as visual tree (true) or raw text (false)
    explain_visual: bool,

    /// SQL pending destructive-query confirmation (waiting for y/n)
    pending_confirm_sql: Option<PendingConfirm>,

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

/// Client-side transaction state tracking.
/// Inferred from query text (BEGIN/COMMIT/ROLLBACK) and error events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    /// No active transaction (autocommit)
    Idle,
    /// Inside an explicit transaction block
    InTransaction,
    /// Transaction block entered error state (requires ROLLBACK)
    Failed,
}

/// Pending destructive query confirmation
struct PendingConfirm {
    sql: String,
    tab_id: usize,
    timeout_ms: u64,
    max_rows: usize,
}

/// Application events from the event loop
pub enum AppEvent {
    /// Keyboard input event
    Key(KeyEvent),
    /// Terminal resize event
    Resize,
    /// Row count progress during streaming query execution
    QueryProgress { rows_fetched: usize, tab_id: usize },
    /// Query execution completed successfully
    QueryCompleted {
        results: QueryResults,
        tab_id: usize,
    },
    /// Query execution failed
    QueryFailed {
        error: String,
        position: Option<u32>, // byte offset in query
        tab_id: usize,
    },
    /// Schema loaded successfully
    SchemaLoaded(SchemaTree),
    /// Schema loading failed
    SchemaFailed(String),
    /// Schema search completed successfully
    SchemaSearchCompleted(SchemaTree),
    /// Schema search failed
    SchemaSearchFailed(String),
    /// Load more items completed
    LoadMoreCompleted {
        schema_name: String,
        category: String,
        items: LoadMoreItems,
    },
    /// Load more items failed
    LoadMoreFailed(String),
    /// Bracketed paste event
    Paste(String),
    /// Background database connection lost on a specific tab
    ConnectionLost { tab_id: usize, message: String },
}

/// Items loaded by load_more operations
#[derive(Debug)]
pub enum LoadMoreItems {
    Tables(Vec<Table>),
    Views(Vec<Table>),
    Functions(Vec<Function>),
    Indexes(Vec<Index>),
}

/// Actions returned by event handlers for the main loop to execute
pub enum Action {
    ExecuteQuery {
        sql: String,
        tab_id: usize,
        timeout_ms: u64,
        max_rows: usize,
    },
    /// Cancel a query on a specific tab's connection.
    /// If `terminate` is true, use pg_terminate_backend() for hard kill.
    CancelQuery {
        tab_id: usize,
        terminate: bool,
    },
    LoadSchema,
    SearchSchema {
        pattern: String,
    },
    LoadMoreCategory {
        schema_name: String,
        category: String,
        offset: usize,
        limit: usize,
    },
    /// A tab was closed — main loop should clean up its connection
    TabClosed {
        tab_id: usize,
    },
    Connect(ConnectionConfig),
    /// Drop a single tab's dead connection so it auto-reconnects on next query
    ReconnectTab {
        tab_id: usize,
    },
    /// Signal to clear all providers and show reconnect dialog
    Disconnect,
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
            is_saved_connection: false,
            focus: PanelFocus::QueryEditor,
            previous_focus: PanelFocus::QueryEditor,
            tree_browser: TreeBrowser::with_settings(
                settings.settings.preview_rows,
                settings.settings.tree_category_limit,
            ),
            command_bar: CommandBar::new(),
            inspector: Inspector::new(),
            help: HelpOverlay::new(),
            connection_dialog: ConnectionDialog::new(),
            tabs: vec![Tab::new(0)],
            active_tab: 0,
            next_tab_id: 1,
            pending_export: None,
            pending_save_query: false,
            history: QueryHistory::load(settings.settings.history_size),
            max_tabs: settings.settings.max_tabs,
            keymap,
            theme: Theme::by_name(&settings.settings.theme).unwrap_or_default(),
            query_timeout_ms: settings.settings.query_timeout_ms,
            max_result_rows: settings.settings.max_result_rows,
            statement_timeout_ms: settings.settings.statement_timeout_ms,
            confirm_destructive: settings.settings.confirm_destructive,
            read_only: settings.settings.read_only,
            default_read_only: settings.settings.read_only,
            explain_visual: settings.settings.explain_visual,
            pending_confirm_sql: None,
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
    pub fn with_connection(
        name: String,
        saved: bool,
        connection_read_only: bool,
        schema: SchemaTree,
        settings: &Settings,
    ) -> Self {
        let mut app = Self::new_with_settings(settings);
        app.connection_name = Some(name.clone());
        app.is_saved_connection = saved;
        app.read_only = app.default_read_only || connection_read_only;
        app.tree_browser.set_schema(schema);
        app.load_saved_queries_for(&name, saved);
        app
    }

    /// Execute a query that has already passed confirmation (or didn't need it).
    /// This handles both the transaction state update and returning the Action.
    fn prepare_execute_query(&mut self, sql: String) -> Action {
        let tab_id = self.tab().id;
        let timeout_ms = self.query_timeout_ms;
        let page_size = self.max_result_rows;

        // Update this tab's transaction state based on query intent
        if let Some(new_state) = detect_transaction_intent(&sql) {
            self.tab_mut().transaction_state = new_state;
        }

        self.tab_mut().query_running = true;
        self.tab_mut().query_start = Some(std::time::Instant::now());
        self.history.push(&sql);

        // Auto-paginate if the query has no user LIMIT and isn't EXPLAIN
        let trimmed = sql.trim();
        let is_explain = trimmed
            .split_whitespace()
            .next()
            .is_some_and(|w| w.eq_ignore_ascii_case("EXPLAIN"));

        if !is_explain && page_size > 0 {
            let analysis = sql_limit::analyze_limit(&sql);
            if analysis.can_paginate() {
                let pagination = PaginationState {
                    original_sql: sql,
                    current_page: 0,
                    page_size,
                    has_more: false,
                    user_has_limit: false,
                    previous_page: None,
                };
                let paged_sql = pagination.paged_sql();
                self.tab_mut().pagination = Some(pagination);
                return Action::ExecuteQuery {
                    sql: paged_sql,
                    tab_id,
                    timeout_ms,
                    max_rows: 0, // LIMIT in SQL controls row count
                };
            }
        }

        // User has LIMIT or EXPLAIN — run as-is with max_rows safety net
        self.tab_mut().pagination = None;
        Action::ExecuteQuery {
            sql,
            tab_id,
            timeout_ms,
            max_rows: page_size,
        }
    }

    /// Execute a confirmed (destructive) query
    fn execute_confirmed_query(&mut self, pending: PendingConfirm) -> Action {
        if let Some(idx) = self.tab_index_by_id(pending.tab_id) {
            if let Some(new_state) = detect_transaction_intent(&pending.sql) {
                self.tabs[idx].transaction_state = new_state;
            }
            self.tabs[idx].query_running = true;
            self.tabs[idx].query_start = Some(std::time::Instant::now());
        }
        self.history.push(&pending.sql);

        Action::ExecuteQuery {
            sql: pending.sql,
            tab_id: pending.tab_id,
            timeout_ms: pending.timeout_ms,
            max_rows: pending.max_rows,
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
            Command::SaveQuery { name } => {
                if !self.is_saved_connection {
                    self.set_status(
                        "Save a connection profile first to use saved queries".to_string(),
                        StatusLevel::Warning,
                    );
                    return Action::None;
                }
                let sql = self.tab().editor.get_content();
                if sql.trim().is_empty() {
                    self.set_status(
                        "Editor is empty — nothing to save".to_string(),
                        StatusLevel::Warning,
                    );
                    return Action::None;
                }
                if let Some(name) = name {
                    self.finish_save_query(&name);
                } else {
                    self.start_save_query_prompt();
                }
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
        let (line_idx, char_col) = self.tabs[idx].editor.cursor();
        let line = match self.tabs[idx].editor.line(line_idx) {
            Some(l) => l.to_string(),
            None => {
                self.clear_completions();
                return;
            }
        };

        // Convert char-based cursor to byte offset for string operations
        let col: usize = line
            .char_indices()
            .nth(char_col)
            .map(|(i, _)| i)
            .unwrap_or(line.len());

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

    /// Whether a destructive-query confirmation prompt is active
    pub fn is_confirm_pending(&self) -> bool {
        self.pending_confirm_sql.is_some()
    }

    /// Show the connection picker dialog
    pub fn show_connection_dialog(&mut self) {
        self.previous_focus = self.focus;
        self.focus = PanelFocus::ConnectionDialog;
        self.connection_dialog.show();
    }

    /// Apply a new connection (after successful connect + schema load).
    /// `connection_read_only` is the per-connection setting; when `true`,
    /// it overrides the global default to enable read-only mode.
    pub fn apply_connection(
        &mut self,
        name: String,
        saved: bool,
        connection_read_only: bool,
        schema: crate::db::schema::SchemaTree,
    ) {
        self.connection_name = Some(name.clone());
        self.is_saved_connection = saved;
        // Per-connection read_only overrides global default
        self.read_only = self.default_read_only || connection_read_only;
        self.tree_browser.set_schema(schema);
        self.load_saved_queries_for(&name, saved);
        // Reset all tabs to fresh state (transaction_state resets via Tab::new)
        self.tabs = vec![Tab::new(0)];
        self.active_tab = 0;
        self.next_tab_id = 1;
        self.focus = PanelFocus::QueryEditor;
    }

    /// Load saved queries into the tree browser for a saved connection
    fn load_saved_queries_for(&mut self, connection_name: &str, saved: bool) {
        if saved {
            match crate::config::saved_queries::load_queries_for_connection(connection_name) {
                Ok(queries) => self.tree_browser.set_saved_queries(queries),
                Err(_) => self.tree_browser.set_saved_queries(Vec::new()),
            }
        } else {
            self.tree_browser.set_saved_queries(Vec::new());
        }
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

    fn start_save_query_prompt(&mut self) {
        self.pending_save_query = true;
        self.previous_focus = self.focus;
        self.focus = PanelFocus::CommandBar;
        self.command_bar
            .activate_with_prompt("Save query as: ".to_string(), String::new());
    }

    fn finish_save_query(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.set_status(
                "Query name cannot be empty".to_string(),
                StatusLevel::Warning,
            );
            return;
        }
        let Some(conn) = self.connection_name.clone() else {
            self.set_status("No active connection".to_string(), StatusLevel::Error);
            return;
        };
        let sql = self.tab().editor.get_content();
        let query = crate::config::SavedQuery {
            connection: conn,
            name: name.to_string(),
            sql,
        };
        if let Err(e) = crate::config::saved_queries::save_query(&query) {
            self.set_status(format!("Failed to save query: {}", e), StatusLevel::Error);
            return;
        }
        self.tree_browser.upsert_saved_query(query);
        self.set_status(format!("Saved query: {}", name), StatusLevel::Success);
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
mod tests;

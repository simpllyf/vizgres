//! Application state and main event loop
//!
//! This module implements the core application state machine following an
//! Elm-like architecture with unidirectional data flow.

use crate::commands::Command;
use crate::config::ConnectionConfig;
use crate::db::{QueryResults, SchemaTree};
use crate::error::Result;
use crossterm::event::KeyEvent;
use std::time::Duration;

/// Main application state
pub struct App {
    /// Current database connection handle (if connected)
    pub connection: Option<Box<dyn crate::db::DatabaseProvider>>,

    /// Name of current connection profile
    pub connection_name: Option<String>,

    /// Which panel currently has focus
    pub focus: PanelFocus,

    /// Database schema tree
    pub schema_tree: Option<SchemaTree>,

    /// Current query being edited
    pub query_buffer: String,

    /// Most recent query results
    pub results: Option<QueryResults>,

    /// Command bar input buffer
    pub command_input: String,

    /// Command history
    pub command_history: Vec<String>,

    /// Status message to display
    pub status_message: Option<StatusMessage>,

    /// Whether the application is running
    pub running: bool,
}

/// Panel focus state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    /// Database tree browser (left panel)
    TreeBrowser,
    /// SQL query editor (top-right panel)
    QueryEditor,
    /// Query results viewer (bottom-right panel)
    ResultsViewer,
    /// Command bar (bottom)
    CommandBar,
    /// Cell value popup/inspector
    CellPopup,
}

/// Status message with severity level
pub struct StatusMessage {
    pub message: String,
    pub level: StatusLevel,
    pub timestamp: std::time::Instant,
}

/// Status message severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// Application events
pub enum AppEvent {
    /// Keyboard input
    Key(KeyEvent),

    /// Terminal resize
    Resize(u16, u16),

    /// Database connection established
    ConnectionEstablished,

    /// Database connection failed
    ConnectionFailed(crate::error::DbError),

    /// Query completed successfully
    QueryCompleted(QueryResults),

    /// Query failed
    QueryFailed(crate::error::DbError),

    /// Schema loaded
    SchemaLoaded(SchemaTree),

    /// Schema load failed
    SchemaLoadFailed(crate::error::DbError),

    /// Command submitted
    CommandSubmitted(Command),

    /// Refresh requested
    RefreshRequested,
}

/// Actions resulting from event handling (side effects)
pub enum Action {
    /// Execute a SQL query
    ExecuteQuery(String),

    /// Connect to a database
    Connect(ConnectionConfig),

    /// Disconnect from database
    Disconnect,

    /// Load/refresh schema
    LoadSchema,

    /// Export results to file
    ExportResults {
        format: crate::commands::ExportFormat,
        path: std::path::PathBuf,
    },

    /// Quit application
    Quit,

    /// No action needed
    None,
}

impl App {
    /// Create a new application instance
    pub fn new() -> Self {
        Self {
            connection: None,
            connection_name: None,
            focus: PanelFocus::QueryEditor,
            schema_tree: None,
            query_buffer: String::new(),
            results: None,
            command_input: String::new(),
            command_history: Vec::new(),
            status_message: None,
            running: true,
        }
    }

    /// Handle an application event and return resulting action
    pub fn handle_event(&mut self, _event: AppEvent) -> Result<Action> {
        // TODO: Implement event handling based on current focus and event type
        // Phase 1: Basic event routing
        // Phase 2: Full panel navigation
        todo!("Event handling not yet implemented")
    }

    /// Handle keyboard input based on current focus
    #[allow(dead_code)]
    fn handle_key(&mut self, _key: KeyEvent) -> Result<Action> {
        // TODO: Route key events to appropriate handlers based on focus
        // Phase 1: Basic quit key
        // Phase 2: Panel navigation with Tab, Ctrl+1/2/3
        todo!("Key handling not yet implemented")
    }

    /// Cycle focus to next panel
    pub fn cycle_focus(&mut self) {
        // TODO: Implement Tab-based focus cycling
        todo!("Focus cycling not yet implemented")
    }

    /// Set focus to specific panel
    pub fn set_focus(&mut self, focus: PanelFocus) {
        self.focus = focus;
    }

    /// Set status message
    pub fn set_status(&mut self, message: String, level: StatusLevel) {
        self.status_message = Some(StatusMessage {
            message,
            level,
            timestamp: std::time::Instant::now(),
        });
    }

    /// Check if status message should be cleared (after timeout)
    pub fn should_clear_status(&self) -> bool {
        if let Some(msg) = &self.status_message {
            msg.timestamp.elapsed() > Duration::from_secs(5)
        } else {
            false
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusMessage {
    /// Create a success status message
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: StatusLevel::Success,
            timestamp: std::time::Instant::now(),
        }
    }

    /// Create an error status message
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: StatusLevel::Error,
            timestamp: std::time::Instant::now(),
        }
    }

    /// Create an info status message
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            level: StatusLevel::Info,
            timestamp: std::time::Instant::now(),
        }
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
    fn test_set_focus_changes_focus() {
        let mut app = App::new();
        app.set_focus(PanelFocus::TreeBrowser);
        assert_eq!(app.focus, PanelFocus::TreeBrowser);
    }

    #[test]
    fn test_status_message_timeout() {
        let app = App::new();
        assert!(!app.should_clear_status());
    }
}

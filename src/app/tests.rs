use super::sql_utils::*;
use super::*;

#[test]
fn test_app_new_has_correct_defaults() {
    let app = App::new();
    assert!(app.connection_name.is_none());
    assert!(!app.is_saved_connection);
    assert_eq!(app.focus, PanelFocus::QueryEditor);
    assert!(app.running);
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.active_tab, 0);
    assert_eq!(app.tabs[0].id, 0);
}

#[test]
fn test_with_connection_constructor() {
    use crate::db::schema::{PaginatedVec, Schema, SchemaTree, Table};
    let schema = SchemaTree {
        schemas: PaginatedVec::from_vec(vec![Schema {
            name: "public".to_string(),
            tables: PaginatedVec::from_vec(vec![Table {
                name: "users".to_string(),
                columns: vec![],
                row_count: None,
            }]),
            views: PaginatedVec::default(),
            indexes: PaginatedVec::default(),
            functions: PaginatedVec::default(),
        }]),
    };
    let app = App::with_connection(
        "test-db".to_string(),
        false,
        false,
        schema,
        &Settings::default(),
    );
    assert_eq!(app.connection_name.as_deref(), Some("test-db"));
    assert!(!app.is_saved_connection);
}

#[test]
fn test_saved_connection_flag() {
    use crate::db::schema::SchemaTree;

    // with_connection(saved: true) sets the flag
    let app = App::with_connection(
        "prod".to_string(),
        true,
        false,
        SchemaTree::new(),
        &Settings::default(),
    );
    assert!(app.is_saved_connection);

    // apply_connection propagates the flag
    let mut app = App::new();
    assert!(!app.is_saved_connection);
    app.apply_connection("prod".to_string(), true, false, SchemaTree::new());
    assert!(app.is_saved_connection);

    // Reconnecting with unsaved clears the flag
    app.apply_connection(
        "adhoc@localhost".to_string(),
        false,
        false,
        SchemaTree::new(),
    );
    assert!(!app.is_saved_connection);
}

#[test]
fn test_connection_lost_preserves_connection_info() {
    use crate::db::schema::SchemaTree;

    let mut app = App::with_connection(
        "prod".to_string(),
        true,
        false,
        SchemaTree::new(),
        &Settings::default(),
    );
    assert!(app.is_saved_connection);
    assert!(app.connection_name.is_some());

    app.handle_event(AppEvent::ConnectionLost {
        tab_id: 0,
        message: "gone".to_string(),
    })
    .unwrap();
    // Auto-reconnect preserves connection info for transparent reconnection
    assert!(app.is_saved_connection);
    assert!(app.connection_name.is_some());
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
    app.focus = PanelFocus::QueryEditor;

    let action = app
        .handle_event(AppEvent::ConnectionLost {
            tab_id: 0,
            message: "server closed".to_string(),
        })
        .unwrap();

    // Should return ReconnectTab (not Disconnect)
    assert!(matches!(action, Action::ReconnectTab { tab_id: 0 }));

    // Should NOT show connection dialog — auto-reconnect is transparent
    assert!(!app.connection_dialog.is_visible());
    assert_eq!(app.focus, PanelFocus::QueryEditor);

    // Status message should be a warning about reconnection
    let msg = &app.status_message.as_ref().unwrap();
    assert!(msg.message.contains("reconnect"));
    assert_eq!(msg.level, StatusLevel::Warning);
}

#[test]
fn test_connection_lost_preserves_focus() {
    let mut app = App::new();
    app.focus = PanelFocus::ResultsViewer;

    app.handle_event(AppEvent::ConnectionLost {
        tab_id: 0,
        message: "timeout".to_string(),
    })
    .unwrap();

    // Focus should stay where it was — no dialog interruption
    assert_eq!(app.focus, PanelFocus::ResultsViewer);
}

#[test]
fn test_connection_lost_while_query_running() {
    let mut app = App::new();
    app.tabs[0].query_running = true;
    app.tabs[0].rows_streaming = Some(500);

    let action = app
        .handle_event(AppEvent::ConnectionLost {
            tab_id: 0,
            message: "connection reset".to_string(),
        })
        .unwrap();

    // Should return ReconnectTab, not show dialog
    assert!(matches!(action, Action::ReconnectTab { tab_id: 0 }));
    assert!(!app.connection_dialog.is_visible());

    // Query running flag still true (cleared when QueryFailed arrives)
    assert!(app.tabs[0].query_running);
    // Streaming state cleared immediately
    assert!(app.tabs[0].rows_streaming.is_none());
}

#[test]
fn test_connection_lost_idempotent() {
    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;

    // First connection loss
    app.handle_event(AppEvent::ConnectionLost {
        tab_id: 0,
        message: "first error".to_string(),
    })
    .unwrap();
    assert_eq!(app.focus, PanelFocus::QueryEditor);

    // Second connection loss (should be idempotent, no panic)
    let action = app
        .handle_event(AppEvent::ConnectionLost {
            tab_id: 0,
            message: "second error".to_string(),
        })
        .unwrap();

    assert!(matches!(action, Action::ReconnectTab { tab_id: 0 }));
    // Status message updated to latest
    let msg = &app.status_message.as_ref().unwrap();
    assert!(msg.message.contains("reconnect"));
}

#[test]
fn test_connection_lost_unknown_tab_no_panic() {
    let mut app = App::new();
    // Tab 99 doesn't exist — should not panic
    let action = app
        .handle_event(AppEvent::ConnectionLost {
            tab_id: 99,
            message: "gone".to_string(),
        })
        .unwrap();
    assert!(matches!(action, Action::ReconnectTab { tab_id: 99 }));
}

#[test]
fn test_connection_lost_isolates_tabs() {
    let mut app = App::new();
    app.new_tab();
    let tab1_id = app.tabs[1].id;

    app.tabs[0].transaction_state = TransactionState::InTransaction;
    app.tabs[1].transaction_state = TransactionState::InTransaction;
    app.tabs[0].rows_streaming = Some(100);
    app.tabs[1].rows_streaming = Some(200);

    // Only tab 1 loses connection
    app.handle_event(AppEvent::ConnectionLost {
        tab_id: tab1_id,
        message: "reset".to_string(),
    })
    .unwrap();

    // Tab 0 completely unaffected
    assert_eq!(
        app.tabs[0].transaction_state,
        TransactionState::InTransaction
    );
    assert_eq!(app.tabs[0].rows_streaming, Some(100));

    // Tab 1 reset
    assert_eq!(app.tabs[1].transaction_state, TransactionState::Idle);
    assert!(app.tabs[1].rows_streaming.is_none());
}

#[test]
fn test_connection_lost_from_tree_browser() {
    let mut app = App::new();
    app.focus = PanelFocus::TreeBrowser;

    app.handle_event(AppEvent::ConnectionLost {
        tab_id: 0,
        message: "server shutdown".to_string(),
    })
    .unwrap();

    // Focus stays on tree browser — no dialog interruption
    assert!(!app.connection_dialog.is_visible());
    assert_eq!(app.focus, PanelFocus::TreeBrowser);
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
            assert_eq!(sql, "EXPLAIN (ANALYZE, FORMAT JSON) SELECT * FROM users");
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn test_explain_visual_disabled_uses_plain_format() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.explain_visual = false;
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0].editor.set_content("SELECT 1".to_string());

    let ctrl_e = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
    let action = app.handle_key(ctrl_e);
    match action {
        Action::ExecuteQuery { sql, .. } => {
            assert_eq!(sql, "EXPLAIN ANALYZE SELECT 1");
            assert!(!sql.contains("FORMAT JSON"));
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn test_explain_visual_sets_pending_flag() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0].editor.set_content("SELECT 1".to_string());

    let ctrl_e = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
    let _action = app.handle_key(ctrl_e);
    assert!(
        app.tab().explain_pending,
        "visual explain should set pending flag"
    );
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
    assert!(matches!(action, Action::CancelQuery { .. }));
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
fn test_query_progress_updates_rows_streaming() {
    let mut app = App::new();
    app.tabs[0].query_running = true;
    assert!(app.tabs[0].rows_streaming.is_none());

    app.handle_event(AppEvent::QueryProgress {
        rows_fetched: 500,
        tab_id: 0,
    })
    .unwrap();
    assert_eq!(app.tabs[0].rows_streaming, Some(500));

    // Second progress update replaces the count
    app.handle_event(AppEvent::QueryProgress {
        rows_fetched: 1200,
        tab_id: 0,
    })
    .unwrap();
    assert_eq!(app.tabs[0].rows_streaming, Some(1200));
}

#[test]
fn test_query_completed_clears_rows_streaming() {
    let mut app = App::new();
    app.tabs[0].query_running = true;
    app.tabs[0].rows_streaming = Some(750);

    let results =
        crate::db::QueryResults::new(vec![], vec![], std::time::Duration::from_millis(10), 0);
    app.handle_event(AppEvent::QueryCompleted { results, tab_id: 0 })
        .unwrap();
    assert!(app.tabs[0].rows_streaming.is_none());
}

#[test]
fn test_query_failed_clears_rows_streaming() {
    let mut app = App::new();
    app.tabs[0].query_running = true;
    app.tabs[0].rows_streaming = Some(300);

    app.handle_event(AppEvent::QueryFailed {
        error: "some error".to_string(),
        position: None,
        tab_id: 0,
    })
    .unwrap();
    assert!(app.tabs[0].rows_streaming.is_none());
}

#[test]
fn test_query_progress_ignores_unknown_tab() {
    let mut app = App::new();
    // Progress for non-existent tab should not panic
    app.handle_event(AppEvent::QueryProgress {
        rows_fetched: 100,
        tab_id: 99,
    })
    .unwrap();
    assert!(app.tabs[0].rows_streaming.is_none());
}

#[test]
fn test_query_completed_truncated_shows_warning() {
    let mut app = App::new();
    app.tabs[0].query_running = true;

    // Create truncated results
    let results = crate::db::types::QueryResults::new_truncated(
        vec![],
        vec![],
        std::time::Duration::from_millis(10),
        1000,
        true,
    );
    app.handle_event(AppEvent::QueryCompleted { results, tab_id: 0 })
        .unwrap();

    let msg = app.status_message.as_ref().unwrap();
    assert!(
        msg.message.contains("limited"),
        "Should contain 'limited', got: {}",
        msg.message
    );
    assert_eq!(msg.level, StatusLevel::Warning);
}

#[test]
fn test_query_completed_not_truncated_shows_success() {
    let mut app = App::new();
    app.tabs[0].query_running = true;

    // Create non-truncated results
    let results = crate::db::types::QueryResults::new_truncated(
        vec![],
        vec![],
        std::time::Duration::from_millis(10),
        50,
        false,
    );
    app.handle_event(AppEvent::QueryCompleted { results, tab_id: 0 })
        .unwrap();

    let msg = app.status_message.as_ref().unwrap();
    assert!(
        !msg.message.contains("truncated"),
        "Should not contain 'truncated'"
    );
    assert_eq!(msg.level, StatusLevel::Success);
}

#[test]
fn test_query_failed_clears_running() {
    let mut app = App::new();
    app.tabs[0].query_running = true;

    app.handle_event(AppEvent::QueryFailed {
        error: "some error".to_string(),
        position: None,
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
        position: None,
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
    use crate::db::schema::{PaginatedVec, Schema, SchemaTree, Table};
    use crossterm::event::{KeyCode, KeyModifiers};

    let schema = SchemaTree {
        schemas: PaginatedVec::from_vec(vec![Schema {
            name: "public".to_string(),
            tables: PaginatedVec::from_vec(vec![Table {
                name: "users".to_string(),
                columns: vec![],
                row_count: None,
            }]),
            views: PaginatedVec::default(),
            indexes: PaginatedVec::default(),
            functions: PaginatedVec::default(),
        }]),
    };
    let mut app = App::with_connection(
        "test".to_string(),
        false,
        false,
        schema,
        &Settings::default(),
    );
    app.focus = PanelFocus::TreeBrowser;

    // Navigate to the "users" table node via public API
    // Auto-expanded items: [0] public, [1] Tables, [2] users
    app.tree_browser.move_down(); // → Tables
    app.tree_browser.move_down(); // → users

    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let action = app.handle_key(enter);

    match action {
        Action::ExecuteQuery { sql, max_rows, .. } => {
            // Paginated: LIMIT page_size+1 OFFSET 0
            assert_eq!(sql, "SELECT * FROM \"public\".\"users\" LIMIT 101 OFFSET 0");
            assert_eq!(max_rows, 0); // LIMIT in SQL controls rows
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
    // Editor shows the display SQL (LIMIT without +1)
    assert_eq!(
        app.tabs[0].editor.get_content(),
        "SELECT * FROM \"public\".\"users\" LIMIT 100"
    );
    // Pagination state should be set
    let pg = app.tabs[0].pagination.as_ref().unwrap();
    assert_eq!(pg.current_page, 0);
    assert_eq!(pg.page_size, 100);
    assert!(!pg.user_has_limit);
}

#[test]
fn test_enter_on_schema_node_expands() {
    use crate::db::schema::{PaginatedVec, Schema, SchemaTree, Table};
    use crossterm::event::{KeyCode, KeyModifiers};

    let schema = SchemaTree {
        schemas: PaginatedVec::from_vec(vec![
            Schema {
                name: "public".to_string(),
                tables: PaginatedVec::from_vec(vec![Table {
                    name: "t".to_string(),
                    columns: vec![],
                    row_count: None,
                }]),
                views: PaginatedVec::default(),
                indexes: PaginatedVec::default(),
                functions: PaginatedVec::default(),
            },
            Schema {
                name: "other".to_string(),
                tables: PaginatedVec::from_vec(vec![Table {
                    name: "x".to_string(),
                    columns: vec![],
                    row_count: None,
                }]),
                views: PaginatedVec::default(),
                indexes: PaginatedVec::default(),
                functions: PaginatedVec::default(),
            },
        ]),
    };
    let mut app = App::with_connection(
        "test".to_string(),
        false,
        false,
        schema,
        &Settings::default(),
    );
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
    use crate::db::schema::{PaginatedVec, Schema, SchemaTree, Table};

    let mut app = App::new();
    // Simulate having multiple tabs
    app.new_tab();
    app.new_tab();
    assert_eq!(app.tabs.len(), 3);

    let schema = SchemaTree {
        schemas: PaginatedVec::from_vec(vec![Schema {
            name: "public".to_string(),
            tables: PaginatedVec::from_vec(vec![Table {
                name: "users".to_string(),
                columns: vec![],
                row_count: None,
            }]),
            views: PaginatedVec::default(),
            indexes: PaginatedVec::default(),
            functions: PaginatedVec::default(),
        }]),
    };

    app.apply_connection("new-db".to_string(), false, false, schema);

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

// ── byte_offset_to_position tests ───────────────────────────

#[test]
fn test_byte_offset_single_line() {
    // PostgreSQL positions are 1-indexed, offset 6 points to char index 5
    let content = "SELECT * FROM foo";
    let (line, col) = byte_offset_to_position(content, 6);
    assert_eq!((line, col), (0, 5));
}

#[test]
fn test_byte_offset_multiline() {
    // "SELECT\nFROM" - offset 8 (1-indexed) is 'F' in FROM
    let content = "SELECT\nFROM";
    let (line, col) = byte_offset_to_position(content, 8);
    assert_eq!((line, col), (1, 0));
}

#[test]
fn test_byte_offset_at_newline() {
    // offset 7 (1-indexed) is the newline character itself
    let content = "SELECT\nFROM";
    let (line, col) = byte_offset_to_position(content, 7);
    assert_eq!((line, col), (0, 6));
}

#[test]
fn test_byte_offset_beyond_content() {
    // offset beyond content length should clamp to end
    let content = "SELECT";
    let (line, col) = byte_offset_to_position(content, 100);
    assert_eq!((line, col), (0, 6));
}

#[test]
fn test_byte_offset_at_start() {
    // offset 1 (1-indexed) is the first character
    let content = "SELECT";
    let (line, col) = byte_offset_to_position(content, 1);
    assert_eq!((line, col), (0, 0));
}

#[test]
fn test_query_failed_with_position_moves_cursor() {
    let mut app = App::new();
    app.tabs[0]
        .editor
        .set_content("SELEC * FROM foo".to_string());
    app.tabs[0].query_running = true;

    // Position 6 (1-indexed) points to the space after "SELEC"
    app.handle_event(AppEvent::QueryFailed {
        error: "syntax error".to_string(),
        position: Some(6),
        tab_id: 0,
    })
    .unwrap();

    assert_eq!(app.tabs[0].editor.cursor(), (0, 5));
}

#[test]
fn test_query_failed_without_position_no_cursor_move() {
    let mut app = App::new();
    app.tabs[0]
        .editor
        .set_content("SELECT * FROM foo".to_string());
    // Move cursor to end
    app.tabs[0].editor.set_cursor_position(0, 17);
    app.tabs[0].query_running = true;

    app.handle_event(AppEvent::QueryFailed {
        error: "connection error".to_string(),
        position: None,
        tab_id: 0,
    })
    .unwrap();

    // Cursor should remain at end
    assert_eq!(app.tabs[0].editor.cursor(), (0, 17));
}

// ── Query timeout tests ─────────────────────────────────────

#[test]
fn test_execute_query_includes_timeout() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0].editor.set_content("SELECT 1".to_string());

    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let action = app.handle_key(f5);

    match action {
        Action::ExecuteQuery { timeout_ms, .. } => {
            // Default timeout is 30000ms
            assert_eq!(timeout_ms, 30000);
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn test_custom_timeout_from_settings() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut settings = Settings::default();
    settings.settings.query_timeout_ms = 5000;
    let mut app = App::new_with_settings(&settings);
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0].editor.set_content("SELECT 1".to_string());

    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let action = app.handle_key(f5);

    match action {
        Action::ExecuteQuery { timeout_ms, .. } => {
            assert_eq!(timeout_ms, 5000);
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn test_zero_timeout_disables_timeout() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut settings = Settings::default();
    settings.settings.query_timeout_ms = 0;
    let mut app = App::new_with_settings(&settings);
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0].editor.set_content("SELECT 1".to_string());

    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let action = app.handle_key(f5);

    match action {
        Action::ExecuteQuery { timeout_ms, .. } => {
            assert_eq!(timeout_ms, 0);
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn test_explain_query_includes_timeout() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0].editor.set_content("SELECT 1".to_string());

    let ctrl_e = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
    let action = app.handle_key(ctrl_e);

    match action {
        Action::ExecuteQuery {
            timeout_ms, sql, ..
        } => {
            assert!(sql.starts_with("EXPLAIN"));
            assert_eq!(timeout_ms, 30000);
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

#[test]
fn test_tree_preview_includes_timeout() {
    use crate::db::schema::{PaginatedVec, Schema, SchemaTree, Table};
    use crossterm::event::{KeyCode, KeyModifiers};

    let schema = SchemaTree {
        schemas: PaginatedVec::from_vec(vec![Schema {
            name: "public".to_string(),
            tables: PaginatedVec::from_vec(vec![Table {
                name: "users".to_string(),
                columns: vec![],
                row_count: None,
            }]),
            views: PaginatedVec::default(),
            indexes: PaginatedVec::default(),
            functions: PaginatedVec::default(),
        }]),
    };
    let mut app = App::with_connection(
        "test".to_string(),
        false,
        false,
        schema,
        &Settings::default(),
    );
    app.focus = PanelFocus::TreeBrowser;

    // Navigate to users table
    app.tree_browser.move_down(); // → Tables
    app.tree_browser.move_down(); // → users

    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    let action = app.handle_key(enter);

    match action {
        Action::ExecuteQuery { timeout_ms, .. } => {
            assert_eq!(timeout_ms, 30000);
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
}

// ── Transaction state tracking tests ─────────────────────

#[test]
fn test_detect_transaction_intent() {
    assert_eq!(
        detect_transaction_intent("BEGIN"),
        Some(TransactionState::InTransaction)
    );
    assert_eq!(
        detect_transaction_intent("  begin  "),
        Some(TransactionState::InTransaction)
    );
    assert_eq!(
        detect_transaction_intent("START TRANSACTION"),
        Some(TransactionState::InTransaction)
    );
    assert_eq!(
        detect_transaction_intent("COMMIT"),
        Some(TransactionState::Idle)
    );
    assert_eq!(
        detect_transaction_intent("END"),
        Some(TransactionState::Idle)
    );
    assert_eq!(
        detect_transaction_intent("ROLLBACK"),
        Some(TransactionState::Idle)
    );
    assert_eq!(
        detect_transaction_intent("rollback"),
        Some(TransactionState::Idle)
    );
    assert_eq!(
        detect_transaction_intent("ABORT"),
        Some(TransactionState::Idle)
    );
    assert_eq!(detect_transaction_intent("SELECT 1"), None);
    assert_eq!(detect_transaction_intent("INSERT INTO t VALUES(1)"), None);
    assert_eq!(detect_transaction_intent("  "), None);

    // Semicolons should be stripped (regression: BEGIN; was not detected)
    assert_eq!(
        detect_transaction_intent("BEGIN;"),
        Some(TransactionState::InTransaction)
    );
    assert_eq!(
        detect_transaction_intent("COMMIT;"),
        Some(TransactionState::Idle)
    );
    assert_eq!(
        detect_transaction_intent("ROLLBACK;"),
        Some(TransactionState::Idle)
    );
}

#[test]
fn test_transaction_state_begin_commit_cycle() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    assert_eq!(app.tabs[0].transaction_state, TransactionState::Idle);

    // Execute BEGIN
    app.tabs[0].editor.set_content("BEGIN".to_string());
    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let action = app.handle_key(f5);
    assert!(matches!(action, Action::ExecuteQuery { .. }));
    assert_eq!(
        app.tabs[0].transaction_state,
        TransactionState::InTransaction
    );

    // Execute a SELECT (no state change)
    app.tabs[0]
        .editor
        .set_content("SELECT * FROM users".to_string());
    let action = app.handle_key(f5);
    assert!(matches!(action, Action::ExecuteQuery { .. }));
    assert_eq!(
        app.tabs[0].transaction_state,
        TransactionState::InTransaction
    );

    // Execute COMMIT
    app.tabs[0].editor.set_content("COMMIT".to_string());
    let action = app.handle_key(f5);
    assert!(matches!(action, Action::ExecuteQuery { .. }));
    assert_eq!(app.tabs[0].transaction_state, TransactionState::Idle);
}

#[test]
fn test_transaction_state_error_transitions_to_failed() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;

    // BEGIN
    app.tabs[0].editor.set_content("BEGIN".to_string());
    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    app.handle_key(f5);
    assert_eq!(
        app.tabs[0].transaction_state,
        TransactionState::InTransaction
    );

    // Simulate query failure while in transaction
    app.tabs[0].query_running = true;
    app.handle_event(AppEvent::QueryFailed {
        error: "relation does not exist".to_string(),
        position: None,
        tab_id: 0,
    })
    .unwrap();
    assert_eq!(app.tabs[0].transaction_state, TransactionState::Failed);

    // ROLLBACK should return to Idle
    app.focus = PanelFocus::QueryEditor; // QueryFailed moves focus to results
    app.tabs[0].editor.set_content("ROLLBACK".to_string());
    app.handle_key(f5);
    assert_eq!(app.tabs[0].transaction_state, TransactionState::Idle);
}

#[test]
fn test_transaction_state_cancel_does_not_fail() {
    let mut app = App::new();
    app.tabs[0].transaction_state = TransactionState::InTransaction;
    app.tabs[0].query_running = true;

    // Cancellation should NOT transition to Failed
    app.handle_event(AppEvent::QueryFailed {
        error: "ERROR: canceling statement due to user request".to_string(),
        position: None,
        tab_id: 0,
    })
    .unwrap();
    assert_eq!(
        app.tabs[0].transaction_state,
        TransactionState::InTransaction
    );
}

#[test]
fn test_transaction_state_reset_on_connection() {
    use crate::db::schema::SchemaTree;

    let mut app = App::new();
    app.tabs[0].transaction_state = TransactionState::InTransaction;
    app.apply_connection("test".to_string(), false, false, SchemaTree::new());
    assert_eq!(app.tabs[0].transaction_state, TransactionState::Idle);
}

#[test]
fn test_transaction_state_reset_on_connection_lost() {
    let mut app = App::new();
    app.tabs[0].transaction_state = TransactionState::InTransaction;
    app.handle_event(AppEvent::ConnectionLost {
        tab_id: 0,
        message: "gone".to_string(),
    })
    .unwrap();
    assert_eq!(app.tabs[0].transaction_state, TransactionState::Idle);
}

// ── Destructive query confirmation tests ─────────────────

#[test]
fn test_is_destructive_query() {
    assert_eq!(is_destructive_query("DROP TABLE users"), Some("DROP"));
    assert_eq!(is_destructive_query("drop table users"), Some("DROP"));
    assert_eq!(is_destructive_query("DROP INDEX idx_name"), Some("DROP"));
    assert_eq!(is_destructive_query("DROP SCHEMA public"), Some("DROP"));
    assert_eq!(is_destructive_query("DROP DATABASE mydb"), Some("DROP"));
    assert_eq!(is_destructive_query("DROP VIEW my_view"), Some("DROP"));
    assert_eq!(
        is_destructive_query("DROP MATERIALIZED VIEW mv"),
        Some("DROP")
    );
    assert_eq!(is_destructive_query("DROP FUNCTION fn()"), Some("DROP"));
    assert_eq!(is_destructive_query("TRUNCATE users"), Some("TRUNCATE"));
    assert_eq!(
        is_destructive_query("DELETE FROM users"),
        Some("DELETE without WHERE")
    );
    assert_eq!(
        is_destructive_query("ALTER TABLE users DROP COLUMN name"),
        Some("ALTER TABLE DROP")
    );

    // Safe queries
    assert_eq!(is_destructive_query("SELECT * FROM users"), None);
    assert_eq!(is_destructive_query("INSERT INTO users VALUES(1)"), None);
    assert_eq!(is_destructive_query("DELETE FROM users WHERE id = 1"), None);
    assert_eq!(
        is_destructive_query("ALTER TABLE users ADD COLUMN age int"),
        None
    );
    assert_eq!(is_destructive_query("UPDATE users SET name = 'x'"), None);
    assert_eq!(is_destructive_query("BEGIN"), None);
    assert_eq!(is_destructive_query("COMMIT"), None);
}

#[test]
fn test_destructive_query_triggers_confirmation() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0]
        .editor
        .set_content("DROP TABLE users".to_string());

    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let action = app.handle_key(f5);

    // Should NOT execute yet
    assert!(matches!(action, Action::None));
    assert!(app.is_confirm_pending());
    let msg = app.status_message.as_ref().unwrap();
    assert!(msg.message.contains("DROP"));
    assert!(msg.message.contains("(y/N)"));
}

#[test]
fn test_confirm_y_executes_query() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0]
        .editor
        .set_content("DROP TABLE users".to_string());

    // Trigger confirmation
    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    app.handle_key(f5);
    assert!(app.is_confirm_pending());

    // Press 'y' to confirm
    let y = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
    let action = app.handle_key(y);
    assert!(matches!(action, Action::ExecuteQuery { .. }));
    assert!(!app.is_confirm_pending());
}

#[test]
fn test_confirm_n_cancels_query() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0]
        .editor
        .set_content("TRUNCATE orders".to_string());

    // Trigger confirmation
    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    app.handle_key(f5);
    assert!(app.is_confirm_pending());

    // Press 'n' to cancel
    let n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
    let action = app.handle_key(n);
    assert!(matches!(action, Action::None));
    assert!(!app.is_confirm_pending());
    assert_eq!(
        app.status_message.as_ref().unwrap().message,
        "Query cancelled"
    );
}

#[test]
fn test_confirm_esc_cancels_query() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0]
        .editor
        .set_content("DROP TABLE users".to_string());

    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    app.handle_key(f5);
    assert!(app.is_confirm_pending());

    let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let action = app.handle_key(esc);
    assert!(matches!(action, Action::None));
    assert!(!app.is_confirm_pending());
}

#[test]
fn test_confirm_disabled_executes_immediately() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut settings = Settings::default();
    settings.settings.confirm_destructive = false;
    let mut app = App::new_with_settings(&settings);
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0]
        .editor
        .set_content("DROP TABLE users".to_string());

    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let action = app.handle_key(f5);
    assert!(matches!(action, Action::ExecuteQuery { .. }));
    assert!(!app.is_confirm_pending());
}

#[test]
fn test_safe_query_no_confirmation() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0]
        .editor
        .set_content("SELECT * FROM users".to_string());

    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let action = app.handle_key(f5);
    assert!(matches!(action, Action::ExecuteQuery { .. }));
    assert!(!app.is_confirm_pending());
}

#[test]
fn test_delete_with_where_is_safe() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.tabs[0]
        .editor
        .set_content("DELETE FROM users WHERE id = 1".to_string());

    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let action = app.handle_key(f5);
    assert!(matches!(action, Action::ExecuteQuery { .. }));
    assert!(!app.is_confirm_pending());
}

#[test]
fn test_apply_connection_resets_transaction_state() {
    use crate::db::schema::SchemaTree;
    let mut app = App::new();
    app.tabs[0].transaction_state = TransactionState::InTransaction;
    app.apply_connection("test-db".to_string(), false, false, SchemaTree::new());
    assert_eq!(app.connection_name.as_deref(), Some("test-db"));
    assert_eq!(app.tabs[0].transaction_state, TransactionState::Idle);
}

#[test]
fn test_close_tab_with_active_transaction_warns() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;

    // Open a second tab so we can close one
    app.new_tab();
    assert_eq!(app.tabs.len(), 2);

    // Set transaction state on active tab (tab 2)
    app.tab_mut().transaction_state = TransactionState::InTransaction;

    // Close tab — should succeed with warning
    let close_key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
    let action = app.handle_key(close_key);
    assert!(matches!(action, Action::TabClosed { .. }));
    assert_eq!(app.tabs.len(), 1);

    // Should have set a warning status about uncommitted transaction
    assert!(app.status_message.is_some());
    let msg = &app.status_message.unwrap().message;
    assert!(msg.contains("uncommitted transaction"));
}

#[test]
fn test_close_tab_blocks_while_query_running() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;

    // Open second tab and mark query as running
    app.new_tab();
    app.tab_mut().query_running = true;

    let close_key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
    let action = app.handle_key(close_key);
    assert!(matches!(action, Action::None));
    assert_eq!(app.tabs.len(), 2); // Tab not closed
}

#[test]
fn test_transaction_state_isolated_per_tab() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;

    // Tab 0: start a transaction
    app.tabs[0].editor.set_content("BEGIN".to_string());
    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    app.handle_key(f5);
    assert_eq!(
        app.tabs[0].transaction_state,
        TransactionState::InTransaction
    );

    // Open tab 1 — should start Idle
    app.new_tab();
    assert_eq!(app.tabs[1].transaction_state, TransactionState::Idle);

    // Run a query on tab 1 — tab 0's state should be unaffected
    app.tabs[1].editor.set_content("SELECT 1".to_string());
    app.handle_key(f5);
    assert_eq!(app.tabs[1].transaction_state, TransactionState::Idle);
    assert_eq!(
        app.tabs[0].transaction_state,
        TransactionState::InTransaction
    );
}

#[test]
fn test_close_tab_with_failed_transaction_warns() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.new_tab();

    // Set failed transaction state on active tab
    app.tab_mut().transaction_state = TransactionState::Failed;

    let close_key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
    let action = app.handle_key(close_key);
    assert!(matches!(action, Action::TabClosed { .. }));
    assert!(app.status_message.is_some());
    assert!(
        app.status_message
            .unwrap()
            .message
            .contains("uncommitted transaction")
    );
}

#[test]
fn test_connection_lost_resets_only_affected_tab() {
    let mut app = App::new();
    app.new_tab();

    // Both tabs have active transactions
    app.tabs[0].transaction_state = TransactionState::InTransaction;
    app.tabs[1].transaction_state = TransactionState::Failed;

    // Only tab 0 loses its connection
    app.handle_event(AppEvent::ConnectionLost {
        tab_id: 0,
        message: "server crashed".to_string(),
    })
    .unwrap();

    // Tab 0 reset, tab 1 unaffected
    assert_eq!(app.tabs[0].transaction_state, TransactionState::Idle);
    assert_eq!(app.tabs[1].transaction_state, TransactionState::Failed);
}

// ── Saved queries tests ──────────────────────────

#[test]
fn test_save_query_requires_saved_connection() {
    let mut app = App::new();
    app.connection_name = Some("test".to_string());
    app.is_saved_connection = false;
    app.tab_mut().editor.set_content("SELECT 1".to_string());

    let action = app.execute_command(Command::SaveQuery {
        name: Some("q1".to_string()),
    });
    assert!(matches!(action, Action::None));
    assert!(
        app.status_message
            .as_ref()
            .unwrap()
            .message
            .contains("Save a connection")
    );
}

#[test]
fn test_save_query_requires_editor_content() {
    let mut app = App::new();
    app.connection_name = Some("test".to_string());
    app.is_saved_connection = true;

    let action = app.execute_command(Command::SaveQuery {
        name: Some("q1".to_string()),
    });
    assert!(matches!(action, Action::None));
    assert!(
        app.status_message
            .as_ref()
            .unwrap()
            .message
            .contains("empty")
    );
}

#[test]
fn test_save_query_no_name_opens_prompt() {
    let mut app = App::new();
    app.connection_name = Some("test".to_string());
    app.is_saved_connection = true;
    app.tab_mut().editor.set_content("SELECT 1".to_string());

    app.execute_command(Command::SaveQuery { name: None });

    assert!(app.pending_save_query);
    assert!(app.command_bar.is_prompt_mode());
    assert_eq!(app.focus, PanelFocus::CommandBar);
}

#[test]
fn test_dismiss_clears_pending_save_query() {
    let mut app = App::new();
    app.connection_name = Some("test".to_string());
    app.is_saved_connection = true;
    app.tab_mut().editor.set_content("SELECT 1".to_string());

    app.execute_command(Command::SaveQuery { name: None });
    assert!(app.pending_save_query);

    // Press Escape to dismiss
    app.handle_event(AppEvent::Key(KeyEvent::from(
        crossterm::event::KeyCode::Esc,
    )))
    .unwrap();

    assert!(!app.pending_save_query);
    assert!(!app.command_bar.is_active());
}

#[test]
fn test_expand_on_saved_query_loads_into_editor() {
    use crate::config::SavedQuery;
    use crate::db::schema::SchemaTree;

    let mut app = App::new();
    app.connection_name = Some("test".to_string());
    app.is_saved_connection = true;
    app.focus = PanelFocus::TreeBrowser;

    // Set an empty schema so the tree can rebuild
    app.tree_browser.set_schema(SchemaTree::new());

    // Add a saved query to the tree (auto-expands the section)
    app.tree_browser.set_saved_queries(vec![SavedQuery {
        connection: "test".to_string(),
        name: "my-query".to_string(),
        sql: "SELECT * FROM users".to_string(),
    }]);

    // Move down from header to the saved query item
    app.tree_browser.move_down();

    // Press Enter (Expand)
    let key = crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Enter);
    let action = app.handle_event(AppEvent::Key(key)).unwrap();

    assert!(matches!(action, Action::None));
    assert_eq!(app.tab().editor.get_content(), "SELECT * FROM users");
    assert_eq!(app.focus, PanelFocus::QueryEditor);
}

// ── Pagination tests ─────────────────────────────

#[test]
fn test_pagination_state_paged_sql() {
    let pg = PaginationState {
        original_sql: "SELECT * FROM users".to_string(),
        current_page: 0,
        page_size: 500,
        has_more: false,
        user_has_limit: false,
        previous_page: None,
    };
    assert_eq!(pg.paged_sql(), "SELECT * FROM users LIMIT 501 OFFSET 0");
    assert_eq!(pg.offset(), 0);

    let pg2 = PaginationState {
        current_page: 2,
        ..pg.clone()
    };
    assert_eq!(pg2.paged_sql(), "SELECT * FROM users LIMIT 501 OFFSET 1000");
    assert_eq!(pg2.offset(), 1000);
}

#[test]
fn test_pagination_state_user_limit_passthrough() {
    let pg = PaginationState {
        original_sql: "SELECT * FROM users LIMIT 10".to_string(),
        current_page: 0,
        page_size: 500,
        has_more: false,
        user_has_limit: true,
        previous_page: None,
    };
    assert_eq!(pg.paged_sql(), "SELECT * FROM users LIMIT 10");
}

#[test]
fn test_prepare_execute_paginates_simple_query() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.tab_mut()
        .editor
        .set_content("SELECT * FROM users".to_string());

    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let action = app.handle_key(f5);

    match action {
        Action::ExecuteQuery { sql, max_rows, .. } => {
            assert!(sql.contains("LIMIT"));
            assert!(sql.contains("OFFSET"));
            assert_eq!(max_rows, 0);
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
    assert!(app.tab().pagination.is_some());
}

#[test]
fn test_prepare_execute_skips_pagination_for_user_limit() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.tab_mut()
        .editor
        .set_content("SELECT * FROM users LIMIT 10".to_string());

    let f5 = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let action = app.handle_key(f5);

    match action {
        Action::ExecuteQuery { sql, max_rows, .. } => {
            assert_eq!(sql, "SELECT * FROM users LIMIT 10");
            assert_eq!(max_rows, 1000); // safety net
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
    assert!(app.tab().pagination.is_none());
}

#[test]
fn test_prepare_execute_skips_pagination_for_explain() {
    use crossterm::event::{KeyCode, KeyModifiers};

    let mut app = App::new();
    app.focus = PanelFocus::QueryEditor;
    app.tab_mut()
        .editor
        .set_content("SELECT * FROM users".to_string());

    let ctrl_e = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
    let action = app.handle_key(ctrl_e);

    match action {
        Action::ExecuteQuery { sql, .. } => {
            assert!(sql.starts_with("EXPLAIN"));
            // EXPLAIN should NOT have LIMIT/OFFSET appended
            assert!(!sql.contains("OFFSET"));
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
    assert!(app.tab().pagination.is_none());
}

#[test]
fn test_next_page_when_has_more() {
    let mut app = App::new();
    app.focus = PanelFocus::ResultsViewer;
    app.tab_mut().pagination = Some(PaginationState {
        original_sql: "SELECT * FROM users".to_string(),
        current_page: 0,
        page_size: 100,
        has_more: true,
        user_has_limit: false,
        previous_page: None,
    });

    let n = KeyEvent::from(crossterm::event::KeyCode::Char('n'));
    let action = app.handle_key(n);

    match action {
        Action::ExecuteQuery { sql, .. } => {
            assert!(sql.contains("OFFSET 100"));
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
    assert_eq!(app.tab().pagination.as_ref().unwrap().current_page, 1);
}

#[test]
fn test_next_page_noop_when_no_more() {
    let mut app = App::new();
    app.focus = PanelFocus::ResultsViewer;
    app.tab_mut().pagination = Some(PaginationState {
        original_sql: "SELECT * FROM users".to_string(),
        current_page: 0,
        page_size: 100,
        has_more: false,
        user_has_limit: false,
        previous_page: None,
    });

    let n = KeyEvent::from(crossterm::event::KeyCode::Char('n'));
    let action = app.handle_key(n);
    assert!(matches!(action, Action::None));
}

#[test]
fn test_prev_page_when_on_page_two() {
    let mut app = App::new();
    app.focus = PanelFocus::ResultsViewer;
    app.tab_mut().pagination = Some(PaginationState {
        original_sql: "SELECT * FROM users".to_string(),
        current_page: 2,
        page_size: 100,
        has_more: true,
        user_has_limit: false,
        previous_page: None,
    });

    let p = KeyEvent::from(crossterm::event::KeyCode::Char('p'));
    let action = app.handle_key(p);

    match action {
        Action::ExecuteQuery { sql, .. } => {
            assert!(sql.contains("OFFSET 100"));
        }
        other => panic!(
            "Expected ExecuteQuery, got {:?}",
            std::mem::discriminant(&other)
        ),
    }
    assert_eq!(app.tab().pagination.as_ref().unwrap().current_page, 1);
}

#[test]
fn test_prev_page_noop_on_first_page() {
    let mut app = App::new();
    app.focus = PanelFocus::ResultsViewer;
    app.tab_mut().pagination = Some(PaginationState {
        original_sql: "SELECT * FROM users".to_string(),
        current_page: 0,
        page_size: 100,
        has_more: true,
        user_has_limit: false,
        previous_page: None,
    });

    let p = KeyEvent::from(crossterm::event::KeyCode::Char('p'));
    let action = app.handle_key(p);
    assert!(matches!(action, Action::None));
}

#[test]
fn test_query_completed_trims_pagination_probe_row() {
    use crate::db::types::{CellValue, ColumnDef, DataType, Row};

    let mut app = App::new();
    app.tab_mut().query_running = true;
    app.tab_mut().pagination = Some(PaginationState {
        original_sql: "SELECT 1".to_string(),
        current_page: 0,
        page_size: 2,
        has_more: false,
        user_has_limit: false,
        previous_page: None,
    });

    // Return 3 rows (page_size + 1) to indicate more exist
    let cols = vec![ColumnDef {
        name: "x".to_string(),
        data_type: DataType::Integer,
        nullable: false,
    }];
    let rows = vec![
        Row {
            values: vec![CellValue::Integer(1)],
        },
        Row {
            values: vec![CellValue::Integer(2)],
        },
        Row {
            values: vec![CellValue::Integer(3)],
        },
    ];
    let results = QueryResults::new(cols, rows, std::time::Duration::from_millis(5), 3);

    app.handle_event(AppEvent::QueryCompleted { results, tab_id: 0 })
        .unwrap();

    // Should have trimmed to page_size (2 rows) and set has_more
    let pg = app.tab().pagination.as_ref().unwrap();
    assert!(pg.has_more);
    assert_eq!(app.tab().results_viewer.results().unwrap().rows.len(), 2);
}

#[test]
fn test_query_failed_rolls_back_pagination_page() {
    let mut app = App::new();
    app.focus = PanelFocus::ResultsViewer;

    // Set up pagination on page 1 with has_more
    app.tab_mut().pagination = Some(PaginationState {
        original_sql: "SELECT * FROM users".to_string(),
        current_page: 1,
        page_size: 100,
        has_more: true,
        user_has_limit: false,
        previous_page: None,
    });

    // Navigate to page 2
    let n = KeyEvent::from(crossterm::event::KeyCode::Char('n'));
    let action = app.handle_key(n);
    assert!(matches!(action, Action::ExecuteQuery { .. }));
    assert_eq!(app.tab().pagination.as_ref().unwrap().current_page, 2);

    // Query fails — should roll back to page 1
    app.handle_event(AppEvent::QueryFailed {
        error: "connection lost".to_string(),
        position: None,
        tab_id: 0,
    })
    .unwrap();

    let pg = app.tab().pagination.as_ref().unwrap();
    assert_eq!(pg.current_page, 1);
    assert!(pg.has_more); // restored
    assert!(pg.previous_page.is_none()); // cleared after rollback
}

#[test]
fn test_query_failed_no_rollback_on_initial_query() {
    let mut app = App::new();
    app.tab_mut().query_running = true;

    // Pagination from initial execute (no previous_page)
    app.tab_mut().pagination = Some(PaginationState {
        original_sql: "SELECT * FROM users".to_string(),
        current_page: 0,
        page_size: 100,
        has_more: false,
        user_has_limit: false,
        previous_page: None,
    });

    app.handle_event(AppEvent::QueryFailed {
        error: "syntax error".to_string(),
        position: None,
        tab_id: 0,
    })
    .unwrap();

    // Page stays at 0 — no rollback needed
    let pg = app.tab().pagination.as_ref().unwrap();
    assert_eq!(pg.current_page, 0);
}

// ── is_write_query tests ──────────────────────────────────────

#[test]
fn test_is_write_query_detects_writes() {
    assert_eq!(
        is_write_query("INSERT INTO users VALUES (1)"),
        Some("INSERT")
    );
    assert_eq!(is_write_query("update users set name='x'"), Some("UPDATE"));
    assert_eq!(is_write_query("  DELETE FROM users"), Some("DELETE"));
    assert_eq!(is_write_query("CREATE TABLE t (id int)"), Some("CREATE"));
    assert_eq!(is_write_query("ALTER TABLE t ADD col int"), Some("ALTER"));
    assert_eq!(is_write_query("DROP TABLE users"), Some("DROP"));
    assert_eq!(is_write_query("TRUNCATE users"), Some("TRUNCATE"));
    assert_eq!(
        is_write_query("GRANT SELECT ON t TO u"),
        Some("GRANT/REVOKE")
    );
    assert_eq!(
        is_write_query("REVOKE ALL ON t FROM u"),
        Some("GRANT/REVOKE")
    );
    assert_eq!(is_write_query("COMMENT ON TABLE t IS 'x'"), Some("COMMENT"));
}

#[test]
fn test_is_write_query_allows_reads() {
    assert_eq!(is_write_query("SELECT 1"), None);
    assert_eq!(is_write_query("select * from users"), None);
    assert_eq!(is_write_query("EXPLAIN ANALYZE SELECT 1"), None);
    assert_eq!(is_write_query("  SHOW server_version"), None);
    assert_eq!(is_write_query("BEGIN"), None);
    assert_eq!(is_write_query("COMMIT"), None);
    assert_eq!(is_write_query("ROLLBACK"), None);
    assert_eq!(is_write_query("SET search_path TO public"), None);
}

// ── Read-only mode tests ──────────────────────────────────────

#[test]
fn test_read_only_blocks_write_query() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = App::new();
    app.read_only = true;
    app.focus = PanelFocus::QueryEditor;
    app.tab_mut()
        .editor
        .insert_text("INSERT INTO users VALUES (1)");

    let execute = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let action = app.handle_key(execute);
    assert!(matches!(action, Action::None));
    assert!(app.status_message.is_some());
    assert!(
        app.status_message
            .as_ref()
            .unwrap()
            .message
            .contains("Read-only"),
        "Expected read-only error, got: {}",
        app.status_message.as_ref().unwrap().message,
    );
}

#[test]
fn test_read_only_allows_select() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = App::new();
    app.read_only = true;
    app.focus = PanelFocus::QueryEditor;
    app.tab_mut().editor.insert_text("SELECT * FROM users");

    let execute = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let action = app.handle_key(execute);
    // Should proceed to execution (returns ExecuteQuery), not block
    assert!(!matches!(action, Action::None));
}

#[test]
fn test_read_only_not_set_allows_writes() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let mut app = App::new();
    app.read_only = false;
    app.focus = PanelFocus::QueryEditor;
    app.tab_mut().editor.insert_text("DROP TABLE users");

    let execute = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
    let _action = app.handle_key(execute);
    // Without read_only, destructive confirmation dialog appears (not blocked)
    assert!(
        app.status_message.is_some()
            && app
                .status_message
                .as_ref()
                .unwrap()
                .message
                .contains("DROP"),
    );
}

#[test]
fn test_apply_connection_sets_read_only() {
    use crate::db::schema::SchemaTree;

    let mut app = App::new();
    assert!(!app.read_only);

    // Connection with read_only = true enables it
    app.apply_connection("prod".to_string(), true, true, SchemaTree::new());
    assert!(app.read_only);

    // Connection with read_only = false disables it (when global is false)
    app.apply_connection("dev".to_string(), false, false, SchemaTree::new());
    assert!(!app.read_only);
}

#[test]
fn test_global_read_only_overrides_connection() {
    use crate::config::settings::Settings;
    use crate::db::schema::SchemaTree;

    let mut settings = Settings::default();
    settings.settings.read_only = true;

    // Global read_only = true, connection read_only = false → still read-only
    let app = App::with_connection(
        "dev".to_string(),
        false,
        false,
        SchemaTree::new(),
        &settings,
    );
    assert!(app.read_only, "global read_only should override connection");
}

#[test]
fn test_translate_meta_command_dt() {
    let sql = translate_meta_command("\\dt").unwrap();
    assert!(sql.contains("relkind = 'r'"), "should query tables");
    assert!(sql.contains("ORDER BY schema, name"));
}

#[test]
fn test_translate_meta_command_dv() {
    let sql = translate_meta_command("\\dv").unwrap();
    assert!(sql.contains("relkind IN ('v', 'm')"), "should query views");
}

#[test]
fn test_translate_meta_command_di() {
    let sql = translate_meta_command("\\di").unwrap();
    assert!(sql.contains("pg_index"), "should query indexes");
    assert!(sql.contains("amname"));
}

#[test]
fn test_translate_meta_command_dn() {
    let sql = translate_meta_command("\\dn").unwrap();
    assert!(sql.contains("pg_namespace"), "should query schemas");
}

#[test]
fn test_translate_meta_command_d_table() {
    let sql = translate_meta_command("\\d users").unwrap();
    assert!(
        sql.contains("relname = 'users'"),
        "should filter by table name"
    );
    assert!(sql.contains("attnum > 0"), "should include columns");
    // Expanded: indexes, constraints, referenced-by, triggers
    assert!(sql.contains("pg_index"), "should include indexes");
    assert!(sql.contains("pg_constraint"), "should include constraints");
    assert!(sql.contains("pg_trigger"), "should include triggers");
    assert!(
        sql.contains("'Referenced by'"),
        "should include referenced-by"
    );
}

#[test]
fn test_translate_meta_command_d_schema_table() {
    let sql = translate_meta_command("\\d public.users").unwrap();
    assert!(sql.contains("relname = 'users'"));
    assert!(sql.contains("nspname = 'public'"));
}

#[test]
fn test_translate_meta_command_d_without_arg_returns_none() {
    assert!(
        translate_meta_command("\\d").is_none(),
        "\\d alone needs a table name"
    );
}

#[test]
fn test_translate_meta_command_rejects_invalid_table_name() {
    assert!(translate_meta_command("\\d users; DROP TABLE--").is_none());
    assert!(translate_meta_command("\\d 'injection'").is_none());
    // Edge cases with dots
    assert!(translate_meta_command("\\d .").is_none());
    assert!(translate_meta_command("\\d schema.").is_none());
    assert!(translate_meta_command("\\d .table").is_none());
    assert!(translate_meta_command("\\d a.b.c").is_none());
}

#[test]
fn test_translate_meta_command_not_meta() {
    assert!(translate_meta_command("SELECT 1").is_none());
    assert!(translate_meta_command("").is_none());
}

#[test]
fn test_translate_meta_command_unknown() {
    assert!(translate_meta_command("\\z").is_none());
    assert!(translate_meta_command("\\c mydb").is_none());
}

#[test]
fn test_translate_meta_command_with_whitespace() {
    assert!(translate_meta_command("  \\dt  ").is_some());
    assert!(translate_meta_command("  \\d  users  ").is_some());
}

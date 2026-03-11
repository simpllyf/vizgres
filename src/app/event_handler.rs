//! Event handling for the application state machine
//
//! Dispatches AppEvents (keyboard input, query results, schema loads, connection
//! events) to the appropriate handler and returns an Action for the main loop.

use crossterm::event::KeyEvent;

use super::sql_utils::byte_offset_to_position;
use super::*;

impl App {
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
            AppEvent::QueryProgress {
                rows_fetched,
                tab_id,
            } => {
                if let Some(idx) = self.tab_index_by_id(tab_id) {
                    self.tabs[idx].rows_streaming = Some(rows_fetched);
                }
                Ok(Action::None)
            }
            AppEvent::QueryCompleted {
                mut results,
                tab_id,
            } => {
                let time = results.execution_time;

                if let Some(idx) = self.tab_index_by_id(tab_id) {
                    self.tabs[idx].query_running = false;
                    self.tabs[idx].query_start = None;
                    self.tabs[idx].rows_streaming = None;

                    // Process pagination: trim the +1 probe row and update state
                    let pagination_info = if let Some(ref mut pg) = self.tabs[idx].pagination {
                        pg.previous_page = None; // navigation succeeded, clear rollback
                        if !pg.user_has_limit && results.rows.len() > pg.page_size {
                            // More rows exist — trim to page_size
                            results.rows.truncate(pg.page_size);
                            results.row_count = pg.page_size;
                            results.truncated = false; // we handle it via pagination
                            pg.has_more = true;
                        } else {
                            pg.has_more = false;
                        }
                        Some(crate::ui::results::PaginationInfo {
                            page_offset: pg.offset(),
                            has_more: pg.has_more,
                            has_prev: pg.current_page > 0,
                        })
                    } else {
                        None
                    };

                    // Route EXPLAIN JSON results to the visual tree viewer
                    if self.tabs[idx].explain_pending {
                        self.tabs[idx].explain_pending = false;
                        let json_str = results
                            .rows
                            .first()
                            .and_then(|r| r.values.first())
                            .and_then(|v| match v {
                                crate::db::types::CellValue::Text(s) => Some(s.as_str()),
                                crate::db::types::CellValue::Json(s) => Some(s.as_str()),
                                _ => None,
                            });
                        if let Some(viewer) =
                            json_str.and_then(|s| ExplainViewer::from_json(s, time))
                        {
                            self.tabs[idx].explain_viewer = Some(viewer);
                            if idx == self.active_tab {
                                self.focus = PanelFocus::ResultsViewer;
                            }
                            self.set_status(
                                format!(
                                    "EXPLAIN in {:.1}ms — t to toggle raw text",
                                    time.as_secs_f64() * 1000.0
                                ),
                                StatusLevel::Success,
                            );
                            return Ok(Action::None);
                        }
                        // JSON parse failed — fall through to normal results display
                    }

                    self.tabs[idx].explain_viewer = None;
                    self.tabs[idx].results_viewer.set_results(results);
                    self.tabs[idx]
                        .results_viewer
                        .set_pagination(pagination_info.clone());

                    if idx == self.active_tab {
                        self.focus = PanelFocus::ResultsViewer;
                    }

                    // Status message
                    if let Some(ref info) = pagination_info {
                        let row_count = self.tabs[idx]
                            .results_viewer
                            .results()
                            .map_or(0, |r| r.rows.len());
                        if row_count == 0 {
                            self.set_status(
                                format!("0 rows in {:.1}ms", time.as_secs_f64() * 1000.0),
                                StatusLevel::Success,
                            );
                        } else {
                            let start = info.page_offset + 1;
                            let end = info.page_offset + row_count;
                            let more = if info.has_more { "+" } else { "" };
                            let hint = if info.has_more {
                                " — n for next page"
                            } else {
                                ""
                            };
                            self.set_status(
                                format!(
                                    "Rows {}-{} of {}{} in {:.1}ms{}",
                                    start,
                                    end,
                                    end,
                                    more,
                                    time.as_secs_f64() * 1000.0,
                                    hint,
                                ),
                                if info.has_more {
                                    StatusLevel::Info
                                } else {
                                    StatusLevel::Success
                                },
                            );
                        }
                    } else {
                        let count = self.tabs[idx]
                            .results_viewer
                            .results()
                            .map_or(0, |r| r.row_count);
                        let truncated = self.tabs[idx]
                            .results_viewer
                            .results()
                            .is_some_and(|r| r.truncated);
                        if truncated {
                            self.set_status(
                                format!(
                                    "{} rows (limited) in {:.1}ms",
                                    count,
                                    time.as_secs_f64() * 1000.0,
                                ),
                                StatusLevel::Warning,
                            );
                        } else {
                            self.set_status(
                                format!("{} rows in {:.1}ms", count, time.as_secs_f64() * 1000.0),
                                StatusLevel::Success,
                            );
                        }
                    }
                } else {
                    // Tab was closed while query was running
                    self.set_status(
                        format!(
                            "{} rows in {:.1}ms",
                            results.row_count,
                            time.as_secs_f64() * 1000.0
                        ),
                        StatusLevel::Success,
                    );
                }
                Ok(Action::None)
            }
            AppEvent::QueryFailed {
                error,
                position,
                tab_id,
            } => {
                let cancelled = error.contains("canceling statement due to user request");

                if let Some(idx) = self.tab_index_by_id(tab_id) {
                    self.tabs[idx].rows_streaming = None;
                    // Transition to Failed if this tab is inside a transaction
                    if self.tabs[idx].transaction_state == TransactionState::InTransaction
                        && !cancelled
                    {
                        self.tabs[idx].transaction_state = TransactionState::Failed;
                    }

                    // Roll back pagination page if a page-navigation query failed
                    if let Some(ref mut pg) = self.tabs[idx].pagination
                        && let Some(prev) = pg.previous_page.take()
                    {
                        pg.current_page = prev;
                        pg.has_more = true;
                    }

                    self.tabs[idx].query_running = false;
                    self.tabs[idx].query_start = None;
                    self.tabs[idx].results_viewer.set_error(error);

                    // Jump cursor to error position if available
                    if let Some(pos) = position {
                        let content = self.tabs[idx].editor.get_content();
                        let (line, col) = byte_offset_to_position(&content, pos);
                        self.tabs[idx].editor.set_cursor_position(line, col);
                    }

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
            AppEvent::SchemaSearchCompleted(results) => {
                self.tree_browser.apply_search_results(results);
                let count = self
                    .tree_browser
                    .schema()
                    .map(|s| {
                        s.schemas
                            .iter()
                            .map(|sc| {
                                sc.tables.len()
                                    + sc.views.len()
                                    + sc.functions.len()
                                    + sc.indexes.len()
                            })
                            .sum::<usize>()
                    })
                    .unwrap_or(0);
                self.set_status(
                    format!("Found {} matching objects", count),
                    StatusLevel::Info,
                );
                Ok(Action::None)
            }
            AppEvent::SchemaSearchFailed(err) => {
                self.tree_browser.set_searching(false);
                self.set_status(format!("Search failed: {}", err), StatusLevel::Error);
                Ok(Action::None)
            }
            AppEvent::LoadMoreCompleted {
                schema_name,
                category,
                items,
            } => {
                match items {
                    LoadMoreItems::Tables(tables) => {
                        let count = tables.len();
                        self.tree_browser.extend_tables(&schema_name, tables);
                        self.set_status(
                            format!("Loaded {} more tables", count),
                            StatusLevel::Success,
                        );
                    }
                    LoadMoreItems::Views(views) => {
                        let count = views.len();
                        self.tree_browser.extend_views(&schema_name, views);
                        self.set_status(
                            format!("Loaded {} more views", count),
                            StatusLevel::Success,
                        );
                    }
                    LoadMoreItems::Functions(functions) => {
                        let count = functions.len();
                        self.tree_browser.extend_functions(&schema_name, functions);
                        self.set_status(
                            format!("Loaded {} more functions", count),
                            StatusLevel::Success,
                        );
                    }
                    LoadMoreItems::Indexes(indexes) => {
                        let count = indexes.len();
                        self.tree_browser.extend_indexes(&schema_name, indexes);
                        self.set_status(
                            format!("Loaded {} more indexes", count),
                            StatusLevel::Success,
                        );
                    }
                }
                let _ = category; // Used implicitly via items variant
                Ok(Action::None)
            }
            AppEvent::LoadMoreFailed(err) => {
                self.set_status(format!("Load more failed: {}", err), StatusLevel::Error);
                Ok(Action::None)
            }
            AppEvent::ConnectionLost { tab_id, .. } => {
                // Reset only the affected tab's transaction state
                if let Some(idx) = self.tab_index_by_id(tab_id) {
                    self.tabs[idx].transaction_state = TransactionState::Idle;
                    self.tabs[idx].rows_streaming = None;
                }
                self.set_status(
                    "Connection lost — will reconnect on next query".to_string(),
                    StatusLevel::Warning,
                );
                Ok(Action::ReconnectTab { tab_id })
            }
        }
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Action {
        self.status_message = None;

        // Destructive-query confirmation intercepts all keys
        if let Some(pending) = self.pending_confirm_sql.take() {
            return self.handle_confirm_key(key, pending);
        }

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

        // Tree filter mode intercepts keys when active
        if self.focus == PanelFocus::TreeBrowser && self.tree_browser.is_filter_active() {
            return self.handle_tree_filter_key(key);
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

    /// Handle key events when tree filter mode is active
    fn handle_tree_filter_key(&mut self, key: KeyEvent) -> Action {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Esc => {
                self.tree_browser.deactivate_filter();
                Action::None
            }
            KeyCode::Enter => {
                // Trigger backend search if there's filter text
                let pattern = self.tree_browser.filter_text().to_string();
                if !pattern.is_empty() {
                    // Keep filter mode active, search will update results
                    self.tree_browser.set_searching(true);
                    Action::SearchSchema { pattern }
                } else {
                    self.tree_browser.deactivate_filter();
                    Action::None
                }
            }
            KeyCode::Backspace => {
                self.tree_browser.filter_backspace();
                Action::None
            }
            KeyCode::Delete => {
                self.tree_browser.filter_delete();
                Action::None
            }
            KeyCode::Left => {
                self.tree_browser.filter_cursor_left();
                Action::None
            }
            KeyCode::Right => {
                self.tree_browser.filter_cursor_right();
                Action::None
            }
            KeyCode::Up => {
                self.tree_browser.move_up();
                Action::None
            }
            KeyCode::Down => {
                self.tree_browser.move_down();
                Action::None
            }
            KeyCode::Char(c) => {
                self.tree_browser.filter_insert_char(c);
                Action::None
            }
            _ => Action::None,
        }
    }

    /// Handle a y/n keypress for destructive query confirmation
    fn handle_confirm_key(&mut self, key: KeyEvent, pending: PendingConfirm) -> Action {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.set_status("Executing...".to_string(), StatusLevel::Info);
                self.execute_confirmed_query(pending)
            }
            _ => {
                // Any other key cancels
                self.set_status("Query cancelled".to_string(), StatusLevel::Warning);
                Action::None
            }
        }
    }
}

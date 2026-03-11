//! Key action dispatch
//
//! Maps resolved KeyAction variants to concrete state mutations and
//! returns an Action for the main loop to execute.

use super::sql_utils::{is_destructive_query, is_write_query, translate_meta_command};
use super::*;

impl App {
    pub(super) fn execute_key_action(&mut self, action: KeyAction) -> Action {
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
                    return Action::None;
                }
                let had_transaction = self.tab().transaction_state != TransactionState::Idle;
                let tab_id = self.tab().id;
                if self.close_tab() {
                    if had_transaction {
                        self.set_status(
                            "Closed tab with uncommitted transaction (auto-rolled back)"
                                .to_string(),
                            StatusLevel::Warning,
                        );
                    }
                    Action::TabClosed { tab_id }
                } else {
                    self.set_status(
                        "Cannot close the last tab".to_string(),
                        StatusLevel::Warning,
                    );
                    Action::None
                }
            }
            KeyAction::NextTab => {
                self.next_tab();
                Action::None
            }

            // ── Navigation ───────────────────────────────────
            KeyAction::MoveUp => {
                match self.focus {
                    PanelFocus::ResultsViewer => {
                        let tab = self.tab_mut();
                        if let Some(ref mut ev) = tab.explain_viewer {
                            ev.move_up();
                        } else {
                            tab.results_viewer.move_up();
                        }
                    }
                    PanelFocus::TreeBrowser => self.tree_browser.move_up(),
                    PanelFocus::Inspector => self.inspector.scroll_up(),
                    PanelFocus::Help => self.help.scroll_up(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::MoveDown => {
                match self.focus {
                    PanelFocus::ResultsViewer => {
                        let tab = self.tab_mut();
                        if let Some(ref mut ev) = tab.explain_viewer {
                            ev.move_down();
                        } else {
                            tab.results_viewer.move_down();
                        }
                    }
                    PanelFocus::TreeBrowser => self.tree_browser.move_down(),
                    PanelFocus::Inspector => self.inspector.scroll_down(),
                    PanelFocus::Help => self.help.scroll_down(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::MoveLeft => {
                if self.focus == PanelFocus::ResultsViewer && self.tab().explain_viewer.is_none() {
                    self.tab_mut().results_viewer.move_left();
                }
                Action::None
            }
            KeyAction::MoveRight => {
                if self.focus == PanelFocus::ResultsViewer && self.tab().explain_viewer.is_none() {
                    self.tab_mut().results_viewer.move_right();
                }
                Action::None
            }
            KeyAction::PageUp => {
                match self.focus {
                    PanelFocus::ResultsViewer => {
                        let tab = self.tab_mut();
                        if let Some(ref mut ev) = tab.explain_viewer {
                            ev.page_up();
                        } else {
                            tab.results_viewer.page_up();
                        }
                    }
                    PanelFocus::Inspector => self.inspector.page_up(),
                    PanelFocus::Help => self.help.page_up(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::PageDown => {
                match self.focus {
                    PanelFocus::ResultsViewer => {
                        let tab = self.tab_mut();
                        if let Some(ref mut ev) = tab.explain_viewer {
                            ev.page_down();
                        } else {
                            tab.results_viewer.page_down();
                        }
                    }
                    PanelFocus::Inspector => self.inspector.page_down(),
                    PanelFocus::Help => self.help.page_down(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::GoToTop => {
                match self.focus {
                    PanelFocus::ResultsViewer => {
                        let tab = self.tab_mut();
                        if let Some(ref mut ev) = tab.explain_viewer {
                            ev.go_to_top();
                        } else {
                            tab.results_viewer.go_to_top();
                        }
                    }
                    PanelFocus::Inspector => self.inspector.scroll_to_top(),
                    PanelFocus::Help => self.help.scroll_to_top(),
                    _ => {}
                }
                Action::None
            }
            KeyAction::GoToBottom => {
                match self.focus {
                    PanelFocus::ResultsViewer => {
                        let tab = self.tab_mut();
                        if let Some(ref mut ev) = tab.explain_viewer {
                            ev.go_to_bottom();
                        } else {
                            tab.results_viewer.go_to_bottom();
                        }
                    }
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
                let raw_sql = self.tab().editor.get_content();
                // Translate psql meta-commands (e.g. \dt) to SQL
                let sql = translate_meta_command(&raw_sql).unwrap_or(raw_sql);
                if !sql.trim().is_empty() {
                    // Block writes in read-only mode
                    if self.read_only
                        && let Some(label) = is_write_query(&sql)
                    {
                        self.set_status(
                            format!("Read-only mode: {} queries are blocked", label),
                            StatusLevel::Error,
                        );
                        return Action::None;
                    }
                    // Check for destructive query
                    if self.confirm_destructive
                        && let Some(label) = is_destructive_query(&sql)
                    {
                        self.pending_confirm_sql = Some(PendingConfirm {
                            sql,
                            tab_id: self.tab().id,
                            timeout_ms: self.query_timeout_ms,
                            max_rows: self.max_result_rows,
                        });
                        self.set_status(
                            format!("This query contains {}. Execute? (y/N)", label),
                            StatusLevel::Warning,
                        );
                        return Action::None;
                    }
                    self.set_status("Executing query...".to_string(), StatusLevel::Info);
                    self.prepare_execute_query(sql)
                } else {
                    Action::None
                }
            }
            KeyAction::ExplainQuery => {
                let sql = self.tab().editor.get_content();
                if !sql.trim().is_empty() {
                    let explain = if self.explain_visual {
                        self.tab_mut().explain_pending = true;
                        format!("EXPLAIN (ANALYZE, FORMAT JSON) {}", sql.trim())
                    } else {
                        format!("EXPLAIN ANALYZE {}", sql.trim())
                    };
                    self.set_status("Running EXPLAIN ANALYZE...".to_string(), StatusLevel::Info);
                    self.prepare_execute_query(explain)
                } else {
                    Action::None
                }
            }
            KeyAction::CancelQuery => {
                // Prefer cancelling the active tab; fall back to any running tab
                let active = &self.tabs[self.active_tab];
                let target = if active.query_running {
                    Some(active.id)
                } else {
                    self.tabs.iter().find(|t| t.query_running).map(|t| t.id)
                };
                if let Some(tab_id) = target {
                    self.set_status("Cancelling query...".to_string(), StatusLevel::Warning);
                    Action::CancelQuery {
                        tab_id,
                        terminate: false,
                    }
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
            KeyAction::ToggleViewMode => {
                let tab = self.tab_mut();
                if let Some(ref mut ev) = tab.explain_viewer {
                    ev.toggle_view_mode();
                } else {
                    tab.results_viewer.toggle_view_mode();
                }
                Action::None
            }
            KeyAction::WidenColumn => {
                self.tab_mut().results_viewer.widen_column();
                Action::None
            }
            KeyAction::NarrowColumn => {
                self.tab_mut().results_viewer.narrow_column();
                Action::None
            }
            KeyAction::ResetColumnWidths => {
                self.tab_mut().results_viewer.reset_column_widths();
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
                if self.focus == PanelFocus::TreeBrowser {
                    // Check if LoadMore is selected - trigger loading more items
                    if self.tree_browser.is_load_more_selected()
                        && let Some((schema_name, category)) = self.tree_browser.load_more_info()
                    {
                        let offset = self.tree_browser.loaded_count(&schema_name, &category);
                        let limit = self.tree_browser.category_limit();
                        self.set_status(
                            format!("Loading more {}...", category.to_lowercase()),
                            StatusLevel::Info,
                        );
                        return Action::LoadMoreCategory {
                            schema_name,
                            category,
                            offset,
                            limit,
                        };
                    }
                    // Check if saved query is selected - load into editor
                    if let Some(sq) = self.tree_browser.selected_saved_query() {
                        let sql = sq.sql.clone();
                        let name = sq.name.clone();
                        self.tab_mut().editor.set_content(sql);
                        self.focus = PanelFocus::QueryEditor;
                        self.set_status(format!("Loaded saved query: {}", name), StatusLevel::Info);
                        return Action::None;
                    }
                    // Check if table/view is selected - run paginated preview
                    if let Some(base_sql) = self.tree_browser.preview_base_query() {
                        let page_size = self.tree_browser.preview_rows();
                        let pagination = PaginationState {
                            original_sql: base_sql.clone(),
                            current_page: 0,
                            page_size,
                            has_more: false,
                            user_has_limit: false,
                            previous_page: None,
                        };
                        let paged_sql = pagination.paged_sql();
                        let display_sql = format!("{} LIMIT {}", base_sql, page_size);
                        let tab_id = self.tab().id;
                        let timeout_ms = self.query_timeout_ms;
                        self.tab_mut().editor.set_content(display_sql);
                        self.tab_mut().pagination = Some(pagination);
                        self.tab_mut().query_running = true;
                        self.tab_mut().query_start = Some(std::time::Instant::now());
                        self.set_status("Executing query...".to_string(), StatusLevel::Info);
                        return Action::ExecuteQuery {
                            sql: paged_sql,
                            tab_id,
                            timeout_ms,
                            max_rows: 0,
                        };
                    }
                }
                self.tree_browser.expand_current();
                Action::None
            }
            KeyAction::Collapse => {
                self.tree_browser.collapse_current();
                Action::None
            }

            KeyAction::FilterTree => {
                if self.focus == PanelFocus::TreeBrowser {
                    self.tree_browser.activate_filter();
                }
                Action::None
            }

            KeyAction::CopyName => {
                if self.focus == PanelFocus::TreeBrowser
                    && let Some(name) = self.tree_browser.selected_qualified_name()
                {
                    self.copy_to_clipboard(&name);
                    self.set_status(format!("Copied: {}", name), StatusLevel::Info);
                }
                Action::None
            }

            KeyAction::ShowDefinition => {
                if self.focus == PanelFocus::TreeBrowser {
                    if let Some((schema, table)) = self.tree_browser.selected_table_info() {
                        // Query to get table DDL
                        let sql = format!(
                            "SELECT \
                                'CREATE TABLE ' || quote_ident(n.nspname) || '.' || quote_ident(c.relname) || ' (' || \
                                string_agg( \
                                    quote_ident(a.attname) || ' ' || pg_catalog.format_type(a.atttypid, a.atttypmod) || \
                                    CASE WHEN a.attnotnull THEN ' NOT NULL' ELSE '' END || \
                                    CASE WHEN ad.adbin IS NOT NULL THEN ' DEFAULT ' || pg_get_expr(ad.adbin, ad.adrelid) ELSE '' END, \
                                    ', ' ORDER BY a.attnum \
                                ) || ')' AS ddl \
                            FROM pg_class c \
                            JOIN pg_namespace n ON n.oid = c.relnamespace \
                            JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped \
                            LEFT JOIN pg_attrdef ad ON ad.adrelid = a.attrelid AND ad.adnum = a.attnum \
                            WHERE n.nspname = '{}' AND c.relname = '{}' \
                            GROUP BY n.nspname, c.relname",
                            schema.replace('\'', "''"),
                            table.replace('\'', "''")
                        );
                        let tab_id = self.tab().id;
                        let timeout_ms = self.query_timeout_ms;
                        let max_rows = self.max_result_rows;
                        self.tab_mut().query_running = true;
                        self.tab_mut().query_start = Some(std::time::Instant::now());
                        self.set_status(
                            format!("Loading definition for {}.{}...", schema, table),
                            StatusLevel::Info,
                        );
                        return Action::ExecuteQuery {
                            sql,
                            tab_id,
                            timeout_ms,
                            max_rows,
                        };
                    } else {
                        self.set_status(
                            "Select a table or view to show definition".to_string(),
                            StatusLevel::Warning,
                        );
                    }
                }
                Action::None
            }

            KeyAction::DeleteSavedQuery => {
                if self.focus == PanelFocus::TreeBrowser {
                    if let Some(name) = self.tree_browser.selected_saved_query_name() {
                        let name = name.to_string();
                        if let Some(conn) = &self.connection_name {
                            let conn = conn.clone();
                            if let Err(e) = crate::config::saved_queries::delete_query(&conn, &name)
                            {
                                self.set_status(
                                    format!("Failed to delete query: {}", e),
                                    StatusLevel::Error,
                                );
                            } else {
                                self.tree_browser.remove_saved_query(&name);
                                self.set_status(
                                    format!("Deleted saved query: {}", name),
                                    StatusLevel::Success,
                                );
                            }
                        }
                    } else {
                        self.set_status(
                            "Select a saved query to delete".to_string(),
                            StatusLevel::Warning,
                        );
                    }
                }
                Action::None
            }

            // ── Pagination ────────────────────────────────────
            KeyAction::NextPage => {
                if self.tab().query_running {
                    return Action::None;
                }
                if let Some(ref pg) = self.tab().pagination {
                    if pg.has_more && !pg.user_has_limit {
                        let mut next = pg.clone();
                        next.previous_page = Some(pg.current_page);
                        next.current_page += 1;
                        next.has_more = false; // will be set on results
                        let sql = next.paged_sql();
                        let tab_id = self.tab().id;
                        let timeout_ms = self.query_timeout_ms;
                        self.tab_mut().pagination = Some(next);
                        self.tab_mut().query_running = true;
                        self.tab_mut().query_start = Some(std::time::Instant::now());
                        self.set_status("Loading next page...".to_string(), StatusLevel::Info);
                        return Action::ExecuteQuery {
                            sql,
                            tab_id,
                            timeout_ms,
                            max_rows: 0,
                        };
                    } else if !pg.has_more {
                        self.set_status("No more rows".to_string(), StatusLevel::Info);
                    }
                }
                Action::None
            }
            KeyAction::PrevPage => {
                if self.tab().query_running {
                    return Action::None;
                }
                if let Some(ref pg) = self.tab().pagination {
                    if pg.current_page > 0 && !pg.user_has_limit {
                        let mut prev = pg.clone();
                        prev.previous_page = Some(pg.current_page);
                        prev.current_page -= 1;
                        prev.has_more = true; // previous page always has a next
                        let sql = prev.paged_sql();
                        let tab_id = self.tab().id;
                        let timeout_ms = self.query_timeout_ms;
                        self.tab_mut().pagination = Some(prev);
                        self.tab_mut().query_running = true;
                        self.tab_mut().query_start = Some(std::time::Instant::now());
                        self.set_status("Loading previous page...".to_string(), StatusLevel::Info);
                        return Action::ExecuteQuery {
                            sql,
                            tab_id,
                            timeout_ms,
                            max_rows: 0,
                        };
                    } else if pg.current_page == 0 {
                        self.set_status("Already on first page".to_string(), StatusLevel::Info);
                    }
                }
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
                        self.pending_save_query = false;
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
                    let save_query = std::mem::take(&mut self.pending_save_query);
                    self.command_bar.deactivate();
                    self.focus = self.previous_focus;

                    if input.is_empty() {
                        return Action::None;
                    }

                    if is_prompt {
                        if let Some(fmt) = format {
                            self.execute_export(fmt, &input);
                        } else if save_query {
                            self.finish_save_query(&input);
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
}

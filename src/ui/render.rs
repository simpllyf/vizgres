//! Top-level render function
//!
//! Orchestrates rendering of all panels using the layout module.

use crate::app::{App, PanelFocus, StatusLevel, TransactionState};
use crate::keymap::KeyAction;
use crate::ui::Component;
use crate::ui::layout::calculate_layout;
use crate::ui::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

/// Render the entire application
pub fn render(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let layout = calculate_layout(frame.area(), true);

    // Tree browser
    render_panel(
        frame,
        theme,
        layout.tree,
        " Schema ",
        app.focus == PanelFocus::TreeBrowser,
        |f, inner| {
            app.tree_browser
                .render(f, inner, app.focus == PanelFocus::TreeBrowser, theme);
        },
    );

    // Tab bar (always visible)
    render_tab_bar(frame, layout.tab_bar, app, theme);

    // Editor (active tab)
    render_panel(
        frame,
        theme,
        layout.editor,
        " Query ",
        app.focus == PanelFocus::QueryEditor,
        |f, inner| {
            app.tab()
                .editor
                .render(f, inner, app.focus == PanelFocus::QueryEditor, theme);
        },
    );

    // Results or EXPLAIN viewer (active tab)
    let results_title = if app.tab().explain_viewer.is_some() {
        " Explain "
    } else {
        " Results "
    };
    render_panel(
        frame,
        theme,
        layout.results,
        results_title,
        app.focus == PanelFocus::ResultsViewer,
        |f, inner| {
            if let Some(ref ev) = app.tab().explain_viewer {
                ev.render(f, inner, app.focus == PanelFocus::ResultsViewer, theme);
            } else {
                app.tab().results_viewer.render(
                    f,
                    inner,
                    app.focus == PanelFocus::ResultsViewer,
                    theme,
                );
            }
        },
    );

    // Inspector overlay (floating popup on top of everything)
    if app.inspector.is_visible() {
        render_inspector_popup(frame, theme, app);
    }

    // Help overlay (on top of everything including inspector)
    if app.help.is_visible() {
        render_help_popup(frame, theme, app);
    }

    // Connection dialog (on top of everything)
    if app.connection_dialog.is_visible() {
        render_connection_dialog_popup(frame, theme, app);
    }

    // Status bar
    render_status_bar(frame, layout.command_bar, app, theme);
}

/// Render the tab bar showing all open tabs with the active tab highlighted
fn render_tab_bar(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let mut spans = Vec::new();
    for (i, tab) in app.tabs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" \u{2502} ", theme.tab_separator));
        }

        let mut label = format!(" Tab {}", i + 1);
        if tab.query_running {
            label.push('*');
        }
        match tab.transaction_state {
            TransactionState::InTransaction => label.push_str(" [TXN]"),
            TransactionState::Failed => label.push_str(" [TXN!]"),
            TransactionState::Idle => {}
        }
        label.push(' ');

        let style = if i == app.active_tab {
            theme.tab_active
        } else {
            theme.tab_inactive
        };
        spans.push(Span::styled(label, style));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Render a panel with consistent focus indication
fn render_panel(
    frame: &mut Frame,
    theme: &Theme,
    area: Rect,
    title: &str,
    focused: bool,
    render_inner: impl FnOnce(&mut Frame, Rect),
) {
    let title = if focused {
        format!(" \u{25b8} {}", title.trim())
    } else {
        title.to_string()
    };

    let title_style = if focused {
        theme.panel_title_focused
    } else {
        theme.panel_title_unfocused
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(format!(" {} ", title.trim()), title_style))
        .border_style(theme.border_style(focused));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    render_inner(frame, inner);
}

/// Render the inspector as a centered floating popup with shadow.
/// Popup sizes itself to fit the content, clamped to min/max bounds.
fn render_inspector_popup(frame: &mut Frame, theme: &Theme, app: &App) {
    let screen = frame.area();
    let (content_w, content_h) = app.inspector.content_size();

    // +2 for borders on each side, +1 for header line inside the popup
    let desired_w = content_w + 4;
    let desired_h = content_h + 4;

    let min_w: u16 = 42;
    let min_h: u16 = 6;
    let max_w = screen.width * 4 / 5;
    let max_h = screen.height * 3 / 4;

    let popup_w = desired_w
        .clamp(min_w, max_w)
        .min(screen.width.saturating_sub(2));
    let popup_h = desired_h
        .clamp(min_h, max_h)
        .min(screen.height.saturating_sub(2));
    let popup_x = (screen.width.saturating_sub(popup_w)) / 2;
    let popup_y = (screen.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_w, popup_h);

    // Shadow (1 cell right and down) — only where it fits
    let shadow_area = Rect::new(
        (popup_x + 1).min(screen.width.saturating_sub(1)),
        (popup_y + 1).min(screen.height.saturating_sub(1)),
        popup_w.min(screen.width.saturating_sub(popup_x + 1)),
        popup_h.min(screen.height.saturating_sub(popup_y + 1)),
    );
    let shadow_style = theme.shadow;
    for y in shadow_area.y..shadow_area.y + shadow_area.height {
        for x in shadow_area.x..shadow_area.x + shadow_area.width {
            if x < screen.width && y < screen.height {
                frame.render_widget(
                    Paragraph::new(" ").style(shadow_style),
                    Rect::new(x, y, 1, 1),
                );
            }
        }
    }

    // Clear the popup area
    frame.render_widget(Clear, popup_area);

    let dismiss_key = key_hint(&app.keymap, Some(PanelFocus::Inspector), KeyAction::Dismiss);
    let copy_key = key_hint(
        &app.keymap,
        Some(PanelFocus::Inspector),
        KeyAction::CopyContent,
    );
    let title = format!(
        " Inspector \u{2014} {} to close, {} to copy ",
        dismiss_key, copy_key
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title, theme.popup_title))
        .border_style(theme.popup_border);

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    app.inspector
        .render(frame, inner, app.focus == PanelFocus::Inspector, theme);
}

/// Render the help overlay as a centered floating popup with shadow.
fn render_help_popup(frame: &mut Frame, theme: &Theme, app: &App) {
    let screen = frame.area();

    let popup_w: u16 = 60.min(screen.width.saturating_sub(2));
    let popup_h: u16 = 28.min(screen.height.saturating_sub(2));
    let popup_x = (screen.width.saturating_sub(popup_w)) / 2;
    let popup_y = (screen.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_w, popup_h);

    // Shadow (1 cell right and down)
    let shadow_area = Rect::new(
        (popup_x + 1).min(screen.width.saturating_sub(1)),
        (popup_y + 1).min(screen.height.saturating_sub(1)),
        popup_w.min(screen.width.saturating_sub(popup_x + 1)),
        popup_h.min(screen.height.saturating_sub(popup_y + 1)),
    );
    let shadow_style = theme.shadow;
    for y in shadow_area.y..shadow_area.y + shadow_area.height {
        for x in shadow_area.x..shadow_area.x + shadow_area.width {
            if x < screen.width && y < screen.height {
                frame.render_widget(
                    Paragraph::new(" ").style(shadow_style),
                    Rect::new(x, y, 1, 1),
                );
            }
        }
    }

    // Clear and draw border
    frame.render_widget(Clear, popup_area);

    let dismiss_key = key_hint(&app.keymap, Some(PanelFocus::Help), KeyAction::Dismiss);
    let title = format!(" Help \u{2014} {} to close ", dismiss_key);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title, theme.popup_title))
        .border_style(theme.popup_border);

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    app.help.render(frame, inner, theme, &app.keymap);
}

/// Render the connection dialog as a centered floating popup with shadow.
fn render_connection_dialog_popup(frame: &mut Frame, theme: &Theme, app: &App) {
    let screen = frame.area();

    let popup_w: u16 = 60.min(screen.width.saturating_sub(2));
    let popup_h: u16 = 19.min(screen.height.saturating_sub(2));
    let popup_x = (screen.width.saturating_sub(popup_w)) / 2;
    let popup_y = (screen.height.saturating_sub(popup_h)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_w, popup_h);

    // Shadow (1 cell right and down)
    let shadow_area = Rect::new(
        (popup_x + 1).min(screen.width.saturating_sub(1)),
        (popup_y + 1).min(screen.height.saturating_sub(1)),
        popup_w.min(screen.width.saturating_sub(popup_x + 1)),
        popup_h.min(screen.height.saturating_sub(popup_y + 1)),
    );
    let shadow_style = theme.shadow;
    for y in shadow_area.y..shadow_area.y + shadow_area.height {
        for x in shadow_area.x..shadow_area.x + shadow_area.width {
            if x < screen.width && y < screen.height {
                frame.render_widget(
                    Paragraph::new(" ").style(shadow_style),
                    Rect::new(x, y, 1, 1),
                );
            }
        }
    }

    // Clear and draw border
    frame.render_widget(Clear, popup_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Connect \u{2014} Enter to connect, Esc to cancel ",
            theme.popup_title,
        ))
        .border_style(theme.popup_border);

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    app.connection_dialog.render(frame, inner, theme);
}

/// Render the status bar with partitioned layout:
/// Left: toast notification (ephemeral, dismissed on next keypress)
/// Right: connection info (ambient context, always visible)
fn render_status_bar(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    if app.command_bar.is_active() {
        app.command_bar.render(frame, area, true, theme);
        return;
    }

    // Right side: RO badge + TXN indicator + connection info (always visible)
    // Shows the active tab's transaction state
    let txn_badge = match app.tab().transaction_state {
        TransactionState::Idle => None,
        TransactionState::InTransaction => Some((" TXN ", theme.status_txn_active)),
        TransactionState::Failed => Some((" TXN FAILED ", theme.status_txn_failed)),
    };

    let ro_badge: Option<&str> = if app.read_only { Some(" RO ") } else { None };

    let (conn_dot, conn_dot_style) = if app.connection_name.is_some() {
        ("\u{25cf} ", Style::default().fg(Color::Green))
    } else {
        ("\u{25cf} ", Style::default().fg(Color::Red))
    };
    let conn_label = if let Some(ref name) = app.connection_name {
        format!("[{}]", name)
    } else {
        "[disconnected]".to_string()
    };

    // Calculate total right-side width (badges + dot + label)
    let dot_len = 2u16; // "● " = 2 cols
    let txn_len = txn_badge
        .as_ref()
        .map_or(0, |(s, _)| super::unicode::display_width(s) as u16);
    let ro_len = ro_badge.map_or(0, |s| super::unicode::display_width(s) as u16);
    let badge_spacer = |len: u16| if len > 0 { 1u16 } else { 0 };
    let right_total = ro_len
        + badge_spacer(ro_len)
        + txn_len
        + badge_spacer(txn_len)
        + dot_len
        + super::unicode::display_width(&conn_label) as u16;
    let right_x = area.x + area.width.saturating_sub(right_total);

    // Render RO badge, TXN badge, then dot + connection info
    let mut cursor_x = right_x;
    if let Some(ro_text) = ro_badge {
        frame.render_widget(
            Paragraph::new(ro_text).style(theme.status_read_only),
            Rect::new(cursor_x, area.y, ro_len.min(area.width), 1),
        );
        cursor_x += ro_len + badge_spacer(ro_len);
    }
    if let Some((badge_text, badge_style)) = txn_badge {
        frame.render_widget(
            Paragraph::new(badge_text).style(badge_style),
            Rect::new(cursor_x, area.y, txn_len.min(area.width), 1),
        );
        cursor_x += txn_len + badge_spacer(txn_len);
    }
    frame.render_widget(
        Paragraph::new(conn_dot).style(conn_dot_style),
        Rect::new(cursor_x, area.y, dot_len.min(area.width), 1),
    );
    cursor_x += dot_len;
    frame.render_widget(
        Paragraph::new(conn_label).style(theme.status_conn_info),
        Rect::new(
            cursor_x,
            area.y,
            (area.width.saturating_sub(cursor_x - area.x)).min(area.width),
            1,
        ),
    );

    // Left side: toast message or default help hint
    let max_left_width = area.width.saturating_sub(right_total + 2);
    if max_left_width < 4 {
        return;
    }

    // Show live elapsed time and row counter when query is running
    let active_tab = &app.tabs[app.active_tab];
    if active_tab.query_running
        && let Some(start) = active_tab.query_start
    {
        let elapsed = start.elapsed();
        let cancel_key = key_hint(&app.keymap, None, KeyAction::CancelQuery);
        let msg = if let Some(rows) = active_tab.rows_streaming {
            format!(
                "Streaming... {:>} rows ({:.1}s) - {} to cancel",
                format_row_count(rows),
                elapsed.as_secs_f64(),
                cancel_key
            )
        } else {
            format!(
                "Executing... ({:.1}s) - {} to cancel",
                elapsed.as_secs_f64(),
                cancel_key
            )
        };
        frame.render_widget(
            Paragraph::new(msg).style(theme.status_info),
            Rect::new(area.x, area.y, max_left_width, 1),
        );
        return;
    }

    if let Some(ref status) = app.status_message {
        let style = match status.level {
            StatusLevel::Info => theme.status_info,
            StatusLevel::Success => theme.status_success,
            StatusLevel::Warning => theme.status_warning,
            StatusLevel::Error => theme.status_error,
        };

        let msg = &status.message;
        let max_cols = max_left_width as usize;
        let display = if super::unicode::display_width(msg) > max_cols {
            super::unicode::truncate_to_width(msg, max_cols)
        } else {
            msg.clone()
        };

        frame.render_widget(
            Paragraph::new(display).style(style),
            Rect::new(area.x, area.y, max_left_width, 1),
        );
    } else {
        let help_key = key_hint(&app.keymap, None, KeyAction::ShowHelp);
        let cmd_key = key_hint(&app.keymap, None, KeyAction::OpenCommandBar);
        let run_key = key_hint(
            &app.keymap,
            Some(PanelFocus::QueryEditor),
            KeyAction::ExecuteQuery,
        );
        let quit_key = key_hint(&app.keymap, None, KeyAction::Quit);
        let hint = format!(
            "{}=help | {}=commands | {}=run | {}=quit",
            help_key, cmd_key, run_key, quit_key
        );
        frame.render_widget(
            Paragraph::new(hint).style(theme.status_help_hint),
            Rect::new(area.x, area.y, max_left_width, 1),
        );
    }
}

/// Get the first key bound to an action, formatted for display hints
fn key_hint(
    keymap: &crate::keymap::KeyMap,
    focus: Option<PanelFocus>,
    action: KeyAction,
) -> String {
    let keys = keymap.keys_for_action(focus, action);
    keys.into_iter()
        .next()
        .unwrap_or_else(|| "(unset)".to_string())
}

/// Format a row count with thousands separators (e.g., 4523 → "4,523")
fn format_row_count(n: usize) -> String {
    if n < 1_000 {
        return n.to_string();
    }
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_row_count_small() {
        assert_eq!(format_row_count(0), "0");
        assert_eq!(format_row_count(1), "1");
        assert_eq!(format_row_count(999), "999");
    }

    #[test]
    fn test_format_row_count_thousands() {
        assert_eq!(format_row_count(1_000), "1,000");
        assert_eq!(format_row_count(4_523), "4,523");
        assert_eq!(format_row_count(999_999), "999,999");
    }

    #[test]
    fn test_format_row_count_millions() {
        assert_eq!(format_row_count(1_000_000), "1,000,000");
        assert_eq!(format_row_count(12_345_678), "12,345,678");
    }
}

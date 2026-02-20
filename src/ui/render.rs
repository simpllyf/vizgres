//! Top-level render function
//!
//! Orchestrates rendering of all panels using the layout module.

use crate::app::{App, PanelFocus, StatusLevel};
use crate::ui::Component;
use crate::ui::layout::calculate_layout;
use crate::ui::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

/// Render the entire application
pub fn render(frame: &mut Frame, app: &App) {
    let theme = &app.theme;
    let show_tab_bar = app.tab_count() > 1;
    let layout = calculate_layout(frame.area(), show_tab_bar);

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

    // Tab bar (only when >1 tab)
    if show_tab_bar {
        render_tab_bar(frame, layout.tab_bar, app, theme);
    }

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

    // Results (active tab)
    render_panel(
        frame,
        theme,
        layout.results,
        " Results ",
        app.focus == PanelFocus::ResultsViewer,
        |f, inner| {
            app.tab().results_viewer.render(
                f,
                inner,
                app.focus == PanelFocus::ResultsViewer,
                theme,
            );
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

        let label = if tab.query_running {
            format!(" Tab {}* ", i + 1)
        } else {
            format!(" Tab {} ", i + 1)
        };

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
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Inspector \u{2014} Esc to close, y to copy ",
            theme.popup_title,
        ))
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
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Help \u{2014} Esc to close ",
            theme.popup_title,
        ))
        .border_style(theme.popup_border);

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);
    app.help.render(frame, inner, theme);
}

/// Render the connection dialog as a centered floating popup with shadow.
fn render_connection_dialog_popup(frame: &mut Frame, theme: &Theme, app: &App) {
    let screen = frame.area();

    let popup_w: u16 = 60.min(screen.width.saturating_sub(2));
    let popup_h: u16 = 18.min(screen.height.saturating_sub(2));
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

    // Right side: connection info (always visible)
    let conn_info = if let Some(ref name) = app.connection_name {
        format!("[{}]", name)
    } else {
        "[disconnected]".to_string()
    };
    let right_len = conn_info.len() as u16;
    let right_x = area.x + area.width.saturating_sub(right_len);

    frame.render_widget(
        Paragraph::new(conn_info).style(theme.status_conn_info),
        Rect::new(right_x, area.y, right_len.min(area.width), 1),
    );

    // Left side: toast message or default help hint
    let max_left_width = area.width.saturating_sub(right_len + 2);
    if max_left_width < 4 {
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
        let display = if msg.len() > max_left_width as usize {
            format!("{}...", &msg[..max_left_width as usize - 3])
        } else {
            msg.clone()
        };

        frame.render_widget(
            Paragraph::new(display).style(style),
            Rect::new(area.x, area.y, max_left_width, 1),
        );
    } else {
        frame.render_widget(
            Paragraph::new("F1=help | Ctrl+P=commands | F5=run | Ctrl+Q=quit")
                .style(theme.status_help_hint),
            Rect::new(area.x, area.y, max_left_width, 1),
        );
    }
}

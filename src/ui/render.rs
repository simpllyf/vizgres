//! Top-level render function
//!
//! Orchestrates rendering of all panels using the layout module.

use crate::app::{App, PanelFocus, StatusLevel};
use crate::ui::Component;
use crate::ui::layout::{calculate_layout, split_results_for_inspector};
use crate::ui::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Render the entire application
pub fn render(frame: &mut Frame, app: &App) {
    let theme = Theme::new();
    let layout = calculate_layout(frame.area());

    // Tree browser
    let tree_block = Block::default()
        .borders(Borders::ALL)
        .title(" Schema ")
        .border_style(theme.border_style(app.focus == PanelFocus::TreeBrowser));
    let tree_inner = tree_block.inner(layout.tree);
    frame.render_widget(tree_block, layout.tree);
    app.tree_browser
        .render(frame, tree_inner, app.focus == PanelFocus::TreeBrowser);

    // Editor
    let editor_block = Block::default()
        .borders(Borders::ALL)
        .title(" Query ")
        .border_style(theme.border_style(app.focus == PanelFocus::QueryEditor));
    let editor_inner = editor_block.inner(layout.editor);
    frame.render_widget(editor_block, layout.editor);
    app.editor
        .render(frame, editor_inner, app.focus == PanelFocus::QueryEditor);

    // Results (possibly split with inspector)
    if app.inspector.is_visible() {
        let split = split_results_for_inspector(layout.results);

        // Results panel
        let results_block = Block::default()
            .borders(Borders::ALL)
            .title(" Results ")
            .border_style(theme.border_style(app.focus == PanelFocus::ResultsViewer));
        let results_inner = results_block.inner(split.results);
        frame.render_widget(results_block, split.results);
        app.results_viewer
            .render(frame, results_inner, app.focus == PanelFocus::ResultsViewer);

        // Inspector panel
        let inspector_block = Block::default()
            .borders(Borders::ALL)
            .title(" Inspector ")
            .border_style(theme.border_style(app.focus == PanelFocus::Inspector));
        let inspector_inner = inspector_block.inner(split.inspector);
        frame.render_widget(inspector_block, split.inspector);
        app.inspector
            .render(frame, inspector_inner, app.focus == PanelFocus::Inspector);
    } else {
        let results_block = Block::default()
            .borders(Borders::ALL)
            .title(" Results ")
            .border_style(theme.border_style(app.focus == PanelFocus::ResultsViewer));
        let results_inner = results_block.inner(layout.results);
        frame.render_widget(results_block, layout.results);
        app.results_viewer
            .render(frame, results_inner, app.focus == PanelFocus::ResultsViewer);
    }

    // Command bar / status bar
    render_command_bar(frame, layout.command_bar, app, &theme);
}

fn render_command_bar(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    if app.command_bar.is_active() {
        app.command_bar.render(frame, area, true);
    } else if let Some(ref status) = app.status_message {
        let style = match status.level {
            StatusLevel::Info => theme.status_info,
            StatusLevel::Success => theme.status_success,
            StatusLevel::Warning => theme.status_warning,
            StatusLevel::Error => theme.status_error,
        };

        let conn_info = if let Some(ref name) = app.connection_name {
            format!("[{}] ", name)
        } else {
            "[disconnected] ".to_string()
        };

        let text = format!("{}{}", conn_info, status.message);
        let paragraph = Paragraph::new(text).style(style);
        frame.render_widget(paragraph, area);
    } else {
        let conn_info = if let Some(ref name) = app.connection_name {
            format!("[{}]", name)
        } else {
            "[disconnected]".to_string()
        };
        let text = format!("{} | Press : for commands, Ctrl+Q to quit", conn_info);
        let paragraph = Paragraph::new(text).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(paragraph, area);
    }
}

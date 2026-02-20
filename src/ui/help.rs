//! Help overlay — keybinding reference modal
//!
//! Displays all keybindings organized by panel context as a centered popup.
//! Follows the same overlay pattern as Inspector.
//! Reads actual bindings from KeyMap for dynamic display.

use std::cell::Cell;

use crate::app::PanelFocus;
use crate::keymap::{KeyAction, KeyMap};
use crate::ui::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Help overlay showing keybinding reference
pub struct HelpOverlay {
    visible: bool,
    scroll_offset: usize,
    /// Cached line count from last build_lines call (Cell for interior mutability in render)
    line_count: Cell<usize>,
}

impl HelpOverlay {
    pub fn new() -> Self {
        Self {
            visible: false,
            scroll_offset: 0,
            line_count: Cell::new(52), // reasonable default, updated on render
        }
    }

    pub fn show(&mut self) {
        self.visible = true;
        self.scroll_offset = 0;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.scroll_offset = 0;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        if self.scroll_offset + 1 < self.line_count.get() {
            self.scroll_offset += 1;
        }
    }

    pub fn page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(20);
    }

    pub fn page_down(&mut self) {
        self.scroll_offset = (self.scroll_offset + 20).min(self.line_count.get().saturating_sub(1));
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.line_count.get().saturating_sub(1);
    }

    /// Build styled help content lines using actual keybindings from the keymap
    pub fn build_lines<'a>(&self, theme: &Theme, km: &KeyMap) -> Vec<Line<'a>> {
        let section = theme.help_section;
        let key = theme.help_key;
        let desc = theme.help_desc;
        let blank = Line::from("");

        // Helper: format keys for an action, or fallback to a static string
        let fmt = |focus: Option<PanelFocus>, action: KeyAction| -> String {
            let keys = km.keys_for_action(focus, action);
            if keys.is_empty() {
                "(unbound)".to_string()
            } else {
                keys.join(" / ")
            }
        };

        vec![
            Line::from(Span::styled("Global", section)),
            help_line(
                &format!("  {}", fmt(None, KeyAction::Quit)),
                "Quit",
                key,
                desc,
            ),
            help_line(
                &format!(
                    "  {} / {}",
                    fmt(None, KeyAction::CycleFocus),
                    fmt(None, KeyAction::CycleFocusReverse)
                ),
                "Cycle panel focus",
                key,
                desc,
            ),
            help_line(
                &format!("  {}", fmt(None, KeyAction::OpenCommandBar)),
                "Command palette",
                key,
                desc,
            ),
            help_line(
                &format!("  {}", fmt(None, KeyAction::ShowHelp)),
                "Help",
                key,
                desc,
            ),
            help_line(
                &format!("  {}", fmt(None, KeyAction::NewTab)),
                "New tab",
                key,
                desc,
            ),
            help_line(
                &format!("  {}", fmt(None, KeyAction::CloseTab)),
                "Close tab",
                key,
                desc,
            ),
            help_line(
                &format!("  {}", fmt(None, KeyAction::NextTab)),
                "Next tab",
                key,
                desc,
            ),
            blank.clone(),
            Line::from(Span::styled("Editor", section)),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::QueryEditor), KeyAction::ExecuteQuery)
                ),
                "Execute query",
                key,
                desc,
            ),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::QueryEditor), KeyAction::ExplainQuery)
                ),
                "EXPLAIN ANALYZE",
                key,
                desc,
            ),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::QueryEditor), KeyAction::ClearEditor)
                ),
                "Clear editor",
                key,
                desc,
            ),
            help_line(
                &format!("  {}", fmt(Some(PanelFocus::QueryEditor), KeyAction::Undo)),
                "Undo",
                key,
                desc,
            ),
            help_line(
                &format!("  {}", fmt(Some(PanelFocus::QueryEditor), KeyAction::Redo)),
                "Redo",
                key,
                desc,
            ),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::QueryEditor), KeyAction::FormatQuery)
                ),
                "Format query",
                key,
                desc,
            ),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::QueryEditor), KeyAction::HistoryBack)
                ),
                "Query history back",
                key,
                desc,
            ),
            help_line("  Right", "Accept completion", key, desc),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::QueryEditor), KeyAction::NextCompletion)
                ),
                "Cycle completions",
                key,
                desc,
            ),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::QueryEditor), KeyAction::CancelQuery)
                ),
                "Cancel running query",
                key,
                desc,
            ),
            blank.clone(),
            Line::from(Span::styled("Results", section)),
            help_line("  j/k  \u{2191}/\u{2193}", "Navigate rows", key, desc),
            help_line("  h/l  \u{2190}/\u{2192}", "Navigate columns", key, desc),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::ResultsViewer), KeyAction::OpenInspector)
                ),
                "Inspect cell",
                key,
                desc,
            ),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::ResultsViewer), KeyAction::CopyCell)
                ),
                "Copy cell",
                key,
                desc,
            ),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::ResultsViewer), KeyAction::CopyRow)
                ),
                "Copy row",
                key,
                desc,
            ),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::ResultsViewer), KeyAction::ExportCsv)
                ),
                "Export CSV",
                key,
                desc,
            ),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::ResultsViewer), KeyAction::ExportJson)
                ),
                "Export JSON",
                key,
                desc,
            ),
            help_line("  g / G", "Top / Bottom", key, desc),
            help_line("  Home / End", "First / Last column", key, desc),
            help_line("  PgUp / PgDn", "Page up / down", key, desc),
            blank.clone(),
            Line::from(Span::styled("Schema Tree", section)),
            help_line("  j/k  \u{2191}/\u{2193}", "Navigate", key, desc),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::TreeBrowser), KeyAction::Expand)
                ),
                "Preview data / Expand",
                key,
                desc,
            ),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::TreeBrowser), KeyAction::ToggleExpand)
                ),
                "Toggle expand",
                key,
                desc,
            ),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::TreeBrowser), KeyAction::Collapse)
                ),
                "Collapse",
                key,
                desc,
            ),
            blank.clone(),
            Line::from(Span::styled("Inspector", section)),
            help_line("  j/k  \u{2191}/\u{2193}", "Scroll", key, desc),
            help_line(
                &format!(
                    "  {}",
                    fmt(Some(PanelFocus::Inspector), KeyAction::CopyContent)
                ),
                "Copy content",
                key,
                desc,
            ),
            help_line(
                &format!("  {}", fmt(Some(PanelFocus::Inspector), KeyAction::Dismiss)),
                "Close",
                key,
                desc,
            ),
            blank.clone(),
            Line::from(Span::styled("Command Palette", section)),
            help_line("  Enter", "Execute command", key, desc),
            help_line("  Esc", "Cancel", key, desc),
            blank.clone(),
            Line::from(Span::styled("Commands", section)),
            help_line("  /help", "Show this help", key, desc),
            help_line("  /connect", "Connection picker", key, desc),
            help_line("  /refresh", "Reload schema", key, desc),
        ]
    }

    /// Render the help content into the given area
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme, km: &KeyMap) {
        if area.height == 0 {
            return;
        }

        let lines = self.build_lines(theme, km);
        self.line_count.set(lines.len());
        let visible_height = area.height as usize;

        for i in 0..visible_height {
            let line_idx = self.scroll_offset + i;
            let y = area.y + i as u16;

            if line_idx < lines.len() {
                frame.render_widget(
                    Paragraph::new(lines[line_idx].clone()),
                    Rect::new(area.x, y, area.width, 1),
                );
            }
        }
    }
}

impl Default for HelpOverlay {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a single help line: "  key           description"
fn help_line<'a>(
    key_text: &str,
    desc_text: &'a str,
    key_style: Style,
    desc_style: Style,
) -> Line<'a> {
    // Pad key to 20 chars for alignment
    let padded_key = format!("{:<20}", key_text);
    Line::from(vec![
        Span::styled(padded_key, key_style),
        Span::styled(desc_text, desc_style),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_default_not_visible() {
        let help = HelpOverlay::new();
        assert!(!help.is_visible());
        assert_eq!(help.scroll_offset, 0);
    }

    #[test]
    fn test_help_show_hide() {
        let mut help = HelpOverlay::new();

        help.show();
        assert!(help.is_visible());
        assert_eq!(help.scroll_offset, 0);

        // Scroll down then hide — should reset
        help.scroll_down();
        assert_eq!(help.scroll_offset, 1);

        help.hide();
        assert!(!help.is_visible());
        assert_eq!(help.scroll_offset, 0);
    }

    #[test]
    fn test_help_scroll_boundaries() {
        let mut help = HelpOverlay::new();
        help.show();

        // scroll_up at top stays at 0
        help.scroll_up();
        assert_eq!(help.scroll_offset, 0);

        // scroll_down increments
        help.scroll_down();
        assert_eq!(help.scroll_offset, 1);

        // scroll_to_bottom goes to last line
        help.scroll_to_bottom();
        assert_eq!(help.scroll_offset, help.line_count.get() - 1);

        // scroll_down at bottom stays at bottom
        help.scroll_down();
        assert_eq!(help.scroll_offset, help.line_count.get() - 1);

        // scroll_to_top resets
        help.scroll_to_top();
        assert_eq!(help.scroll_offset, 0);

        // page_down from 0 goes to 20
        help.page_down();
        assert_eq!(help.scroll_offset, 20);

        // page_up from 20 goes to 0
        help.page_up();
        assert_eq!(help.scroll_offset, 0);
    }

    #[test]
    fn test_help_line_count_matches_content() {
        let help = HelpOverlay::new();
        let theme = Theme::default();
        let km = KeyMap::default();
        let lines = help.build_lines(&theme, &km);
        // Verify we get a reasonable number of lines
        assert!(
            lines.len() > 40,
            "Expected at least 40 help lines, got {}",
            lines.len()
        );
    }
}

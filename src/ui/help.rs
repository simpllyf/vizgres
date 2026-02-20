//! Help overlay — keybinding reference modal
//!
//! Displays all keybindings organized by panel context as a centered popup.
//! Follows the same overlay pattern as Inspector.

use crate::ui::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Help overlay showing keybinding reference
pub struct HelpOverlay {
    visible: bool,
    scroll_offset: usize,
}

/// Total number of content lines in the help text
const HELP_LINE_COUNT: usize = 51;

impl HelpOverlay {
    pub fn new() -> Self {
        Self {
            visible: false,
            scroll_offset: 0,
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
        if self.scroll_offset + 1 < HELP_LINE_COUNT {
            self.scroll_offset += 1;
        }
    }

    pub fn page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(20);
    }

    pub fn page_down(&mut self) {
        self.scroll_offset = (self.scroll_offset + 20).min(HELP_LINE_COUNT.saturating_sub(1));
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = HELP_LINE_COUNT.saturating_sub(1);
    }

    /// Build styled help content lines
    fn build_lines<'a>(&self, theme: &Theme) -> Vec<Line<'a>> {
        let section = theme.help_section;
        let key = theme.help_key;
        let desc = theme.help_desc;
        let blank = Line::from("");

        vec![
            Line::from(Span::styled("Global", section)),
            help_line("  Ctrl+Q", "Quit", key, desc),
            help_line("  Tab / Shift+Tab", "Cycle panel focus", key, desc),
            help_line("  Ctrl+P", "Command palette", key, desc),
            help_line("  F1 / ?", "Help", key, desc),
            help_line("  Ctrl+T", "New tab", key, desc),
            help_line("  Ctrl+W", "Close tab", key, desc),
            help_line("  Ctrl+N", "Next tab", key, desc),
            blank.clone(),
            Line::from(Span::styled("Editor", section)),
            help_line("  F5 / Ctrl+Enter", "Execute query", key, desc),
            help_line("  Ctrl+E", "EXPLAIN ANALYZE", key, desc),
            help_line("  Ctrl+L", "Clear editor", key, desc),
            help_line("  Ctrl+Z", "Undo", key, desc),
            help_line("  Ctrl+Shift+Z", "Redo", key, desc),
            help_line("  Ctrl+Alt+F", "Format query", key, desc),
            help_line("  Ctrl+Up/Down", "Query history", key, desc),
            help_line("  Right", "Accept completion", key, desc),
            help_line("  Alt+Down/Up", "Cycle completions", key, desc),
            help_line("  Esc", "Cancel running query", key, desc),
            blank.clone(),
            Line::from(Span::styled("Results", section)),
            help_line("  j/k  \u{2191}/\u{2193}", "Navigate rows", key, desc),
            help_line("  h/l  \u{2190}/\u{2192}", "Navigate columns", key, desc),
            help_line("  Enter", "Inspect cell", key, desc),
            help_line("  y", "Copy cell", key, desc),
            help_line("  Y", "Copy row", key, desc),
            help_line("  Ctrl+S", "Export CSV", key, desc),
            help_line("  Ctrl+J", "Export JSON", key, desc),
            help_line("  g / G", "Top / Bottom", key, desc),
            help_line("  Home / End", "First / Last column", key, desc),
            help_line("  PgUp / PgDn", "Page up / down", key, desc),
            blank.clone(),
            Line::from(Span::styled("Schema Tree", section)),
            help_line("  j/k  \u{2191}/\u{2193}", "Navigate", key, desc),
            help_line("  Enter", "Preview data / Expand", key, desc),
            help_line("  Space", "Toggle expand", key, desc),
            help_line("  h", "Collapse", key, desc),
            blank.clone(),
            Line::from(Span::styled("Inspector", section)),
            help_line("  j/k  \u{2191}/\u{2193}", "Scroll", key, desc),
            help_line("  y", "Copy content", key, desc),
            help_line("  Esc", "Close", key, desc),
            blank.clone(),
            Line::from(Span::styled("Command Palette", section)),
            help_line("  Enter", "Execute command", key, desc),
            help_line("  Esc", "Cancel", key, desc),
            blank.clone(),
            Line::from(Span::styled("Commands", section)),
            help_line("  /help", "Show this help", key, desc),
            help_line("  /refresh", "Reload schema", key, desc),
        ]
    }

    /// Render the help content into the given area
    pub fn render(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if area.height == 0 {
            return;
        }

        let lines = self.build_lines(theme);
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
    key_text: &'a str,
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
        assert_eq!(help.scroll_offset, HELP_LINE_COUNT - 1);

        // scroll_down at bottom stays at bottom
        help.scroll_down();
        assert_eq!(help.scroll_offset, HELP_LINE_COUNT - 1);

        // scroll_to_top resets
        help.scroll_to_top();
        assert_eq!(help.scroll_offset, 0);

        // page_down from 0 goes to 20
        help.page_down();
        assert_eq!(help.scroll_offset, 20);

        // page_up from 20 goes to 0
        help.page_up();
        assert_eq!(help.scroll_offset, 0);

        // page_up at top stays at 0
        help.page_up();
        assert_eq!(help.scroll_offset, 0);
    }

    #[test]
    fn test_help_line_count_matches_content() {
        let help = HelpOverlay::new();
        let theme = Theme::default();
        let lines = help.build_lines(&theme);
        assert_eq!(
            lines.len(),
            HELP_LINE_COUNT,
            "HELP_LINE_COUNT constant ({}) doesn't match actual line count ({})",
            HELP_LINE_COUNT,
            lines.len()
        );
    }
}

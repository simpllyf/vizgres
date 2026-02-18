//! Cell value inspector panel
//!
//! Displays full cell content as a right-side split panel.
//! JSON values are pretty-printed. Scrollable for large content.

use crate::ui::Component;
use crate::ui::theme::Theme;
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Cell value inspector (split panel, not overlay)
pub struct Inspector {
    /// The content to display (pre-formatted)
    content: Option<String>,
    /// Column name
    column_name: String,
    /// Data type display string
    data_type: String,
    /// Scroll offset for large content
    scroll_offset: usize,
    /// Total lines in content
    total_lines: usize,
}

impl Inspector {
    pub fn new() -> Self {
        Self {
            content: None,
            column_name: String::new(),
            data_type: String::new(),
            scroll_offset: 0,
            total_lines: 0,
        }
    }

    /// Show cell content in the inspector
    pub fn show(&mut self, content: String, column_name: String, data_type: String) {
        self.total_lines = content.lines().count().max(1);
        self.content = Some(content);
        self.column_name = column_name;
        self.data_type = data_type;
        self.scroll_offset = 0;
    }

    pub fn hide(&mut self) {
        self.content = None;
        self.scroll_offset = 0;
    }

    pub fn is_visible(&self) -> bool {
        self.content.is_some()
    }

    /// Get the raw content text (for clipboard copy)
    pub fn content_text(&self) -> Option<String> {
        self.content.clone()
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        if self.scroll_offset + 1 < self.total_lines {
            self.scroll_offset += 1;
        }
    }

    pub fn page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(20);
    }

    pub fn page_down(&mut self) {
        self.scroll_offset = (self.scroll_offset + 20).min(self.total_lines.saturating_sub(1));
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.total_lines.saturating_sub(1);
    }

    /// Measure content dimensions (width, height) for variable-size popup.
    /// Returns (0, 0) if no content.
    pub fn content_size(&self) -> (u16, u16) {
        match &self.content {
            Some(text) => {
                let max_width = text.lines().map(|l| l.len()).max().unwrap_or(0) as u16;
                (max_width, self.total_lines as u16)
            }
            None => (0, 0),
        }
    }
}

impl Default for Inspector {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Inspector {
    fn render(&self, frame: &mut Frame, area: Rect, _focused: bool, theme: &Theme) {
        let content = match &self.content {
            Some(c) => c,
            None => return,
        };

        if area.height < 2 {
            return;
        }

        // Header: column name and type
        let header = format!("{} ({})", self.column_name, self.data_type);
        frame.render_widget(
            Paragraph::new(header).style(theme.inspector_header),
            Rect::new(area.x, area.y, area.width, 1),
        );

        // Content area
        let content_area = Rect::new(area.x, area.y + 1, area.width, area.height - 1);
        let lines: Vec<&str> = content.lines().collect();
        let visible_height = content_area.height as usize;

        for i in 0..visible_height {
            let line_idx = self.scroll_offset + i;
            let y = content_area.y + i as u16;

            if line_idx < lines.len() {
                let line = lines[line_idx];
                let width = content_area.width as usize;
                let display: String = line.chars().take(width).collect();
                frame.render_widget(
                    Paragraph::new(display).style(theme.inspector_text),
                    Rect::new(content_area.x, y, content_area.width, 1),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspector_new() {
        let inspector = Inspector::new();
        assert!(!inspector.is_visible());
    }

    #[test]
    fn test_show_hide() {
        let mut inspector = Inspector::new();
        inspector.show(
            "test content".to_string(),
            "col".to_string(),
            "text".to_string(),
        );
        assert!(inspector.is_visible());
        assert_eq!(inspector.content_text(), Some("test content".to_string()));
        inspector.hide();
        assert!(!inspector.is_visible());
    }

    #[test]
    fn test_content_size_empty() {
        let inspector = Inspector::new();
        assert_eq!(inspector.content_size(), (0, 0));
    }

    #[test]
    fn test_content_size_single_line() {
        let mut inspector = Inspector::new();
        inspector.show("hello".to_string(), "col".to_string(), "text".to_string());
        assert_eq!(inspector.content_size(), (5, 1));
    }

    #[test]
    fn test_content_size_multiline() {
        let mut inspector = Inspector::new();
        let content = "short\na longer line here\nmed";
        inspector.show(content.to_string(), "col".to_string(), "json".to_string());
        // width = longest line ("a longer line here" = 18), height = 3 lines
        assert_eq!(inspector.content_size(), (18, 3));
    }

    #[test]
    fn test_scroll_boundaries() {
        let mut inspector = Inspector::new();
        let content = (0..50)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        inspector.show(content, "col".to_string(), "text".to_string());
        assert_eq!(inspector.scroll_offset, 0);
        assert_eq!(inspector.total_lines, 50);

        // scroll_up at top stays at 0
        inspector.scroll_up();
        assert_eq!(inspector.scroll_offset, 0);

        // scroll_down increments
        inspector.scroll_down();
        assert_eq!(inspector.scroll_offset, 1);

        // scroll_to_bottom goes to last line
        inspector.scroll_to_bottom();
        assert_eq!(inspector.scroll_offset, 49);

        // scroll_down at bottom stays at bottom
        inspector.scroll_down();
        assert_eq!(inspector.scroll_offset, 49);

        // scroll_to_top resets
        inspector.scroll_to_top();
        assert_eq!(inspector.scroll_offset, 0);

        // page_down from 0 goes to 20
        inspector.page_down();
        assert_eq!(inspector.scroll_offset, 20);

        // page_up from 20 goes to 0
        inspector.page_up();
        assert_eq!(inspector.scroll_offset, 0);

        // page_up at top stays at 0
        inspector.page_up();
        assert_eq!(inspector.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_no_content() {
        let mut inspector = Inspector::new();
        // Should not panic on empty inspector
        inspector.scroll_up();
        inspector.scroll_down();
        inspector.page_up();
        inspector.page_down();
        inspector.scroll_to_top();
        inspector.scroll_to_bottom();
        assert_eq!(inspector.scroll_offset, 0);
    }
}

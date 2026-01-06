//! Database tree browser widget
//!
//! Displays database schemas, tables, views, etc. in a hierarchical tree.

use crate::db::schema::SchemaTree;
use crate::ui::Component;
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

/// Tree browser component
pub struct TreeBrowser {
    /// The database schema to display
    schema: Option<SchemaTree>,

    /// Currently selected item index
    selected: usize,

    /// Scroll offset
    scroll_offset: usize,

    /// Expanded nodes (paths)
    _expanded: Vec<String>,
}

impl TreeBrowser {
    /// Create a new tree browser
    pub fn new() -> Self {
        Self {
            schema: None,
            selected: 0,
            scroll_offset: 0,
            _expanded: Vec::new(),
        }
    }

    /// Update the schema tree
    pub fn set_schema(&mut self, schema: SchemaTree) {
        self.schema = Some(schema);
        self.selected = 0;
        self.scroll_offset = 0;
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        // TODO: Phase 3 - Implement navigation
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        // TODO: Phase 3 - Implement navigation
        self.selected += 1;
    }

    /// Expand/collapse current node
    pub fn toggle_expand(&mut self) {
        // TODO: Phase 3 - Implement expand/collapse
        todo!("Tree expand/collapse not yet implemented")
    }
}

impl Default for TreeBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TreeBrowser {
    fn handle_key(&mut self, _key: KeyEvent) -> bool {
        // TODO: Phase 3 - Handle navigation keys (j/k, h/l, Enter, /)
        false
    }

    fn render(&self, _frame: &mut Frame, _area: Rect, _focused: bool) {
        // TODO: Phase 3 - Render tree with proper formatting
        // - Show expand/collapse indicators (▶/▼)
        // - Indent based on depth
        // - Highlight selected item
        // - Show icons for different node types
    }

    fn min_size(&self) -> (u16, u16) {
        (20, 10) // Minimum width for tree, minimum height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_browser_new() {
        let tree = TreeBrowser::new();
        assert!(tree.schema.is_none());
        assert_eq!(tree.selected, 0);
    }

    #[test]
    fn test_move_up_at_top() {
        let mut tree = TreeBrowser::new();
        tree.move_up();
        assert_eq!(tree.selected, 0); // Should stay at 0
    }

    #[test]
    fn test_move_down() {
        let mut tree = TreeBrowser::new();
        tree.move_down();
        assert_eq!(tree.selected, 1);
    }
}

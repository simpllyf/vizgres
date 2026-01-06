//! Panel layout management
//!
//! Handles the arrangement of panels and terminal screen layout.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Calculate panel layout for the main screen
///
/// Returns (tree_area, editor_area, results_area, command_area)
pub fn calculate_layout(area: Rect) -> (Rect, Rect, Rect, Rect) {
    // TODO: Phase 2 - Implement flexible layout
    // Main layout:
    // - Top row for main panels
    // - Bottom row for command bar
    // Left panel for tree, right side split for editor/results

    // Temporary stub - returns dummy areas
    let tree = Rect::new(0, 0, 20, area.height);
    let editor = Rect::new(20, 0, area.width - 20, area.height / 2);
    let results = Rect::new(20, area.height / 2, area.width - 20, area.height / 2);
    let command = Rect::new(0, area.height - 1, area.width, 1);

    (tree, editor, results, command)
}

/// Calculate layout with custom tree width
pub fn calculate_layout_with_tree_width(area: Rect, tree_width: u16) -> (Rect, Rect, Rect, Rect) {
    // TODO: Phase 2 - Allow resizable tree panel
    calculate_layout(area)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_layout() {
        let area = Rect::new(0, 0, 100, 50);
        let (tree, editor, results, command) = calculate_layout(area);

        assert!(tree.width > 0);
        assert!(editor.width > 0);
        assert!(results.width > 0);
        assert_eq!(command.height, 1);
    }
}

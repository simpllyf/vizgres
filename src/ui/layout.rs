//! Panel layout management
//!
//! Handles the arrangement of panels and terminal screen layout.

use ratatui::layout::Rect;

/// Layout areas for the main screen
pub struct AppLayout {
    /// Left panel: schema tree browser
    pub tree: Rect,
    /// Right top: query editor
    pub editor: Rect,
    /// Right bottom: query results
    pub results: Rect,
    /// Bottom row: command/status bar
    pub command_bar: Rect,
}

/// Calculate panel layout for the main screen
pub fn calculate_layout(area: Rect) -> AppLayout {
    if area.height < 4 || area.width < 20 {
        // Too small - give everything to results
        return AppLayout {
            tree: Rect::new(area.x, area.y, 0, 0),
            editor: Rect::new(area.x, area.y, 0, 0),
            results: Rect::new(area.x, area.y, area.width, area.height.saturating_sub(1)),
            command_bar: Rect::new(
                area.x,
                area.y + area.height.saturating_sub(1),
                area.width,
                1,
            ),
        };
    }

    // Reserve bottom row for command bar
    let main_height = area.height - 1;
    let command_bar = Rect::new(area.x, area.y + main_height, area.width, 1);

    // Left panel: tree (25% width, min 20, max 40)
    let tree_width = (area.width / 4).clamp(20, 40).min(area.width / 2);
    let tree = Rect::new(area.x, area.y, tree_width, main_height);

    // Right side
    let right_x = area.x + tree_width;
    let right_width = area.width - tree_width;

    // Editor gets 40% of right side height, results gets 60%
    let editor_height = (main_height * 2 / 5).max(3);
    let results_height = main_height - editor_height;

    let editor = Rect::new(right_x, area.y, right_width, editor_height);
    let results = Rect::new(right_x, area.y + editor_height, right_width, results_height);

    AppLayout {
        tree,
        editor,
        results,
        command_bar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_layout() {
        let area = Rect::new(0, 0, 100, 50);
        let layout = calculate_layout(area);

        assert!(layout.tree.width > 0);
        assert!(layout.editor.width > 0);
        assert!(layout.results.width > 0);
        assert_eq!(layout.command_bar.height, 1);
    }

    #[test]
    fn test_layout_reserves_command_bar() {
        let area = Rect::new(0, 0, 80, 40);
        let layout = calculate_layout(area);

        // Command bar should be at the very bottom
        assert_eq!(layout.command_bar.y, area.height - 1);
        assert_eq!(layout.command_bar.height, 1);
        assert_eq!(layout.command_bar.width, area.width);
    }
}

//! Panel layout management
//!
//! Handles the arrangement of panels and terminal screen layout.

use ratatui::layout::Rect;

/// Layout areas for the main screen
pub struct AppLayout {
    /// Left panel: schema tree browser
    pub tree: Rect,
    /// Tab bar (1 row above editor, only when >1 tab)
    pub tab_bar: Rect,
    /// Right top: query editor
    pub editor: Rect,
    /// Right bottom: query results
    pub results: Rect,
    /// Bottom row: command/status bar
    pub command_bar: Rect,
}

/// Calculate panel layout for the main screen
pub fn calculate_layout(area: Rect, show_tab_bar: bool) -> AppLayout {
    if area.height < 4 || area.width < 20 {
        // Too small - give everything to results
        return AppLayout {
            tree: Rect::new(area.x, area.y, 0, 0),
            tab_bar: Rect::new(0, 0, 0, 0),
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

    // Tab bar steals 1 row from the top of the right side when visible
    let tab_bar_height: u16 = if show_tab_bar { 1 } else { 0 };
    let tab_bar = if show_tab_bar {
        Rect::new(right_x, area.y, right_width, 1)
    } else {
        Rect::new(0, 0, 0, 0)
    };
    let right_top_y = area.y + tab_bar_height;
    let right_main_height = main_height - tab_bar_height;

    // Editor gets 40% of right side height, results gets 60%
    let editor_height = (right_main_height * 2 / 5).max(3).min(right_main_height);
    let results_height = right_main_height - editor_height;

    let editor = Rect::new(right_x, right_top_y, right_width, editor_height);
    let results = Rect::new(
        right_x,
        right_top_y + editor_height,
        right_width,
        results_height,
    );

    AppLayout {
        tree,
        tab_bar,
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
        let layout = calculate_layout(area, false);

        assert!(layout.tree.width > 0);
        assert!(layout.editor.width > 0);
        assert!(layout.results.width > 0);
        assert_eq!(layout.command_bar.height, 1);
        assert_eq!(layout.tab_bar.width, 0);
    }

    #[test]
    fn test_layout_reserves_command_bar() {
        let area = Rect::new(0, 0, 80, 40);
        let layout = calculate_layout(area, false);

        // Command bar should be at the very bottom
        assert_eq!(layout.command_bar.y, area.height - 1);
        assert_eq!(layout.command_bar.height, 1);
        assert_eq!(layout.command_bar.width, area.width);
    }

    #[test]
    fn test_tab_bar_steals_from_editor() {
        let area = Rect::new(0, 0, 100, 50);
        let without = calculate_layout(area, false);
        let with = calculate_layout(area, true);

        // Tab bar should be 1 row, positioned at the top of the right side
        assert_eq!(with.tab_bar.height, 1);
        assert_eq!(with.tab_bar.y, area.y);
        assert_eq!(with.tab_bar.width, without.editor.width);

        // Editor should start 1 row lower
        assert_eq!(with.editor.y, without.editor.y + 1);

        // Tree and command bar unaffected
        assert_eq!(with.tree, without.tree);
        assert_eq!(with.command_bar, without.command_bar);
    }

    #[test]
    fn test_tab_bar_small_terminal_no_underflow() {
        // height=4 with tab bar: main_height=3, tab_bar=1, right_main_height=2
        // editor_height would be max(0,3)=3 which exceeds 2 → must be clamped
        let area = Rect::new(0, 0, 80, 4);
        let layout = calculate_layout(area, true);
        // Should not panic; results_height should be 0
        assert!(layout.editor.height <= 2);
    }
}

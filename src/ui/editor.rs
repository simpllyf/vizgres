//! Query editor widget
//!
//! Multi-line SQL editor with line numbers and cursor.

use std::cell::Cell;

use crate::ui::Component;
use crate::ui::ComponentAction;
use crate::ui::highlight::{self, TokenKind};
use crate::ui::theme::Theme;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

/// Maximum number of undo snapshots to retain
const UNDO_CAPACITY: usize = 100;

/// Frozen editor state for undo/redo
#[derive(Clone)]
struct EditorSnapshot {
    lines: Vec<String>,
    cursor: (usize, usize),
}

/// Edit operation categories for coalescing
#[derive(PartialEq)]
enum EditOp {
    Insert,
    Backspace,
    DeleteForward,
    NewLine,
    Clear,
}

/// Query editor component
pub struct QueryEditor {
    /// Lines of text
    lines: Vec<String>,

    /// Cursor position (line, column)
    cursor: (usize, usize),

    /// Scroll offset (first visible line)
    scroll_offset: usize,

    /// Undo history (most recent at end)
    undo_stack: Vec<EditorSnapshot>,

    /// Redo history (most recent at end)
    redo_stack: Vec<EditorSnapshot>,

    /// Tracks current coalescing run
    last_op: Option<EditOp>,

    /// Viewport height from last render (set via Cell for interior mutability)
    visible_height: Cell<usize>,
}

impl QueryEditor {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: (0, 0),
            scroll_offset: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_op: None,
            visible_height: Cell::new(0),
        }
    }

    /// Get the full content as a single string
    pub fn get_content(&self) -> String {
        self.lines.join("\n")
    }

    /// Clear all editor content (undoable)
    pub fn clear(&mut self) {
        self.maybe_snapshot(EditOp::Clear);
        self.lines = vec![String::new()];
        self.cursor = (0, 0);
        self.scroll_offset = 0;
    }

    /// Replace all content, preserving undo history.
    /// Used by format operations — Ctrl+Z reverts to pre-format state.
    pub fn replace_content(&mut self, content: String) {
        self.maybe_snapshot(EditOp::Clear);
        self.lines = content.lines().map(String::from).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor = (0, 0);
        self.scroll_offset = 0;
    }

    /// Set the editor content (used by query history navigation).
    /// Resets both undo/redo stacks — history nav is its own undo mechanism.
    pub fn set_content(&mut self, content: String) {
        self.lines = content.lines().map(String::from).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor = (0, 0);
        self.scroll_offset = 0;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.last_op = None;
    }

    /// Snapshot current state before a mutation, with coalescing.
    ///
    /// Consecutive same-type coalescable operations (Insert, Backspace,
    /// DeleteForward) are grouped into a single undo step. NewLine and Clear
    /// always create a new snapshot.
    fn maybe_snapshot(&mut self, op: EditOp) {
        let coalescable = matches!(
            op,
            EditOp::Insert | EditOp::Backspace | EditOp::DeleteForward
        );
        let coalesced = coalescable && self.last_op.as_ref() == Some(&op);

        if !coalesced {
            self.undo_stack.push(EditorSnapshot {
                lines: self.lines.clone(),
                cursor: self.cursor,
            });
            if self.undo_stack.len() > UNDO_CAPACITY {
                self.undo_stack.remove(0);
            }
            self.redo_stack.clear();
        }
        self.last_op = Some(op);
    }

    /// Restore the previous editor state
    pub fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            self.redo_stack.push(EditorSnapshot {
                lines: self.lines.clone(),
                cursor: self.cursor,
            });
            self.lines = snapshot.lines;
            self.cursor = snapshot.cursor;
            self.last_op = None;
            self.ensure_cursor_visible();
        }
    }

    /// Re-apply a previously undone change
    pub fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            self.undo_stack.push(EditorSnapshot {
                lines: self.lines.clone(),
                cursor: self.cursor,
            });
            self.lines = snapshot.lines;
            self.cursor = snapshot.cursor;
            self.last_op = None;
            self.ensure_cursor_visible();
        }
    }

    fn insert_char(&mut self, c: char) {
        self.maybe_snapshot(EditOp::Insert);
        let line = &mut self.lines[self.cursor.0];
        // Handle cursor beyond line length
        let col = self.cursor.1.min(line.len());
        line.insert(col, c);
        self.cursor.1 = col + 1;
    }

    fn backspace(&mut self) {
        self.maybe_snapshot(EditOp::Backspace);
        if self.cursor.1 > 0 {
            let col = self.cursor.1.min(self.lines[self.cursor.0].len());
            if col > 0 {
                self.lines[self.cursor.0].remove(col - 1);
                self.cursor.1 = col - 1;
            }
        } else if self.cursor.0 > 0 {
            // Join with previous line
            let current_line = self.lines.remove(self.cursor.0);
            self.cursor.0 -= 1;
            self.cursor.1 = self.lines[self.cursor.0].len();
            self.lines[self.cursor.0].push_str(&current_line);
        }
    }

    fn delete_forward(&mut self) {
        self.maybe_snapshot(EditOp::DeleteForward);
        let line_len = self.lines[self.cursor.0].len();
        let col = self.cursor.1.min(line_len);
        if col < line_len {
            self.lines[self.cursor.0].remove(col);
        } else if self.cursor.0 < self.lines.len() - 1 {
            // Join next line into current
            let next_line = self.lines.remove(self.cursor.0 + 1);
            self.lines[self.cursor.0].push_str(&next_line);
        }
    }

    fn new_line(&mut self) {
        self.maybe_snapshot(EditOp::NewLine);
        let col = self.cursor.1.min(self.lines[self.cursor.0].len());
        let rest = self.lines[self.cursor.0][col..].to_string();
        self.lines[self.cursor.0].truncate(col);
        self.cursor.0 += 1;
        self.cursor.1 = 0;
        self.lines.insert(self.cursor.0, rest);
    }

    fn move_up(&mut self) {
        if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
            self.cursor.1 = self.cursor.1.min(self.lines[self.cursor.0].len());
        }
        self.last_op = None;
    }

    fn move_down(&mut self) {
        if self.cursor.0 < self.lines.len() - 1 {
            self.cursor.0 += 1;
            self.cursor.1 = self.cursor.1.min(self.lines[self.cursor.0].len());
        }
        self.last_op = None;
    }

    fn move_left(&mut self) {
        if self.cursor.1 > 0 {
            self.cursor.1 -= 1;
        } else if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
            self.cursor.1 = self.lines[self.cursor.0].len();
        }
        self.last_op = None;
    }

    fn move_right(&mut self) {
        let line_len = self.lines[self.cursor.0].len();
        if self.cursor.1 < line_len {
            self.cursor.1 += 1;
        } else if self.cursor.0 < self.lines.len() - 1 {
            self.cursor.0 += 1;
            self.cursor.1 = 0;
        }
        self.last_op = None;
    }

    fn move_home(&mut self) {
        self.cursor.1 = 0;
        self.last_op = None;
    }

    fn move_end(&mut self) {
        self.cursor.1 = self.lines[self.cursor.0].len();
        self.last_op = None;
    }

    fn ensure_cursor_visible(&mut self) {
        let h = self.visible_height.get();
        if h == 0 {
            return;
        }
        if self.cursor.0 < self.scroll_offset {
            self.scroll_offset = self.cursor.0;
        } else if self.cursor.0 >= self.scroll_offset + h {
            self.scroll_offset = self.cursor.0 - h + 1;
        }
    }
}

impl Default for QueryEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for QueryEditor {
    fn handle_key(&mut self, key: KeyEvent) -> ComponentAction {
        let result = match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    return ComponentAction::Ignored; // Let parent handle Ctrl combos
                }
                self.insert_char(c);
                ComponentAction::Consumed
            }
            KeyCode::Backspace => {
                self.backspace();
                ComponentAction::Consumed
            }
            KeyCode::Delete => {
                self.delete_forward();
                ComponentAction::Consumed
            }
            KeyCode::Enter => {
                self.new_line();
                ComponentAction::Consumed
            }
            KeyCode::Up => {
                self.move_up();
                ComponentAction::Consumed
            }
            KeyCode::Down => {
                self.move_down();
                ComponentAction::Consumed
            }
            KeyCode::Left => {
                self.move_left();
                ComponentAction::Consumed
            }
            KeyCode::Right => {
                self.move_right();
                ComponentAction::Consumed
            }
            KeyCode::Home => {
                self.move_home();
                ComponentAction::Consumed
            }
            KeyCode::End => {
                self.move_end();
                ComponentAction::Consumed
            }
            _ => ComponentAction::Ignored,
        };
        if matches!(result, ComponentAction::Consumed) {
            self.ensure_cursor_visible();
        }
        result
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, theme: &Theme) {
        if area.width < 2 || area.height == 0 {
            return;
        }

        self.visible_height.set(area.height as usize);
        let visible_height = area.height as usize;
        let line_num_width = format!("{}", self.lines.len()).len().max(2) as u16;
        let content_x = area.x + line_num_width + 1; // +1 for space after line number
        let content_width = area.width.saturating_sub(line_num_width + 1);

        // Pre-scan lines above viewport for block-comment state
        let mut in_block_comment = false;
        for line in &self.lines[..self.scroll_offset] {
            in_block_comment = highlight::scan_block_comment_state(line, in_block_comment);
        }

        for i in 0..visible_height {
            let line_idx = self.scroll_offset + i;
            let y = area.y + i as u16;

            if line_idx < self.lines.len() {
                // Line number
                let line_num = format!("{:>width$}", line_idx + 1, width = line_num_width as usize);
                let num_style = theme.editor_line_number;
                frame.render_widget(
                    Paragraph::new(line_num).style(num_style),
                    Rect::new(area.x, y, line_num_width, 1),
                );

                // Highlighted line content
                let line = &self.lines[line_idx];
                let max_w = content_width as usize;
                let (tokens, next_bc) = highlight::highlight_sql(line, in_block_comment);
                in_block_comment = next_bc;

                let spans: Vec<Span> = tokens
                    .iter()
                    .filter_map(|(kind, range)| {
                        let start = range.start.min(max_w);
                        let end = range.end.min(max_w);
                        if start >= end {
                            return None;
                        }
                        let style = match kind {
                            TokenKind::Keyword => theme.editor_keyword,
                            TokenKind::String => theme.editor_string,
                            TokenKind::Number => theme.editor_number,
                            TokenKind::Comment => theme.editor_comment,
                            TokenKind::Normal => theme.editor_text,
                        };
                        Some(Span::styled(&line[start..end], style))
                    })
                    .collect();

                frame.render_widget(
                    Paragraph::new(Line::from(spans)),
                    Rect::new(content_x, y, content_width, 1),
                );

                // Cursor
                if focused && line_idx == self.cursor.0 {
                    let cursor_col = self.cursor.1.min(line.len());
                    let cursor_x = content_x + cursor_col as u16;
                    if cursor_x < area.x + area.width {
                        frame.set_cursor_position(Position::new(cursor_x, y));
                    }
                }
            } else {
                // Empty line indicator
                let tilde = Paragraph::new("~").style(theme.editor_tilde);
                frame.render_widget(tilde, Rect::new(area.x, y, 1, 1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_new() {
        let editor = QueryEditor::new();
        assert_eq!(editor.get_content(), "");
    }

    #[test]
    fn test_insert_chars() {
        let mut editor = QueryEditor::new();
        editor.insert_char('S');
        editor.insert_char('E');
        editor.insert_char('L');
        assert_eq!(editor.get_content(), "SEL");
    }

    #[test]
    fn test_new_line() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.new_line();
        editor.insert_char('b');
        assert_eq!(editor.get_content(), "a\nb");
    }

    #[test]
    fn test_backspace_joins_lines() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.new_line();
        editor.insert_char('b');
        editor.cursor = (1, 0);
        editor.backspace();
        assert_eq!(editor.get_content(), "ab");
    }

    #[test]
    fn test_delete_forward() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('c');
        editor.cursor = (0, 1);
        editor.delete_forward();
        assert_eq!(editor.get_content(), "ac");
    }

    #[test]
    fn test_delete_forward_joins_lines() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.new_line();
        editor.insert_char('b');
        editor.cursor = (0, 1);
        editor.delete_forward();
        assert_eq!(editor.get_content(), "ab");
    }

    #[test]
    fn test_set_content() {
        let mut editor = QueryEditor::new();
        editor.set_content("SELECT *\nFROM users".to_string());
        assert_eq!(editor.get_content(), "SELECT *\nFROM users");
        assert_eq!(editor.lines.len(), 2);
    }

    #[test]
    fn test_clear() {
        let mut editor = QueryEditor::new();
        editor.set_content("SELECT * FROM users".to_string());
        editor.clear();
        assert_eq!(editor.get_content(), "");
    }

    // ── Undo / redo tests ────────────────────────────────────

    #[test]
    fn test_undo_single_char() {
        let mut editor = QueryEditor::new();
        editor.insert_char('x');
        assert_eq!(editor.get_content(), "x");
        editor.undo();
        assert_eq!(editor.get_content(), "");
    }

    #[test]
    fn test_redo_after_undo() {
        let mut editor = QueryEditor::new();
        editor.insert_char('x');
        editor.undo();
        assert_eq!(editor.get_content(), "");
        editor.redo();
        assert_eq!(editor.get_content(), "x");
    }

    #[test]
    fn test_undo_coalesces_inserts() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('c');
        assert_eq!(editor.get_content(), "abc");
        // Single undo should revert the entire coalesced group
        editor.undo();
        assert_eq!(editor.get_content(), "");
    }

    #[test]
    fn test_cursor_move_breaks_coalescing() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.move_home(); // breaks coalescing
        editor.move_end();
        editor.insert_char('b');
        assert_eq!(editor.get_content(), "ab");
        // First undo reverts 'b'
        editor.undo();
        assert_eq!(editor.get_content(), "a");
        // Second undo reverts 'a'
        editor.undo();
        assert_eq!(editor.get_content(), "");
    }

    #[test]
    fn test_undo_new_line() {
        let mut editor = QueryEditor::new();
        editor.new_line();
        editor.new_line();
        assert_eq!(editor.lines.len(), 3);
        // Each new_line is its own undo step (never coalesced)
        editor.undo();
        assert_eq!(editor.lines.len(), 2);
        editor.undo();
        assert_eq!(editor.lines.len(), 1);
    }

    #[test]
    fn test_undo_clear() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('c');
        assert_eq!(editor.get_content(), "abc");
        editor.clear();
        assert_eq!(editor.get_content(), "");
        // Undo should recover the cleared content
        editor.undo();
        assert_eq!(editor.get_content(), "abc");
    }

    #[test]
    fn test_set_content_resets_undo() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        // set_content resets undo/redo stacks
        editor.set_content("new content".to_string());
        editor.undo();
        // Should be no-op since stacks were cleared
        assert_eq!(editor.get_content(), "new content");
    }

    #[test]
    fn test_undo_backspace_coalesces() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('c');
        // Break coalescing so backspaces form their own group
        editor.move_end();
        editor.backspace();
        editor.backspace();
        assert_eq!(editor.get_content(), "a");
        // Single undo should revert both backspaces
        editor.undo();
        assert_eq!(editor.get_content(), "abc");
    }

    #[test]
    fn test_replace_content_is_undoable() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('c');
        assert_eq!(editor.get_content(), "abc");

        editor.replace_content("formatted\ncontent".to_string());
        assert_eq!(editor.get_content(), "formatted\ncontent");
        assert_eq!(editor.cursor, (0, 0));

        // Undo should restore original content
        editor.undo();
        assert_eq!(editor.get_content(), "abc");
    }

    #[test]
    fn test_redo_cleared_on_new_edit() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.move_end(); // break coalescing
        editor.insert_char('b');
        editor.undo(); // back to "a"
        assert_eq!(editor.get_content(), "a");
        // New edit should clear redo stack
        editor.insert_char('c');
        assert_eq!(editor.get_content(), "ac");
        // Redo should be empty
        editor.redo();
        assert_eq!(editor.get_content(), "ac");
    }

    // ── Scroll tests ────────────────────────────────────────

    #[test]
    fn test_scroll_follows_cursor_down() {
        let mut editor = QueryEditor::new();
        editor.visible_height.set(3);
        // Create 6 lines via handle_key (which calls ensure_cursor_visible)
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        for c in ['a', 'b', 'c', 'd', 'e'] {
            editor.handle_key(key(KeyCode::Char(c)));
            editor.handle_key(key(KeyCode::Enter));
        }
        editor.handle_key(key(KeyCode::Char('f')));
        // Cursor is at line 5, viewport is 3 → scroll_offset should adjust
        assert_eq!(editor.cursor.0, 5);
        assert!(editor.scroll_offset >= 3);
        assert!(editor.cursor.0 < editor.scroll_offset + 3);
    }

    #[test]
    fn test_scroll_follows_cursor_up() {
        let mut editor = QueryEditor::new();
        editor.visible_height.set(3);
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        // Create 6 lines and land at line 5
        for c in ['a', 'b', 'c', 'd', 'e'] {
            editor.handle_key(key(KeyCode::Char(c)));
            editor.handle_key(key(KeyCode::Enter));
        }
        editor.handle_key(key(KeyCode::Char('f')));
        // Now move back up past the viewport top
        for _ in 0..5 {
            editor.handle_key(key(KeyCode::Up));
        }
        assert_eq!(editor.cursor.0, 0);
        assert_eq!(editor.scroll_offset, 0);
    }

    #[test]
    fn test_undo_scrolls_to_cursor() {
        let mut editor = QueryEditor::new();
        editor.visible_height.set(3);
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        // Type on line 0, then add many newlines
        editor.handle_key(key(KeyCode::Char('x')));
        editor.handle_key(key(KeyCode::End)); // break coalescing
        for _ in 0..5 {
            editor.handle_key(key(KeyCode::Enter));
        }
        // cursor is now at line 5, scrolled down
        assert!(editor.scroll_offset > 0);
        // Undo last newline → cursor goes back toward top
        editor.undo(); // undo() calls ensure_cursor_visible internally
        assert!(editor.cursor.0 >= editor.scroll_offset);
        assert!(editor.cursor.0 < editor.scroll_offset + 3);
    }
}

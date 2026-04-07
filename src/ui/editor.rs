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

/// Get character count of a string (not byte count).
#[inline]
fn char_count(s: &str) -> usize {
    s.chars().count()
}

/// Convert character index to byte index in a string.
/// Returns `s.len()` if char_idx is beyond the string length.
#[inline]
fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
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

    /// Ghost text suffix shown after cursor for auto-completion
    ghost_text: Option<String>,
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
            ghost_text: None,
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
        // Handle cursor beyond line length (char-based)
        let char_col = self.cursor.1.min(char_count(line));
        let byte_idx = char_to_byte_idx(line, char_col);
        line.insert(byte_idx, c);
        self.cursor.1 = char_col + 1;
    }

    fn backspace(&mut self) {
        self.maybe_snapshot(EditOp::Backspace);
        if self.cursor.1 > 0 {
            let line = &self.lines[self.cursor.0];
            let char_col = self.cursor.1.min(char_count(line));
            if char_col > 0 {
                let byte_idx = char_to_byte_idx(line, char_col - 1);
                self.lines[self.cursor.0].remove(byte_idx);
                self.cursor.1 = char_col - 1;
            }
        } else if self.cursor.0 > 0 {
            // Join with previous line
            let current_line = self.lines.remove(self.cursor.0);
            self.cursor.0 -= 1;
            self.cursor.1 = char_count(&self.lines[self.cursor.0]);
            self.lines[self.cursor.0].push_str(&current_line);
        }
    }

    fn delete_forward(&mut self) {
        self.maybe_snapshot(EditOp::DeleteForward);
        let line = &self.lines[self.cursor.0];
        let line_char_len = char_count(line);
        let char_col = self.cursor.1.min(line_char_len);
        if char_col < line_char_len {
            let byte_idx = char_to_byte_idx(line, char_col);
            self.lines[self.cursor.0].remove(byte_idx);
        } else if self.cursor.0 < self.lines.len() - 1 {
            // Join next line into current
            let next_line = self.lines.remove(self.cursor.0 + 1);
            self.lines[self.cursor.0].push_str(&next_line);
        }
    }

    fn new_line(&mut self) {
        self.maybe_snapshot(EditOp::NewLine);
        let line = &self.lines[self.cursor.0];
        let char_col = self.cursor.1.min(char_count(line));
        let byte_idx = char_to_byte_idx(line, char_col);
        let rest = self.lines[self.cursor.0][byte_idx..].to_string();
        self.lines[self.cursor.0].truncate(byte_idx);
        self.cursor.0 += 1;
        self.cursor.1 = 0;
        self.lines.insert(self.cursor.0, rest);
    }

    fn move_up(&mut self) {
        if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
            self.cursor.1 = self.cursor.1.min(char_count(&self.lines[self.cursor.0]));
        }
        self.last_op = None;
    }

    fn move_down(&mut self) {
        if self.cursor.0 < self.lines.len() - 1 {
            self.cursor.0 += 1;
            self.cursor.1 = self.cursor.1.min(char_count(&self.lines[self.cursor.0]));
        }
        self.last_op = None;
    }

    fn move_left(&mut self) {
        if self.cursor.1 > 0 {
            self.cursor.1 -= 1;
        } else if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
            self.cursor.1 = char_count(&self.lines[self.cursor.0]);
        }
        self.last_op = None;
    }

    fn move_right(&mut self) {
        let line_char_len = char_count(&self.lines[self.cursor.0]);
        if self.cursor.1 < line_char_len {
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
        self.cursor.1 = char_count(&self.lines[self.cursor.0]);
        self.last_op = None;
    }

    /// Bulk-insert text (e.g. from paste). Single undo step, CRLF normalized.
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.maybe_snapshot(EditOp::Clear);

        let text = text.replace('\r', "");
        let line = &self.lines[self.cursor.0];
        let char_col = self.cursor.1.min(char_count(line));
        let byte_idx = char_to_byte_idx(line, char_col);
        let after_cursor = self.lines[self.cursor.0][byte_idx..].to_string();
        self.lines[self.cursor.0].truncate(byte_idx);

        let parts: Vec<&str> = text.split('\n').collect();

        // First part appends to current line
        self.lines[self.cursor.0].push_str(parts[0]);

        // Remaining parts become new lines
        for (i, part) in parts[1..].iter().enumerate() {
            self.lines.insert(self.cursor.0 + 1 + i, part.to_string());
        }

        // Cursor at end of last inserted part, before after_cursor (char-based)
        let last_line = self.cursor.0 + parts.len() - 1;
        let last_col = char_count(&self.lines[last_line]);
        self.lines[last_line].push_str(&after_cursor);
        self.cursor = (last_line, last_col);
        self.ensure_cursor_visible();
    }

    /// Set or clear the ghost text suffix.
    pub fn set_ghost_text(&mut self, text: Option<String>) {
        self.ghost_text = text;
    }

    /// Get current ghost text, if any.
    #[cfg(test)]
    pub fn ghost_text(&self) -> Option<&str> {
        self.ghost_text.as_deref()
    }

    /// Current cursor position (line, column).
    pub fn cursor(&self) -> (usize, usize) {
        self.cursor
    }

    /// Set cursor position (char-based), clamping to valid bounds.
    pub fn set_cursor_position(&mut self, line: usize, col: usize) {
        let line = line.min(self.lines.len().saturating_sub(1));
        let col = col.min(char_count(&self.lines[line]));
        self.cursor = (line, col);
        self.ensure_cursor_visible();
    }

    /// Get a line by index.
    pub fn line(&self, idx: usize) -> Option<&str> {
        self.lines.get(idx).map(|s| s.as_str())
    }

    /// Accept ghost text: insert it at cursor and clear. Returns true if accepted.
    pub fn accept_ghost_text(&mut self) -> bool {
        if let Some(text) = self.ghost_text.take() {
            // Break coalescing so acceptance is its own undo step
            self.last_op = None;
            self.maybe_snapshot(EditOp::Insert);
            let line = &self.lines[self.cursor.0];
            let char_col = self.cursor.1.min(char_count(line));
            let byte_idx = char_to_byte_idx(line, char_col);
            self.lines[self.cursor.0].insert_str(byte_idx, &text);
            self.cursor.1 = char_col + char_count(&text);
            true
        } else {
            false
        }
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
        // Right arrow: accept ghost text before clearing (so it can be inserted)
        if key.code == KeyCode::Right
            && key.modifiers == KeyModifiers::NONE
            && self.ghost_text.is_some()
        {
            self.accept_ghost_text();
            self.ensure_cursor_visible();
            return ComponentAction::Consumed;
        }

        // Clear ghost text on any other key — App will re-set if still appropriate
        self.ghost_text = None;

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
                let visible_str = super::unicode::truncate_to_width(line, content_width as usize);
                let max_byte = visible_str.len().min(line.len());
                let (tokens, next_bc) = highlight::highlight_sql(line, in_block_comment);
                in_block_comment = next_bc;

                let spans: Vec<Span> = tokens
                    .iter()
                    .filter_map(|(kind, range)| {
                        // Clamp byte ranges to visible width (converted to bytes)
                        let start = range.start.min(max_byte);
                        let end = range.end.min(max_byte);
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

                // Cursor and ghost text
                if focused && line_idx == self.cursor.0 {
                    let cursor_col = self.cursor.1.min(char_count(line));
                    let text_before_cursor = &line[..char_to_byte_idx(line, cursor_col)];
                    let cursor_x =
                        content_x + super::unicode::display_width(text_before_cursor) as u16;
                    if cursor_x < area.x + area.width {
                        frame.set_cursor_position(Position::new(cursor_x, y));
                    }

                    // Render ghost text after cursor
                    if let Some(ref ghost) = self.ghost_text {
                        let ghost_x = cursor_x;
                        let avail = (area.x + area.width).saturating_sub(ghost_x) as usize;
                        let ghost_width = super::unicode::display_width(ghost);
                        if avail > 0 && ghost_width > 0 {
                            let visible = super::unicode::truncate_to_width(ghost, avail);
                            let visible_width = super::unicode::display_width(&visible) as u16;
                            frame.render_widget(
                                Paragraph::new(Span::styled(visible, theme.editor_ghost)),
                                Rect::new(ghost_x, y, visible_width, 1),
                            );
                        }
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

    // ── Paste / insert_text tests ───────────────────────────

    #[test]
    fn test_insert_text_single_line() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.insert_text("bc");
        assert_eq!(editor.get_content(), "abc");
        assert_eq!(editor.cursor, (0, 3));
    }

    #[test]
    fn test_insert_text_multiline() {
        let mut editor = QueryEditor::new();
        editor.insert_text("SELECT *\nFROM users\nWHERE id = 1");
        assert_eq!(editor.lines.len(), 3);
        assert_eq!(editor.get_content(), "SELECT *\nFROM users\nWHERE id = 1");
        assert_eq!(editor.cursor, (2, 12));
    }

    #[test]
    fn test_insert_text_is_undoable() {
        let mut editor = QueryEditor::new();
        editor.insert_char('x');
        editor.move_end(); // break coalescing
        editor.insert_text("hello\nworld");
        assert_eq!(editor.get_content(), "xhello\nworld");
        editor.undo();
        assert_eq!(editor.get_content(), "x");
    }

    #[test]
    fn test_insert_text_normalizes_crlf() {
        let mut editor = QueryEditor::new();
        editor.insert_text("a\r\nb\r\nc");
        assert_eq!(editor.get_content(), "a\nb\nc");
        assert_eq!(editor.lines.len(), 3);
    }

    // ── Ghost text tests ───────────────────────────────────

    #[test]
    fn test_accept_ghost_text() {
        let mut editor = QueryEditor::new();
        editor.insert_char('S');
        editor.insert_char('E');
        editor.insert_char('L');
        editor.set_ghost_text(Some("ECT".to_string()));
        assert!(editor.accept_ghost_text());
        assert_eq!(editor.get_content(), "SELECT");
        assert_eq!(editor.cursor, (0, 6));
        assert!(editor.ghost_text().is_none());
    }

    #[test]
    fn test_accept_ghost_text_when_none() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        assert!(!editor.accept_ghost_text());
        assert_eq!(editor.get_content(), "a");
    }

    #[test]
    fn test_right_arrow_accepts_ghost() {
        let mut editor = QueryEditor::new();
        editor.insert_char('S');
        editor.insert_char('E');
        editor.insert_char('L');
        editor.set_ghost_text(Some("ECT".to_string()));
        let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        editor.handle_key(right);
        assert_eq!(editor.get_content(), "SELECT");
    }

    #[test]
    fn test_right_arrow_moves_without_ghost() {
        let mut editor = QueryEditor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.cursor = (0, 0);
        let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        editor.handle_key(right);
        assert_eq!(editor.cursor, (0, 1));
        assert_eq!(editor.get_content(), "ab");
    }

    #[test]
    fn test_ghost_cleared_on_insert() {
        let mut editor = QueryEditor::new();
        editor.insert_char('S');
        editor.set_ghost_text(Some("ELECT".to_string()));
        let key_e = KeyEvent::new(KeyCode::Char('E'), KeyModifiers::NONE);
        editor.handle_key(key_e);
        assert!(editor.ghost_text().is_none());
        assert_eq!(editor.get_content(), "SE");
    }

    #[test]
    fn test_ghost_cleared_on_backspace() {
        let mut editor = QueryEditor::new();
        editor.insert_char('S');
        editor.insert_char('E');
        editor.set_ghost_text(Some("LECT".to_string()));
        let bs = KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE);
        editor.handle_key(bs);
        assert!(editor.ghost_text().is_none());
        assert_eq!(editor.get_content(), "S");
    }

    #[test]
    fn test_accept_ghost_text_is_undoable() {
        let mut editor = QueryEditor::new();
        editor.insert_char('S');
        editor.insert_char('E');
        editor.insert_char('L');
        editor.set_ghost_text(Some("ECT".to_string()));
        editor.accept_ghost_text();
        assert_eq!(editor.get_content(), "SELECT");
        editor.undo();
        assert_eq!(editor.get_content(), "SEL");
    }

    // ── set_cursor_position tests ───────────────────────────────

    #[test]
    fn test_set_cursor_position_valid() {
        let mut editor = QueryEditor::new();
        editor.set_content("SELECT *\nFROM users".to_string());
        editor.set_cursor_position(1, 5);
        assert_eq!(editor.cursor(), (1, 5));
    }

    #[test]
    fn test_set_cursor_position_clamps_line() {
        let mut editor = QueryEditor::new();
        editor.set_content("SELECT *\nFROM users".to_string());
        editor.set_cursor_position(10, 0);
        // Should clamp to last line (1)
        assert_eq!(editor.cursor(), (1, 0));
    }

    #[test]
    fn test_set_cursor_position_clamps_column() {
        let mut editor = QueryEditor::new();
        editor.set_content("SELECT *\nFROM users".to_string());
        editor.set_cursor_position(0, 100);
        // Should clamp to line length (8)
        assert_eq!(editor.cursor(), (0, 8));
    }

    #[test]
    fn test_set_cursor_position_empty_line() {
        let mut editor = QueryEditor::new();
        editor.set_content("SELECT\n\nFROM".to_string());
        editor.set_cursor_position(1, 5);
        // Empty line has length 0, should clamp to 0
        assert_eq!(editor.cursor(), (1, 0));
    }

    // ── UTF-8 / multi-byte character tests ──────────────────────

    #[test]
    fn test_utf8_insert_after_emoji() {
        let mut editor = QueryEditor::new();
        editor.set_content("👋".to_string()); // 4 bytes, 1 char
        editor.set_cursor_position(0, 1); // After the emoji
        editor.insert_char('x');
        assert_eq!(editor.get_content(), "👋x");
        assert_eq!(editor.cursor(), (0, 2));
    }

    #[test]
    fn test_utf8_insert_before_emoji() {
        let mut editor = QueryEditor::new();
        editor.set_content("👋".to_string());
        editor.set_cursor_position(0, 0); // Before the emoji
        editor.insert_char('x');
        assert_eq!(editor.get_content(), "x👋");
        assert_eq!(editor.cursor(), (0, 1));
    }

    #[test]
    fn test_utf8_backspace_emoji() {
        let mut editor = QueryEditor::new();
        editor.set_content("a👋b".to_string()); // a(1) + 👋(4) + b(1) = 6 bytes, 3 chars
        editor.set_cursor_position(0, 2); // After the emoji
        editor.backspace();
        assert_eq!(editor.get_content(), "ab");
        assert_eq!(editor.cursor(), (0, 1));
    }

    #[test]
    fn test_utf8_delete_emoji() {
        let mut editor = QueryEditor::new();
        editor.set_content("a👋b".to_string());
        editor.set_cursor_position(0, 1); // Before the emoji
        editor.delete_forward();
        assert_eq!(editor.get_content(), "ab");
        assert_eq!(editor.cursor(), (0, 1));
    }

    #[test]
    fn test_utf8_accented_chars() {
        let mut editor = QueryEditor::new();
        editor.set_content("café".to_string()); // 5 bytes, 4 chars (é is 2 bytes)
        assert_eq!(editor.lines[0].len(), 5); // Byte length
        editor.set_cursor_position(0, 4); // End of line (char-based)
        assert_eq!(editor.cursor(), (0, 4));
        editor.insert_char('!');
        assert_eq!(editor.get_content(), "café!");
        assert_eq!(editor.cursor(), (0, 5));
    }

    #[test]
    fn test_utf8_cursor_movement() {
        let mut editor = QueryEditor::new();
        editor.set_content("a👋b🎉c".to_string()); // 5 chars
        editor.set_cursor_position(0, 0);

        // Move right through the string
        editor.move_right();
        assert_eq!(editor.cursor(), (0, 1)); // After 'a'
        editor.move_right();
        assert_eq!(editor.cursor(), (0, 2)); // After '👋'
        editor.move_right();
        assert_eq!(editor.cursor(), (0, 3)); // After 'b'
        editor.move_right();
        assert_eq!(editor.cursor(), (0, 4)); // After '🎉'
        editor.move_right();
        assert_eq!(editor.cursor(), (0, 5)); // After 'c' (end)

        // Move left back
        editor.move_left();
        assert_eq!(editor.cursor(), (0, 4));
        editor.move_left();
        assert_eq!(editor.cursor(), (0, 3));
    }

    #[test]
    fn test_utf8_new_line_splits_correctly() {
        let mut editor = QueryEditor::new();
        editor.set_content("café☕".to_string()); // "café" + coffee emoji
        editor.set_cursor_position(0, 4); // After 'é', before '☕'
        editor.new_line();
        assert_eq!(editor.lines.len(), 2);
        assert_eq!(editor.lines[0], "café");
        assert_eq!(editor.lines[1], "☕");
        assert_eq!(editor.cursor(), (1, 0));
    }

    #[test]
    fn test_utf8_paste_with_emoji() {
        let mut editor = QueryEditor::new();
        editor.set_content("hello".to_string());
        editor.set_cursor_position(0, 5); // End
        editor.insert_text(" 🌍🌎🌏"); // 3 earth emojis
        assert_eq!(editor.get_content(), "hello 🌍🌎🌏");
        assert_eq!(editor.cursor(), (0, 9)); // 5 + 1 space + 3 emoji = 9 chars
    }

    #[test]
    fn test_utf8_multiline_cursor_clamp() {
        let mut editor = QueryEditor::new();
        editor.set_content("long line here\n短".to_string()); // "短" is 1 char
        editor.set_cursor_position(0, 10); // Middle of first line
        editor.move_down();
        // Second line only has 1 char, cursor should clamp
        assert_eq!(editor.cursor(), (1, 1));
    }

    #[test]
    fn test_utf8_home_end() {
        let mut editor = QueryEditor::new();
        editor.set_content("🎵音楽🎵".to_string()); // 4 chars
        editor.set_cursor_position(0, 2);
        editor.move_home();
        assert_eq!(editor.cursor(), (0, 0));
        editor.move_end();
        assert_eq!(editor.cursor(), (0, 4));
    }

    #[test]
    fn test_utf8_ghost_text_acceptance() {
        let mut editor = QueryEditor::new();
        editor.set_content("SELECT 名前".to_string()); // "SELECT " + 2 Japanese chars
        editor.set_cursor_position(0, 9); // End (7 + 2)
        editor.set_ghost_text(Some(" FROM 表".to_string())); // ghost with Japanese
        assert!(editor.accept_ghost_text());
        assert_eq!(editor.get_content(), "SELECT 名前 FROM 表");
        assert_eq!(editor.cursor(), (0, 16)); // 9 + 7 chars in ghost
    }

    #[test]
    fn test_utf8_undo_preserves_content() {
        let mut editor = QueryEditor::new();
        editor.set_content("🚀".to_string());
        editor.set_cursor_position(0, 1);
        editor.insert_char('x');
        assert_eq!(editor.get_content(), "🚀x");
        editor.undo();
        assert_eq!(editor.get_content(), "🚀");
    }

    #[test]
    fn test_utf8_mixed_content_editing() {
        let mut editor = QueryEditor::new();
        // Mix of ASCII, accents, emoji, CJK
        editor.set_content("Hello café 👋 世界".to_string());
        // Byte count: 5 + 1 + 5 + 1 + 4 + 1 + 6 = 23 bytes
        // Char count: 5 + 1 + 4 + 1 + 1 + 1 + 2 = 15 chars

        editor.set_cursor_position(0, 15); // End
        assert_eq!(editor.cursor(), (0, 15));

        // Insert at end
        editor.insert_char('!');
        assert_eq!(editor.get_content(), "Hello café 👋 世界!");

        // Delete from middle (the emoji)
        editor.set_cursor_position(0, 12); // After the emoji
        editor.backspace();
        assert_eq!(editor.get_content(), "Hello café  世界!");
    }
}

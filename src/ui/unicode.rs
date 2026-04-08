//! Unicode-aware display width utilities for terminal rendering.
//!
//! CJK and other full-width characters occupy 2 terminal columns but are
//! counted as 1 by `.chars().count()` and as 3 bytes by `.len()`. All UI
//! width calculations must use these helpers instead.

use unicode_truncate::UnicodeTruncateStr;
use unicode_width::UnicodeWidthStr;

/// Terminal display width of a string (accounts for full-width characters).
pub fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Truncate a string to fit within `max_cols` terminal columns,
/// appending "..." if truncated. Returns the original string if it fits.
pub fn truncate_to_width(s: &str, max_cols: usize) -> String {
    let w = display_width(s);
    if w <= max_cols {
        return s.to_string();
    }
    if max_cols <= 3 {
        let (t, _) = s.unicode_truncate(max_cols);
        return t.to_string();
    }
    let (t, _) = s.unicode_truncate(max_cols - 3);
    format!("{}...", t)
}

/// Pad a string with trailing spaces so it occupies exactly `target_cols`
/// terminal columns. If the string is already wider, returns it as-is.
pub fn pad_to_width(s: &str, target_cols: usize) -> String {
    let w = display_width(s);
    if w >= target_cols {
        return s.to_string();
    }
    let mut result = String::with_capacity(s.len() + target_cols - w);
    result.push_str(s);
    for _ in 0..(target_cols - w) {
        result.push(' ');
    }
    result
}

/// Right-align a string within `target_cols` terminal columns by prepending spaces.
pub fn rpad_to_width(s: &str, target_cols: usize) -> String {
    let w = display_width(s);
    if w >= target_cols {
        return s.to_string();
    }
    let mut result = String::with_capacity(s.len() + target_cols - w);
    for _ in 0..(target_cols - w) {
        result.push(' ');
    }
    result.push_str(s);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_width_ascii() {
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn test_display_width_cjk() {
        assert_eq!(display_width("日本語"), 6);
        assert_eq!(display_width("カルメンチータ"), 14);
    }

    #[test]
    fn test_display_width_mixed() {
        assert_eq!(display_width("hello日本"), 9); // 5 + 4
    }

    #[test]
    fn test_truncate_fits() {
        assert_eq!(truncate_to_width("hello", 10), "hello");
        assert_eq!(truncate_to_width("日本語", 6), "日本語");
    }

    #[test]
    fn test_truncate_ascii() {
        assert_eq!(truncate_to_width("hello world", 8), "hello...");
    }

    #[test]
    fn test_truncate_cjk() {
        // "日本語テスト" = 12 cols, truncate to 9 => 6 cols of chars + "..."
        assert_eq!(truncate_to_width("日本語テスト", 9), "日本語...");
    }

    #[test]
    fn test_truncate_tiny() {
        assert_eq!(truncate_to_width("hello", 2), "he");
    }

    #[test]
    fn test_truncate_cjk_boundary() {
        // 7 cols budget, minus 3 for "..." = 4 cols. "日本" fits (4 cols).
        assert_eq!(truncate_to_width("日本語テスト", 7), "日本...");
    }

    #[test]
    fn test_pad_to_width_ascii() {
        assert_eq!(pad_to_width("hi", 5), "hi   ");
    }

    #[test]
    fn test_pad_to_width_cjk() {
        // "日本" = 4 cols, pad to 6 => 2 spaces
        assert_eq!(pad_to_width("日本", 6), "日本  ");
    }

    #[test]
    fn test_pad_already_wide() {
        assert_eq!(pad_to_width("hello", 3), "hello");
    }

    #[test]
    fn test_rpad_to_width() {
        assert_eq!(rpad_to_width("hi", 5), "   hi");
        assert_eq!(rpad_to_width("日本", 6), "  日本");
    }
}

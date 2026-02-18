//! Query history with shell-like navigation
//!
//! Ring buffer of executed queries, navigable with Ctrl+Up/Down.
//! Saves the current editor content as a "draft" when entering browse mode,
//! and restores it when navigating past the newest entry.
//!
//! History is persisted to `~/.vizgres/history` using null-byte separators
//! (multi-line SQL is preserved). Persistence is best-effort: failures
//! are silently ignored so the app never crashes over history I/O.

use std::collections::VecDeque;
use std::path::PathBuf;

/// Separator between history entries on disk. Null bytes never appear in SQL,
/// so this cleanly handles multi-line queries without escaping.
const ENTRY_SEPARATOR: char = '\0';

pub struct QueryHistory {
    entries: VecDeque<String>,
    capacity: usize,
    /// `None` = not browsing, `Some(i)` = showing `entries[i]`
    position: Option<usize>,
    /// Editor content saved when entering browse mode
    draft: Option<String>,
    /// File path for persistence (`None` = in-memory only)
    path: Option<PathBuf>,
}

impl QueryHistory {
    /// Create an in-memory-only history (no persistence).
    #[cfg(test)]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "QueryHistory capacity must be > 0");
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
            position: None,
            draft: None,
            path: None,
        }
    }

    /// Load history from `~/.vizgres/history`, creating an empty history
    /// if the file doesn't exist or can't be read.
    pub fn load(capacity: usize) -> Self {
        let path = dirs::home_dir().map(|h| h.join(".vizgres").join("history"));
        Self::load_from(path, capacity)
    }

    fn load_from(path: Option<PathBuf>, capacity: usize) -> Self {
        assert!(capacity > 0, "QueryHistory capacity must be > 0");
        let mut entries: VecDeque<String> = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|content| {
                content
                    .split(ENTRY_SEPARATOR)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        // Trim to capacity (keep newest)
        while entries.len() > capacity {
            entries.pop_front();
        }

        Self {
            entries,
            capacity,
            position: None,
            draft: None,
            path,
        }
    }

    /// Write all entries to disk. Best-effort: errors are silently ignored.
    fn save(&self) {
        let Some(path) = &self.path else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let content: String = self
            .entries
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(&ENTRY_SEPARATOR.to_string());
        let _ = std::fs::write(path, content);
    }

    /// Record an executed query. Trims whitespace, skips empty,
    /// deduplicates consecutive entries, drops oldest at capacity.
    pub fn push(&mut self, query: &str) {
        let trimmed = query.trim().to_string();
        if trimmed.is_empty() {
            return;
        }
        // Skip consecutive duplicates
        if self.entries.back() == Some(&trimmed) {
            self.reset_position();
            return;
        }
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(trimmed);
        self.reset_position();
        self.save();
    }

    /// Navigate to an older entry. On first call, saves `current_content` as draft.
    /// Returns `None` when already at the oldest entry.
    pub fn back(&mut self, current_content: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let new_pos = match self.position {
            None => {
                // Entering browse mode — save draft
                self.draft = Some(current_content.to_string());
                self.entries.len() - 1
            }
            Some(0) => return None, // already at oldest
            Some(p) => p - 1,
        };
        self.position = Some(new_pos);
        Some(&self.entries[new_pos])
    }

    /// Navigate to a newer entry. When moving past the newest,
    /// restores the draft and exits browse mode.
    /// Returns `None` when not browsing.
    pub fn forward(&mut self) -> Option<&str> {
        let pos = self.position?;
        if pos + 1 < self.entries.len() {
            self.position = Some(pos + 1);
            Some(&self.entries[pos + 1])
        } else {
            // Past newest — restore draft
            self.position = None;
            // Return draft content; caller will set_content with it
            self.draft.as_deref()
        }
    }

    fn reset_position(&mut self) {
        self.position = None;
        self.draft = None;
    }

    #[cfg(test)]
    fn is_browsing(&self) -> bool {
        self.position.is_some()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_history_is_empty() {
        let h = QueryHistory::new(100);
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
        assert!(!h.is_browsing());
    }

    #[test]
    fn test_push_and_len() {
        let mut h = QueryHistory::new(100);
        h.push("SELECT 1");
        h.push("SELECT 2");
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn test_push_trims_whitespace() {
        let mut h = QueryHistory::new(100);
        h.push("  SELECT 1  \n  ");
        assert_eq!(h.entries[0], "SELECT 1");
    }

    #[test]
    fn test_push_ignores_empty() {
        let mut h = QueryHistory::new(100);
        h.push("");
        h.push("   ");
        h.push("\n\t");
        assert!(h.is_empty());
    }

    #[test]
    fn test_consecutive_dedup() {
        let mut h = QueryHistory::new(100);
        h.push("SELECT 1");
        h.push("SELECT 1");
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn test_consecutive_dedup_after_trim() {
        let mut h = QueryHistory::new(100);
        h.push("SELECT 1");
        h.push("  SELECT 1  ");
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn test_non_consecutive_not_deduped() {
        let mut h = QueryHistory::new(100);
        h.push("SELECT 1");
        h.push("SELECT 2");
        h.push("SELECT 1");
        assert_eq!(h.len(), 3);
    }

    #[test]
    fn test_capacity_drops_oldest() {
        let mut h = QueryHistory::new(3);
        h.push("a");
        h.push("b");
        h.push("c");
        h.push("d");
        assert_eq!(h.len(), 3);
        assert_eq!(h.entries[0], "b");
        assert_eq!(h.entries[2], "d");
    }

    #[test]
    fn test_back_from_empty() {
        let mut h = QueryHistory::new(100);
        assert!(h.back("draft").is_none());
    }

    #[test]
    fn test_back_returns_newest_first() {
        let mut h = QueryHistory::new(100);
        h.push("SELECT 1");
        h.push("SELECT 2");
        assert_eq!(h.back("draft"), Some("SELECT 2"));
    }

    #[test]
    fn test_back_stops_at_oldest() {
        let mut h = QueryHistory::new(100);
        h.push("SELECT 1");
        h.push("SELECT 2");
        h.back("draft"); // → SELECT 2
        h.back("draft"); // → SELECT 1
        assert!(h.back("draft").is_none()); // at oldest
    }

    #[test]
    fn test_forward_without_browsing() {
        let mut h = QueryHistory::new(100);
        h.push("SELECT 1");
        assert!(h.forward().is_none());
    }

    #[test]
    fn test_forward_returns_newer() {
        let mut h = QueryHistory::new(100);
        h.push("SELECT 1");
        h.push("SELECT 2");
        h.back("draft"); // → SELECT 2
        h.back("draft"); // → SELECT 1
        assert_eq!(h.forward(), Some("SELECT 2"));
    }

    #[test]
    fn test_forward_past_newest_restores_draft() {
        let mut h = QueryHistory::new(100);
        h.push("SELECT 1");
        h.back("my draft"); // → SELECT 1
        let restored = h.forward(); // past newest
        assert_eq!(restored, Some("my draft"));
        assert!(!h.is_browsing());
    }

    #[test]
    fn test_push_resets_position() {
        let mut h = QueryHistory::new(100);
        h.push("SELECT 1");
        h.push("SELECT 2");
        h.back("draft");
        assert!(h.is_browsing());
        h.push("SELECT 3");
        assert!(!h.is_browsing());
    }

    #[test]
    fn test_back_forward_round_trip() {
        let mut h = QueryHistory::new(100);
        h.push("a");
        h.push("b");
        h.push("c");
        assert_eq!(h.back("draft"), Some("c"));
        assert_eq!(h.back("draft"), Some("b"));
        assert_eq!(h.back("draft"), Some("a"));
        assert!(h.back("draft").is_none());
        assert_eq!(h.forward(), Some("b"));
        assert_eq!(h.forward(), Some("c"));
        assert_eq!(h.forward(), Some("draft"));
        assert!(!h.is_browsing());
    }

    #[test]
    fn test_re_enter_browse_after_draft_restore() {
        let mut h = QueryHistory::new(100);
        h.push("SELECT 1");

        // First browse cycle
        h.back("draft1");
        h.forward(); // restores draft1
        assert!(!h.is_browsing());

        // Second browse cycle with new draft
        assert_eq!(h.back("draft2"), Some("SELECT 1"));
        assert_eq!(h.forward(), Some("draft2"));
        assert!(!h.is_browsing());
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn test_zero_capacity_panics() {
        QueryHistory::new(0);
    }

    // ── Persistence tests ───────────────────────────────────

    fn temp_history_path(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vizgres-test-{}-{}", std::process::id(), name));
        let _ = std::fs::create_dir_all(&dir);
        dir.join("history")
    }

    fn cleanup(path: &std::path::Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn test_load_missing_file_returns_empty() {
        let path = temp_history_path("missing");
        cleanup(&path);
        let h = QueryHistory::load_from(Some(path.clone()), 100);
        assert!(h.is_empty());
        cleanup(&path);
    }

    #[test]
    fn test_save_and_load_round_trip() {
        let path = temp_history_path("round-trip");
        cleanup(&path);
        {
            let mut h = QueryHistory::load_from(Some(path.clone()), 100);
            h.push("SELECT 1");
            h.push("SELECT 2");
            h.push("SELECT 3");
        }
        let h = QueryHistory::load_from(Some(path.clone()), 100);
        assert_eq!(h.len(), 3);
        assert_eq!(h.entries[0], "SELECT 1");
        assert_eq!(h.entries[1], "SELECT 2");
        assert_eq!(h.entries[2], "SELECT 3");
        cleanup(&path);
    }

    #[test]
    fn test_multiline_queries_survive_round_trip() {
        let path = temp_history_path("multiline");
        cleanup(&path);
        {
            let mut h = QueryHistory::load_from(Some(path.clone()), 100);
            h.push("SELECT *\nFROM users\nWHERE id = 1");
            h.push("INSERT INTO t\nVALUES (1, 'hello')");
        }
        let h = QueryHistory::load_from(Some(path.clone()), 100);
        assert_eq!(h.len(), 2);
        assert_eq!(h.entries[0], "SELECT *\nFROM users\nWHERE id = 1");
        assert_eq!(h.entries[1], "INSERT INTO t\nVALUES (1, 'hello')");
        cleanup(&path);
    }

    #[test]
    fn test_load_trims_to_capacity() {
        let path = temp_history_path("trim-capacity");
        cleanup(&path);
        {
            let mut h = QueryHistory::load_from(Some(path.clone()), 100);
            for i in 0..10 {
                h.push(&format!("SELECT {}", i));
            }
        }
        // Reload with smaller capacity — keeps newest
        let h = QueryHistory::load_from(Some(path.clone()), 3);
        assert_eq!(h.len(), 3);
        assert_eq!(h.entries[0], "SELECT 7");
        assert_eq!(h.entries[1], "SELECT 8");
        assert_eq!(h.entries[2], "SELECT 9");
        cleanup(&path);
    }

    #[test]
    fn test_no_path_skips_persistence() {
        let mut h = QueryHistory::load_from(None, 100);
        h.push("SELECT 1");
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn test_push_persists_incrementally() {
        let path = temp_history_path("incremental");
        cleanup(&path);
        let mut h = QueryHistory::load_from(Some(path.clone()), 100);
        h.push("first");

        // Load a second instance — should see the entry
        let h2 = QueryHistory::load_from(Some(path.clone()), 100);
        assert_eq!(h2.len(), 1);
        assert_eq!(h2.entries[0], "first");

        // Push more and verify
        h.push("second");
        let h3 = QueryHistory::load_from(Some(path.clone()), 100);
        assert_eq!(h3.len(), 2);
        cleanup(&path);
    }
}

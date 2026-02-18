//! Query history with shell-like navigation
//!
//! Ring buffer of executed queries, navigable with Ctrl+Up/Down.
//! Saves the current editor content as a "draft" when entering browse mode,
//! and restores it when navigating past the newest entry.

use std::collections::VecDeque;

pub struct QueryHistory {
    entries: VecDeque<String>,
    capacity: usize,
    /// `None` = not browsing, `Some(i)` = showing `entries[i]`
    position: Option<usize>,
    /// Editor content saved when entering browse mode
    draft: Option<String>,
}

impl QueryHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
            position: None,
            draft: None,
        }
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
}

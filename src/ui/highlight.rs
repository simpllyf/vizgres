//! SQL syntax highlighting
//!
//! Line-by-line tokenizer for the query editor. Keywords are loaded from
//! `data/sql_keywords.txt` (embedded at compile time via `include_str!()`).

use std::collections::HashSet;
use std::ops::Range;
use std::sync::LazyLock;

/// Token classification for syntax highlighting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Keyword,
    String,
    Number,
    Comment,
    Normal,
}

/// SQL keywords from `data/sql_keywords.txt`, embedded at compile time.
static SQL_KEYWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    include_str!("../../data/sql_keywords.txt")
        .lines()
        .filter(|l| !l.is_empty())
        .collect()
});

/// Tokenize a single line for syntax highlighting.
///
/// Returns `(tokens, ends_in_block_comment)` — the bool must be threaded
/// into the next line to handle multi-line `/* ... */` comments.
pub fn highlight_sql(line: &str, in_block_comment: bool) -> (Vec<(TokenKind, Range<usize>)>, bool) {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut in_bc = in_block_comment;

    while i < len {
        // ── Inside a block comment: scan for */ ──────────────
        if in_bc {
            let start = i;
            loop {
                if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    in_bc = false;
                    break;
                }
                i += 1;
                if i >= len {
                    break;
                }
            }
            tokens.push((TokenKind::Comment, start..i));
            continue;
        }

        let b = bytes[i];

        // ── Line comment: -- to end of line ──────────────────
        if b == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
            tokens.push((TokenKind::Comment, i..len));
            return (tokens, false);
        }

        // ── Block comment start: /* ──────────────────────────
        if b == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            let start = i;
            i += 2;
            in_bc = true;
            loop {
                if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    in_bc = false;
                    break;
                }
                i += 1;
                if i >= len {
                    break;
                }
            }
            tokens.push((TokenKind::Comment, start..i));
            continue;
        }

        // ── String literal: 'text' with '' escape ────────────
        if b == b'\'' {
            let start = i;
            i += 1;
            loop {
                if i >= len {
                    break; // unterminated — highlight to end of line
                }
                if bytes[i] == b'\'' {
                    i += 1;
                    if i < len && bytes[i] == b'\'' {
                        i += 1; // escaped ''
                        continue;
                    }
                    break;
                }
                i += 1;
            }
            tokens.push((TokenKind::String, start..i));
            continue;
        }

        // ── Number: digits, optional decimal ─────────────────
        if b.is_ascii_digit() || (b == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            let start = i;
            if b == b'.' {
                i += 1;
            }
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if b != b'.' && i < len && bytes[i] == b'.' {
                i += 1;
                while i < len && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            tokens.push((TokenKind::Number, start..i));
            continue;
        }

        // ── Identifier / keyword ─────────────────────────────
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &line[start..i];
            let upper = word.to_ascii_uppercase();
            if SQL_KEYWORDS.contains(upper.as_str()) {
                tokens.push((TokenKind::Keyword, start..i));
            } else {
                tokens.push((TokenKind::Normal, start..i));
            }
            continue;
        }

        // ── Everything else: operators, whitespace, etc. ─────
        tokens.push((TokenKind::Normal, i..i + 1));
        i += 1;
    }

    (tokens, in_bc)
}

/// Expose the static keyword set for use by the completer.
pub fn sql_keywords() -> &'static HashSet<&'static str> {
    &SQL_KEYWORDS
}

/// Advance block-comment state through a line without allocating tokens.
///
/// Used to pre-scan lines above the visible viewport so the first visible
/// line knows whether it starts inside a block comment.
pub fn scan_block_comment_state(line: &str, in_block_comment: bool) -> bool {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_bc = in_block_comment;

    while i < len {
        if in_bc {
            if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                i += 2;
                in_bc = false;
            } else {
                i += 1;
            }
        } else if bytes[i] == b'-' && i + 1 < len && bytes[i + 1] == b'-' {
            return in_bc; // rest is line comment — no state change
        } else if bytes[i] == b'/' && i + 1 < len && bytes[i + 1] == b'*' {
            i += 2;
            in_bc = true;
        } else if bytes[i] == b'\'' {
            // Skip string literal so we don't match /* inside strings
            i += 1;
            while i < len {
                if bytes[i] == b'\'' {
                    i += 1;
                    if i < len && bytes[i] == b'\'' {
                        i += 1;
                        continue;
                    }
                    break;
                }
                i += 1;
            }
        } else {
            i += 1;
        }
    }

    in_bc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: collect token kinds from a line (not in block comment)
    fn kinds(line: &str) -> Vec<(TokenKind, &str)> {
        let (tokens, _) = highlight_sql(line, false);
        tokens.iter().map(|(k, r)| (*k, &line[r.clone()])).collect()
    }

    // ── Keywords ──────────────────────────────────────────

    #[test]
    fn keywords_basic() {
        let result = kinds("SELECT * FROM users");
        let kws: Vec<_> = result
            .iter()
            .filter(|(k, _)| *k == TokenKind::Keyword)
            .collect();
        assert_eq!(kws.len(), 2);
        assert_eq!(kws[0].1, "SELECT");
        assert_eq!(kws[1].1, "FROM");
        // "users" is not a keyword
        let normals: Vec<_> = result
            .iter()
            .filter(|(k, t)| *k == TokenKind::Normal && !t.trim().is_empty())
            .collect();
        assert!(normals.iter().any(|(_, t)| *t == "users"));
    }

    #[test]
    fn keywords_case_insensitive() {
        let result = kinds("select FROM Where");
        assert_eq!(result[0], (TokenKind::Keyword, "select"));
        assert_eq!(result[2], (TokenKind::Keyword, "FROM"));
        assert_eq!(result[4], (TokenKind::Keyword, "Where"));
    }

    #[test]
    fn non_keyword_identifier() {
        let result = kinds("username");
        assert_eq!(result, vec![(TokenKind::Normal, "username")]);
    }

    #[test]
    fn identifier_with_underscore() {
        let result = kinds("user_name");
        assert_eq!(result, vec![(TokenKind::Normal, "user_name")]);
    }

    // ── Strings ───────────────────────────────────────────

    #[test]
    fn string_simple() {
        let result = kinds("'hello'");
        assert_eq!(result, vec![(TokenKind::String, "'hello'")]);
    }

    #[test]
    fn string_escaped_quote() {
        let result = kinds("'it''s'");
        assert_eq!(result, vec![(TokenKind::String, "'it''s'")]);
    }

    #[test]
    fn string_unterminated() {
        let result = kinds("'unterminated");
        assert_eq!(result, vec![(TokenKind::String, "'unterminated")]);
    }

    #[test]
    fn string_in_context() {
        let result = kinds("WHERE name = 'Alice'");
        let strings: Vec<_> = result
            .iter()
            .filter(|(k, _)| *k == TokenKind::String)
            .collect();
        assert_eq!(strings.len(), 1);
        assert_eq!(strings[0].1, "'Alice'");
    }

    // ── Numbers ───────────────────────────────────────────

    #[test]
    fn number_integer() {
        let result = kinds("42");
        assert_eq!(result, vec![(TokenKind::Number, "42")]);
    }

    #[test]
    fn number_decimal() {
        let result = kinds("3.14");
        assert_eq!(result, vec![(TokenKind::Number, "3.14")]);
    }

    #[test]
    fn number_leading_dot() {
        let result = kinds(".5");
        assert_eq!(result, vec![(TokenKind::Number, ".5")]);
    }

    #[test]
    fn dot_alone_is_normal() {
        let result = kinds(".");
        assert_eq!(result, vec![(TokenKind::Normal, ".")]);
    }

    #[test]
    fn number_in_expression() {
        let result = kinds("id > 10");
        let numbers: Vec<_> = result
            .iter()
            .filter(|(k, _)| *k == TokenKind::Number)
            .collect();
        assert_eq!(numbers.len(), 1);
        assert_eq!(numbers[0].1, "10");
    }

    // ── Line comments ─────────────────────────────────────

    #[test]
    fn line_comment_whole_line() {
        let (tokens, bc) = highlight_sql("-- this is a comment", false);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].0, TokenKind::Comment);
        assert!(!bc);
    }

    #[test]
    fn line_comment_after_code() {
        let result = kinds("SELECT 1 -- one");
        let comments: Vec<_> = result
            .iter()
            .filter(|(k, _)| *k == TokenKind::Comment)
            .collect();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].1, "-- one");
    }

    // ── Block comments ────────────────────────────────────

    #[test]
    fn block_comment_single_line() {
        let result = kinds("/* comment */ SELECT");
        assert_eq!(result[0], (TokenKind::Comment, "/* comment */"));
        // SELECT should be a keyword after the comment
        let kws: Vec<_> = result
            .iter()
            .filter(|(k, _)| *k == TokenKind::Keyword)
            .collect();
        assert_eq!(kws.len(), 1);
    }

    #[test]
    fn block_comment_opens_multiline() {
        let (tokens, bc) = highlight_sql("SELECT /* start", false);
        assert!(bc, "should end in block comment");
        let comments: Vec<_> = tokens
            .iter()
            .filter(|(k, _)| k == &TokenKind::Comment)
            .collect();
        assert_eq!(comments.len(), 1);
    }

    #[test]
    fn block_comment_continuation() {
        let (tokens, bc) = highlight_sql("still in comment", true);
        assert!(bc);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].0, TokenKind::Comment);
    }

    #[test]
    fn block_comment_closes() {
        let (tokens, bc) = highlight_sql("end */ SELECT 1", true);
        assert!(!bc);
        let comments: Vec<_> = tokens
            .iter()
            .filter(|(k, _)| k == &TokenKind::Comment)
            .collect();
        assert_eq!(comments.len(), 1);
        let kws: Vec<_> = tokens
            .iter()
            .filter(|(k, _)| k == &TokenKind::Keyword)
            .collect();
        assert_eq!(kws.len(), 1);
    }

    // ── scan_block_comment_state ──────────────────────────

    #[test]
    fn scan_opens_block_comment() {
        assert!(scan_block_comment_state("SELECT /* start", false));
    }

    #[test]
    fn scan_closes_block_comment() {
        assert!(!scan_block_comment_state("end */ SELECT", true));
    }

    #[test]
    fn scan_open_and_close_same_line() {
        assert!(!scan_block_comment_state("/* comment */", false));
    }

    #[test]
    fn scan_no_change() {
        assert!(!scan_block_comment_state("SELECT 1", false));
        assert!(scan_block_comment_state("still in comment", true));
    }

    #[test]
    fn scan_ignores_block_comment_in_string() {
        assert!(!scan_block_comment_state("'/* not a comment */'", false));
    }

    // ── Edge cases ────────────────────────────────────────

    #[test]
    fn empty_line() {
        let (tokens, bc) = highlight_sql("", false);
        assert!(tokens.is_empty());
        assert!(!bc);
    }

    #[test]
    fn comment_inside_string_ignored() {
        // -- and /* inside strings must not start a comment
        let result = kinds("'hello -- world'");
        assert_eq!(result, vec![(TokenKind::String, "'hello -- world'")]);
        let result = kinds("'hello /* world */'");
        assert_eq!(result, vec![(TokenKind::String, "'hello /* world */'")]);
    }

    #[test]
    fn adjacent_tokens_no_space() {
        let result = kinds("SELECT(1)");
        let kws: Vec<_> = result
            .iter()
            .filter(|(k, _)| *k == TokenKind::Keyword)
            .collect();
        assert_eq!(kws.len(), 1);
        assert_eq!(kws[0].1, "SELECT");
        let nums: Vec<_> = result
            .iter()
            .filter(|(k, _)| *k == TokenKind::Number)
            .collect();
        assert_eq!(nums.len(), 1);
        assert_eq!(nums[0].1, "1");
    }

    #[test]
    fn negative_number_is_two_tokens() {
        // Minus is an operator, not part of the number
        let result = kinds("-42");
        assert_eq!(result[0], (TokenKind::Normal, "-"));
        assert_eq!(result[1], (TokenKind::Number, "42"));
    }

    #[test]
    fn qualified_name_dot_not_number() {
        // schema.tbl — dot between identifiers must not become a number
        let result = kinds("s.tbl");
        assert_eq!(result[0], (TokenKind::Normal, "s"));
        assert_eq!(result[1], (TokenKind::Normal, "."));
        assert_eq!(result[2], (TokenKind::Normal, "tbl"));
    }

    #[test]
    fn whitespace_only() {
        let (tokens, bc) = highlight_sql("   ", false);
        assert_eq!(tokens.len(), 3);
        assert!(tokens.iter().all(|(k, _)| *k == TokenKind::Normal));
        assert!(!bc);
    }

    #[test]
    fn realistic_query() {
        let line = "SELECT name, age FROM users WHERE id > 10 AND status = 'active' -- filter";
        let result = kinds(line);

        let kw_texts: Vec<&str> = result
            .iter()
            .filter(|(k, _)| *k == TokenKind::Keyword)
            .map(|(_, t)| *t)
            .collect();
        assert!(kw_texts.contains(&"SELECT"));
        assert!(kw_texts.contains(&"FROM"));
        assert!(kw_texts.contains(&"WHERE"));
        assert!(kw_texts.contains(&"AND"));

        let str_texts: Vec<&str> = result
            .iter()
            .filter(|(k, _)| *k == TokenKind::String)
            .map(|(_, t)| *t)
            .collect();
        assert_eq!(str_texts, vec!["'active'"]);

        let num_texts: Vec<&str> = result
            .iter()
            .filter(|(k, _)| *k == TokenKind::Number)
            .map(|(_, t)| *t)
            .collect();
        assert_eq!(num_texts, vec!["10"]);

        let comment_texts: Vec<&str> = result
            .iter()
            .filter(|(k, _)| *k == TokenKind::Comment)
            .map(|(_, t)| *t)
            .collect();
        assert_eq!(comment_texts, vec!["-- filter"]);
    }
}

//! SQL LIMIT/OFFSET detection
//!
//! Parenthesis-depth-aware scanner that detects whether a SQL query
//! already contains LIMIT or OFFSET at the outermost level.
//! Used to decide whether to add automatic pagination.

/// Result of analyzing user SQL for outer LIMIT/OFFSET
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitAnalysis {
    pub has_outer_limit: bool,
    pub has_outer_offset: bool,
}

impl LimitAnalysis {
    /// Whether the query can be safely paginated (no user LIMIT/OFFSET)
    pub fn can_paginate(&self) -> bool {
        !self.has_outer_limit && !self.has_outer_offset
    }
}

/// Analyze SQL to detect LIMIT/OFFSET at the outermost parenthesis level.
///
/// Handles: single-line comments (--), block comments (/* */),
/// string literals ('...'), dollar-quoted strings ($$...$$),
/// quoted identifiers ("..."), and FETCH FIRST N ROWS ONLY syntax.
pub fn analyze_limit(sql: &str) -> LimitAnalysis {
    let tokens = tokenize_outer(sql);
    let mut has_limit = false;
    let mut has_offset = false;

    for (i, token) in tokens.iter().enumerate() {
        match token.as_str() {
            "LIMIT" => has_limit = true,
            "OFFSET" => has_offset = true,
            "FETCH" => {
                // FETCH FIRST ... ROWS ONLY (SQL standard)
                if tokens
                    .get(i + 1)
                    .is_some_and(|t| t == "FIRST" || t == "NEXT")
                {
                    has_limit = true;
                }
            }
            _ => {}
        }
    }

    LimitAnalysis {
        has_outer_limit: has_limit,
        has_outer_offset: has_offset,
    }
}

/// Extract uppercase keyword tokens at parenthesis depth 0,
/// skipping comments, strings, and quoted identifiers.
fn tokenize_outer(sql: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut depth: usize = 0;

    while i < len {
        let ch = chars[i];

        // Single-line comment
        if ch == '-' && i + 1 < len && chars[i + 1] == '-' {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Block comment (PostgreSQL supports nesting)
        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            let mut nest = 1;
            while i < len && nest > 0 {
                if chars[i] == '/' && i + 1 < len && chars[i + 1] == '*' {
                    nest += 1;
                    i += 2;
                } else if chars[i] == '*' && i + 1 < len && chars[i + 1] == '/' {
                    nest -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }

        // String literal
        if ch == '\'' {
            i += 1;
            while i < len {
                if chars[i] == '\'' {
                    i += 1;
                    if i < len && chars[i] == '\'' {
                        i += 1; // escaped ''
                    } else {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            continue;
        }

        // Dollar-quoted string: $tag$...$tag$
        if ch == '$'
            && let Some(tag_len) = find_dollar_tag(&chars, i)
        {
            let tag: String = chars[i..i + tag_len].iter().collect();
            i += tag_len;
            // Find closing tag
            while i + tag_len <= len {
                let candidate: String = chars[i..i + tag_len].iter().collect();
                if candidate == tag {
                    i += tag_len;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Quoted identifier
        if ch == '"' {
            i += 1;
            while i < len {
                if chars[i] == '"' {
                    i += 1;
                    if i < len && chars[i] == '"' {
                        i += 1; // escaped ""
                    } else {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            continue;
        }

        // Parenthesis tracking
        if ch == '(' {
            depth += 1;
            i += 1;
            continue;
        }
        if ch == ')' {
            depth = depth.saturating_sub(1);
            i += 1;
            continue;
        }

        // At depth 0, collect keyword tokens
        if depth == 0 && ch.is_ascii_alphabetic() {
            let start = i;
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            tokens.push(word.to_uppercase());
            continue;
        }

        i += 1;
    }

    tokens
}

/// Check if position `i` starts a dollar-quote tag: $, optional identifier chars, $
/// Returns the length of the tag (including both $ delimiters) or None.
fn find_dollar_tag(chars: &[char], i: usize) -> Option<usize> {
    if chars[i] != '$' {
        return None;
    }
    let mut j = i + 1;
    // Tag name: optional sequence of identifier chars (letters, digits, _)
    while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
        j += 1;
    }
    if j < chars.len() && chars[j] == '$' {
        Some(j - i + 1) // includes both $ delimiters
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_no_limit() {
        let a = analyze_limit("SELECT * FROM users");
        assert!(a.can_paginate());
        assert!(!a.has_outer_limit);
        assert!(!a.has_outer_offset);
    }

    #[test]
    fn test_with_limit() {
        let a = analyze_limit("SELECT * FROM users LIMIT 10");
        assert!(!a.can_paginate());
        assert!(a.has_outer_limit);
    }

    #[test]
    fn test_with_limit_and_offset() {
        let a = analyze_limit("SELECT * FROM users LIMIT 10 OFFSET 20");
        assert!(!a.can_paginate());
        assert!(a.has_outer_limit);
        assert!(a.has_outer_offset);
    }

    #[test]
    fn test_offset_only() {
        let a = analyze_limit("SELECT * FROM users OFFSET 10");
        assert!(!a.can_paginate());
        assert!(a.has_outer_offset);
    }

    #[test]
    fn test_case_insensitive() {
        let a = analyze_limit("select * from users limit 10");
        assert!(a.has_outer_limit);

        let a = analyze_limit("SELECT * FROM users Limit 10 Offset 5");
        assert!(a.has_outer_limit);
        assert!(a.has_outer_offset);
    }

    #[test]
    fn test_limit_inside_subquery() {
        let a = analyze_limit("SELECT * FROM (SELECT * FROM users LIMIT 10) sub");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_limit_inside_cte() {
        let a =
            analyze_limit("WITH active AS (SELECT * FROM users LIMIT 100) SELECT * FROM active");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_both_inner_and_outer_limit() {
        let a = analyze_limit("SELECT * FROM (SELECT * FROM t LIMIT 5) sub LIMIT 10");
        assert!(a.has_outer_limit);
    }

    #[test]
    fn test_limit_in_string_literal() {
        let a = analyze_limit("SELECT * FROM users WHERE name = 'LIMIT 10'");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_limit_in_line_comment() {
        let a = analyze_limit("SELECT * FROM users -- LIMIT 10\nWHERE active = true");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_limit_in_block_comment() {
        let a = analyze_limit("SELECT * FROM users /* LIMIT 10 */ WHERE active = true");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_nested_block_comment() {
        let a = analyze_limit("SELECT * FROM users /* outer /* LIMIT 10 */ still comment */");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_dollar_quoted_string() {
        let a = analyze_limit("SELECT * FROM users WHERE body = $$LIMIT 10$$");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_tagged_dollar_quote() {
        let a = analyze_limit("SELECT * FROM t WHERE body = $fn$LIMIT 10$fn$");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_quoted_identifier() {
        let a = analyze_limit("SELECT * FROM \"LIMIT\" WHERE id = 1");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_fetch_first() {
        let a = analyze_limit("SELECT * FROM users FETCH FIRST 10 ROWS ONLY");
        assert!(a.has_outer_limit);
    }

    #[test]
    fn test_fetch_next() {
        let a = analyze_limit("SELECT * FROM users FETCH NEXT 10 ROWS ONLY");
        assert!(a.has_outer_limit);
    }

    #[test]
    fn test_fetch_inside_subquery() {
        let a = analyze_limit("SELECT * FROM (SELECT * FROM users FETCH FIRST 10 ROWS ONLY) sub");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_empty_input() {
        let a = analyze_limit("");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_whitespace_only() {
        let a = analyze_limit("   \n\t  ");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_union_no_outer_limit() {
        let a = analyze_limit("SELECT * FROM a UNION ALL SELECT * FROM b");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_union_with_outer_limit() {
        let a = analyze_limit("SELECT * FROM a UNION ALL SELECT * FROM b LIMIT 10");
        assert!(a.has_outer_limit);
    }

    #[test]
    fn test_escaped_string_literal() {
        let a = analyze_limit("SELECT * FROM t WHERE name = 'it''s a LIMIT'");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_complex_cte_no_outer_limit() {
        let sql = "WITH
            active AS (SELECT * FROM users WHERE active LIMIT 100),
            recent AS (SELECT * FROM orders WHERE date > now() - interval '1 day')
            SELECT a.*, r.* FROM active a JOIN recent r ON a.id = r.user_id";
        let a = analyze_limit(sql);
        assert!(a.can_paginate());
    }

    #[test]
    fn test_semicolon_terminated() {
        let a = analyze_limit("SELECT * FROM users LIMIT 10;");
        assert!(a.has_outer_limit);
    }

    #[test]
    fn test_explain_analyze() {
        let a = analyze_limit("EXPLAIN ANALYZE SELECT * FROM users");
        assert!(a.can_paginate());
    }

    #[test]
    fn test_order_by_no_limit() {
        let a = analyze_limit("SELECT * FROM users ORDER BY name");
        assert!(a.can_paginate());
    }
}

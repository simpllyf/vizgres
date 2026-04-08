//! SQL analysis utilities
//!
//! Pure functions for analyzing SQL text: transaction intent detection,
//! destructive query classification, write detection, psql meta-command
//! translation, and error position mapping.

use super::TransactionState;

/// Detect the transaction intent of a SQL statement by looking at the first keyword.
/// Returns the new TransactionState if the query changes it, or None if no change.
pub(super) fn detect_transaction_intent(sql: &str) -> Option<TransactionState> {
    let trimmed = sql.trim();
    // Find the first word (case-insensitive)
    let first_word = trimmed
        .split_whitespace()
        .next()?
        .trim_end_matches(';')
        .to_uppercase();
    match first_word.as_str() {
        "BEGIN" | "START" => Some(TransactionState::InTransaction),
        "COMMIT" | "END" => Some(TransactionState::Idle),
        "ROLLBACK" | "ABORT" => Some(TransactionState::Idle),
        _ => None,
    }
}

/// Check if a SQL statement is destructive and return a label describing the operation.
/// Returns None if the query is safe, or Some("LABEL") for destructive queries.
pub(super) fn is_destructive_query(sql: &str) -> Option<&'static str> {
    let trimmed = sql.trim();
    // Normalize to uppercase for matching, but only the prefix we need
    let upper: String = trimmed.chars().take(200).collect::<String>().to_uppercase();

    if upper.starts_with("DROP TABLE")
        || upper.starts_with("DROP INDEX")
        || upper.starts_with("DROP SCHEMA")
        || upper.starts_with("DROP DATABASE")
        || upper.starts_with("DROP VIEW")
        || upper.starts_with("DROP MATERIALIZED VIEW")
        || upper.starts_with("DROP FUNCTION")
        || upper.starts_with("DROP PROCEDURE")
        || upper.starts_with("DROP TRIGGER")
        || upper.starts_with("DROP SEQUENCE")
        || upper.starts_with("DROP TYPE")
        || upper.starts_with("DROP EXTENSION")
        || upper.starts_with("DROP ROLE")
        || upper.starts_with("DROP USER")
    {
        return Some("DROP");
    }

    if upper.starts_with("TRUNCATE") {
        return Some("TRUNCATE");
    }

    // DELETE without WHERE
    if upper.starts_with("DELETE") && !upper.contains("WHERE") {
        return Some("DELETE without WHERE");
    }

    // ALTER TABLE ... DROP (column, constraint, etc.)
    if upper.starts_with("ALTER TABLE") && upper.contains(" DROP ") {
        return Some("ALTER TABLE DROP");
    }

    None
}

/// Check if a SQL statement is a write operation that should be blocked in read-only mode.
/// Returns None for read-only queries (SELECT, EXPLAIN, SHOW, etc.),
/// or Some("LABEL") for write operations.
pub(super) fn is_write_query(sql: &str) -> Option<&'static str> {
    let trimmed = sql.trim();
    let upper: String = trimmed.chars().take(200).collect::<String>().to_uppercase();

    if upper.starts_with("INSERT") {
        return Some("INSERT");
    }
    if upper.starts_with("UPDATE") {
        return Some("UPDATE");
    }
    if upper.starts_with("DELETE") {
        return Some("DELETE");
    }
    if upper.starts_with("CREATE") {
        return Some("CREATE");
    }
    if upper.starts_with("ALTER") {
        return Some("ALTER");
    }
    if upper.starts_with("DROP") {
        return Some("DROP");
    }
    if upper.starts_with("TRUNCATE") {
        return Some("TRUNCATE");
    }
    if upper.starts_with("GRANT") || upper.starts_with("REVOKE") {
        return Some("GRANT/REVOKE");
    }
    if upper.starts_with("COMMENT") {
        return Some("COMMENT");
    }
    None
}

/// Translate psql-style meta-commands to equivalent SQL queries.
/// Returns Some(sql) if the input is a recognized meta-command, None otherwise.
pub(super) fn translate_meta_command(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if !trimmed.starts_with('\\') {
        return None;
    }

    // Split into command and optional argument
    let (cmd, arg) = match trimmed.find(char::is_whitespace) {
        Some(pos) => (&trimmed[..pos], Some(trimmed[pos..].trim())),
        None => (trimmed, None),
    };

    match cmd {
        "\\dt" => Some(
            "SELECT n.nspname AS schema, c.relname AS name, \
             pg_catalog.pg_get_userbyid(c.relowner) AS owner \
             FROM pg_catalog.pg_class c \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relkind = 'r' \
             AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
             ORDER BY schema, name"
                .to_string(),
        ),
        "\\dv" => Some(
            "SELECT n.nspname AS schema, c.relname AS name, \
             CASE c.relkind WHEN 'v' THEN 'view' WHEN 'm' THEN 'materialized view' END AS type \
             FROM pg_catalog.pg_class c \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relkind IN ('v', 'm') \
             AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
             ORDER BY schema, name"
                .to_string(),
        ),
        "\\di" => Some(
            "SELECT n.nspname AS schema, ci.relname AS name, \
             ct.relname AS tbl, \
             am.amname AS method \
             FROM pg_catalog.pg_index ix \
             JOIN pg_catalog.pg_class ci ON ci.oid = ix.indexrelid \
             JOIN pg_catalog.pg_class ct ON ct.oid = ix.indrelid \
             JOIN pg_catalog.pg_namespace n ON n.oid = ci.relnamespace \
             JOIN pg_catalog.pg_am am ON am.oid = ci.relam \
             WHERE n.nspname NOT IN ('pg_catalog', 'information_schema') \
             ORDER BY schema, tbl, name"
                .to_string(),
        ),
        "\\dn" => Some(
            "SELECT n.nspname AS name, \
             pg_catalog.pg_get_userbyid(n.nspowner) AS owner \
             FROM pg_catalog.pg_namespace n \
             WHERE n.nspname NOT LIKE 'pg_%' \
             AND n.nspname <> 'information_schema' \
             ORDER BY name"
                .to_string(),
        ),
        "\\d" => translate_describe_table(arg?),
        _ => None,
    }
}

/// Build the SQL for `\d table` — expanded to match psql output.
///
/// Shows columns, indexes, constraints (CHECK, FK, UNIQUE), referenced-by
/// relationships, and triggers as a unified result set with a "section" column.
fn translate_describe_table(table_arg: &str) -> Option<String> {
    let table = table_arg.trim();
    if table.is_empty() {
        return None;
    }
    // Validate: only alphanumeric, underscore, dot (for schema.table)
    if !table
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
    {
        return None;
    }
    // Parse schema.table or just table
    let (schema_filter, table_name) = if let Some(dot) = table.find('.') {
        let s = &table[..dot];
        let t = &table[dot + 1..];
        if s.is_empty() || t.is_empty() || t.contains('.') {
            return None;
        }
        (format!("AND n.nspname = '{}'", s), t.to_string())
    } else {
        (
            "AND n.nspname NOT IN ('pg_catalog', 'information_schema')".to_string(),
            table.to_string(),
        )
    };

    // CTE to resolve the table OID once, then reuse across all sections
    Some(format!(
        "WITH tbl AS (\
            SELECT c.oid, n.nspname, c.relname \
            FROM pg_catalog.pg_class c \
            JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
            WHERE c.relname = '{table_name}' {schema_filter} \
            LIMIT 1\
         ) \
         SELECT section, name, definition FROM (\
            SELECT 1 AS section_order, 'Column' AS section, \
                   a.attname AS name, \
                   pg_catalog.format_type(a.atttypid, a.atttypmod) \
                   || CASE WHEN a.attnotnull THEN ' NOT NULL' ELSE '' END \
                   || CASE WHEN d.adbin IS NOT NULL \
                           THEN ' DEFAULT ' || pg_catalog.pg_get_expr(d.adbin, d.adrelid) \
                           ELSE '' END AS definition, \
                   a.attnum AS sub_order \
            FROM tbl \
            JOIN pg_catalog.pg_attribute a ON a.attrelid = tbl.oid \
            LEFT JOIN pg_catalog.pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
            WHERE a.attnum > 0 AND NOT a.attisdropped \
         UNION ALL \
            SELECT 2, 'Index', ci.relname, \
                   pg_catalog.pg_get_indexdef(ix.indexrelid), \
                   0 \
            FROM tbl \
            JOIN pg_catalog.pg_index ix ON ix.indrelid = tbl.oid \
            JOIN pg_catalog.pg_class ci ON ci.oid = ix.indexrelid \
         UNION ALL \
            SELECT 3, \
                   CASE con.contype \
                       WHEN 'c' THEN 'Check' \
                       WHEN 'f' THEN 'FK' \
                       WHEN 'u' THEN 'Unique' \
                       WHEN 'p' THEN 'PK' \
                       WHEN 'x' THEN 'Exclusion' \
                   END, \
                   con.conname, \
                   pg_catalog.pg_get_constraintdef(con.oid, true), \
                   0 \
            FROM tbl \
            JOIN pg_catalog.pg_constraint con ON con.conrelid = tbl.oid \
         UNION ALL \
            SELECT 4, 'Referenced by', \
                   ref_n.nspname || '.' || ref_c.relname, \
                   pg_catalog.pg_get_constraintdef(con.oid, true), \
                   0 \
            FROM tbl \
            JOIN pg_catalog.pg_constraint con ON con.confrelid = tbl.oid AND con.contype = 'f' \
            JOIN pg_catalog.pg_class ref_c ON ref_c.oid = con.conrelid \
            JOIN pg_catalog.pg_namespace ref_n ON ref_n.oid = ref_c.relnamespace \
         UNION ALL \
            SELECT 5, 'Trigger', t.tgname, \
                   pg_catalog.pg_get_triggerdef(t.oid, true), \
                   0 \
            FROM tbl \
            JOIN pg_catalog.pg_trigger t ON t.tgrelid = tbl.oid \
            WHERE NOT t.tgisinternal \
         ) sub \
         ORDER BY section_order, sub_order",
    ))
}

/// Convert a 1-based byte offset (from PostgreSQL error) to (line, col) position.
/// PostgreSQL positions are 1-indexed, so we subtract 1 to get 0-indexed offset.
pub(super) fn byte_offset_to_position(content: &str, offset: u32) -> (usize, usize) {
    let offset = (offset.saturating_sub(1)) as usize; // Convert 1-indexed to 0-indexed
    let mut line = 0;
    let mut col = 0;
    for (i, ch) in content.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

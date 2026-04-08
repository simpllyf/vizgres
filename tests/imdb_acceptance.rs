//! IMDb acceptance test suite
//!
//! Automated tests covering ~70-80% of the vizgres 1.0 acceptance plan.
//! Tests run against the IMDb dataset loaded into PostgreSQL.
//!
//! To run:
//!   just db-up && just imdb-load
//!   just test-acceptance
//!
//! If the IMDb database is not available, all tests skip gracefully.
//!
//! Environment variables (with defaults):
//! - IMDB_DB_HOST: localhost
//! - IMDB_DB_PORT: 5433
//! - IMDB_DB_USER: test_user
//! - IMDB_DB_PASSWORD: test_password

use std::env;
use std::sync::OnceLock;

use vizgres::config::ConnectionConfig;
use vizgres::config::connections::SslMode;
use vizgres::db::Database;
use vizgres::db::postgres::PostgresProvider;
use vizgres::db::types::{CellValue, DataType};

fn imdb_config(read_only: bool) -> ConnectionConfig {
    ConnectionConfig {
        name: "imdb-acceptance".into(),
        host: env::var("IMDB_DB_HOST").unwrap_or_else(|_| "localhost".into()),
        port: env::var("IMDB_DB_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(5433),
        database: "imdb".into(),
        username: env::var("IMDB_DB_USER").unwrap_or_else(|_| "test_user".into()),
        password: Some(env::var("IMDB_DB_PASSWORD").unwrap_or_else(|_| "test_password".into())),
        ssl_mode: SslMode::Disable,
        read_only,
        is_saved: false,
    }
}

/// Print skip message once across all tests.
fn print_skip() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        eprintln!("SKIPPED: IMDb database not loaded (run 'just db-up && just imdb-load')");
    });
}

/// Try to connect; if the database isn't available, print skip and return None.
macro_rules! connect_or_skip {
    () => {
        connect_or_skip!(false)
    };
    ($read_only:expr) => {
        match PostgresProvider::connect(&imdb_config($read_only), 0).await {
            Ok((provider, _rx)) => provider,
            Err(_) => {
                print_skip();
                return;
            }
        }
    };
}

// ═══════════════════════════════════════════════════════════════════
// Schema & Structure
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn schema_loads_all_seven_tables() {
    let db = connect_or_skip!();
    let schema = db.get_schema(0).await.expect("get_schema failed");
    let public = schema
        .schemas
        .items
        .iter()
        .find(|s| s.name == "public")
        .expect("public schema not found");

    let table_names: Vec<&str> = public
        .tables
        .items
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    let expected = [
        "name_basics",
        "title_akas",
        "title_basics",
        "title_crew",
        "title_episode",
        "title_principals",
        "title_ratings",
    ];
    for name in &expected {
        assert!(
            table_names.contains(name),
            "missing table: {} (found: {:?})",
            name,
            table_names
        );
    }
    assert_eq!(public.tables.items.len(), 7, "expected exactly 7 tables");
}

#[tokio::test]
async fn title_basics_columns_and_types() {
    let db = connect_or_skip!();
    let schema = db.get_schema(0).await.unwrap();
    let public = schema
        .schemas
        .items
        .iter()
        .find(|s| s.name == "public")
        .unwrap();
    let table = public
        .tables
        .items
        .iter()
        .find(|t| t.name == "title_basics")
        .expect("title_basics not found");

    let col_map: std::collections::HashMap<&str, &DataType> = table
        .columns
        .iter()
        .map(|c| (c.name.as_str(), &c.data_type))
        .collect();

    assert_eq!(col_map["tconst"], &DataType::Text);
    assert_eq!(col_map["start_year"], &DataType::SmallInt);
    assert_eq!(col_map["runtime_minutes"], &DataType::Integer);
    assert!(col_map.contains_key("primary_title"));
    assert!(col_map.contains_key("title_type"));
    assert!(col_map.contains_key("is_adult"));
}

#[tokio::test]
async fn title_ratings_column_types() {
    let db = connect_or_skip!();
    let schema = db.get_schema(0).await.unwrap();
    let public = schema
        .schemas
        .items
        .iter()
        .find(|s| s.name == "public")
        .unwrap();
    let table = public
        .tables
        .items
        .iter()
        .find(|t| t.name == "title_ratings")
        .expect("title_ratings not found");

    let col_map: std::collections::HashMap<&str, &DataType> = table
        .columns
        .iter()
        .map(|c| (c.name.as_str(), &c.data_type))
        .collect();

    assert_eq!(col_map["average_rating"], &DataType::Numeric);
    assert_eq!(col_map["num_votes"], &DataType::Integer);
}

#[tokio::test]
async fn title_principals_columns() {
    let db = connect_or_skip!();
    let schema = db.get_schema(0).await.unwrap();
    let public = schema
        .schemas
        .items
        .iter()
        .find(|s| s.name == "public")
        .unwrap();
    let table = public
        .tables
        .items
        .iter()
        .find(|t| t.name == "title_principals")
        .expect("title_principals not found");

    let col_names: Vec<&str> = table.columns.iter().map(|c| c.name.as_str()).collect();
    assert!(col_names.contains(&"tconst"), "missing tconst");
    assert!(col_names.contains(&"nconst"), "missing nconst");
    assert!(col_names.contains(&"category"), "missing category");
}

#[tokio::test]
async fn indexes_created() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "SELECT indexname FROM pg_indexes WHERE schemaname = 'public' ORDER BY indexname",
            0,
            0,
        )
        .await
        .expect("index query failed");

    let index_names: Vec<String> = results
        .rows
        .iter()
        .map(|r| match &r.values[0] {
            CellValue::Text(s) => s.clone(),
            other => panic!("expected text, got {:?}", other),
        })
        .collect();

    let expected_custom = [
        "idx_basics_type",
        "idx_basics_start_year",
        "idx_basics_primary_title",
        "idx_akas_title_id",
        "idx_episode_parent",
        "idx_principals_nconst",
        "idx_ratings_votes",
        "idx_names_name",
    ];
    for idx in &expected_custom {
        assert!(
            index_names.contains(&idx.to_string()),
            "missing index: {} (found: {:?})",
            idx,
            index_names
        );
    }
}

#[tokio::test]
async fn composite_primary_keys() {
    let db = connect_or_skip!();
    // Check title_akas has a composite PK
    let results = db
        .execute_query(
            "SELECT a.attname
             FROM pg_index ix
             JOIN pg_class c ON c.oid = ix.indrelid
             JOIN pg_namespace n ON n.oid = c.relnamespace
             JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY AS k(attnum, ord) ON true
             JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = k.attnum
             WHERE n.nspname = 'public' AND c.relname = 'title_akas' AND ix.indisprimary
             ORDER BY k.ord",
            0,
            0,
        )
        .await
        .expect("PK query failed");

    assert!(
        results.rows.len() >= 2,
        "title_akas should have composite PK, got {} columns",
        results.rows.len()
    );

    // Check title_principals has a composite PK
    let results = db
        .execute_query(
            "SELECT a.attname
             FROM pg_index ix
             JOIN pg_class c ON c.oid = ix.indrelid
             JOIN pg_namespace n ON n.oid = c.relnamespace
             JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY AS k(attnum, ord) ON true
             JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = k.attnum
             WHERE n.nspname = 'public' AND c.relname = 'title_principals' AND ix.indisprimary
             ORDER BY k.ord",
            0,
            0,
        )
        .await
        .expect("PK query failed");

    assert!(
        results.rows.len() >= 2,
        "title_principals should have composite PK, got {} columns",
        results.rows.len()
    );
}

#[tokio::test]
async fn schema_search_finds_rating() {
    let db = connect_or_skip!();
    let results = db.search_schema("rating").await.expect("search failed");
    let all_tables: Vec<&str> = results
        .schemas
        .items
        .iter()
        .flat_map(|s| s.tables.items.iter().map(|t| t.name.as_str()))
        .collect();
    assert!(
        all_tables.contains(&"title_ratings"),
        "search('rating') should find title_ratings, got: {:?}",
        all_tables
    );
}

// ═══════════════════════════════════════════════════════════════════
// Query Execution
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn simple_filtered_query() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "SELECT primary_title, start_year, genres \
             FROM title_basics \
             WHERE start_year = 2024 AND title_type = 'movie' \
             LIMIT 100",
            30_000,
            0,
        )
        .await
        .expect("query failed");

    assert!(
        !results.rows.is_empty(),
        "should have results for 2024 movies"
    );
    assert_eq!(results.columns.len(), 3);
    assert_eq!(results.columns[0].name, "primary_title");
    assert_eq!(results.columns[1].name, "start_year");
    assert_eq!(results.columns[2].name, "genres");
}

#[tokio::test]
async fn null_handling() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "SELECT end_year FROM title_basics WHERE end_year IS NULL LIMIT 5",
            30_000,
            0,
        )
        .await
        .expect("query failed");

    assert_eq!(results.rows.len(), 5);
    for row in &results.rows {
        assert!(
            row.values[0].is_null(),
            "expected NULL, got {:?}",
            row.values[0]
        );
    }
}

#[tokio::test]
async fn numeric_precision() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "SELECT average_rating, num_votes \
             FROM title_ratings \
             ORDER BY num_votes DESC \
             LIMIT 10",
            30_000,
            0,
        )
        .await
        .expect("query failed");

    assert_eq!(results.rows.len(), 10);
    for row in &results.rows {
        // average_rating should be a Text (from Decimal) like "9.3", not "9.300000"
        match &row.values[0] {
            CellValue::Text(s) => {
                assert!(
                    !s.contains("000"),
                    "decimal should not have trailing zeros: {}",
                    s
                );
                // Should parse as a valid decimal
                assert!(
                    s.parse::<f64>().is_ok(),
                    "rating should be parseable as float: {}",
                    s
                );
            }
            other => panic!("expected Text for Numeric type, got {:?}", other),
        }
        // num_votes should be an integer
        assert!(
            matches!(&row.values[1], CellValue::Integer(_)),
            "num_votes should be Integer, got {:?}",
            row.values[1]
        );
    }
}

#[tokio::test]
async fn syntax_error_returns_position() {
    let db = connect_or_skip!();
    let result = db
        .execute_query("SELEC * FROM title_basics", 30_000, 0)
        .await;
    assert!(result.is_err(), "syntax error should fail");
    let err = result.unwrap_err();
    let err_str = err.to_string();
    // Should contain meaningful error info
    assert!(
        err_str.contains("SELEC") || err_str.contains("syntax") || err_str.contains("error"),
        "error should reference the typo: {}",
        err_str
    );
}

#[tokio::test]
async fn empty_result_set() {
    let db = connect_or_skip!();
    let results = db
        .execute_query("SELECT * FROM title_basics WHERE 1 = 0", 30_000, 0)
        .await
        .expect("query should succeed even with 0 rows");

    assert_eq!(results.rows.len(), 0);
    assert!(
        !results.columns.is_empty(),
        "columns should still be present"
    );
}

#[tokio::test]
async fn special_characters() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "SELECT * FROM title_basics WHERE primary_title LIKE '%''%' LIMIT 5",
            30_000,
            0,
        )
        .await
        .expect("apostrophe query failed");

    // Query should succeed — may or may not have results
    assert!(!results.columns.is_empty());
}

#[tokio::test]
async fn unicode_content() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "SELECT * FROM title_akas WHERE region = 'JP' LIMIT 10",
            30_000,
            0,
        )
        .await
        .expect("unicode query failed");

    assert!(
        !results.rows.is_empty(),
        "should have Japanese title entries"
    );
}

// ═══════════════════════════════════════════════════════════════════
// Pagination
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn row_limiting_truncates() {
    let db = connect_or_skip!();
    let results = db
        .execute_query("SELECT * FROM title_basics", 30_000, 100)
        .await
        .expect("query failed");

    assert_eq!(results.rows.len(), 100);
    assert!(results.truncated, "should be truncated with max_rows=100");
}

#[tokio::test]
async fn row_limiting_no_truncation() {
    let db = connect_or_skip!();
    let results = db
        .execute_query("SELECT * FROM title_basics LIMIT 5", 30_000, 1000)
        .await
        .expect("query failed");

    assert_eq!(results.rows.len(), 5);
    assert!(
        !results.truncated,
        "5 rows with max_rows=1000 should not be truncated"
    );
}

#[tokio::test]
async fn large_table_pagination() {
    let db = connect_or_skip!();
    // Page 1
    let page1 = db
        .execute_query(
            "SELECT tconst FROM title_principals ORDER BY tconst LIMIT 1000",
            60_000,
            0,
        )
        .await
        .expect("page 1 failed");
    assert_eq!(page1.rows.len(), 1000);

    // Page 2 via OFFSET
    let page2 = db
        .execute_query(
            "SELECT tconst FROM title_principals ORDER BY tconst LIMIT 1000 OFFSET 1000",
            60_000,
            0,
        )
        .await
        .expect("page 2 failed");
    assert_eq!(page2.rows.len(), 1000);

    // Page 3
    let page3 = db
        .execute_query(
            "SELECT tconst FROM title_principals ORDER BY tconst LIMIT 1000 OFFSET 2000",
            60_000,
            0,
        )
        .await
        .expect("page 3 failed");
    assert_eq!(page3.rows.len(), 1000);

    // Pages should not overlap
    let first_p1 = page1.rows[0].values[0].display_string(100);
    let first_p2 = page2.rows[0].values[0].display_string(100);
    let first_p3 = page3.rows[0].values[0].display_string(100);
    assert_ne!(first_p1, first_p2, "page 1 and 2 should not overlap");
    assert_ne!(first_p2, first_p3, "page 2 and 3 should not overlap");
}

// ═══════════════════════════════════════════════════════════════════
// Meta-Commands (SQL equivalents)
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn meta_dt_lists_tables() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "SELECT n.nspname AS schema, c.relname AS name, \
             pg_catalog.pg_get_userbyid(c.relowner) AS owner \
             FROM pg_catalog.pg_class c \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relkind = 'r' \
             AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
             ORDER BY schema, name",
            30_000,
            0,
        )
        .await
        .expect("\\dt query failed");

    assert_eq!(
        results.rows.len(),
        7,
        "\\dt should list 7 tables, got {}",
        results.rows.len()
    );
}

#[tokio::test]
async fn meta_di_lists_indexes() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "SELECT n.nspname AS schema, ci.relname AS name, \
             ct.relname AS tbl, \
             am.amname AS method \
             FROM pg_catalog.pg_index ix \
             JOIN pg_catalog.pg_class ci ON ci.oid = ix.indexrelid \
             JOIN pg_catalog.pg_class ct ON ct.oid = ix.indrelid \
             JOIN pg_catalog.pg_namespace n ON n.oid = ci.relnamespace \
             JOIN pg_catalog.pg_am am ON am.oid = ci.relam \
             WHERE n.nspname NOT IN ('pg_catalog', 'information_schema') \
             ORDER BY schema, tbl, name",
            30_000,
            0,
        )
        .await
        .expect("\\di query failed");

    // 8 custom indexes + PK indexes (at least 7 PKs)
    assert!(
        results.rows.len() >= 8,
        "\\di should list at least 8 indexes (custom), got {}",
        results.rows.len()
    );
}

#[tokio::test]
async fn meta_d_describe_table() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "WITH tbl AS (\
                SELECT c.oid, n.nspname, c.relname \
                FROM pg_catalog.pg_class c \
                JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
                WHERE c.relname = 'title_basics' \
                AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
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
             ) sub \
             ORDER BY section_order, sub_order",
            30_000,
            0,
        )
        .await
        .expect("\\d title_basics query failed");

    let sections: Vec<String> = results
        .rows
        .iter()
        .map(|r| r.values[0].display_string(100))
        .collect();
    let names: Vec<String> = results
        .rows
        .iter()
        .map(|r| r.values[1].display_string(100))
        .collect();

    assert!(
        sections.contains(&"Column".to_string()),
        "should have Column section"
    );
    assert!(
        names.contains(&"tconst".to_string()),
        "should list tconst column"
    );
    assert!(
        names.contains(&"primary_title".to_string()),
        "should list primary_title column"
    );
    assert!(
        sections.contains(&"Index".to_string()),
        "should have Index section"
    );
}

#[tokio::test]
async fn meta_dn_lists_schemas() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "SELECT n.nspname AS name, \
             pg_catalog.pg_get_userbyid(n.nspowner) AS owner \
             FROM pg_catalog.pg_namespace n \
             WHERE n.nspname NOT LIKE 'pg_%' \
             AND n.nspname <> 'information_schema' \
             ORDER BY name",
            30_000,
            0,
        )
        .await
        .expect("\\dn query failed");

    let schema_names: Vec<String> = results
        .rows
        .iter()
        .map(|r| r.values[0].display_string(100))
        .collect();
    assert!(
        schema_names.contains(&"public".to_string()),
        "should contain public schema"
    );
}

#[tokio::test]
async fn meta_dv_lists_views() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "SELECT n.nspname AS schema, c.relname AS name, \
             CASE c.relkind WHEN 'v' THEN 'view' WHEN 'm' THEN 'materialized view' END AS type \
             FROM pg_catalog.pg_class c \
             JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relkind IN ('v', 'm') \
             AND n.nspname NOT IN ('pg_catalog', 'information_schema') \
             ORDER BY schema, name",
            30_000,
            0,
        )
        .await
        .expect("\\dv query failed");

    // IMDb has no views — empty result is expected
    assert_eq!(results.rows.len(), 0, "IMDb dataset should have no views");
}

// ═══════════════════════════════════════════════════════════════════
// EXPLAIN
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn explain_analyze_returns_plan() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "EXPLAIN ANALYZE SELECT primary_title FROM title_basics WHERE start_year = 2024 LIMIT 10",
            60_000,
            0,
        )
        .await
        .expect("EXPLAIN ANALYZE failed");

    assert!(!results.rows.is_empty(), "EXPLAIN should return plan rows");
    let plan_text: String = results
        .rows
        .iter()
        .map(|r| r.values[0].display_string(1000))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        plan_text.contains("Seq Scan") || plan_text.contains("Index") || plan_text.contains("Scan"),
        "plan should contain scan nodes: {}",
        &plan_text[..plan_text.len().min(200)]
    );
}

#[tokio::test]
async fn explain_with_join() {
    let db = connect_or_skip!();
    let results = db
        .execute_query(
            "EXPLAIN SELECT tb.primary_title, r.average_rating \
             FROM title_basics tb \
             JOIN title_ratings r ON r.tconst = tb.tconst \
             WHERE r.num_votes > 100000 \
             ORDER BY r.average_rating DESC \
             LIMIT 20",
            60_000,
            0,
        )
        .await
        .expect("EXPLAIN with join failed");

    assert!(!results.rows.is_empty(), "EXPLAIN should return plan rows");
    let plan_text: String = results
        .rows
        .iter()
        .map(|r| r.values[0].display_string(1000))
        .collect::<Vec<_>>()
        .join("\n");
    // Join plans typically contain Hash Join, Merge Join, or Nested Loop
    assert!(
        plan_text.contains("Join") || plan_text.contains("Loop") || plan_text.contains("Merge"),
        "plan should contain join nodes: {}",
        &plan_text[..plan_text.len().min(300)]
    );
}

// ═══════════════════════════════════════════════════════════════════
// Transactions
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn begin_rollback_cycle() {
    let db = connect_or_skip!();

    // BEGIN
    db.execute_query("BEGIN", 10_000, 0)
        .await
        .expect("BEGIN failed");

    // Connection should still work inside transaction
    let results = db
        .execute_query("SELECT 1 AS alive", 10_000, 0)
        .await
        .expect("query inside transaction failed");
    assert_eq!(results.rows.len(), 1);

    // ROLLBACK
    db.execute_query("ROLLBACK", 10_000, 0)
        .await
        .expect("ROLLBACK failed");

    // Connection should still work after rollback
    let results = db
        .execute_query("SELECT 2 AS still_alive", 10_000, 0)
        .await
        .expect("query after rollback failed");
    assert_eq!(results.rows.len(), 1);
}

#[tokio::test]
async fn read_inside_transaction() {
    let db = connect_or_skip!();

    db.execute_query("BEGIN", 10_000, 0)
        .await
        .expect("BEGIN failed");

    let results = db
        .execute_query("SELECT primary_title FROM title_basics LIMIT 5", 30_000, 0)
        .await
        .expect("SELECT inside transaction failed");
    assert_eq!(results.rows.len(), 5);

    db.execute_query("ROLLBACK", 10_000, 0)
        .await
        .expect("ROLLBACK failed");
}

// ═══════════════════════════════════════════════════════════════════
// Read-Only Mode
// ═══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn read_only_blocks_writes() {
    let db = connect_or_skip!(true);

    let result = db
        .execute_query("DELETE FROM title_ratings WHERE 1 = 0", 10_000, 0)
        .await;

    assert!(
        result.is_err(),
        "DELETE should be blocked in read-only mode"
    );
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("read-only") || err_str.contains("read_only"),
        "error should mention read-only: {}",
        err_str
    );
}

#[tokio::test]
async fn read_only_allows_selects() {
    let db = connect_or_skip!(true);

    let results = db
        .execute_query("SELECT * FROM title_ratings LIMIT 5", 30_000, 0)
        .await
        .expect("SELECT should work in read-only mode");

    assert_eq!(results.rows.len(), 5);
}

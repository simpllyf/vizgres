//! Integration tests for PostgresProvider
//!
//! These tests require the test PostgreSQL database to be running.
//! Start it with: docker-compose -f docker-compose.test.yml up -d

use vizgres::config::ConnectionConfig;
use vizgres::config::connections::SslMode;
use vizgres::db::Database;
use vizgres::db::postgres::PostgresProvider;
use vizgres::db::types::CellValue;

/// Get test database connection config
fn test_config() -> ConnectionConfig {
    ConnectionConfig {
        name: "integration-test".to_string(),
        host: std::env::var("TEST_DB_HOST").unwrap_or_else(|_| "localhost".to_string()),
        port: std::env::var("TEST_DB_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(5433),
        database: std::env::var("TEST_DB_NAME").unwrap_or_else(|_| "test_db".to_string()),
        username: std::env::var("TEST_DB_USER").unwrap_or_else(|_| "test_user".to_string()),
        password: Some(
            std::env::var("TEST_DB_PASSWORD").unwrap_or_else(|_| "test_password".to_string()),
        ),
        ssl_mode: SslMode::Disable,
    }
}

#[tokio::test]
async fn test_connect_to_database() {
    let config = test_config();
    let result = PostgresProvider::connect(&config, 0).await;

    match result {
        Ok(_) => {}
        Err(e) => {
            eprintln!(
                "Skipping test: Database not available at {}:{} - {}",
                config.host, config.port, e
            );
            return;
        }
    }
}

#[tokio::test]
async fn test_execute_simple_query() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    let results = provider
        .execute_query("SELECT 1 as num, 'hello' as msg", 0, 0)
        .await;
    assert!(results.is_ok(), "Query should succeed");

    let results = results.unwrap();
    assert_eq!(results.columns.len(), 2);
    assert_eq!(results.columns[0].name, "num");
    assert_eq!(results.columns[1].name, "msg");
    assert_eq!(results.rows.len(), 1);
    assert_eq!(results.row_count, 1);

    // Check values
    let row = &results.rows[0];
    match &row.values[0] {
        CellValue::Integer(n) => assert_eq!(*n, 1),
        other => panic!("Expected Integer, got {:?}", other),
    }
    match &row.values[1] {
        CellValue::Text(s) => assert_eq!(s, "hello"),
        other => panic!("Expected Text, got {:?}", other),
    }
}

#[tokio::test]
async fn test_query_users_table() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    let results = provider
        .execute_query(
            "SELECT id, name, email, active FROM users ORDER BY id",
            0,
            0,
        )
        .await;
    assert!(results.is_ok(), "Query should succeed: {:?}", results.err());

    let results = results.unwrap();
    assert_eq!(results.columns.len(), 4);
    assert!(results.row_count >= 4, "Should have at least 4 users");

    // Check first user (Alice)
    let first_row = &results.rows[0];
    match &first_row.values[1] {
        CellValue::Text(s) => assert_eq!(s, "Alice Smith"),
        other => panic!("Expected Text for name, got {:?}", other),
    }
    match &first_row.values[3] {
        CellValue::Boolean(b) => assert!(*b, "Alice should be active"),
        other => panic!("Expected Boolean for active, got {:?}", other),
    }
}

#[tokio::test]
async fn test_query_json_data() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    let results = provider
        .execute_query(
            "SELECT name, metadata FROM users WHERE metadata IS NOT NULL ORDER BY id",
            0,
            0,
        )
        .await;
    assert!(results.is_ok(), "Query should succeed");

    let results = results.unwrap();
    assert!(!results.rows.is_empty(), "Should have users with metadata");

    let first_row = &results.rows[0];
    match &first_row.values[1] {
        CellValue::Json(s) => {
            let v: serde_json::Value = serde_json::from_str(s).unwrap();
            assert!(v.is_object(), "Metadata should be a JSON object");
            assert!(v.get("role").is_some(), "Should have role field");
        }
        other => panic!("Expected Json for metadata, got {:?}", other),
    }
}

#[tokio::test]
async fn test_query_null_values() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    let results = provider
        .execute_query(
            "SELECT name, metadata FROM users WHERE metadata IS NULL",
            0,
            0,
        )
        .await;
    assert!(results.is_ok(), "Query should succeed");

    let results = results.unwrap();
    assert!(
        !results.rows.is_empty(),
        "Should have users with NULL metadata"
    );

    let first_row = &results.rows[0];
    assert!(first_row.values[1].is_null(), "Metadata should be NULL");
}

#[tokio::test]
async fn test_query_numeric_types() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    let results = provider
        .execute_query("SELECT id, amount FROM orders ORDER BY id LIMIT 1", 0, 0)
        .await;
    assert!(results.is_ok(), "Query should succeed");

    let results = results.unwrap();
    assert_eq!(results.row_count, 1);

    let row = &results.rows[0];
    match &row.values[0] {
        CellValue::Integer(_) => {}
        other => panic!("Expected Integer for id, got {:?}", other),
    }
    // NUMERIC is extracted via rust_decimal as a Text string
    match &row.values[1] {
        CellValue::Text(s) => {
            assert!(
                s.parse::<f64>().is_ok(),
                "Amount should be a valid number string, got: {}",
                s
            );
        }
        other => panic!("Expected Text for NUMERIC amount, got {:?}", other),
    }
}

#[tokio::test]
async fn test_query_timestamps() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    let results = provider
        .execute_query("SELECT created_at FROM users LIMIT 1", 0, 0)
        .await;
    assert!(results.is_ok(), "Query should succeed");

    let results = results.unwrap();
    let row = &results.rows[0];
    match &row.values[0] {
        CellValue::DateTime(s) => {
            assert!(!s.is_empty(), "DateTime should have value");
        }
        other => panic!("Expected DateTime for created_at, got {:?}", other),
    }
}

#[tokio::test]
async fn test_get_schema() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    let schema = provider.get_schema(0).await;
    assert!(
        schema.is_ok(),
        "Schema load should succeed: {:?}",
        schema.err()
    );

    let schema = schema.unwrap();
    assert!(
        !schema.schemas.is_empty(),
        "Should have at least one schema"
    );

    // Find public schema
    let public_schema = schema.schemas.iter().find(|s| s.name == "public");
    assert!(public_schema.is_some(), "Should have public schema");

    let public = public_schema.unwrap();
    assert!(
        !public.tables.is_empty(),
        "Public schema should have tables"
    );

    // Check for our test tables
    let table_names: Vec<&str> = public.tables.iter().map(|t| t.name.as_str()).collect();
    assert!(table_names.contains(&"users"), "Should have users table");
    assert!(table_names.contains(&"orders"), "Should have orders table");
    assert!(
        table_names.contains(&"products"),
        "Should have products table"
    );

    // Check users table columns
    let users_table = public.tables.iter().find(|t| t.name == "users").unwrap();
    let col_names: Vec<&str> = users_table
        .columns
        .iter()
        .map(|c| c.name.as_str())
        .collect();
    assert!(col_names.contains(&"id"), "Users should have id column");
    assert!(col_names.contains(&"name"), "Users should have name column");
    assert!(
        col_names.contains(&"email"),
        "Users should have email column"
    );
}

#[tokio::test]
async fn test_invalid_query() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    let results = provider
        .execute_query("SELECT * FROM nonexistent_table", 0, 0)
        .await;
    assert!(results.is_err(), "Invalid query should return error");
}

#[tokio::test]
async fn test_connection_failure() {
    let mut config = test_config();
    config.host = "invalid-host-that-does-not-exist.local".to_string();
    config.port = 59999;

    let result = PostgresProvider::connect(&config, 0).await;
    assert!(result.is_err(), "Should fail to connect to invalid host");
}

#[tokio::test]
async fn test_multiple_schemas() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    let schema = provider.get_schema(0).await.unwrap();

    // Should have both public and test_schema
    let schema_names: Vec<&str> = schema.schemas.iter().map(|s| s.name.as_str()).collect();
    assert!(
        schema_names.contains(&"public"),
        "Should have public schema"
    );
    assert!(
        schema_names.contains(&"test_schema"),
        "Should have test_schema"
    );

    // Check test_schema has settings table
    let test_schema = schema.schemas.iter().find(|s| s.name == "test_schema");
    assert!(test_schema.is_some());
    let test_schema = test_schema.unwrap();
    let table_names: Vec<&str> = test_schema.tables.iter().map(|t| t.name.as_str()).collect();
    assert!(
        table_names.contains(&"settings"),
        "test_schema should have settings table"
    );
}

#[tokio::test]
async fn test_query_array_types() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    let results = provider
        .execute_query(
            "SELECT name, tags FROM products WHERE tags IS NOT NULL ORDER BY id LIMIT 1",
            0,
            0,
        )
        .await;
    assert!(results.is_ok(), "Query should succeed: {:?}", results.err());

    let results = results.unwrap();
    let row = &results.rows[0];
    match &row.values[1] {
        CellValue::Array(items) => {
            assert!(!items.is_empty(), "Tags array should not be empty");
            match &items[0] {
                CellValue::Text(_) => {}
                other => panic!("Expected Text elements in array, got {:?}", other),
            }
        }
        other => panic!("Expected Array for tags, got {:?}", other),
    }
}

#[tokio::test]
async fn test_query_aggregation_numeric() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    let results = provider
        .execute_query("SELECT SUM(amount) as total FROM orders", 0, 0)
        .await;
    assert!(
        results.is_ok(),
        "Aggregation query should succeed: {:?}",
        results.err()
    );

    let results = results.unwrap();
    assert_eq!(results.row_count, 1);
    // SUM of NUMERIC returns NUMERIC, extracted as Text via rust_decimal
    match &results.rows[0].values[0] {
        CellValue::Text(s) => {
            assert!(
                s.parse::<f64>().is_ok(),
                "SUM should be a valid number, got: {}",
                s
            );
        }
        other => panic!("Expected Text for SUM(numeric), got {:?}", other),
    }
}

#[tokio::test]
async fn test_row_limiting_truncates_results() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    // First, get all users to know the total count
    let all_results = provider
        .execute_query("SELECT id FROM users ORDER BY id", 0, 0)
        .await
        .unwrap();
    let total_users = all_results.row_count;
    assert!(
        total_users >= 4,
        "Should have at least 4 users for this test"
    );
    assert!(
        !all_results.truncated,
        "Unlimited query should not be truncated"
    );

    // Now query with a limit smaller than total users
    let limited_results = provider
        .execute_query("SELECT id FROM users ORDER BY id", 0, 2)
        .await
        .unwrap();

    assert_eq!(limited_results.row_count, 2, "Should return exactly 2 rows");
    assert_eq!(limited_results.rows.len(), 2, "Should have 2 rows in vec");
    assert!(
        limited_results.truncated,
        "Results should be marked as truncated"
    );
}

#[tokio::test]
async fn test_row_limiting_no_truncation_when_within_limit() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    // Query with a limit larger than expected results
    let results = provider
        .execute_query("SELECT id FROM users ORDER BY id LIMIT 2", 0, 1000)
        .await
        .unwrap();

    assert_eq!(results.row_count, 2, "Should return 2 rows");
    assert!(
        !results.truncated,
        "Results should not be truncated when within limit"
    );
}

#[tokio::test]
async fn test_row_limiting_zero_means_unlimited() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    // Query all users with max_rows=0 (unlimited)
    let results = provider
        .execute_query("SELECT id FROM users", 0, 0)
        .await
        .unwrap();

    assert!(results.row_count >= 4, "Should have all users");
    assert!(
        !results.truncated,
        "Unlimited query should never be truncated"
    );
}

#[tokio::test]
async fn test_search_schema_finds_tables() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    // Search for "user" should find the users table
    let results = provider.search_schema("user").await;
    assert!(
        results.is_ok(),
        "Search should succeed: {:?}",
        results.err()
    );

    let schema_tree = results.unwrap();
    assert!(
        !schema_tree.schemas.is_empty(),
        "Should find matching schemas"
    );

    // Find public schema
    let public = schema_tree.schemas.iter().find(|s| s.name == "public");
    assert!(public.is_some(), "Should find public schema");

    let public = public.unwrap();
    // Should find 'users' table
    assert!(
        public.tables.iter().any(|t| t.name == "users"),
        "Should find users table"
    );
}

#[tokio::test]
async fn test_search_schema_finds_columns() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    // Search for "email" should find tables with email columns
    let results = provider.search_schema("email").await;
    assert!(results.is_ok(), "Search should succeed");

    let schema_tree = results.unwrap();
    // Should find tables that have 'email' column
    let has_email_col = schema_tree.schemas.iter().any(|s| {
        s.tables.iter().any(|t| {
            t.columns
                .iter()
                .any(|c| c.name.to_lowercase().contains("email"))
        })
    });
    assert!(has_email_col, "Should find tables with email column");
}

#[tokio::test]
async fn test_search_schema_no_results() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    // Search for something that doesn't exist
    let results = provider.search_schema("xyzzyznonexistent12345").await;
    assert!(
        results.is_ok(),
        "Search should succeed even with no results"
    );

    let schema_tree = results.unwrap();
    assert!(
        schema_tree.schemas.is_empty(),
        "Should find no matching schemas"
    );
}

#[tokio::test]
async fn test_search_schema_special_characters() {
    let config = test_config();
    let provider = match PostgresProvider::connect(&config, 0).await {
        Ok((p, _)) => p,
        Err(_) => {
            eprintln!("Skipping test: Database not available");
            return;
        }
    };

    // Search with special LIKE characters should not cause errors
    let results = provider.search_schema("%_\\").await;
    assert!(
        results.is_ok(),
        "Search with special chars should not error"
    );
}

//! Common test utilities and helpers
//!
//! Shared test infrastructure for integration and unit tests.

use vizgres::config::ConnectionConfig;
use vizgres::config::connections::SslMode;
use vizgres::db::schema::{Column, PaginatedVec, Schema, SchemaTree, Table};
use vizgres::db::types::DataType;

/// Create a standard test schema for consistent testing
pub fn test_schema() -> SchemaTree {
    SchemaTree {
        schemas: PaginatedVec::from_vec(vec![Schema {
            name: "public".to_string(),
            tables: PaginatedVec::from_vec(vec![
                Table {
                    name: "users".to_string(),
                    row_count: Some(100),
                    columns: vec![
                        Column {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: true,
                            foreign_key: None,
                        },
                        Column {
                            name: "name".to_string(),
                            data_type: DataType::Text,
                            is_primary_key: false,
                            foreign_key: None,
                        },
                        Column {
                            name: "email".to_string(),
                            data_type: DataType::Varchar(Some(255)),
                            is_primary_key: false,
                            foreign_key: None,
                        },
                        Column {
                            name: "active".to_string(),
                            data_type: DataType::Boolean,
                            is_primary_key: false,
                            foreign_key: None,
                        },
                    ],
                },
                Table {
                    name: "orders".to_string(),
                    row_count: Some(50),
                    columns: vec![
                        Column {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: true,
                            foreign_key: None,
                        },
                        Column {
                            name: "user_id".to_string(),
                            data_type: DataType::Integer,
                            is_primary_key: false,
                            foreign_key: None,
                        },
                    ],
                },
            ]),
            views: PaginatedVec::default(),
            indexes: PaginatedVec::default(),
            functions: PaginatedVec::default(),
        }]),
    }
}

/// Create a test connection configuration
pub fn test_connection_config() -> ConnectionConfig {
    ConnectionConfig {
        name: "test".to_string(),
        host: "localhost".to_string(),
        port: 5432,
        database: "test_db".to_string(),
        username: "test_user".to_string(),
        password: Some("test_password".to_string()),
        ssl_mode: SslMode::Disable,
        read_only: false,
        is_saved: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_has_tables() {
        let schema = test_schema();
        assert_eq!(schema.schemas.len(), 1);
        assert_eq!(schema.schemas.items[0].tables.len(), 2);
        assert!(!schema.schemas.items[0].tables.is_truncated());
    }

    #[test]
    fn test_connection_config_creation() {
        let config = test_connection_config();
        assert_eq!(config.name, "test");
        assert_eq!(config.host, "localhost");
    }
}

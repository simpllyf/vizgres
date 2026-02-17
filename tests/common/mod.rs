//! Common test utilities and helpers
//!
//! Shared test infrastructure for integration and unit tests.

use vizgres::config::ConnectionConfig;
use vizgres::config::connections::SslMode;
use vizgres::db::schema::{Column, Schema, SchemaTree, Table};
use vizgres::db::types::DataType;

/// Create a standard test schema for consistent testing
pub fn test_schema() -> SchemaTree {
    SchemaTree {
        schemas: vec![Schema {
            name: "public".to_string(),
            tables: vec![
                Table {
                    name: "users".to_string(),
                    columns: vec![
                        Column {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                        },
                        Column {
                            name: "name".to_string(),
                            data_type: DataType::Text,
                        },
                        Column {
                            name: "email".to_string(),
                            data_type: DataType::Varchar(Some(255)),
                        },
                        Column {
                            name: "active".to_string(),
                            data_type: DataType::Boolean,
                        },
                    ],
                },
                Table {
                    name: "orders".to_string(),
                    columns: vec![
                        Column {
                            name: "id".to_string(),
                            data_type: DataType::Integer,
                        },
                        Column {
                            name: "user_id".to_string(),
                            data_type: DataType::Integer,
                        },
                    ],
                },
            ],
        }],
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_has_tables() {
        let schema = test_schema();
        assert_eq!(schema.schemas.len(), 1);
        assert_eq!(schema.schemas[0].tables.len(), 2);
    }

    #[test]
    fn test_connection_config_creation() {
        let config = test_connection_config();
        assert_eq!(config.name, "test");
        assert_eq!(config.host, "localhost");
    }
}

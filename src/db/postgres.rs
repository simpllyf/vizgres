//! PostgreSQL database provider
//!
//! Concrete implementation using tokio-postgres. No trait abstraction needed yet.

use crate::config::ConnectionConfig;
use crate::config::connections::SslMode;
use crate::db::schema::{Column, Schema, SchemaTree, Table};
use crate::db::types::{CellValue, ColumnDef, DataType, QueryResults, Row};
use crate::error::DbResult;
use tokio_postgres::Client;
use tokio_postgres::types::Type;

/// PostgreSQL database provider
pub struct PostgresProvider {
    /// The tokio-postgres client
    client: Client,

    /// Cached schema tree (invalidated on refresh)
    schema_cache: Option<SchemaTree>,
}

impl PostgresProvider {
    /// Connect to a PostgreSQL database
    pub async fn connect(config: &ConnectionConfig) -> DbResult<Self> {
        let conn_string = config.connection_string_with_password();

        let client = match config.ssl_mode {
            SslMode::Disable => {
                let (client, connection) =
                    tokio_postgres::connect(&conn_string, tokio_postgres::NoTls)
                        .await
                        .map_err(|e| crate::error::DbError::ConnectionFailed(e.to_string()))?;
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("Database connection error: {}", e);
                    }
                });
                client
            }
            SslMode::Prefer | SslMode::Require => {
                let tls_config = make_tls_config();
                let tls = tokio_postgres_rustls::MakeRustlsConnect::new(tls_config);
                let (client, connection) = tokio_postgres::connect(&conn_string, tls)
                    .await
                    .map_err(|e| crate::error::DbError::ConnectionFailed(e.to_string()))?;
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("Database connection error: {}", e);
                    }
                });
                client
            }
        };

        Ok(Self {
            client,
            schema_cache: None,
        })
    }

    /// Execute a SQL query and return results
    pub async fn execute_query(&self, sql: &str) -> DbResult<QueryResults> {
        let start = std::time::Instant::now();

        let stmt = self
            .client
            .prepare(sql)
            .await
            .map_err(|e| crate::error::DbError::QueryFailed(e.to_string()))?;

        let columns: Vec<ColumnDef> = stmt
            .columns()
            .iter()
            .map(|col| ColumnDef {
                name: col.name().to_string(),
                data_type: pg_type_to_datatype(col.type_()),
                nullable: true,
            })
            .collect();

        let pg_rows = self
            .client
            .query(&stmt, &[])
            .await
            .map_err(|e| crate::error::DbError::QueryFailed(e.to_string()))?;

        let row_count = pg_rows.len();
        let mut rows = Vec::with_capacity(row_count);

        for pg_row in &pg_rows {
            let mut values = Vec::with_capacity(columns.len());
            for (i, col_def) in columns.iter().enumerate() {
                let value = extract_cell_value(pg_row, i, &col_def.data_type);
                values.push(value);
            }
            rows.push(Row { values });
        }

        Ok(QueryResults {
            columns,
            rows,
            execution_time: start.elapsed(),
            row_count,
        })
    }

    /// Get the complete database schema tree
    pub async fn get_schema(&mut self) -> DbResult<SchemaTree> {
        if let Some(ref cached) = self.schema_cache {
            return Ok(cached.clone());
        }

        let tree = self.fetch_schema_from_db().await?;
        self.schema_cache = Some(tree.clone());
        Ok(tree)
    }

    /// Invalidate the schema cache
    pub fn invalidate_cache(&mut self) {
        self.schema_cache = None;
    }

    async fn fetch_schema_from_db(&self) -> DbResult<SchemaTree> {
        // Get schemas (exclude pg_ internal schemas)
        let schema_rows = self
            .client
            .query(
                "SELECT schema_name FROM information_schema.schemata \
                 WHERE schema_name NOT LIKE 'pg_%' \
                 AND schema_name != 'information_schema' \
                 ORDER BY schema_name",
                &[],
            )
            .await
            .map_err(|e| crate::error::DbError::SchemaLoadFailed(e.to_string()))?;

        let mut schemas = Vec::new();

        for schema_row in &schema_rows {
            let schema_name: String = schema_row.get(0);

            // Get tables for this schema
            let table_rows = self
                .client
                .query(
                    "SELECT table_name FROM information_schema.tables \
                     WHERE table_schema = $1 AND table_type = 'BASE TABLE' \
                     ORDER BY table_name",
                    &[&schema_name],
                )
                .await
                .map_err(|e| crate::error::DbError::SchemaLoadFailed(e.to_string()))?;

            let mut tables = Vec::new();

            for table_row in &table_rows {
                let table_name: String = table_row.get(0);

                // Get columns for this table
                let col_rows = self
                    .client
                    .query(
                        "SELECT column_name, data_type \
                         FROM information_schema.columns \
                         WHERE table_schema = $1 AND table_name = $2 \
                         ORDER BY ordinal_position",
                        &[&schema_name, &table_name],
                    )
                    .await
                    .map_err(|e| crate::error::DbError::SchemaLoadFailed(e.to_string()))?;

                let columns: Vec<Column> = col_rows
                    .iter()
                    .map(|r| {
                        let col_name: String = r.get(0);
                        let type_name: String = r.get(1);

                        Column {
                            name: col_name,
                            data_type: datatype_from_info_schema(&type_name),
                        }
                    })
                    .collect();

                tables.push(Table {
                    name: table_name,
                    columns,
                });
            }

            schemas.push(Schema {
                name: schema_name,
                tables,
            });
        }

        Ok(SchemaTree { schemas })
    }
}

/// Map tokio_postgres Type to our DataType enum
fn pg_type_to_datatype(pg_type: &Type) -> DataType {
    match *pg_type {
        Type::INT2 => DataType::SmallInt,
        Type::INT4 => DataType::Integer,
        Type::INT8 => DataType::BigInt,
        Type::FLOAT4 => DataType::Real,
        Type::FLOAT8 => DataType::Double,
        Type::NUMERIC => DataType::Numeric,
        Type::TEXT | Type::NAME => DataType::Text,
        Type::VARCHAR => DataType::Varchar(None),
        Type::CHAR | Type::BPCHAR => DataType::Char(None),
        Type::BOOL => DataType::Boolean,
        Type::DATE => DataType::Date,
        Type::TIME => DataType::Time,
        Type::TIMESTAMP => DataType::Timestamp,
        Type::TIMESTAMPTZ => DataType::TimestampTz,
        Type::INTERVAL => DataType::Interval,
        Type::JSON => DataType::Json,
        Type::JSONB => DataType::Jsonb,
        Type::BYTEA => DataType::Bytea,
        Type::UUID => DataType::Uuid,
        _ => DataType::Unknown(pg_type.name().to_string()),
    }
}

/// Map information_schema type strings to our DataType enum
fn datatype_from_info_schema(type_name: &str) -> DataType {
    match type_name {
        "smallint" => DataType::SmallInt,
        "integer" => DataType::Integer,
        "bigint" => DataType::BigInt,
        "real" => DataType::Real,
        "double precision" => DataType::Double,
        "numeric" => DataType::Numeric,
        "text" => DataType::Text,
        "character varying" => DataType::Varchar(None),
        "character" => DataType::Char(None),
        "boolean" => DataType::Boolean,
        "date" => DataType::Date,
        "time without time zone" | "time with time zone" => DataType::Time,
        "timestamp without time zone" => DataType::Timestamp,
        "timestamp with time zone" => DataType::TimestampTz,
        "interval" => DataType::Interval,
        "json" => DataType::Json,
        "jsonb" => DataType::Jsonb,
        "bytea" => DataType::Bytea,
        "uuid" => DataType::Uuid,
        other => DataType::Unknown(other.to_string()),
    }
}

/// Build a rustls ClientConfig that trusts OS certificates (with Mozilla roots as fallback)
fn make_tls_config() -> rustls::ClientConfig {
    let mut root_store = rustls::RootCertStore::empty();

    let native_certs = rustls_native_certs::load_native_certs();
    let mut loaded = 0;
    for cert in native_certs.certs {
        if root_store.add(cert).is_ok() {
            loaded += 1;
        }
    }
    if loaded == 0 {
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth()
}

/// Extract a cell value from a tokio_postgres Row based on the column's DataType.
///
/// This function attempts to extract values using the expected type first,
/// then falls back to string representation if the type doesn't match.
/// Returns CellValue::Null only for actual NULL values or when all fallbacks fail.
fn extract_cell_value(row: &tokio_postgres::Row, idx: usize, data_type: &DataType) -> CellValue {
    match data_type {
        DataType::SmallInt => match row.try_get::<_, Option<i16>>(idx) {
            Ok(Some(v)) => CellValue::Integer(v as i64),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Integer => match row.try_get::<_, Option<i32>>(idx) {
            Ok(Some(v)) => CellValue::Integer(v as i64),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::BigInt => match row.try_get::<_, Option<i64>>(idx) {
            Ok(Some(v)) => CellValue::Integer(v),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Real => match row.try_get::<_, Option<f32>>(idx) {
            Ok(Some(v)) => CellValue::Float(v as f64),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Double | DataType::Numeric => match row.try_get::<_, Option<f64>>(idx) {
            Ok(Some(v)) => CellValue::Float(v),
            Ok(None) => CellValue::Null,
            Err(_) => {
                // Numeric might not map to f64 directly, try as string
                try_as_string(row, idx)
            }
        },
        DataType::Boolean => match row.try_get::<_, Option<bool>>(idx) {
            Ok(Some(v)) => CellValue::Boolean(v),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Json | DataType::Jsonb => {
            match row.try_get::<_, Option<serde_json::Value>>(idx) {
                Ok(Some(v)) => CellValue::Json(v),
                Ok(None) => CellValue::Null,
                Err(_) => try_as_string(row, idx),
            }
        }
        DataType::Bytea => match row.try_get::<_, Option<Vec<u8>>>(idx) {
            Ok(Some(v)) => CellValue::Binary(v),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Uuid => match row.try_get::<_, Option<String>>(idx) {
            Ok(Some(v)) => CellValue::Uuid(v),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Timestamp
        | DataType::TimestampTz
        | DataType::Date
        | DataType::Time
        | DataType::Interval => match row.try_get::<_, Option<String>>(idx) {
            Ok(Some(v)) => CellValue::DateTime(v),
            Ok(None) => CellValue::Null,
            Err(_) => {
                // Try chrono types for date/time columns
                if let Ok(Some(v)) = row.try_get::<_, Option<chrono::NaiveDateTime>>(idx) {
                    return CellValue::DateTime(v.to_string());
                }
                if let Ok(Some(v)) = row.try_get::<_, Option<chrono::DateTime<chrono::Utc>>>(idx) {
                    return CellValue::DateTime(v.to_string());
                }
                if let Ok(Some(v)) = row.try_get::<_, Option<chrono::NaiveDate>>(idx) {
                    return CellValue::DateTime(v.to_string());
                }
                if let Ok(Some(v)) = row.try_get::<_, Option<chrono::NaiveTime>>(idx) {
                    return CellValue::DateTime(v.to_string());
                }
                try_as_string(row, idx)
            }
        },
        // Text types and fallback for unknown types
        _ => try_as_string(row, idx),
    }
}

/// Try to extract a value as a string (fallback for type mismatches)
fn try_as_string(row: &tokio_postgres::Row, idx: usize) -> CellValue {
    match row.try_get::<_, Option<String>>(idx) {
        Ok(Some(v)) => CellValue::Text(v),
        Ok(None) => CellValue::Null,
        Err(_) => CellValue::Text("<unable to display>".to_string()),
    }
}

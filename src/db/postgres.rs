//! PostgreSQL database provider
//!
//! Concrete implementation using tokio-postgres.

use crate::config::ConnectionConfig;
use crate::config::connections::SslMode;
use crate::db::Database;
use crate::db::schema::{Column, ForeignKey, Function, Index, Schema, SchemaTree, Table};
use crate::db::types::{CellValue, ColumnDef, DataType, QueryResults, Row};
use crate::error::DbResult;
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tokio_postgres::Client;
use tokio_postgres::types::Type;

/// PostgreSQL database provider
pub struct PostgresProvider {
    /// The tokio-postgres client
    client: Client,
    /// Token for cancelling in-flight queries
    cancel_token: tokio_postgres::CancelToken,
    /// SSL mode (needed to cancel over the right transport)
    ssl_mode: SslMode,
}

impl PostgresProvider {
    /// Connect to a PostgreSQL database.
    ///
    /// Returns the provider and a receiver that fires if the background
    /// connection is lost (e.g. server restart, idle timeout).
    pub async fn connect(
        config: &ConnectionConfig,
    ) -> DbResult<(Self, mpsc::UnboundedReceiver<String>)> {
        let conn_string = config.connection_string_with_password();
        let (conn_err_tx, conn_err_rx) = mpsc::unbounded_channel();

        let client = match config.ssl_mode {
            SslMode::Disable => {
                let (client, connection) =
                    tokio_postgres::connect(&conn_string, tokio_postgres::NoTls)
                        .await
                        .map_err(|e| crate::error::DbError::ConnectionFailed(e.to_string()))?;
                tokio::spawn(async move {
                    if let Err(e) = connection.await {
                        let _ = conn_err_tx.send(format!("Connection lost: {}", e));
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
                        let _ = conn_err_tx.send(format!("Connection lost: {}", e));
                    }
                });
                client
            }
        };

        let cancel_token = client.cancel_token();
        let ssl_mode = config.ssl_mode;

        Ok((
            Self {
                client,
                cancel_token,
                ssl_mode,
            },
            conn_err_rx,
        ))
    }

    /// Send a cancel request for the currently running query.
    pub async fn cancel_query(&self) -> DbResult<()> {
        match self.ssl_mode {
            SslMode::Disable => self.cancel_token.cancel_query(tokio_postgres::NoTls).await,
            SslMode::Prefer | SslMode::Require => {
                let tls = tokio_postgres_rustls::MakeRustlsConnect::new(make_tls_config());
                self.cancel_token.cancel_query(tls).await
            }
        }
        .map_err(|e| crate::error::DbError::QueryFailed(format!("Cancel failed: {}", e)))
    }
}

impl Database for PostgresProvider {
    async fn execute_query(&self, sql: &str) -> DbResult<QueryResults> {
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

        Ok(QueryResults::new(columns, rows, start.elapsed(), row_count))
    }

    async fn get_schema(&self) -> DbResult<SchemaTree> {
        let map_err =
            |e: tokio_postgres::Error| crate::error::DbError::SchemaLoadFailed(e.to_string());

        // Query 1: Schemas (exclude pg_ internal and information_schema)
        let schema_rows = self
            .client
            .query(
                "SELECT nspname FROM pg_namespace \
                 WHERE nspname NOT LIKE 'pg_%' \
                 AND nspname != 'information_schema' \
                 ORDER BY nspname",
                &[],
            )
            .await
            .map_err(&map_err)?;

        let schema_names: Vec<String> = schema_rows.iter().map(|r| r.get(0)).collect();

        // Query 2: Tables + views + columns (relkind: r=table, v=view, m=materialized view)
        let rel_rows = self
            .client
            .query(
                "SELECT n.nspname, c.relname, c.relkind::text, \
                        a.attname, format_type(a.atttypid, a.atttypmod), a.attnum \
                 FROM pg_class c \
                 JOIN pg_namespace n ON n.oid = c.relnamespace \
                 JOIN pg_attribute a ON a.attrelid = c.oid \
                 WHERE c.relkind IN ('r','v','m') \
                   AND n.nspname NOT LIKE 'pg_%' \
                   AND n.nspname != 'information_schema' \
                   AND a.attnum > 0 AND NOT a.attisdropped \
                 ORDER BY n.nspname, c.relname, a.attnum",
                &[],
            )
            .await
            .map_err(&map_err)?;

        // Query 3: PK + FK constraints
        let constraint_rows = self
            .client
            .query(
                "SELECT n.nspname, c.relname, con.contype::text, \
                        a.attname, \
                        fn.nspname AS fk_schema, fc.relname AS fk_table, fa.attname AS fk_col \
                 FROM pg_constraint con \
                 JOIN pg_class c ON c.oid = con.conrelid \
                 JOIN pg_namespace n ON n.oid = c.relnamespace \
                 JOIN LATERAL unnest(con.conkey) WITH ORDINALITY AS u(attnum, ord) ON true \
                 JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = u.attnum \
                 LEFT JOIN pg_class fc ON fc.oid = con.confrelid \
                 LEFT JOIN pg_namespace fn ON fn.oid = fc.relnamespace \
                 LEFT JOIN LATERAL unnest(con.confkey) WITH ORDINALITY AS fu(attnum, ord) ON fu.ord = u.ord \
                 LEFT JOIN pg_attribute fa ON fa.attrelid = fc.oid AND fa.attnum = fu.attnum \
                 WHERE con.contype IN ('p', 'f') \
                   AND n.nspname NOT LIKE 'pg_%' \
                   AND n.nspname != 'information_schema' \
                 ORDER BY n.nspname, c.relname, con.contype, u.ord",
                &[],
            )
            .await
            .map_err(&map_err)?;

        // Query 4: Indexes (exclude primary key indexes — those show via constraints)
        let index_rows = self
            .client
            .query(
                "SELECT n.nspname, ct.relname AS table_name, ci.relname AS index_name, \
                        ix.indisunique, ix.indisprimary, \
                        array_agg(a.attname ORDER BY k.ord) AS columns \
                 FROM pg_index ix \
                 JOIN pg_class ci ON ci.oid = ix.indexrelid \
                 JOIN pg_class ct ON ct.oid = ix.indrelid \
                 JOIN pg_namespace n ON n.oid = ct.relnamespace \
                 JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY AS k(attnum, ord) ON true \
                 JOIN pg_attribute a ON a.attrelid = ct.oid AND a.attnum = k.attnum \
                 WHERE n.nspname NOT LIKE 'pg_%' \
                   AND n.nspname != 'information_schema' \
                   AND a.attnum > 0 \
                 GROUP BY n.nspname, ct.relname, ci.relname, ix.indisunique, ix.indisprimary \
                 ORDER BY n.nspname, ci.relname",
                &[],
            )
            .await
            .map_err(&map_err)?;

        // Query 5: Functions + procedures (exclude aggregates, window fns, internal)
        let func_rows = self
            .client
            .query(
                "SELECT n.nspname, p.proname, \
                        pg_get_function_identity_arguments(p.oid) AS args, \
                        pg_get_function_result(p.oid) AS return_type \
                 FROM pg_proc p \
                 JOIN pg_namespace n ON n.oid = p.pronamespace \
                 WHERE n.nspname NOT LIKE 'pg_%' \
                   AND n.nspname != 'information_schema' \
                   AND p.prokind IN ('f', 'p') \
                 ORDER BY n.nspname, p.proname",
                &[],
            )
            .await
            .map_err(&map_err)?;

        // ── Assembly ────────────────────────────────────────────

        // Build PK set: (schema, table, column) → true
        let mut pk_set: HashSet<(String, String, String)> = HashSet::new();
        // Build FK map: (schema, table, column) → ForeignKey
        let mut fk_map: HashMap<(String, String, String), ForeignKey> = HashMap::new();

        for row in &constraint_rows {
            let schema: String = row.get(0);
            let table: String = row.get(1);
            let contype: String = row.get(2);
            let col: String = row.get(3);

            if contype == "p" {
                pk_set.insert((schema, table, col));
            } else if contype == "f" {
                let fk_schema: String = row.get(4);
                let fk_table: String = row.get(5);
                let fk_col: String = row.get(6);
                let target_table = if fk_schema == schema {
                    fk_table
                } else {
                    format!("{}.{}", fk_schema, fk_table)
                };
                fk_map.insert(
                    (schema, table, col),
                    ForeignKey {
                        target_table,
                        target_column: fk_col,
                    },
                );
            }
        }

        // Build indexes per schema
        let mut index_map: HashMap<String, Vec<Index>> = HashMap::new();
        for row in &index_rows {
            let schema: String = row.get(0);
            let table_name: String = row.get(1);
            let index_name: String = row.get(2);
            let is_unique: bool = row.get(3);
            let is_primary: bool = row.get(4);
            let columns: Vec<String> = row.get(5);

            index_map.entry(schema).or_default().push(Index {
                name: index_name,
                columns,
                is_unique,
                is_primary,
                table_name,
            });
        }

        // Build functions per schema
        let mut func_map: HashMap<String, Vec<Function>> = HashMap::new();
        for row in &func_rows {
            let schema: String = row.get(0);
            let name: String = row.get(1);
            let args: String = row.get(2);
            // pg_get_function_result() returns NULL for procedures
            let return_type: Option<String> = row.get(3);

            func_map.entry(schema).or_default().push(Function {
                name,
                args,
                return_type: return_type.unwrap_or_default(),
            });
        }

        // Build tables and views per schema from rel_rows
        // Key: (schema, relname) → (relkind, Vec<Column>)
        let mut rel_map: HashMap<(String, String), (String, Vec<Column>)> = HashMap::new();
        // Track insertion order per schema
        let mut rel_order: HashMap<String, Vec<String>> = HashMap::new();

        for row in &rel_rows {
            let schema: String = row.get(0);
            let relname: String = row.get(1);
            let relkind: String = row.get(2);
            let col_name: String = row.get(3);
            let type_name: String = row.get(4);

            let key = (schema.clone(), relname.clone());
            let entry = rel_map.entry(key).or_insert_with(|| {
                rel_order
                    .entry(schema.clone())
                    .or_default()
                    .push(relname.clone());
                (relkind.clone(), Vec::new())
            });

            let is_pk = pk_set.contains(&(schema.clone(), relname.clone(), col_name.clone()));
            let fk = fk_map.remove(&(schema, relname, col_name.clone()));

            entry.1.push(Column {
                name: col_name,
                data_type: datatype_from_format_type(&type_name),
                is_primary_key: is_pk,
                foreign_key: fk,
            });
        }

        // Assemble schemas
        let mut schemas = Vec::new();
        for schema_name in &schema_names {
            let mut tables = Vec::new();
            let mut views = Vec::new();

            if let Some(rel_names) = rel_order.get(schema_name) {
                for relname in rel_names {
                    if let Some((relkind, columns)) =
                        rel_map.remove(&(schema_name.clone(), relname.clone()))
                    {
                        let table = Table {
                            name: relname.clone(),
                            columns,
                        };
                        match relkind.as_str() {
                            "r" => tables.push(table),
                            "v" | "m" => views.push(table),
                            _ => {}
                        }
                    }
                }
            }

            schemas.push(Schema {
                name: schema_name.clone(),
                tables,
                views,
                indexes: index_map.remove(schema_name).unwrap_or_default(),
                functions: func_map.remove(schema_name).unwrap_or_default(),
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
        // Array types
        Type::BOOL_ARRAY => DataType::Array(Box::new(DataType::Boolean)),
        Type::INT2_ARRAY => DataType::Array(Box::new(DataType::SmallInt)),
        Type::INT4_ARRAY => DataType::Array(Box::new(DataType::Integer)),
        Type::INT8_ARRAY => DataType::Array(Box::new(DataType::BigInt)),
        Type::FLOAT4_ARRAY => DataType::Array(Box::new(DataType::Real)),
        Type::FLOAT8_ARRAY => DataType::Array(Box::new(DataType::Double)),
        Type::TEXT_ARRAY | Type::VARCHAR_ARRAY | Type::NAME_ARRAY => {
            DataType::Array(Box::new(DataType::Text))
        }
        Type::UUID_ARRAY => DataType::Array(Box::new(DataType::Uuid)),
        Type::JSONB_ARRAY => DataType::Array(Box::new(DataType::Jsonb)),
        Type::JSON_ARRAY => DataType::Array(Box::new(DataType::Json)),
        Type::NUMERIC_ARRAY => DataType::Array(Box::new(DataType::Numeric)),
        _ => DataType::Unknown(pg_type.name().to_string()),
    }
}

/// Map `format_type()` output to our DataType enum.
///
/// `format_type()` returns strings like "integer", "character varying(255)",
/// "numeric(10,2)", "timestamp with time zone", "text[]", etc.
fn datatype_from_format_type(type_name: &str) -> DataType {
    // Handle array types (e.g. "text[]", "integer[]")
    if let Some(inner) = type_name.strip_suffix("[]") {
        return DataType::Array(Box::new(datatype_from_format_type(inner)));
    }

    // Handle parameterized types: extract base name and optional params
    let (base, params) = if let Some(paren_pos) = type_name.find('(') {
        let base = type_name[..paren_pos].trim();
        let params_str = &type_name[paren_pos + 1..type_name.len() - 1];
        (base, Some(params_str))
    } else {
        (type_name.trim(), None)
    };

    match base {
        "smallint" => DataType::SmallInt,
        "integer" => DataType::Integer,
        "bigint" => DataType::BigInt,
        "real" => DataType::Real,
        "double precision" => DataType::Double,
        "numeric" => DataType::Numeric,
        "text" | "name" => DataType::Text,
        "character varying" => {
            let len = params.and_then(|p| p.parse::<usize>().ok());
            DataType::Varchar(len)
        }
        "character" => {
            let len = params.and_then(|p| p.parse::<usize>().ok());
            DataType::Char(len)
        }
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
        "ARRAY" => DataType::Array(Box::new(DataType::Unknown("array".to_string()))),
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
        DataType::Double => match row.try_get::<_, Option<f64>>(idx) {
            Ok(Some(v)) => CellValue::Float(v),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Numeric => match row.try_get::<_, Option<Decimal>>(idx) {
            Ok(Some(v)) => CellValue::Text(v.to_string()),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
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
        DataType::Uuid => match row.try_get::<_, Option<uuid::Uuid>>(idx) {
            Ok(Some(v)) => CellValue::Uuid(v.to_string()),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Array(inner) => extract_array_value(row, idx, inner),
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

/// Extract an array value from a tokio_postgres Row.
///
/// Tries typed extraction based on inner element type, falling back to
/// Vec<String> for types without a direct Rust mapping.
fn extract_array_value(row: &tokio_postgres::Row, idx: usize, inner: &DataType) -> CellValue {
    match inner {
        DataType::Text | DataType::Varchar(_) | DataType::Char(_) => {
            match row.try_get::<_, Option<Vec<String>>>(idx) {
                Ok(Some(v)) => CellValue::Array(v.into_iter().map(CellValue::Text).collect()),
                Ok(None) => CellValue::Null,
                Err(_) => try_as_string(row, idx),
            }
        }
        DataType::SmallInt => match row.try_get::<_, Option<Vec<i16>>>(idx) {
            Ok(Some(v)) => CellValue::Array(
                v.into_iter()
                    .map(|n| CellValue::Integer(n as i64))
                    .collect(),
            ),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Integer => match row.try_get::<_, Option<Vec<i32>>>(idx) {
            Ok(Some(v)) => CellValue::Array(
                v.into_iter()
                    .map(|n| CellValue::Integer(n as i64))
                    .collect(),
            ),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::BigInt => match row.try_get::<_, Option<Vec<i64>>>(idx) {
            Ok(Some(v)) => CellValue::Array(v.into_iter().map(CellValue::Integer).collect()),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Real => match row.try_get::<_, Option<Vec<f32>>>(idx) {
            Ok(Some(v)) => {
                CellValue::Array(v.into_iter().map(|n| CellValue::Float(n as f64)).collect())
            }
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Double => match row.try_get::<_, Option<Vec<f64>>>(idx) {
            Ok(Some(v)) => CellValue::Array(v.into_iter().map(CellValue::Float).collect()),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Boolean => match row.try_get::<_, Option<Vec<bool>>>(idx) {
            Ok(Some(v)) => CellValue::Array(v.into_iter().map(CellValue::Boolean).collect()),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Uuid => match row.try_get::<_, Option<Vec<uuid::Uuid>>>(idx) {
            Ok(Some(v)) => CellValue::Array(
                v.into_iter()
                    .map(|u| CellValue::Uuid(u.to_string()))
                    .collect(),
            ),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        DataType::Json | DataType::Jsonb => {
            match row.try_get::<_, Option<Vec<serde_json::Value>>>(idx) {
                Ok(Some(v)) => CellValue::Array(v.into_iter().map(CellValue::Json).collect()),
                Ok(None) => CellValue::Null,
                Err(_) => try_as_string(row, idx),
            }
        }
        DataType::Numeric => match row.try_get::<_, Option<Vec<Decimal>>>(idx) {
            Ok(Some(v)) => CellValue::Array(
                v.into_iter()
                    .map(|d| CellValue::Text(d.to_string()))
                    .collect(),
            ),
            Ok(None) => CellValue::Null,
            Err(_) => try_as_string(row, idx),
        },
        _ => try_as_string(row, idx),
    }
}

/// Try to extract a value as a string (fallback for type mismatches).
///
/// When even the string fallback fails, includes the postgres type name
/// in the message so the user knows what type couldn't be displayed.
fn try_as_string(row: &tokio_postgres::Row, idx: usize) -> CellValue {
    match row.try_get::<_, Option<String>>(idx) {
        Ok(Some(v)) => CellValue::Text(v),
        Ok(None) => CellValue::Null,
        Err(_) => {
            let type_name = row
                .columns()
                .get(idx)
                .map_or("unknown", |c| c.type_().name());
            CellValue::Text(format!("<unable to display: {}>", type_name))
        }
    }
}

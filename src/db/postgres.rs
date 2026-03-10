//! PostgreSQL database provider
//!
//! Concrete implementation using tokio-postgres.

use crate::config::ConnectionConfig;
use crate::config::connections::SslMode;
use crate::db::Database;
use crate::db::schema::{
    Column, ForeignKey, Function, Index, PaginatedVec, Schema, SchemaTree, Table,
};
use crate::db::types::{CellValue, ColumnDef, DataType, QueryResults, Row};
use crate::error::{DbError, DbResult};
use rust_decimal::Decimal;
use std::collections::{HashMap, HashSet};
use tokio::sync::{Mutex, mpsc};
use tokio::time::{Duration, timeout};
use tokio_postgres::Client;
use tokio_postgres::types::Type;

/// Result of a cancel operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelResult {
    /// Query was successfully cancelled via pg_cancel_backend()
    Cancelled,
    /// Backend was terminated via pg_terminate_backend()
    Terminated,
    /// Query had already finished (no action needed)
    AlreadyFinished,
    /// Cancel failed, fell back to CancelToken
    FellBackToToken,
}

/// PostgreSQL database provider
pub struct PostgresProvider {
    /// The tokio-postgres client
    client: Client,
    /// Token for cancelling in-flight queries (fallback mechanism)
    cancel_token: tokio_postgres::CancelToken,
    /// SSL mode (needed to cancel over the right transport)
    ssl_mode: SslMode,
    /// Connection string for establishing control connection (no statement_timeout)
    control_conn_string: String,
    /// Control connection for pg_cancel_backend() (lazy initialized)
    /// Uses authenticated SQL channel instead of unauthenticated CancelRequest
    control_conn: Mutex<Option<Client>>,
    /// Backend PID of the main connection (stable for connection lifetime)
    /// Fetched once at connect time, used for pg_cancel_backend/pg_terminate_backend
    backend_pid: i32,
}

impl PostgresProvider {
    /// Connect to a PostgreSQL database.
    ///
    /// If `statement_timeout_ms` > 0, the connection is configured with
    /// PostgreSQL's `statement_timeout` so the server enforces it for every
    /// query, even if the client crashes.
    ///
    /// Returns the provider and a receiver that fires if the background
    /// connection is lost (e.g. server restart, idle timeout).
    pub async fn connect(
        config: &ConnectionConfig,
        statement_timeout_ms: u64,
    ) -> DbResult<(Self, mpsc::UnboundedReceiver<String>)> {
        let conn_string = config.connection_string_with_password(statement_timeout_ms);
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

        // Fetch backend PID once — it's stable for the lifetime of this connection
        let pid: i32 = client
            .query_one("SELECT pg_backend_pid()", &[])
            .await
            .map(|row| row.get(0))
            .unwrap_or(0);

        let cancel_token = client.cancel_token();
        let ssl_mode = config.ssl_mode;
        // Control connection doesn't need statement_timeout
        let control_conn_string = config.connection_string_with_password(0);

        Ok((
            Self {
                client,
                cancel_token,
                ssl_mode,
                control_conn_string,
                control_conn: Mutex::new(None),
                backend_pid: pid,
            },
            conn_err_rx,
        ))
    }

    /// Ensure the control connection is established (lazy initialization).
    /// The control connection is used for pg_cancel_backend() and pg_terminate_backend(),
    /// providing authenticated cancellation over TLS instead of the unauthenticated
    /// CancelRequest protocol.
    async fn ensure_control_connection(&self) -> DbResult<()> {
        let mut guard = self.control_conn.lock().await;
        if guard.is_some() {
            return Ok(());
        }

        // Establish control connection with same SSL mode
        let client = match self.ssl_mode {
            SslMode::Disable => {
                let (client, connection) =
                    tokio_postgres::connect(&self.control_conn_string, tokio_postgres::NoTls)
                        .await
                        .map_err(|e| {
                            DbError::ConnectionFailed(format!("Control connection failed: {}", e))
                        })?;
                // Spawn connection task (we don't monitor this one for errors,
                // we'll just try to reconnect on next cancel if needed)
                tokio::spawn(async move {
                    let _ = connection.await;
                });
                client
            }
            SslMode::Prefer | SslMode::Require => {
                let tls_config = make_tls_config();
                let tls = tokio_postgres_rustls::MakeRustlsConnect::new(tls_config);
                let (client, connection) = tokio_postgres::connect(&self.control_conn_string, tls)
                    .await
                    .map_err(|e| {
                        DbError::ConnectionFailed(format!("Control connection failed: {}", e))
                    })?;
                tokio::spawn(async move {
                    let _ = connection.await;
                });
                client
            }
        };

        *guard = Some(client);
        Ok(())
    }

    /// Cancel the current query using pg_cancel_backend() over the control connection.
    /// This is more secure than CancelRequest as it uses an authenticated TLS channel.
    /// Returns Ok(true) if the cancel was sent, Ok(false) if query already finished.
    async fn cancel_via_backend(&self, pid: i32) -> DbResult<bool> {
        self.ensure_control_connection().await?;

        let guard = self.control_conn.lock().await;
        if let Some(ref ctrl) = *guard {
            let row = ctrl
                .query_one("SELECT pg_cancel_backend($1)", &[&pid])
                .await
                .map_err(|e| DbError::QueryFailed {
                    message: format!("pg_cancel_backend failed: {}", e),
                    position: None,
                })?;
            Ok(row.get(0))
        } else {
            Err(DbError::QueryFailed {
                message: "Control connection not available".to_string(),
                position: None,
            })
        }
    }

    /// Terminate the backend using pg_terminate_backend() over the control connection.
    /// This is a hard kill that terminates the entire backend process.
    /// Use with caution - only after graceful cancel has failed.
    /// Returns Ok(true) if terminate was sent, Ok(false) if backend already gone.
    async fn terminate_via_backend(&self, pid: i32) -> DbResult<bool> {
        self.ensure_control_connection().await?;

        let guard = self.control_conn.lock().await;
        if let Some(ref ctrl) = *guard {
            let row = ctrl
                .query_one("SELECT pg_terminate_backend($1)", &[&pid])
                .await
                .map_err(|e| DbError::QueryFailed {
                    message: format!("pg_terminate_backend failed: {}", e),
                    position: None,
                })?;
            Ok(row.get(0))
        } else {
            Err(DbError::QueryFailed {
                message: "Control connection not available".to_string(),
                position: None,
            })
        }
    }

    /// Reset control connection (e.g., after it fails)
    async fn reset_control_connection(&self) {
        let mut guard = self.control_conn.lock().await;
        *guard = None;
    }

    /// Cancel the current query with enhanced two-phase mechanism.
    /// Phase 1: pg_cancel_backend() - graceful, query-level cancel
    /// Phase 2 (if requested): pg_terminate_backend() - hard, backend termination
    /// Falls back to CancelToken if control connection fails.
    pub async fn cancel_query_enhanced(&self, terminate: bool) -> DbResult<CancelResult> {
        let pid = self.backend_pid;
        if pid == 0 {
            // PID unknown — skip pg_cancel_backend/pg_terminate_backend and fall
            // back to the CancelToken protocol which doesn't require a PID.
            let _ = self.cancel_query().await;
            return Ok(CancelResult::FellBackToToken);
        }

        if terminate {
            // Hard terminate requested
            match self.terminate_via_backend(pid).await {
                Ok(true) => return Ok(CancelResult::Terminated),
                Ok(false) => return Ok(CancelResult::AlreadyFinished),
                Err(_) => {
                    // Fall back to CancelToken (best we can do)
                    self.reset_control_connection().await;
                    let _ = self.cancel_query().await;
                    return Ok(CancelResult::FellBackToToken);
                }
            }
        }

        // Graceful cancel via pg_cancel_backend
        match self.cancel_via_backend(pid).await {
            Ok(true) => Ok(CancelResult::Cancelled),
            Ok(false) => Ok(CancelResult::AlreadyFinished),
            Err(_) => {
                // Control connection failed, fall back to CancelToken
                self.reset_control_connection().await;
                let _ = self.cancel_query().await;
                Ok(CancelResult::FellBackToToken)
            }
        }
    }

    /// Get the backend PID of this connection (stable for its lifetime)
    pub fn current_backend_pid(&self) -> i32 {
        self.backend_pid
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
        .map_err(|e| crate::error::DbError::QueryFailed {
            message: format!("Cancel failed: {}", e),
            position: None,
        })
    }

    /// Inner query execution logic (without timeout wrapper)
    ///
    /// If `max_rows` is 0, all rows are returned. Otherwise, results are
    /// limited to `max_rows` and the `truncated` flag is set if more rows
    /// were available.
    ///
    /// Server-side `statement_timeout` is set at connection level (via the
    /// connection string) so every query is automatically protected.
    async fn execute_query_inner(&self, sql: &str, max_rows: usize) -> DbResult<QueryResults> {
        use futures::TryStreamExt;

        let start = std::time::Instant::now();

        let stmt = self
            .client
            .prepare(sql)
            .await
            .map_err(extract_query_error)?;

        let columns: Vec<ColumnDef> = stmt
            .columns()
            .iter()
            .map(|col| ColumnDef {
                name: col.name().to_string(),
                data_type: pg_type_to_datatype(col.type_()),
                nullable: true,
            })
            .collect();

        // Use streaming to limit memory when max_rows is set
        let row_stream = self
            .client
            .query_raw(&stmt, std::iter::empty::<i32>())
            .await
            .map_err(extract_query_error)?;

        let mut rows = Vec::new();
        let mut truncated = false;

        // Fetch max_rows + 1 to detect if there are more rows
        let fetch_limit = if max_rows > 0 {
            max_rows + 1
        } else {
            usize::MAX
        };

        futures::pin_mut!(row_stream);
        while let Some(pg_row) = row_stream.try_next().await.map_err(extract_query_error)? {
            if rows.len() >= fetch_limit {
                // We've fetched enough to know there are more rows
                truncated = true;
                break;
            }

            let mut values = Vec::with_capacity(columns.len());
            for (i, col_def) in columns.iter().enumerate() {
                let value = extract_cell_value(&pg_row, i, &col_def.data_type);
                values.push(value);
            }
            rows.push(Row { values });
        }

        // If we fetched max_rows + 1, truncate to max_rows
        if max_rows > 0 && rows.len() > max_rows {
            truncated = true;
            rows.truncate(max_rows);
        }

        let row_count = rows.len();
        Ok(QueryResults::new_truncated(
            columns,
            rows,
            start.elapsed(),
            row_count,
            truncated,
        ))
    }

    /// Inner schema loading logic. Pass limit=0 for unlimited.
    ///
    /// Uses efficient two-phase loading when limit > 0:
    /// 1. Query table/view NAMES with LIMIT
    /// 2. Query columns and constraints only for those limited names
    async fn get_schema_inner(&self, limit: usize) -> DbResult<SchemaTree> {
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

        // Count queries for pagination metadata (only if limit > 0)
        let table_counts: HashMap<String, i64>;
        let view_counts: HashMap<String, i64>;
        let func_counts: HashMap<String, i64>;
        let index_counts: HashMap<String, i64>;

        if limit > 0 {
            // Count tables per schema
            let table_count_rows = self
                .client
                .query(
                    "SELECT n.nspname, COUNT(DISTINCT c.oid)::bigint
                     FROM pg_class c
                     JOIN pg_namespace n ON n.oid = c.relnamespace
                     WHERE c.relkind = 'r'
                       AND n.nspname NOT LIKE 'pg_%'
                       AND n.nspname != 'information_schema'
                     GROUP BY n.nspname",
                    &[],
                )
                .await
                .map_err(&map_err)?;
            table_counts = table_count_rows
                .iter()
                .map(|r| (r.get::<_, String>(0), r.get::<_, i64>(1)))
                .collect();

            // Count views per schema
            let view_count_rows = self
                .client
                .query(
                    "SELECT n.nspname, COUNT(DISTINCT c.oid)::bigint
                     FROM pg_class c
                     JOIN pg_namespace n ON n.oid = c.relnamespace
                     WHERE c.relkind IN ('v', 'm')
                       AND n.nspname NOT LIKE 'pg_%'
                       AND n.nspname != 'information_schema'
                     GROUP BY n.nspname",
                    &[],
                )
                .await
                .map_err(&map_err)?;
            view_counts = view_count_rows
                .iter()
                .map(|r| (r.get::<_, String>(0), r.get::<_, i64>(1)))
                .collect();

            // Count functions per schema
            let func_count_rows = self
                .client
                .query(
                    "SELECT n.nspname, COUNT(*)::bigint
                     FROM pg_proc p
                     JOIN pg_namespace n ON n.oid = p.pronamespace
                     WHERE n.nspname NOT LIKE 'pg_%'
                       AND n.nspname != 'information_schema'
                       AND p.prokind IN ('f', 'p')
                     GROUP BY n.nspname",
                    &[],
                )
                .await
                .map_err(&map_err)?;
            func_counts = func_count_rows
                .iter()
                .map(|r| (r.get::<_, String>(0), r.get::<_, i64>(1)))
                .collect();

            // Count indexes per schema
            let index_count_rows = self
                .client
                .query(
                    "SELECT n.nspname, COUNT(DISTINCT ci.oid)::bigint
                     FROM pg_index ix
                     JOIN pg_class ci ON ci.oid = ix.indexrelid
                     JOIN pg_class ct ON ct.oid = ix.indrelid
                     JOIN pg_namespace n ON n.oid = ct.relnamespace
                     WHERE n.nspname NOT LIKE 'pg_%'
                       AND n.nspname != 'information_schema'
                     GROUP BY n.nspname",
                    &[],
                )
                .await
                .map_err(&map_err)?;
            index_counts = index_count_rows
                .iter()
                .map(|r| (r.get::<_, String>(0), r.get::<_, i64>(1)))
                .collect();
        } else {
            table_counts = HashMap::new();
            view_counts = HashMap::new();
            func_counts = HashMap::new();
            index_counts = HashMap::new();
        }

        // Build schemas using two-phase loading for efficiency
        let mut schemas = Vec::new();

        for schema_name in &schema_names {
            // Phase 1: Get limited table/view names
            let table_names = self.load_relation_names(schema_name, "r", 0, limit).await?;
            let view_names = self
                .load_relation_names(schema_name, "v,m", 0, limit)
                .await?;

            // Phase 2: Get columns and constraints only for those tables/views
            let tables = if table_names.is_empty() {
                Vec::new()
            } else {
                let columns = self
                    .load_columns_for_relations(schema_name, &table_names)
                    .await?;
                let (pk_set, fk_map) = self
                    .load_constraints_for_tables(schema_name, &table_names)
                    .await?;
                assemble_tables(schema_name, table_names, columns, pk_set, fk_map)
            };

            let views = if view_names.is_empty() {
                Vec::new()
            } else {
                let columns = self
                    .load_columns_for_relations(schema_name, &view_names)
                    .await?;
                // Views don't have PK/FK constraints
                assemble_tables(
                    schema_name,
                    view_names,
                    columns,
                    HashSet::new(),
                    HashMap::new(),
                )
            };

            // Functions and indexes with LIMIT (simple queries, already efficient)
            let functions = self.load_functions_limited(schema_name, 0, limit).await?;
            let indexes = self.load_indexes_limited(schema_name, 0, limit).await?;

            // Get total counts (from COUNT queries if limit > 0, else from vec length)
            let table_total = if limit > 0 {
                *table_counts.get(schema_name).unwrap_or(&0) as usize
            } else {
                tables.len()
            };
            let view_total = if limit > 0 {
                *view_counts.get(schema_name).unwrap_or(&0) as usize
            } else {
                views.len()
            };
            let index_total = if limit > 0 {
                *index_counts.get(schema_name).unwrap_or(&0) as usize
            } else {
                indexes.len()
            };
            let func_total = if limit > 0 {
                *func_counts.get(schema_name).unwrap_or(&0) as usize
            } else {
                functions.len()
            };

            schemas.push(Schema {
                name: schema_name.clone(),
                tables: PaginatedVec::new(tables, table_total),
                views: PaginatedVec::new(views, view_total),
                indexes: PaginatedVec::new(indexes, index_total),
                functions: PaginatedVec::new(functions, func_total),
            });
        }

        Ok(SchemaTree {
            schemas: PaginatedVec::from_vec(schemas),
        })
    }

    // ── Two-phase loading helpers ─────────────────────────────────────────

    /// Load relation (table/view) names for a schema with optional offset/limit.
    /// `relkind` can be "r" for tables or "v,m" for views/materialized views.
    async fn load_relation_names(
        &self,
        schema_name: &str,
        relkind: &str,
        offset: usize,
        limit: usize,
    ) -> DbResult<Vec<String>> {
        let map_err =
            |e: tokio_postgres::Error| crate::error::DbError::SchemaLoadFailed(e.to_string());

        let (relkind_clause, query) = if relkind == "r" {
            (
                "c.relkind = 'r'",
                if limit > 0 {
                    "SELECT c.relname
                     FROM pg_class c
                     JOIN pg_namespace n ON n.oid = c.relnamespace
                     WHERE c.relkind = 'r'
                       AND n.nspname = $1
                     ORDER BY c.relname
                     OFFSET $2 LIMIT $3"
                } else {
                    "SELECT c.relname
                     FROM pg_class c
                     JOIN pg_namespace n ON n.oid = c.relnamespace
                     WHERE c.relkind = 'r'
                       AND n.nspname = $1
                     ORDER BY c.relname"
                },
            )
        } else {
            (
                "c.relkind IN ('v', 'm')",
                if limit > 0 {
                    "SELECT c.relname
                     FROM pg_class c
                     JOIN pg_namespace n ON n.oid = c.relnamespace
                     WHERE c.relkind IN ('v', 'm')
                       AND n.nspname = $1
                     ORDER BY c.relname
                     OFFSET $2 LIMIT $3"
                } else {
                    "SELECT c.relname
                     FROM pg_class c
                     JOIN pg_namespace n ON n.oid = c.relnamespace
                     WHERE c.relkind IN ('v', 'm')
                       AND n.nspname = $1
                     ORDER BY c.relname"
                },
            )
        };
        let _ = relkind_clause; // Suppress warning

        let rows = if limit > 0 {
            self.client
                .query(query, &[&schema_name, &(offset as i64), &(limit as i64)])
                .await
                .map_err(&map_err)?
        } else {
            self.client
                .query(query, &[&schema_name])
                .await
                .map_err(&map_err)?
        };

        Ok(rows.iter().map(|r| r.get(0)).collect())
    }

    /// Load columns for specific tables/views in a schema.
    async fn load_columns_for_relations(
        &self,
        schema_name: &str,
        table_names: &[String],
    ) -> DbResult<HashMap<String, Vec<(String, String)>>> {
        let map_err =
            |e: tokio_postgres::Error| crate::error::DbError::SchemaLoadFailed(e.to_string());

        let col_rows = self
            .client
            .query(
                "SELECT c.relname, a.attname, format_type(a.atttypid, a.atttypmod)
                 FROM pg_class c
                 JOIN pg_namespace n ON n.oid = c.relnamespace
                 JOIN pg_attribute a ON a.attrelid = c.oid
                 WHERE n.nspname = $1
                   AND c.relname = ANY($2)
                   AND a.attnum > 0 AND NOT a.attisdropped
                 ORDER BY c.relname, a.attnum",
                &[&schema_name, &table_names],
            )
            .await
            .map_err(&map_err)?;

        let mut result: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for row in col_rows {
            let table: String = row.get(0);
            let col_name: String = row.get(1);
            let type_name: String = row.get(2);
            result.entry(table).or_default().push((col_name, type_name));
        }
        Ok(result)
    }

    /// Load PK/FK constraints for specific tables in a schema.
    async fn load_constraints_for_tables(
        &self,
        schema_name: &str,
        table_names: &[String],
    ) -> DbResult<(
        HashSet<(String, String)>,
        HashMap<(String, String), ForeignKey>,
    )> {
        let map_err =
            |e: tokio_postgres::Error| crate::error::DbError::SchemaLoadFailed(e.to_string());

        let constraint_rows = self
            .client
            .query(
                "SELECT c.relname, con.contype::text, a.attname,
                        fn.nspname AS fk_schema, fc.relname AS fk_table, fa.attname AS fk_col
                 FROM pg_constraint con
                 JOIN pg_class c ON c.oid = con.conrelid
                 JOIN pg_namespace n ON n.oid = c.relnamespace
                 JOIN LATERAL unnest(con.conkey) WITH ORDINALITY AS u(attnum, ord) ON true
                 JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = u.attnum
                 LEFT JOIN pg_class fc ON fc.oid = con.confrelid
                 LEFT JOIN pg_namespace fn ON fn.oid = fc.relnamespace
                 LEFT JOIN LATERAL unnest(con.confkey) WITH ORDINALITY AS fu(attnum, ord) ON fu.ord = u.ord
                 LEFT JOIN pg_attribute fa ON fa.attrelid = fc.oid AND fa.attnum = fu.attnum
                 WHERE con.contype IN ('p', 'f')
                   AND n.nspname = $1
                   AND c.relname = ANY($2)
                 ORDER BY c.relname, con.contype, u.ord",
                &[&schema_name, &table_names],
            )
            .await
            .map_err(&map_err)?;

        let mut pk_set: HashSet<(String, String)> = HashSet::new();
        let mut fk_map: HashMap<(String, String), ForeignKey> = HashMap::new();

        for row in constraint_rows {
            let table: String = row.get(0);
            let contype: String = row.get(1);
            let col: String = row.get(2);

            if contype == "p" {
                pk_set.insert((table, col));
            } else if contype == "f" {
                let fk_schema: Option<String> = row.get(3);
                let fk_table: String = row.get(4);
                let fk_col: String = row.get(5);
                let target_table = match fk_schema {
                    Some(ref s) if s != schema_name => format!("{}.{}", s, fk_table),
                    _ => fk_table,
                };
                fk_map.insert(
                    (table, col),
                    ForeignKey {
                        target_table,
                        target_column: fk_col,
                    },
                );
            }
        }

        Ok((pk_set, fk_map))
    }

    /// Load functions for a schema with optional offset/limit.
    async fn load_functions_limited(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> DbResult<Vec<Function>> {
        let map_err =
            |e: tokio_postgres::Error| crate::error::DbError::SchemaLoadFailed(e.to_string());

        let func_rows = if limit > 0 {
            self.client
                .query(
                    "SELECT p.proname,
                            pg_get_function_identity_arguments(p.oid) AS args,
                            pg_get_function_result(p.oid) AS return_type
                     FROM pg_proc p
                     JOIN pg_namespace n ON n.oid = p.pronamespace
                     WHERE n.nspname = $1
                       AND p.prokind IN ('f', 'p')
                     ORDER BY p.proname
                     OFFSET $2 LIMIT $3",
                    &[&schema_name, &(offset as i64), &(limit as i64)],
                )
                .await
                .map_err(&map_err)?
        } else {
            self.client
                .query(
                    "SELECT p.proname,
                            pg_get_function_identity_arguments(p.oid) AS args,
                            pg_get_function_result(p.oid) AS return_type
                     FROM pg_proc p
                     JOIN pg_namespace n ON n.oid = p.pronamespace
                     WHERE n.nspname = $1
                       AND p.prokind IN ('f', 'p')
                     ORDER BY p.proname",
                    &[&schema_name],
                )
                .await
                .map_err(&map_err)?
        };

        Ok(func_rows
            .iter()
            .map(|row| Function {
                name: row.get(0),
                args: row.get(1),
                return_type: row.get::<_, Option<String>>(2).unwrap_or_default(),
            })
            .collect())
    }

    /// Load indexes for a schema with optional offset/limit.
    async fn load_indexes_limited(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> DbResult<Vec<Index>> {
        let map_err =
            |e: tokio_postgres::Error| crate::error::DbError::SchemaLoadFailed(e.to_string());

        let index_rows = if limit > 0 {
            self.client
                .query(
                    "SELECT ct.relname AS table_name, ci.relname AS index_name,
                            ix.indisunique, ix.indisprimary,
                            array_agg(a.attname ORDER BY k.ord) AS columns
                     FROM pg_index ix
                     JOIN pg_class ci ON ci.oid = ix.indexrelid
                     JOIN pg_class ct ON ct.oid = ix.indrelid
                     JOIN pg_namespace n ON n.oid = ct.relnamespace
                     JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY AS k(attnum, ord) ON true
                     JOIN pg_attribute a ON a.attrelid = ct.oid AND a.attnum = k.attnum
                     WHERE n.nspname = $1
                       AND a.attnum > 0
                     GROUP BY ct.relname, ci.relname, ix.indisunique, ix.indisprimary
                     ORDER BY ci.relname
                     OFFSET $2 LIMIT $3",
                    &[&schema_name, &(offset as i64), &(limit as i64)],
                )
                .await
                .map_err(&map_err)?
        } else {
            self.client
                .query(
                    "SELECT ct.relname AS table_name, ci.relname AS index_name,
                            ix.indisunique, ix.indisprimary,
                            array_agg(a.attname ORDER BY k.ord) AS columns
                     FROM pg_index ix
                     JOIN pg_class ci ON ci.oid = ix.indexrelid
                     JOIN pg_class ct ON ct.oid = ix.indrelid
                     JOIN pg_namespace n ON n.oid = ct.relnamespace
                     JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY AS k(attnum, ord) ON true
                     JOIN pg_attribute a ON a.attrelid = ct.oid AND a.attnum = k.attnum
                     WHERE n.nspname = $1
                       AND a.attnum > 0
                     GROUP BY ct.relname, ci.relname, ix.indisunique, ix.indisprimary
                     ORDER BY ci.relname",
                    &[&schema_name],
                )
                .await
                .map_err(&map_err)?
        };

        Ok(index_rows
            .iter()
            .map(|row| Index {
                table_name: row.get(0),
                name: row.get(1),
                is_unique: row.get(2),
                is_primary: row.get(3),
                columns: row.get(4),
            })
            .collect())
    }

    /// Search schema objects by name pattern (case-insensitive substring match).
    /// Returns tables, views, functions, indexes, and columns that match the pattern.
    async fn search_schema_inner(&self, pattern: &str) -> DbResult<SchemaTree> {
        let map_err = |e: tokio_postgres::Error| DbError::QueryFailed {
            message: e.to_string(),
            position: None,
        };

        // Escape special LIKE characters and create pattern
        let escaped = pattern
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let like_pattern = format!("%{}%", escaped);

        // Query 1: Get schemas that have matching objects
        let schema_rows = self
            .client
            .query(
                "SELECT DISTINCT n.nspname
                 FROM pg_namespace n
                 WHERE n.nspname NOT LIKE 'pg_%'
                   AND n.nspname != 'information_schema'
                   AND (
                     -- Schema has matching tables/views
                     EXISTS (
                       SELECT 1 FROM pg_class c
                       WHERE c.relnamespace = n.oid
                         AND c.relkind IN ('r', 'v', 'm')
                         AND c.relname ILIKE $1
                     )
                     -- Or has matching columns
                     OR EXISTS (
                       SELECT 1 FROM pg_class c
                       JOIN pg_attribute a ON a.attrelid = c.oid
                       WHERE c.relnamespace = n.oid
                         AND c.relkind IN ('r', 'v', 'm')
                         AND a.attnum > 0 AND NOT a.attisdropped
                         AND a.attname ILIKE $1
                     )
                     -- Or has matching functions
                     OR EXISTS (
                       SELECT 1 FROM pg_proc p
                       WHERE p.pronamespace = n.oid
                         AND p.prokind IN ('f', 'p')
                         AND p.proname ILIKE $1
                     )
                     -- Or has matching indexes
                     OR EXISTS (
                       SELECT 1 FROM pg_index ix
                       JOIN pg_class ci ON ci.oid = ix.indexrelid
                       JOIN pg_class ct ON ct.oid = ix.indrelid
                       WHERE ct.relnamespace = n.oid
                         AND ci.relname ILIKE $1
                     )
                   )
                 ORDER BY n.nspname",
                &[&like_pattern],
            )
            .await
            .map_err(&map_err)?;

        let schema_names: Vec<String> = schema_rows.iter().map(|r| r.get(0)).collect();

        if schema_names.is_empty() {
            return Ok(SchemaTree {
                schemas: PaginatedVec::from_vec(vec![]),
            });
        }

        // Query 2: Get matching tables/views with their columns
        // Include tables that match OR have matching columns
        let rel_rows = self
            .client
            .query(
                "SELECT n.nspname, c.relname, c.relkind::text, a.attname,
                        format_type(a.atttypid, a.atttypmod) AS formatted_type,
                        c.relname ILIKE $1 AS table_matches,
                        a.attname ILIKE $1 AS col_matches
                 FROM pg_class c
                 JOIN pg_namespace n ON n.oid = c.relnamespace
                 JOIN pg_attribute a ON a.attrelid = c.oid
                 WHERE c.relkind IN ('r', 'v', 'm')
                   AND n.nspname NOT LIKE 'pg_%'
                   AND n.nspname != 'information_schema'
                   AND a.attnum > 0 AND NOT a.attisdropped
                   AND (c.relname ILIKE $1 OR a.attname ILIKE $1)
                 ORDER BY n.nspname, c.relname, a.attnum",
                &[&like_pattern],
            )
            .await
            .map_err(&map_err)?;

        // Query 3: PK + FK constraints for matching tables
        let constraint_rows = self
            .client
            .query(
                "SELECT n.nspname, c.relname, con.contype::text,
                        a.attname,
                        fn.nspname AS fk_schema, fc.relname AS fk_table, fa.attname AS fk_col
                 FROM pg_constraint con
                 JOIN pg_class c ON c.oid = con.conrelid
                 JOIN pg_namespace n ON n.oid = c.relnamespace
                 JOIN LATERAL unnest(con.conkey) WITH ORDINALITY AS u(attnum, ord) ON true
                 JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = u.attnum
                 LEFT JOIN pg_class fc ON fc.oid = con.confrelid
                 LEFT JOIN pg_namespace fn ON fn.oid = fc.relnamespace
                 LEFT JOIN LATERAL unnest(con.confkey) WITH ORDINALITY AS fu(attnum, ord) ON fu.ord = u.ord
                 LEFT JOIN pg_attribute fa ON fa.attrelid = fc.oid AND fa.attnum = fu.attnum
                 WHERE con.contype IN ('p', 'f')
                   AND n.nspname NOT LIKE 'pg_%'
                   AND n.nspname != 'information_schema'
                   AND (c.relname ILIKE $1 OR EXISTS (
                     SELECT 1 FROM pg_attribute a2
                     WHERE a2.attrelid = c.oid AND a2.attnum > 0
                       AND NOT a2.attisdropped AND a2.attname ILIKE $1
                   ))
                 ORDER BY n.nspname, c.relname, con.contype, u.ord",
                &[&like_pattern],
            )
            .await
            .map_err(&map_err)?;

        // Query 4: Matching indexes
        let index_rows = self
            .client
            .query(
                "SELECT n.nspname, ct.relname AS table_name, ci.relname AS index_name,
                        ix.indisunique, ix.indisprimary,
                        array_agg(a.attname ORDER BY k.ord) AS columns
                 FROM pg_index ix
                 JOIN pg_class ci ON ci.oid = ix.indexrelid
                 JOIN pg_class ct ON ct.oid = ix.indrelid
                 JOIN pg_namespace n ON n.oid = ct.relnamespace
                 JOIN LATERAL unnest(ix.indkey) WITH ORDINALITY AS k(attnum, ord) ON true
                 JOIN pg_attribute a ON a.attrelid = ct.oid AND a.attnum = k.attnum
                 WHERE n.nspname NOT LIKE 'pg_%'
                   AND n.nspname != 'information_schema'
                   AND a.attnum > 0
                   AND ci.relname ILIKE $1
                 GROUP BY n.nspname, ct.relname, ci.relname, ix.indisunique, ix.indisprimary
                 ORDER BY n.nspname, ci.relname",
                &[&like_pattern],
            )
            .await
            .map_err(&map_err)?;

        // Query 5: Matching functions
        let func_rows = self
            .client
            .query(
                "SELECT n.nspname, p.proname,
                        pg_get_function_identity_arguments(p.oid) AS args,
                        pg_get_function_result(p.oid) AS return_type
                 FROM pg_proc p
                 JOIN pg_namespace n ON n.oid = p.pronamespace
                 WHERE n.nspname NOT LIKE 'pg_%'
                   AND n.nspname != 'information_schema'
                   AND p.prokind IN ('f', 'p')
                   AND p.proname ILIKE $1
                 ORDER BY n.nspname, p.proname",
                &[&like_pattern],
            )
            .await
            .map_err(&map_err)?;

        // ── Assembly (similar to get_schema_inner) ────────────────────

        // Build PK set
        let mut pk_set: HashSet<(String, String, String)> = HashSet::new();
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
            let return_type: Option<String> = row.get(3);

            func_map.entry(schema).or_default().push(Function {
                name,
                args,
                return_type: return_type.unwrap_or_default(),
            });
        }

        // Build tables and views from rel_rows
        let mut rel_map: HashMap<(String, String), (String, Vec<Column>)> = HashMap::new();
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
                tables: PaginatedVec::from_vec(tables),
                views: PaginatedVec::from_vec(views),
                indexes: PaginatedVec::from_vec(index_map.remove(schema_name).unwrap_or_default()),
                functions: PaginatedVec::from_vec(func_map.remove(schema_name).unwrap_or_default()),
            });
        }

        Ok(SchemaTree {
            schemas: PaginatedVec::from_vec(schemas),
        })
    }

    /// Load more tables for a specific schema with offset and limit.
    /// Uses shared two-phase loading helpers.
    async fn load_more_tables_inner(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> DbResult<Vec<Table>> {
        // Phase 1: Get table names with offset/limit
        let table_names = self
            .load_relation_names(schema_name, "r", offset, limit)
            .await?;
        if table_names.is_empty() {
            return Ok(Vec::new());
        }

        // Phase 2: Get columns and constraints for those tables
        let columns = self
            .load_columns_for_relations(schema_name, &table_names)
            .await?;
        let (pk_set, fk_map) = self
            .load_constraints_for_tables(schema_name, &table_names)
            .await?;

        Ok(assemble_tables(
            schema_name,
            table_names,
            columns,
            pk_set,
            fk_map,
        ))
    }

    /// Load more views for a specific schema with offset and limit.
    /// Uses shared two-phase loading helpers.
    async fn load_more_views_inner(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> DbResult<Vec<Table>> {
        // Phase 1: Get view names with offset/limit
        let view_names = self
            .load_relation_names(schema_name, "v,m", offset, limit)
            .await?;
        if view_names.is_empty() {
            return Ok(Vec::new());
        }

        // Phase 2: Get columns for those views (views don't have PK/FK)
        let columns = self
            .load_columns_for_relations(schema_name, &view_names)
            .await?;

        Ok(assemble_tables(
            schema_name,
            view_names,
            columns,
            HashSet::new(),
            HashMap::new(),
        ))
    }

    /// Load more functions for a specific schema with offset and limit.
    /// Uses shared helper.
    async fn load_more_functions_inner(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> DbResult<Vec<Function>> {
        self.load_functions_limited(schema_name, offset, limit)
            .await
    }

    /// Load more indexes for a specific schema with offset and limit.
    /// Uses shared helper.
    async fn load_more_indexes_inner(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> DbResult<Vec<Index>> {
        self.load_indexes_limited(schema_name, offset, limit).await
    }
}

impl Database for PostgresProvider {
    async fn execute_query(
        &self,
        sql: &str,
        timeout_ms: u64,
        max_rows: usize,
    ) -> DbResult<QueryResults> {
        let query_future = self.execute_query_inner(sql, max_rows);

        if timeout_ms == 0 {
            query_future.await
        } else {
            match timeout(Duration::from_millis(timeout_ms), query_future).await {
                Ok(result) => result,
                Err(_) => {
                    // Timeout elapsed - cancel the backend query using enhanced cancel
                    let _ = self.cancel_query_enhanced(false).await;
                    Err(DbError::Timeout(timeout_ms))
                }
            }
        }
    }

    async fn get_schema(&self, limit: usize) -> DbResult<SchemaTree> {
        self.get_schema_inner(limit).await
    }

    async fn search_schema(&self, pattern: &str) -> DbResult<SchemaTree> {
        self.search_schema_inner(pattern).await
    }

    async fn load_more_tables(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> DbResult<Vec<Table>> {
        self.load_more_tables_inner(schema_name, offset, limit)
            .await
    }

    async fn load_more_views(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> DbResult<Vec<Table>> {
        self.load_more_views_inner(schema_name, offset, limit).await
    }

    async fn load_more_functions(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> DbResult<Vec<Function>> {
        self.load_more_functions_inner(schema_name, offset, limit)
            .await
    }

    async fn load_more_indexes(
        &self,
        schema_name: &str,
        offset: usize,
        limit: usize,
    ) -> DbResult<Vec<Index>> {
        self.load_more_indexes_inner(schema_name, offset, limit)
            .await
    }
}

/// Assemble Table structs from names, columns, and constraints.
fn assemble_tables(
    schema_name: &str,
    table_names: Vec<String>,
    mut columns: HashMap<String, Vec<(String, String)>>,
    pk_set: HashSet<(String, String)>,
    mut fk_map: HashMap<(String, String), ForeignKey>,
) -> Vec<Table> {
    let _ = schema_name; // May be used for FK formatting in future
    table_names
        .into_iter()
        .map(|name| {
            let cols = columns.remove(&name).unwrap_or_default();
            let columns = cols
                .into_iter()
                .map(|(col_name, type_name)| {
                    let is_pk = pk_set.contains(&(name.clone(), col_name.clone()));
                    let fk = fk_map.remove(&(name.clone(), col_name.clone()));
                    Column {
                        name: col_name,
                        data_type: datatype_from_format_type(&type_name),
                        is_primary_key: is_pk,
                        foreign_key: fk,
                    }
                })
                .collect();
            Table { name, columns }
        })
        .collect()
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

/// Extract error information from a tokio_postgres error, preserving position if available.
fn extract_query_error(e: tokio_postgres::Error) -> crate::error::DbError {
    if let Some(db_err) = e.as_db_error() {
        let position = match db_err.position() {
            Some(tokio_postgres::error::ErrorPosition::Original(p)) => Some(*p),
            _ => None,
        };
        crate::error::DbError::QueryFailed {
            message: db_err.message().to_string(),
            position,
        }
    } else {
        crate::error::DbError::QueryFailed {
            message: e.to_string(),
            position: None,
        }
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
                Ok(Some(v)) => CellValue::Json(v.to_string()),
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
                Ok(Some(v)) => CellValue::Array(
                    v.into_iter()
                        .map(|j| CellValue::Json(j.to_string()))
                        .collect(),
                ),
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

//! Per-tab database connection management.
//!
//! Each tab gets its own PostgreSQL connection, lazily created on first query.
//! This gives each tab independent transaction state and allows concurrent queries.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::config::ConnectionConfig;
use crate::db;

/// Manages per-tab database connections.
///
/// Each tab gets its own PostgreSQL connection (lazily created on first query).
/// This gives each tab independent transaction state and allows concurrent queries.
pub struct ConnectionManager {
    /// Per-tab providers: tab_id → (provider, connection-error receiver)
    tabs: HashMap<usize, (Arc<db::PostgresProvider>, mpsc::UnboundedReceiver<String>)>,
    /// Connection config (shared — all tabs connect to the same database)
    config: Option<ConnectionConfig>,
    /// Statement timeout for new connections
    statement_timeout_ms: u64,
}

impl ConnectionManager {
    pub fn new(config: Option<ConnectionConfig>, statement_timeout_ms: u64) -> Self {
        Self {
            tabs: HashMap::new(),
            config,
            statement_timeout_ms,
        }
    }

    /// Register an already-connected provider for a tab.
    pub fn insert(
        &mut self,
        tab_id: usize,
        provider: Arc<db::PostgresProvider>,
        rx: mpsc::UnboundedReceiver<String>,
    ) {
        self.tabs.insert(tab_id, (provider, rx));
    }

    /// Get the provider for a tab (if connected).
    pub fn get(&self, tab_id: usize) -> Option<&Arc<db::PostgresProvider>> {
        self.tabs.get(&tab_id).map(|(p, _)| p)
    }

    /// Get any available provider (for schema operations that don't need a specific tab).
    pub fn any_provider(&self) -> Option<&Arc<db::PostgresProvider>> {
        self.tabs.values().next().map(|(p, _)| p)
    }

    /// Connect a tab lazily. Returns the provider on success.
    pub async fn ensure_connected(
        &mut self,
        tab_id: usize,
    ) -> Result<Arc<db::PostgresProvider>, String> {
        if let Some((prov, _)) = self.tabs.get(&tab_id) {
            return Ok(Arc::clone(prov));
        }

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| "Not connected".to_string())?;

        let (prov, rx) = db::PostgresProvider::connect(config, self.statement_timeout_ms)
            .await
            .map_err(|e| format!("Connection failed: {}", e))?;

        let prov = Arc::new(prov);
        self.tabs.insert(tab_id, (Arc::clone(&prov), rx));
        Ok(prov)
    }

    /// Remove a tab's connection (on tab close).
    pub fn remove(&mut self, tab_id: usize) {
        self.tabs.remove(&tab_id);
    }

    /// Drop all connections (on disconnect / reconnect).
    pub fn disconnect_all(&mut self) {
        self.tabs.clear();
        self.config = None;
    }

    /// Set the connection config (on new connect).
    pub fn set_config(&mut self, config: ConnectionConfig, statement_timeout_ms: u64) {
        self.config = Some(config);
        self.statement_timeout_ms = statement_timeout_ms;
    }

    /// Poll all connection-error receivers, returning the first error with its tab_id.
    /// Returns Pending if no errors ready.
    pub fn poll_connection_errors(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<(usize, String)> {
        for (&tab_id, (_, rx)) in self.tabs.iter_mut() {
            match rx.poll_recv(cx) {
                std::task::Poll::Ready(Some(msg)) => {
                    return std::task::Poll::Ready((tab_id, msg));
                }
                std::task::Poll::Ready(None) => {
                    // Channel closed — provider dropped, will be cleaned up
                }
                std::task::Poll::Pending => {}
            }
        }
        std::task::Poll::Pending
    }
}

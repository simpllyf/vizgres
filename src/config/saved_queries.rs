//! Saved query storage
//!
//! Manages named queries stored in ~/.vizgres/saved_queries.toml,
//! each tied to a saved connection profile.

use crate::error::ConfigResult;
use serde::{Deserialize, Serialize};

/// A named query tied to a connection profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedQuery {
    /// Which saved connection this query belongs to
    pub connection: String,
    /// User-chosen name for the query
    pub name: String,
    /// The SQL text
    pub sql: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SavedQueriesFile {
    #[serde(default)]
    queries: Vec<SavedQuery>,
}

/// Load all saved queries from ~/.vizgres/saved_queries.toml
pub fn load_saved_queries() -> ConfigResult<Vec<SavedQuery>> {
    let path = super::connections::ConnectionConfig::config_dir()?.join("saved_queries.toml");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path)?;
    let file: SavedQueriesFile = toml::from_str(&content)?;
    Ok(file.queries)
}

/// Load saved queries for a specific connection
pub fn load_queries_for_connection(connection_name: &str) -> ConfigResult<Vec<SavedQuery>> {
    let all = load_saved_queries()?;
    Ok(all
        .into_iter()
        .filter(|q| q.connection == connection_name)
        .collect())
}

/// Save or update a query. Overwrites any existing query with the same
/// connection + name combination.
pub fn save_query(query: &SavedQuery) -> ConfigResult<()> {
    let mut all = load_saved_queries()?;
    // Remove existing with same connection + name
    all.retain(|q| !(q.connection == query.connection && q.name == query.name));
    all.push(query.clone());
    write_queries(&all)
}

/// Delete a query by connection + name
pub fn delete_query(connection_name: &str, query_name: &str) -> ConfigResult<()> {
    let mut all = load_saved_queries()?;
    all.retain(|q| !(q.connection == connection_name && q.name == query_name));
    write_queries(&all)
}

fn write_queries(queries: &[SavedQuery]) -> ConfigResult<()> {
    let file = SavedQueriesFile {
        queries: queries.to_vec(),
    };
    let content = toml::to_string_pretty(&file)?;
    let path = super::connections::ConnectionConfig::config_dir()?.join("saved_queries.toml");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saved_query_roundtrip_toml() {
        let query = SavedQuery {
            connection: "prod".to_string(),
            name: "active users".to_string(),
            sql: "SELECT * FROM users WHERE active = true".to_string(),
        };
        let file = SavedQueriesFile {
            queries: vec![query],
        };
        let toml_str = toml::to_string_pretty(&file).unwrap();
        let parsed: SavedQueriesFile = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.queries.len(), 1);
        assert_eq!(parsed.queries[0].connection, "prod");
        assert_eq!(parsed.queries[0].name, "active users");
        assert_eq!(
            parsed.queries[0].sql,
            "SELECT * FROM users WHERE active = true"
        );
    }

    #[test]
    fn test_empty_file_returns_empty_vec() {
        let parsed: SavedQueriesFile = toml::from_str("").unwrap();
        assert!(parsed.queries.is_empty());
    }

    #[test]
    fn test_multiple_queries_serialize() {
        let file = SavedQueriesFile {
            queries: vec![
                SavedQuery {
                    connection: "prod".to_string(),
                    name: "q1".to_string(),
                    sql: "SELECT 1".to_string(),
                },
                SavedQuery {
                    connection: "staging".to_string(),
                    name: "q2".to_string(),
                    sql: "SELECT 2".to_string(),
                },
            ],
        };
        let toml_str = toml::to_string_pretty(&file).unwrap();
        let parsed: SavedQueriesFile = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.queries.len(), 2);
    }

    #[test]
    fn test_multiline_sql_roundtrip() {
        let query = SavedQuery {
            connection: "prod".to_string(),
            name: "complex".to_string(),
            sql: "SELECT u.id, u.name\nFROM users u\nWHERE u.active = true\nORDER BY u.name"
                .to_string(),
        };
        let file = SavedQueriesFile {
            queries: vec![query],
        };
        let toml_str = toml::to_string_pretty(&file).unwrap();
        let parsed: SavedQueriesFile = toml::from_str(&toml_str).unwrap();
        assert!(parsed.queries[0].sql.contains('\n'));
        assert_eq!(parsed.queries[0].sql.lines().count(), 4);
    }
}

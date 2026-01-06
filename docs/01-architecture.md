# 01 - Architecture

> Module structure, database abstraction layer, state management, and testing strategy.

---

## Design Principles

1. **Separation of Concerns**: UI, database, and business logic in distinct modules
2. **Trait-Based Abstraction**: Database operations behind traits for testability and future extensibility
3. **Immutable State Updates**: Predictable state transitions, easy to test
4. **Async-First**: Non-blocking database operations, responsive UI
5. **Fail Gracefully**: Errors displayed to user, never crash

---

## Module Overview

```
src/
├── main.rs          # Entry point
├── app.rs           # Core application state machine
├── ui/              # Terminal UI components
├── db/              # Database abstraction layer
├── commands/        # Command bar parsing and execution
├── config/          # Configuration management
└── sql/             # SQL utilities (formatting, completion)
```

---

## Core Application (`app.rs`)

The application follows an Elm-like architecture with unidirectional data flow:

```
┌──────────────────────────────────────────────────────────────────┐
│                         Event Loop                                │
│  ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐       │
│  │ Terminal│───▶│  Event  │───▶│  State  │───▶│  View   │───┐   │
│  │  Event  │    │ Handler │    │ Update  │    │ Render  │   │   │
│  └─────────┘    └─────────┘    └─────────┘    └─────────┘   │   │
│       ▲                                                      │   │
│       └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────┘
```

### Application State

```rust
pub struct App {
    // Connection state
    connection: Option<ConnectionHandle>,
    connection_name: Option<String>,

    // UI state
    focus: PanelFocus,
    panels: Panels,

    // Data state
    schema_tree: SchemaTree,
    query_buffer: String,
    results: Option<QueryResults>,

    // Command bar state
    command_input: String,
    command_history: Vec<String>,

    // Global state
    status_message: Option<StatusMessage>,
    running: bool,
}

pub enum PanelFocus {
    TreeBrowser,
    QueryEditor,
    ResultsViewer,
    CommandBar,
    CellPopup,
}
```

### Event Handling

```rust
pub enum AppEvent {
    // Terminal events
    Key(KeyEvent),
    Resize(u16, u16),

    // Database events
    ConnectionEstablished(ConnectionHandle),
    ConnectionFailed(Error),
    QueryCompleted(QueryResults),
    QueryFailed(Error),
    SchemaLoaded(SchemaTree),

    // Internal events
    CommandSubmitted(Command),
    RefreshRequested,
}

impl App {
    pub fn handle_event(&mut self, event: AppEvent) -> Option<Action> {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::QueryCompleted(results) => {
                self.results = Some(results);
                self.status_message = Some(StatusMessage::success("Query completed"));
                None
            }
            // ... other handlers
        }
    }
}
```

### Actions (Side Effects)

```rust
pub enum Action {
    ExecuteQuery(String),
    Connect(ConnectionConfig),
    Disconnect,
    LoadSchema,
    ExportResults(ExportFormat, PathBuf),
    Quit,
}
```

---

## Database Abstraction (`db/`)

### Provider Trait

The core abstraction allowing multiple database backends:

```rust
// db/provider.rs

#[async_trait]
pub trait DatabaseProvider: Send + Sync {
    /// Establish connection to database
    async fn connect(config: &ConnectionConfig) -> Result<Self, DbError>
    where
        Self: Sized;

    /// Close the connection
    async fn disconnect(&mut self) -> Result<(), DbError>;

    /// Check if connection is alive
    async fn is_connected(&self) -> bool;

    /// Execute a query and return results
    async fn execute_query(&self, sql: &str) -> Result<QueryResults, DbError>;

    /// Get database schema tree
    async fn get_schema(&self) -> Result<SchemaTree, DbError>;

    /// Get columns for a specific table
    async fn get_table_columns(
        &self,
        schema: &str,
        table: &str
    ) -> Result<Vec<ColumnInfo>, DbError>;

    /// Get EXPLAIN output for a query
    async fn explain_query(&self, sql: &str) -> Result<ExplainPlan, DbError>;

    /// Get completion candidates for current context
    async fn get_completions(
        &self,
        context: &CompletionContext
    ) -> Result<Vec<Completion>, DbError>;
}
```

### PostgreSQL Implementation

```rust
// db/postgres.rs

pub struct PostgresProvider {
    client: tokio_postgres::Client,
    schema_cache: Option<SchemaTree>,
}

#[async_trait]
impl DatabaseProvider for PostgresProvider {
    async fn connect(config: &ConnectionConfig) -> Result<Self, DbError> {
        let (client, connection) = tokio_postgres::connect(
            &config.connection_string(),
            NoTls,
        ).await?;

        // Spawn connection handler
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {}", e);
            }
        });

        Ok(Self {
            client,
            schema_cache: None
        })
    }

    async fn get_schema(&self) -> Result<SchemaTree, DbError> {
        // Query information_schema and pg_catalog
        // See db/schema.rs for query details
    }

    // ... other implementations
}
```

### Data Types

```rust
// db/types.rs

pub struct QueryResults {
    pub columns: Vec<ColumnDef>,
    pub rows: Vec<Row>,
    pub execution_time: Duration,
    pub row_count: usize,
}

pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

pub enum DataType {
    Integer,
    BigInt,
    Float,
    Numeric,
    Text,
    Varchar(usize),
    Boolean,
    Date,
    Timestamp,
    Timestamptz,
    Json,
    Jsonb,
    Uuid,
    Array(Box<DataType>),
    Unknown(String),
}

pub struct Row {
    pub values: Vec<CellValue>,
}

pub enum CellValue {
    Null,
    Integer(i64),
    Float(f64),
    Text(String),
    Boolean(bool),
    Json(serde_json::Value),
    Binary(Vec<u8>),
}
```

### Schema Introspection

```rust
// db/schema.rs

pub struct SchemaTree {
    pub schemas: Vec<Schema>,
}

pub struct Schema {
    pub name: String,
    pub tables: Vec<Table>,
    pub views: Vec<View>,
    pub functions: Vec<Function>,
    pub sequences: Vec<Sequence>,
}

pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub indexes: Vec<Index>,
    pub constraints: Vec<Constraint>,
    pub row_estimate: u64,
}

pub struct Column {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub default: Option<String>,
    pub is_primary_key: bool,
}
```

---

## UI Layer (`ui/`)

### Component Trait

```rust
// ui/mod.rs

pub trait Component {
    /// Handle a key event, return true if consumed
    fn handle_key(&mut self, key: KeyEvent) -> bool;

    /// Render the component to the frame
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool);

    /// Get the minimum size this component needs
    fn min_size(&self) -> (u16, u16);
}
```

### Theme System

```rust
// ui/theme.rs

pub struct Theme {
    // Panel borders
    pub border_focused: Style,
    pub border_unfocused: Style,

    // Tree browser
    pub tree_schema: Style,
    pub tree_table: Style,
    pub tree_column: Style,
    pub tree_selected: Style,

    // Query editor
    pub editor_text: Style,
    pub editor_keyword: Style,
    pub editor_string: Style,
    pub editor_cursor: Style,

    // Results table
    pub results_header: Style,
    pub results_row_even: Style,
    pub results_row_odd: Style,
    pub results_selected: Style,
    pub results_null: Style,

    // Command bar
    pub command_prompt: Style,
    pub command_input: Style,
    pub command_autocomplete: Style,

    // Status
    pub status_success: Style,
    pub status_error: Style,
    pub status_info: Style,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            border_focused: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            border_unfocused: Style::default().fg(Color::DarkGray),
            // ... sensible defaults
        }
    }
}
```

---

## Command System (`commands/`)

### Command Parser

```rust
// commands/parser.rs

pub enum Command {
    Connect(String),           // :connect <name>
    Disconnect,                // :disconnect
    SaveConnection(String),    // :save <name>
    Refresh,                   // :refresh
    Export(ExportFormat),      // :export csv|json|sql
    Set(String, String),       // :set <key> <value>
    Help,                      // :help
    Quit,                      // :quit
}

pub fn parse_command(input: &str) -> Result<Command, ParseError> {
    let parts: Vec<&str> = input.trim().split_whitespace().collect();

    match parts.first().map(|s| s.trim_start_matches(':')) {
        Some("connect") | Some("c") => {
            let name = parts.get(1).ok_or(ParseError::MissingArgument)?;
            Ok(Command::Connect(name.to_string()))
        }
        Some("disconnect") | Some("dc") => Ok(Command::Disconnect),
        Some("refresh") | Some("r") => Ok(Command::Refresh),
        // ... other commands
        _ => Err(ParseError::UnknownCommand),
    }
}
```

### Command Autocomplete

```rust
// commands/parser.rs

pub struct CommandCompleter {
    commands: Vec<CommandDef>,
    connections: Vec<String>,
}

impl CommandCompleter {
    pub fn complete(&self, input: &str) -> Vec<Completion> {
        // Parse partial input, suggest completions
    }
}
```

---

## Configuration (`config/`)

### Connection Profiles

```rust
// config/connections.rs

#[derive(Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    #[serde(skip_serializing)]  // Don't persist passwords in plain text
    pub password: Option<String>,
    pub ssl_mode: SslMode,
}

impl ConnectionConfig {
    pub fn connection_string(&self) -> String {
        format!(
            "host={} port={} dbname={} user={}",
            self.host, self.port, self.database, self.username
        )
    }
}

pub fn load_connections() -> Result<Vec<ConnectionConfig>, ConfigError> {
    let path = dirs::home_dir()
        .ok_or(ConfigError::NoHomeDir)?
        .join(".vizgres")
        .join("connections.toml");

    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&path)?;
    let config: ConnectionsFile = toml::from_str(&content)?;
    Ok(config.connections)
}
```

---

## SQL Utilities (`sql/`)

### Formatter

```rust
// sql/formatter.rs

use sqlformat::{format, FormatOptions, QueryParams, Indent};

pub fn format_sql(sql: &str) -> String {
    let options = FormatOptions {
        indent: Indent::Spaces(2),
        uppercase: true,
        lines_between_queries: 2,
    };

    format(sql, &QueryParams::None, options)
}
```

### Autocomplete

```rust
// sql/completer.rs

pub struct SqlCompleter {
    schema: SchemaTree,
}

pub struct CompletionContext {
    pub text: String,
    pub cursor_position: usize,
    pub tables_in_query: Vec<TableRef>,
}

impl SqlCompleter {
    pub fn complete(&self, ctx: &CompletionContext) -> Vec<Completion> {
        // Tokenize query up to cursor
        // Determine context (after SELECT, FROM, WHERE, etc.)
        // Return relevant completions
    }
}
```

---

## Error Handling

### Error Types

```rust
// src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum VizgresError {
    #[error("Database error: {0}")]
    Database(#[from] DbError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Terminal error: {0}")]
    Terminal(String),
}

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Query execution failed: {0}")]
    QueryFailed(String),

    #[error("Not connected")]
    NotConnected,

    #[error("Timeout")]
    Timeout,
}
```

---

## Testing Strategy

> **CRITICAL**: Comprehensive testing is non-negotiable. Every module must have tests.

### Unit Testing Guidelines

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Test naming: test_<function>_<scenario>_<expected>

    #[test]
    fn test_parse_command_connect_with_valid_name_returns_command() {
        let result = parse_command(":connect prod");
        assert!(matches!(result, Ok(Command::Connect(name)) if name == "prod"));
    }

    #[test]
    fn test_parse_command_connect_without_name_returns_error() {
        let result = parse_command(":connect");
        assert!(matches!(result, Err(ParseError::MissingArgument)));
    }
}
```

### Module Testing Requirements

| Module | Test Type | Minimum Coverage | Notes |
|--------|-----------|------------------|-------|
| `db/provider.rs` | Trait contract tests | N/A | Define expected behavior |
| `db/postgres.rs` | Integration | 90% | Use testcontainers |
| `db/schema.rs` | Unit + Integration | 85% | Test SQL parsing |
| `db/types.rs` | Unit | 95% | Type conversion edge cases |
| `ui/tree.rs` | Unit + Snapshot | 80% | Use insta for snapshots |
| `ui/results.rs` | Unit + Snapshot | 80% | Table rendering |
| `ui/editor.rs` | Unit | 85% | Cursor movement, editing |
| `commands/parser.rs` | Unit | 100% | All command variants |
| `config/*` | Unit | 90% | Serialization/deserialization |
| `sql/formatter.rs` | Unit | 85% | Various SQL inputs |
| `sql/completer.rs` | Unit | 80% | Completion contexts |
| `app.rs` | Unit + Integration | 75% | State transitions |

### Integration Testing

```rust
// tests/integration/query_execution.rs

use testcontainers::{clients, images::postgres};

#[tokio::test]
async fn test_execute_simple_query_returns_results() {
    let docker = clients::Cli::default();
    let pg_container = docker.run(postgres::Postgres::default());

    let config = ConnectionConfig {
        host: "localhost".to_string(),
        port: pg_container.get_host_port_ipv4(5432),
        database: "postgres".to_string(),
        username: "postgres".to_string(),
        password: Some("postgres".to_string()),
        ssl_mode: SslMode::Disable,
    };

    let provider = PostgresProvider::connect(&config).await.unwrap();

    // Setup test data
    provider.execute_query("CREATE TABLE test (id INT, name TEXT)").await.unwrap();
    provider.execute_query("INSERT INTO test VALUES (1, 'Alice')").await.unwrap();

    // Test query
    let results = provider.execute_query("SELECT * FROM test").await.unwrap();

    assert_eq!(results.columns.len(), 2);
    assert_eq!(results.rows.len(), 1);
}
```

### UI Snapshot Testing

```rust
// tests/ui/tree_browser.rs

use insta::assert_snapshot;

#[test]
fn test_tree_browser_with_schemas_renders_correctly() {
    let schema_tree = SchemaTree {
        schemas: vec![
            Schema {
                name: "public".to_string(),
                tables: vec![
                    Table { name: "users".to_string(), .. },
                    Table { name: "orders".to_string(), .. },
                ],
                ..
            }
        ],
    };

    let tree = TreeBrowser::new(schema_tree);
    let output = render_to_string(&tree, Rect::new(0, 0, 30, 10));

    assert_snapshot!(output);
}
```

### End-to-End Testing

```rust
// tests/e2e/basic_workflow.rs

#[tokio::test]
async fn test_connect_query_disconnect_workflow() {
    // Start headless terminal
    let mut app = App::new_for_testing();

    // Simulate: Open command bar
    app.handle_event(AppEvent::Key(KeyCode::Char(':').into()));

    // Simulate: Type connect command
    for c in "connect test".chars() {
        app.handle_event(AppEvent::Key(KeyCode::Char(c).into()));
    }

    // Simulate: Submit command
    app.handle_event(AppEvent::Key(KeyCode::Enter.into()));

    // Verify connection state
    assert!(app.connection.is_some());

    // ... continue workflow
}
```

### Test Infrastructure

```rust
// tests/common/mod.rs

/// Create a mock database provider for testing
pub fn mock_provider() -> impl DatabaseProvider {
    MockProvider::new()
        .with_schema(test_schema())
        .with_query_response("SELECT 1", test_results())
}

/// Standard test schema for consistent testing
pub fn test_schema() -> SchemaTree {
    SchemaTree {
        schemas: vec![
            Schema {
                name: "public".to_string(),
                tables: vec![
                    Table {
                        name: "users".to_string(),
                        columns: vec![
                            Column { name: "id".to_string(), data_type: DataType::Integer, .. },
                            Column { name: "name".to_string(), data_type: DataType::Text, .. },
                        ],
                        ..
                    },
                ],
                ..
            },
        ],
    }
}
```

### CI/CD Pipeline Requirements

```yaml
# .github/workflows/ci.yml (conceptual)
test:
  steps:
    - cargo fmt --check        # Formatting
    - cargo clippy -- -D warnings  # Lints
    - cargo test               # Unit tests
    - cargo test --test '*'    # Integration tests (with testcontainers)
    - cargo llvm-cov           # Coverage report (fail if < 80%)
```

---

## Performance Considerations

### Async Architecture

```rust
// All database operations are async
async fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        // Application runs here
    });
}
```

### Virtual Scrolling for Large Results

```rust
// Only load visible rows into memory
pub struct VirtualTable {
    total_rows: usize,
    visible_range: Range<usize>,
    cached_rows: HashMap<usize, Row>,
}
```

### Schema Caching

```rust
// Cache schema, refresh on demand
impl PostgresProvider {
    pub async fn get_schema(&self) -> Result<SchemaTree, DbError> {
        if let Some(cached) = &self.schema_cache {
            return Ok(cached.clone());
        }

        let schema = self.fetch_schema_from_db().await?;
        self.schema_cache = Some(schema.clone());
        Ok(schema)
    }

    pub fn invalidate_cache(&mut self) {
        self.schema_cache = None;
    }
}
```

---

## Dependencies

```toml
# Cargo.toml

[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# TUI
ratatui = "0.28"
crossterm = "0.28"

# Database
tokio-postgres = "0.7"

# SQL utilities
sqlformat = "0.2"

# Configuration
serde = { version = "1", features = ["derive"] }
toml = "0.8"
dirs = "5"

# Error handling
thiserror = "1"
anyhow = "1"

# Async traits
async-trait = "0.1"

# JSON handling
serde_json = "1"

[dev-dependencies]
# Testing
testcontainers = "0.20"
insta = "1"
tokio-test = "0.4"
```

---

## Next Steps

See [09-roadmap.md](./09-roadmap.md) for implementation phases.

# Code Conventions

This document outlines the coding standards and conventions for vizgres.

## Rust Edition and MSRV

- **Edition**: 2024
- **MSRV**: 1.85 (minimum supported Rust version)
- **Target**: Latest stable (1.92+)

## Code Style

### Formatting

All code must be formatted with `rustfmt`:

```bash
cargo fmt
```

Configuration is in `rustfmt.toml`:
- 100 character line width
- 4 spaces for indentation
- Unix line endings

### Linting

All code must pass `clippy` without warnings:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### Naming Conventions

- **Types**: `PascalCase` (e.g., `DatabaseProvider`, `QueryResults`)
- **Functions/Methods**: `snake_case` (e.g., `execute_query`, `load_schema`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `DEFAULT_PORT`, `MAX_RETRIES`)
- **Modules**: `snake_case` (e.g., `db`, `command_bar`)

## Module Organization

### File Structure

```
src/
├── main.rs          # Binary entry point (minimal)
├── lib.rs           # Library root with re-exports
├── app.rs           # Application state machine
├── error.rs         # Error types
├── db/              # Database abstraction
│   ├── mod.rs       # Module root with re-exports
│   ├── provider.rs  # Trait definitions
│   ├── postgres.rs  # PostgreSQL implementation
│   ├── schema.rs    # Schema types
│   └── types.rs     # Data types
├── ui/              # UI components
├── commands/        # Command parsing
├── config/          # Configuration
└── sql/             # SQL utilities
```

### Module Documentation

Every module must have a module-level doc comment:

```rust
//! Brief description of module purpose
//!
//! Longer explanation if needed, including:
//! - What problems this module solves
//! - How it fits into the architecture
//! - Key types and functions
```

## Documentation

### Public API Documentation

All public items must have doc comments:

```rust
/// Brief one-line description
///
/// Longer explanation with examples if helpful.
///
/// # Arguments
/// * `arg1` - Description
/// * `arg2` - Description
///
/// # Returns
/// Description of return value
///
/// # Errors
/// When this function can return an error and why
///
/// # Examples
/// ```ignore
/// let result = function(arg1, arg2)?;
/// ```
pub fn function(arg1: Type1, arg2: Type2) -> Result<ReturnType> {
    // ...
}
```

### Error Documentation

Document all error cases:

```rust
/// Get table columns
///
/// # Errors
/// - `DbError::NotConnected` if not connected to database
/// - `DbError::QueryFailed` if table doesn't exist or query fails
async fn get_table_columns(&self, schema: &str, table: &str) -> DbResult<Vec<ColumnInfo>>;
```

## Error Handling

### Error Types

Use `thiserror` for library errors:

```rust
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
}
```

Use `anyhow` for application-level errors in `main.rs`.

### Error Propagation

Prefer `?` operator over manual error handling:

```rust
// Good
let result = operation()?;

// Avoid
let result = match operation() {
    Ok(r) => r,
    Err(e) => return Err(e.into()),
};
```

### Custom Error Context

Add context when propagating errors:

```rust
operation()
    .map_err(|e| DbError::ConnectionFailed(format!("Failed to connect: {}", e)))?;
```

## Testing

### Test Organization

- Unit tests: In the same file as the code (`#[cfg(test)] mod tests`)
- Integration tests: In `tests/integration/`
- UI tests: In `tests/ui/`

### Test Naming

```rust
#[test]
fn test_<function>_<scenario>_<expected_outcome>() {
    // Example: test_parse_command_with_valid_input_returns_command
}
```

### Test Structure

Use Arrange-Act-Assert pattern:

```rust
#[test]
fn test_something() {
    // Arrange: Set up test data
    let input = "test";

    // Act: Perform the operation
    let result = function(input);

    // Assert: Verify the outcome
    assert_eq!(result, expected);
}
```

### Async Tests

Use `#[tokio::test]` for async tests:

```rust
#[tokio::test]
async fn test_async_operation() {
    let result = async_function().await;
    assert!(result.is_ok());
}
```

### Test Coverage

Target coverage by module:
- Database layer: 90%+
- UI components: 80%+
- State management: 95%+
- Command parsing: 100%

## Async Programming

### Runtime

Use `tokio` with full features:

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // ...
}
```

### Async Traits

Use `async-trait` for trait definitions:

```rust
use async_trait::async_trait;

#[async_trait]
pub trait DatabaseProvider: Send + Sync {
    async fn connect(config: &ConnectionConfig) -> Result<Self>
    where
        Self: Sized;
}
```

### Cancellation

All async operations should be cancellation-safe.

## Performance

### Allocations

Minimize allocations in hot paths:

```rust
// Prefer borrowed data
fn process(&self, data: &str) -> &str

// Use String only when necessary
fn process_owned(&self, data: String) -> String
```

### Cloning

Avoid unnecessary clones:

```rust
// Good: Borrow when possible
fn display(&self, schema: &SchemaTree)

// Only clone when necessary
fn store(&mut self, schema: SchemaTree) {
    self.schema = Some(schema);
}
```

### Lazy Evaluation

Use lazy loading for expensive operations:

```rust
// Load schema only when needed
pub async fn get_schema(&self) -> Result<SchemaTree> {
    if let Some(cached) = &self.schema_cache {
        return Ok(cached.clone());
    }
    // Load and cache
}
```

## Dependencies

### Adding Dependencies

1. Only add well-maintained, popular crates
2. Check license compatibility (MIT/Apache-2.0)
3. Justify the addition in PR description
4. Prefer crates with minimal transitive dependencies

### Version Pinning

- Use `=` only for critical dependencies
- Prefer `^` for most dependencies (default in Cargo)
- Document version constraints in `Cargo.toml` comments

## Git Workflow

### Commit Messages

Format:
```
Brief imperative summary (50 chars or less)

Longer explanation if needed:
- What changed
- Why it changed
- Any breaking changes
```

Examples:
```
Add PostgreSQL connection implementation

Implement DatabaseProvider trait for PostgreSQL using tokio-postgres.
Includes connection pooling and error handling.
```

### Branch Naming

- Feature: `feature/description`
- Bug fix: `fix/description`
- Docs: `docs/description`

### Pull Requests

1. All tests must pass
2. Code must be formatted (`cargo fmt`)
3. No clippy warnings
4. Add tests for new functionality
5. Update documentation

## Phase-Based Development

Code should be marked with phase comments when implementing features:

```rust
// TODO: Phase 3 - Implement schema caching
fn cache_schema(&mut self, schema: SchemaTree) {
    todo!("Schema caching not yet implemented")
}
```

Phases:
1. Foundation (connection, basic query)
2. Core UI (panels, navigation)
3. Tree Browser (schema introspection)
4. Query Editor (text editing, formatting)
5. Results Viewer (table display, scrolling)
6. Polish & Integration (EXPLAIN, export)
7. Autocomplete & Intelligence
8. Connection Management

## Security

### Input Validation

Always validate user input:

```rust
// Validate connection parameters
if config.host.is_empty() {
    return Err(ConfigError::Invalid("Host cannot be empty".into()));
}
```

### SQL Injection

Use parameterized queries (never string concatenation):

```rust
// Good: Parameterized
client.query("SELECT * FROM users WHERE id = $1", &[&user_id]).await?;

// Never: String concatenation
// client.query(&format!("SELECT * FROM users WHERE id = {}", user_id)).await?;
```

### Secrets

Never commit secrets:
- Use `.gitignore` for config files
- Document keychain usage
- Provide example configs without real credentials

## Performance Benchmarks

### Adding Benchmarks

Use `criterion` for performance-critical code:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_function(c: &mut Criterion) {
    c.bench_function("function_name", |b| {
        b.iter(|| function(black_box(input)))
    });
}

criterion_group!(benches, benchmark_function);
criterion_main!(benches);
```

## Code Review

### Review Checklist

- [ ] Code follows conventions
- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] No clippy warnings
- [ ] Formatted with rustfmt
- [ ] Security considerations addressed
- [ ] Performance impact considered

### Giving Feedback

- Be constructive and specific
- Suggest alternatives, don't just criticize
- Explain *why* something should change
- Acknowledge good code

## Questions?

Open an issue or discussion on GitHub for clarification on conventions.

# Test Suite

This directory contains the test suite for vizgres.

## Structure

```
tests/
├── common/           # Shared test utilities and helpers
├── integration/      # Integration tests (require real PostgreSQL)
├── ui/              # UI snapshot tests
└── README.md        # This file
```

## Running Tests

### Unit Tests

Unit tests are colocated with the source code in `src/` modules.

```bash
cargo test --lib
```

### Integration Tests

Integration tests require Docker for testcontainers:

```bash
# Start PostgreSQL container and run integration tests
cargo test --test '*'
```

### All Tests

```bash
cargo test
```

### Ignored Tests

Some tests are marked with `#[ignore]` because they depend on features
not yet implemented. To run them:

```bash
cargo test -- --ignored
```

## Test Categories

### Phase 1 Tests
- Database connection
- Query execution
- Basic error handling

### Phase 3 Tests
- Schema introspection
- Tree browser rendering
- Schema navigation

### Phase 4 Tests
- Query editor functionality
- Cursor movement
- Text editing

### Phase 5 Tests
- Results table rendering
- Result scrolling
- Cell inspection

## Writing Tests

### Unit Tests

Place unit tests in the same file as the code being tested:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        // Test code
    }
}
```

### Integration Tests

Create new files in `tests/integration/`:

```rust
#[tokio::test]
async fn test_database_feature() {
    // Use testcontainers for real database
}
```

### UI Snapshot Tests

Use `insta` for snapshot testing:

```rust
#[test]
fn test_ui_component() {
    let output = render_component();
    assert_snapshot!(output);
}
```

## Test Helpers

Common test utilities are in `tests/common/mod.rs`:

- `test_schema()` - Standard test database schema
- `test_connection_config()` - Test connection configuration
- `setup()` / `teardown()` - Test environment management

## Coverage

Target coverage levels:

- Database layer: 90%+
- UI components: 80%+
- State management: 95%+
- Command parsing: 100%

Generate coverage report:

```bash
cargo llvm-cov --html
open target/llvm-cov/html/index.html
```

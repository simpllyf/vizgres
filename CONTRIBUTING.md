# Contributing to vizgres

Thank you for considering contributing to vizgres! This document provides guidelines and instructions for contributing.

## Code of Conduct

Please be respectful and constructive in all interactions. We welcome contributors of all skill levels.

## Getting Started

### Prerequisites

- Rust 1.93 or later (check `rust-toolchain.toml`)
- Docker and Docker Compose (for integration tests)
- Git

### Setting Up the Development Environment

1. Clone the repository:
   ```bash
   git clone https://github.com/simpllyf/vizgres.git
   cd vizgres
   ```

2. Build the project:
   ```bash
   cargo build
   ```

3. Run unit tests:
   ```bash
   cargo test --lib
   ```

4. Run integration tests (requires Docker):
   ```bash
   docker-compose -f docker-compose.test.yml up -d
   cargo test --test integration
   docker-compose -f docker-compose.test.yml down
   ```

## Development Workflow

### Before Starting Work

1. Check existing issues and pull requests
2. For significant changes, open an issue first to discuss the approach
3. Create a feature branch from `main`

### Code Style

We enforce consistent code style through automated tools:

- **Formatting**: Run `cargo fmt` before committing
- **Linting**: Run `cargo clippy --all-targets -- -D warnings`
- **Tests**: Run `cargo test` to ensure all tests pass

### Pre-commit Checklist

Before submitting a pull request:

1. `cargo fmt --all` - Format all code
2. `cargo clippy --all-targets -- -D warnings` - No clippy warnings
3. `cargo test --lib` - All unit tests pass
4. `cargo test --doc` - Doc tests pass
5. Update documentation if needed

### Commit Messages

- Use clear, descriptive commit messages
- Start with a verb (Add, Fix, Update, Remove, Refactor)
- Keep the first line under 72 characters
- Reference issues when applicable: `Fix #123`

Example:
```
Add JSON pretty-printing in inspector panel

- Format JSON values with indentation
- Add scroll support for long content
- Update tests for new formatting

Closes #42
```

## Project Structure

```
src/
├── main.rs         # Entry point, terminal setup, event loop
├── lib.rs          # Library exports
├── app.rs          # Application state and event handling
├── error.rs        # Error type hierarchy
├── config/         # Configuration management
├── commands/       # Command parsing
├── db/             # Database layer (PostgreSQL)
└── ui/             # TUI components
    ├── tree.rs     # Schema tree browser
    ├── editor.rs   # Query editor
    ├── results.rs  # Results viewer
    └── ...

tests/
├── common/         # Shared test utilities
├── fixtures/       # Test data (SQL)
└── integration/    # Integration tests
```

## Testing

### Unit Tests

Unit tests are co-located with source code using `#[cfg(test)]` modules:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example() {
        // ...
    }
}
```

### Integration Tests

Integration tests require a PostgreSQL database. Use Docker Compose:

```bash
# Start test database
docker-compose -f docker-compose.test.yml up -d

# Run integration tests
cargo test --test integration

# Stop test database
docker-compose -f docker-compose.test.yml down
```

### Test Database Configuration

Integration tests use these defaults (configurable via environment variables):

| Variable | Default |
|----------|---------|
| `TEST_DB_HOST` | localhost |
| `TEST_DB_PORT` | 5433 |
| `TEST_DB_NAME` | test_db |
| `TEST_DB_USER` | test_user |
| `TEST_DB_PASSWORD` | test_password |

## Pull Request Process

1. **Fork** the repository and create your branch from `main`
2. **Make** your changes following the guidelines above
3. **Test** thoroughly (unit tests + integration tests if applicable)
4. **Push** your branch and open a pull request
5. **Describe** your changes clearly in the PR description
6. **Respond** to review feedback

### PR Description Template

```markdown
## Summary
Brief description of what this PR does.

## Changes
- List of specific changes

## Testing
How was this tested?

## Related Issues
Closes #XX (if applicable)
```

## Architecture Guidelines

### Adding New Features

1. Start with the data model (in `db/types.rs` or `db/schema.rs`)
2. Implement database queries (in `db/postgres.rs`)
3. Add UI components (in `ui/`)
4. Wire up in `app.rs`
5. Add tests at each layer

### Error Handling

- Use `thiserror` for typed errors in library code
- Use `anyhow` for error context in `main.rs`
- Never use `unwrap()` in production code
- Prefer `?` operator over manual match

### UI Components

UI components implement the `Component` trait:

```rust
pub trait Component {
    fn handle_key(&mut self, key: KeyEvent) -> bool;
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool);
}
```

## Questions?

- Open an issue for bugs or feature requests
- Check existing issues for common questions

Thank you for contributing!

# Code Conventions

## Rust Edition and MSRV

- **Edition**: 2024
- **MSRV**: 1.85
- **Target**: Latest stable

## Code Style

### Formatting

```bash
cargo fmt
```

Configuration in `rustfmt.toml`: 100 char lines, 4 space indent, Unix line endings.

### Linting

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### Naming

- **Types**: `PascalCase` (e.g., `PostgresProvider`, `QueryResults`)
- **Functions/Methods**: `snake_case` (e.g., `execute_query`, `get_schema`)
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Modules**: `snake_case`

## Architecture

### Data Flow

Events → `App::handle_event()` → `Action` → Main loop executes → UI renders from state.

### Component Trait

Each UI panel implements:
```rust
pub trait Component {
    fn handle_key(&mut self, key: KeyEvent) -> bool;
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool);
}
```

### Error Handling

Use `thiserror` for typed errors, `anyhow` in `main.rs`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
}
```

Prefer `?` operator over manual match.

## Testing

### Organization

Unit tests live in the same file as the code (`#[cfg(test)] mod tests`).

### Running

```bash
cargo test              # All tests
cargo test --lib        # Unit tests only
cargo test test_name    # Specific test
```

### Test Naming

```rust
#[test]
fn test_<function>_<scenario>() { ... }
```

## Git

### Commit Messages

Brief imperative summary (50 chars or less), then details if needed.

### Quality

Before committing:
1. `cargo fmt`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test`

## Security

- Use parameterized queries (never string concatenation)
- Never commit secrets
- Validate user input at system boundaries

# Code Conventions

## Rust Edition and MSRV

- **Edition**: 2024
- **MSRV**: 1.93
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

### Connection Lifecycle

Connection is established **before** the TUI starts. The app validates
the URL, connects, and loads the schema. If anything fails, the process
exits with a clear error — the TUI never opens in a disconnected state.

### Component Trait

Each UI panel implements:
```rust
pub trait Component {
    fn handle_key(&mut self, key: KeyEvent) -> bool;
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool);
}
```

### UI Rendering Patterns

- **Panels** use `render_panel()` in `render.rs` for consistent focus styling
- **Overlays** (like Inspector) are floating popups rendered last with `Clear` + shadow
- **Status bar** is partitioned: left = persistent info, right = ephemeral toast
- **Errors** from queries display in the results panel, not the status bar
- Focus is shown via border color (cyan), title arrow prefix (▸), and bold title

### Clipboard

On Linux, the `arboard::Clipboard` object must be kept alive (stored in `App`)
to avoid the "dropped too quickly" race condition. Keyboard copy (`y`/`Y`) is
the primary copy mechanism — mouse selection in TUI includes terminal padding
(this is a fundamental terminal grid limitation, not a bug).

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

## Keybindings

### Global
- `Ctrl+Q` — Quit
- `Ctrl+P` — Open command bar (works from any panel)
- `Tab` / `Shift+Tab` — Cycle panel focus

### Query Editor
- `F5` / `Ctrl+Enter` — Execute query
- `Ctrl+L` — Clear editor
- `Delete` — Forward delete

### Results Viewer
- `h/j/k/l` or arrow keys — Navigate cells
- `Enter` — Open inspector popup
- `y` — Copy cell to clipboard
- `Y` — Copy row to clipboard
- `g` / `G` — Jump to first/last row

### Inspector (popup)
- `Esc` — Close
- `y` — Copy content
- `j/k` or arrows — Scroll

### Command Bar
- `/refresh` / `/r` — Reload schema
- `/clear` / `/cl` — Clear query editor
- `/help` / `/h` — Show help
- `/quit` / `/q` — Quit

## Security

- Use parameterized queries (never string concatenation)
- Never commit secrets
- Validate user input at system boundaries

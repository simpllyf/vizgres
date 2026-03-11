# Code Conventions

## Rust Edition and MSRV

- **Edition**: 2024
- **MSRV**: 1.93
- **Target**: Latest stable

## Code Style

### Formatting and Linting

```bash
just fmt    # Format code
just lint   # Format check + clippy (same as CI)
```

Configuration in `rustfmt.toml`: 100 char lines, 4 space indent, Unix line endings.

### Naming

- **Types**: `PascalCase` (e.g., `PostgresProvider`, `QueryResults`)
- **Functions/Methods**: `snake_case` (e.g., `execute_query`, `get_schema`)
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Modules**: `snake_case`

## Architecture

### Data Flow

Events → `App::handle_event()` → `Action` → Main loop executes → UI renders from state.

### Connection Lifecycle

When a target is provided on the CLI, the connection is established
**before** the TUI starts — if it fails, the process exits with a clear error.
Without a target, the TUI opens in disconnected state and shows the
connection dialog. Connections are managed per-tab via `ConnectionManager`
with automatic reconnection on connection loss.

### Component Trait

Each UI panel implements:
```rust
pub trait Component {
    fn handle_key(&mut self, key: KeyEvent) -> ComponentAction;
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, theme: &Theme);
}
```

Components only handle free-form text input (editor, command bar). All
navigation and action keybindings are resolved by `KeyMap` before reaching
the component. `ComponentAction::Consumed`/`Ignored` signals whether the
component consumed the event.

### Theme

All colors are defined in `src/ui/theme.rs`. Components receive `&Theme`
in their `render()` method and use `theme.field_name` instead of hardcoded
`Color::` literals. To change the palette, edit only `theme.rs`.

### KeyMap (Data-Driven Keybindings)

All keybindings are defined as data in `src/keymap.rs`:

```rust
// In KeyMap::default()
editor.insert(
    KeyBind { code: KeyCode::F(5), modifiers: KeyModifiers::NONE },
    KeyAction::ExecuteQuery,
);
```

**To add a new keybinding:**
1. Add a variant to `KeyAction` if the action is new
2. Add the binding entry in `KeyMap::default()` under the appropriate panel
3. Handle the `KeyAction` variant in `App::execute_key_action()`

**Resolution order:** `KeyMap::resolve()` checks global bindings first, then
panel-specific bindings. Unresolved keys fall through to `Component::handle_key()`
for text input.

### UI Rendering Patterns

- **Panels** use `render_panel()` in `render.rs` for consistent focus styling
- **Overlays** (like Inspector) are floating popups rendered last with `Clear` + shadow
- **Status bar** is partitioned: left = ephemeral toast, right = connection info
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
just test              # Unit + doc tests (same as CI)
just db-up             # Start test PostgreSQL
just test-integration  # Run integration tests
just db-down           # Stop test PostgreSQL
cargo test test_name   # Specific test
```

### Integration Test Database

Integration tests use these defaults (configurable via environment variables):

| Variable | Default |
|----------|---------|
| `TEST_DB_HOST` | localhost |
| `TEST_DB_PORT` | 5433 |
| `TEST_DB_NAME` | test_db |
| `TEST_DB_USER` | test_user |
| `TEST_DB_PASSWORD` | test_password |

### Test Naming

```rust
#[test]
fn test_<function>_<scenario>() { ... }
```

## Git

### Commit Messages

Use [conventional commits](https://www.conventionalcommits.org/), lowercase:

```
feat: add table data preview
fix: handle null values in results viewer
docs: update keybinding reference
build: migrate CI to just
```

### Quality

Before committing:
1. `just lint`
2. `just test`

## Keybindings

### Global
- `Ctrl+Q` — Quit
- `Ctrl+P` — Open command bar (works from any panel)
- `Tab` / `Shift+Tab` — Cycle panel focus

### Query Editor
- `F5` / `Ctrl+Enter` — Execute query
- `Ctrl+E` — EXPLAIN ANALYZE
- `Ctrl+L` — Clear editor
- `Ctrl+Z` / `Ctrl+Shift+Z` — Undo / Redo
- `Ctrl+Alt+F` — Format SQL
- `Ctrl+Up/Down` — Query history
- `Escape` — Cancel running query

### Results Viewer
- `h/j/k/l` or arrow keys — Navigate cells
- `Enter` — Open inspector popup
- `v` — Toggle view mode (vertical / explain tree↔text)
- `Shift+H` / `Shift+L` — Narrow / Widen column
- `Shift+R` — Reset column widths
- `y` — Copy cell to clipboard
- `Y` — Copy row to clipboard
- `Ctrl+S` — Export CSV
- `Ctrl+J` — Export JSON
- `g` / `G` — Jump to first/last row
- `n` / `p` — Next / Previous page

### Inspector (popup)
- `Esc` — Close
- `y` — Copy content
- `j/k` or arrows — Scroll

### Command Bar
- `/connect [url]` — Connect to database
- `/refresh` / `/r` — Reload schema
- `/save-query [name]` — Save current query
- `/clear` / `/cl` — Clear query editor
- `/help` / `/h` — Show help
- `/quit` / `/q` — Quit

## Security

- Use parameterized queries (never string concatenation)
- Never commit secrets
- Validate user input at system boundaries

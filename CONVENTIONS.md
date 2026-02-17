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

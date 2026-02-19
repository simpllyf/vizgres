# AI Agent Guide

See [CONVENTIONS.md](CONVENTIONS.md) for code style, architecture, and testing conventions.

## Module Map

| Module | Purpose |
|--------|---------|
| `main.rs` | Terminal init, event loop, action executor |
| `app.rs` | App state, event→action routing, focus management |
| `keymap.rs` | Data-driven keybinding config |
| `history.rs` | Query history ring buffer with disk persistence |
| `db/postgres.rs` | PostgreSQL connection, query execution, schema loading |
| `db/types.rs` | CellValue, DataType, QueryResults, Row, ColumnDef |
| `db/schema.rs` | SchemaTree, Schema, Table, Column |
| `ui/render.rs` | Top-level render function |
| `ui/layout.rs` | Panel layout calculation (AppLayout struct) |
| `ui/tree.rs` | Schema tree browser (flattened items, expand/collapse) |
| `ui/editor.rs` | Multi-line SQL editor with undo/redo |
| `ui/results.rs` | Results table with cell-level navigation |
| `ui/inspector.rs` | Cell value inspector (floating popup) |
| `ui/help.rs` | Scrollable help overlay |
| `ui/command_bar.rs` | Command input bar |
| `ui/theme.rs` | Colors and styles (single source of truth) |
| `commands/parser.rs` | `/command` parsing |
| `config/connections.rs` | URL parsing, SSL config, libpq connection string |
| `error.rs` | Error hierarchy (VizgresError, DbError, ConfigError, CommandError) |

## How to Extend

### Adding a Command

1. Add variant to `Command` enum in `src/commands/parser.rs`
2. Add match arm in `parse_command()`
3. Handle in `App::execute_command()` in `src/app.rs`
4. Write tests

### Adding a UI Component

1. Create `src/ui/<component>.rs`
2. Implement `Component` trait (`handle_key` → `ComponentAction`, `render`)
3. Add module to `src/ui/mod.rs`
4. Wire into `App` struct and key handling in `app.rs`
5. Add to render function in `ui/render.rs`

### Adding a Keybinding

1. Add a variant to `KeyAction` if the action is new
2. Add the binding entry in `KeyMap::default()` under the appropriate panel
3. Handle the `KeyAction` variant in `App::execute_key_action()`

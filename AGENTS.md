# AI Agent Guide

See [CONVENTIONS.md](CONVENTIONS.md) for code style, architecture, and testing conventions.

## Module Map

| Module | Purpose |
|--------|---------|
| `main.rs` | Terminal init, event loop, action executor |
| `app/mod.rs` | App struct, types, constructors, helper methods |
| `app/event_handler.rs` | Event dispatch (key, query results, connection loss) |
| `app/actions.rs` | Key action execution (execute_key_action) |
| `app/sql_utils.rs` | SQL analysis (destructive/write detection, meta-commands, error position) |
| `keymap.rs` | Data-driven keybinding config |
| `history.rs` | Query history ring buffer with disk persistence |
| `connection_manager.rs` | Per-tab connection management with auto-reconnect |
| `export.rs` | CSV/JSON export |
| `db/postgres.rs` | PostgreSQL connection, query execution, schema loading |
| `db/types.rs` | CellValue, DataType, QueryResults, Row, ColumnDef |
| `db/schema.rs` | SchemaTree, Schema, Table, Column, PaginatedVec |
| `ui/render.rs` | Top-level render function |
| `ui/layout.rs` | Panel layout calculation (AppLayout struct) |
| `ui/tree.rs` | Schema tree browser (flattened items, expand/collapse) |
| `ui/editor.rs` | Multi-line SQL editor with undo/redo |
| `ui/results.rs` | Results table with cell-level navigation and column resize |
| `ui/explain.rs` | EXPLAIN tree viewer with color-coded timing |
| `ui/inspector.rs` | Cell value inspector (floating popup) |
| `ui/help.rs` | Scrollable help overlay (cached rendering) |
| `ui/command_bar.rs` | Command input bar |
| `ui/theme.rs` | Color themes (dark, light, midnight, ember) |
| `commands/parser.rs` | `/command` parsing |
| `config/connections.rs` | URL parsing, SSL config, libpq connection string |
| `config/settings.rs` | App settings with defaults |
| `error.rs` | Error hierarchy (VizgresError, DbError, ConfigError, CommandError) |

## How to Extend

### Adding a Command

1. Add variant to `Command` enum in `src/commands/parser.rs`
2. Add match arm in `parse_command()`
3. Handle in `App::execute_command()` in `src/app/mod.rs`
4. Write tests

### Adding a UI Component

1. Create `src/ui/<component>.rs`
2. Implement `Component` trait (`handle_key` → `ComponentAction`, `render`)
3. Add module to `src/ui/mod.rs`
4. Wire into `App` struct in `src/app/mod.rs`
5. Handle key actions in `src/app/actions.rs`
6. Add to render function in `ui/render.rs`

### Adding a Keybinding

1. Add a variant to `KeyAction` if the action is new
2. Add the binding entry in `KeyMap::default()` under the appropriate panel
3. Handle the `KeyAction` variant in `execute_key_action()` in `src/app/actions.rs`

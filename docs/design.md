# Vizgres Design

## Overview

Vizgres is a keyboard-driven PostgreSQL TUI client. The goal is to replace DataGrip/pgAdmin for daily use — fast startup, keyboard-first, terminal-native.

## Status: MVP Complete

The MVP provides: connect to a database, browse schema, write and execute queries, view results, and inspect cell values. Everything below under "Architecture" is implemented and working.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│ main.rs - Terminal setup, event loop, action runner  │
├──────────┬──────────────────────────────────────────┤
│ Tree     │ Editor         (top-right)               │
│ Browser  ├──────────────────────────────────────────┤
│ (left)   │ Results Viewer  (bottom-right)           │
│          │          ┌─────────────────┐             │
│          │          │ Inspector (opt) │             │
├──────────┴──────────┴─────────────────┴─────────────┤
│ Command Bar / Status Bar (bottom 1 row)             │
└─────────────────────────────────────────────────────┘
```

### Data Flow

1. Terminal events → `App::handle_event()` → returns `Action`
2. Main loop executes `Action` (spawn DB tasks, quit, etc.)
3. DB task results sent back via `tokio::sync::mpsc` channel
4. `App` state updated → `render(&app, &mut frame)` draws UI

### Key Design Decisions

- **No trait abstraction for DB**: `PostgresProvider` used directly. Postgres-only for now.
- **Component trait**: Each UI panel implements `handle_key` + `render`.
- **Manual table rendering**: Results viewer renders cells manually (not ratatui Table widget) to support cell-level selection highlight.
- **Inspector as split panel**: Not an overlay. Right-side 40% split of results area when open.

## Module Map

| Module | Purpose |
|--------|---------|
| `main.rs` | Terminal init, event loop, action executor |
| `app.rs` | App state, event→action routing, focus management |
| `db/postgres.rs` | PostgreSQL connection, query execution, schema loading |
| `db/types.rs` | CellValue, DataType, QueryResults, Row, ColumnDef |
| `db/schema.rs` | SchemaTree, Schema, Table, Column |
| `ui/render.rs` | Top-level render function |
| `ui/layout.rs` | Panel layout calculation |
| `ui/tree.rs` | Schema tree browser |
| `ui/editor.rs` | Multi-line SQL editor |
| `ui/results.rs` | Results table with cell navigation |
| `ui/inspector.rs` | Cell value inspector (split panel) |
| `ui/command_bar.rs` | Command input bar |
| `ui/theme.rs` | Colors and styles |
| `commands/parser.rs` | `:command` parsing |
| `commands/handlers.rs` | Command execution |
| `config/connections.rs` | Connection profiles + URL parsing |

## Type Mapping (PostgreSQL → Vizgres)

| PG Type | DataType | CellValue |
|---------|----------|-----------|
| INT2/INT4/INT8 | SmallInt/Integer/BigInt | Integer(i64) |
| FLOAT4/FLOAT8 | Real/Double | Float(f64) |
| TEXT/VARCHAR/NAME | Text/Varchar | Text(String) |
| BOOL | Boolean | Boolean(bool) |
| JSON/JSONB | Json/Jsonb | Json(serde_json::Value) |
| TIMESTAMP/TIMESTAMPTZ | Timestamp/TimestampTz | DateTime(String) |
| UUID | Uuid | Uuid(String) |
| Fallback | Unknown | Text(to_string) |

## Keyboard Reference

| Key | Context | Action |
|-----|---------|--------|
| Tab | Global | Cycle focus: Tree → Editor → Results |
| Ctrl+Q | Global | Quit |
| `:` | Global (not editor) | Open command bar |
| j/k | Tree, Results | Move down/up |
| h/l | Tree | Collapse/expand |
| h/l | Results | Move left/right column |
| Enter | Tree | Toggle expand/collapse |
| Enter | Results | Open inspector |
| Escape | Inspector, Command | Close, return focus |
| Ctrl+Enter | Editor | Execute query |
| y | Results | Copy cell |
| Y | Results | Copy row |
| y | Inspector | Copy content |
| Arrows | Editor | Cursor movement |
| Home/End | Editor | Start/end of line |

## MVP Limitations

Things the MVP intentionally does not handle:

- No syntax highlighting in the editor
- No SQL autocomplete
- No connection profile file management (URL-only via CLI arg or `:connect`)
- No query history
- No multiple result tabs
- No resizable panels
- No mouse support
- Queries block the UI while running (no async cancellation)

## Post-MVP Roadmap

Roughly in priority order. Each item is independent and can be tackled standalone.

### Tier 1: Daily-driver essentials

| Feature | What | Key files |
|---------|------|-----------|
| **SQL syntax highlighting** | Keyword/string/number coloring in editor | `ui/editor.rs` — tokenize lines, apply styles per token |
| **Query history** | Up/Down in editor recalls previous queries | `app.rs` — `Vec<String>` history, `ui/editor.rs` — recall keybinds |
| **Async query execution** | Non-blocking queries with cancel (Ctrl+C) | `main.rs` — spawn query in tokio task, add `Action::CancelQuery` |
| **Connection profiles** | Save/load connections from `~/.vizgres/connections.toml` | `config/connections.rs` — already has `load_connections()`, wire into `:connect <name>` |
| **Error location in editor** | Highlight the line/column where a SQL error occurred | `app.rs` — parse pg error position, `ui/editor.rs` — error gutter |

### Tier 2: Productivity features

| Feature | What | Key files |
|---------|------|-----------|
| **Schema autocomplete** | Tab-complete table/column names in editor | New `sql/completer.rs` — build from SchemaTree, `ui/editor.rs` — popup widget |
| **EXPLAIN plan viewer** | Visual query plan display | New `ui/explain.rs`, `db/postgres.rs` — `EXPLAIN (FORMAT JSON)` |
| **Export results** | CSV, JSON, SQL INSERT export | New `export.rs`, `commands/parser.rs` — `:export csv filename` |
| **Multiple result tabs** | Keep results from previous queries | `app.rs` — `Vec<QueryResults>`, tab switching keybinds |
| **Table data preview** | Enter on table in tree → `SELECT * FROM table LIMIT 100` | `app.rs` — wire tree Enter to auto-query |
| **Find/replace in editor** | `/pattern` search, `Ctrl+H` replace | `ui/editor.rs` — search state, highlight matches |
| **Resizable panels** | Drag panel borders or `Ctrl+Arrow` to resize | `ui/layout.rs` — store user-adjusted widths/heights |

### Tier 3: Power features

| Feature | What |
|---------|------|
| **Transaction support** | `:begin`, `:commit`, `:rollback` commands, transaction indicator in status bar |
| **Multi-database** | Multiple simultaneous connections, switch with `:use <name>` |
| **Vim mode** | Full vim keybindings in editor (normal/insert/visual modes) |
| **SQL formatter** | `sqlformat` crate already in deps — wire to `:format` command |
| **Saved queries** | `:save <name>`, `:load <name>` for frequently-used queries |
| **Column stats** | `Ctrl+I` on column in results → count, distinct, min, max, nulls |
| **Mouse support** | Click to focus panel, click cell to select, scroll wheel |

### Architecture notes for future work

**Adding a new UI panel**: Create `src/ui/foo.rs` implementing `Component` trait → add field to `App` → add `PanelFocus::Foo` variant → wire key routing in `app.rs` → render in `ui/render.rs`.

**Adding a new command**: Add variant to `Command` enum in `commands/parser.rs` → match in `parse_command()` → handle in `App::execute_command()`.

**Making queries async**: The main loop currently awaits queries inline. To make them non-blocking: spawn the query as a tokio task, send `AppEvent::QueryCompleted` on the channel when done, add `AppEvent::QueryCancelled` and wire Ctrl+C to drop the task handle.

**Trait abstraction**: If a second database backend is ever needed, extract `PostgresProvider`'s public methods into a `DatabaseProvider` trait. Until then, the concrete struct is simpler.

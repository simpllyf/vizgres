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

## Current Limitations

- No syntax highlighting in the editor
- No SQL autocomplete
- No connection profile file management (URL-only via CLI arg or `:connect`)
- No query history
- No multiple result tabs
- No resizable panels
- No mouse support
- Queries block the UI while running (no async cancellation)

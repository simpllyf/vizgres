# Vizgres Documentation

> A fast, keyboard-driven PostgreSQL client for the terminal.

## Vision

Vizgres aims to be a lightweight alternative to GUI database tools like DataGrip. It provides a focused, distraction-free interface for exploring PostgreSQL databases, running queries, and inspecting results—all without leaving your terminal.

**Core Principles:**
- **Keyboard-first**: Every action accessible via keyboard shortcuts
- **Fast**: Rust-powered, minimal latency, responsive UI
- **Focused**: PostgreSQL-specific, not trying to be everything
- **Clean**: Uncluttered interface, information when you need it

---

## UI Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ [vizgres]                    localhost:5432/mydb                    Ctrl+? │
├─────────────────────┬───────────────────────────────────────────────────────┤
│ ▼ public            │ SELECT                                               │
│   ▼ Tables          │   u.id,                                              │
│     ▶ users         │   u.name,                                            │
│     ▶ orders        │   u.email                                            │
│     ▶ products      │ FROM users u                                         │
│   ▶ Views           │ WHERE u.active = true                                │
│   ▶ Functions       │ ORDER BY u.created_at DESC                           │
│ ▶ auth              │ LIMIT 100;                                           │
│ ▶ analytics         │                                                      │
│                     │                                                      │
│                     ├───────────────────────────────────────────────────────┤
│                     │  id │ name       │ email            │ created_at     │
│                     │─────┼────────────┼──────────────────┼────────────────│
│                     │  1  │ Alice      │ alice@example.co │ 2024-01-15     │
│                     │  2  │ Bob        │ bob@example.com  │ 2024-01-14     │
│                     │  3  │ Charlie    │ charlie@test.com │ 2024-01-13     │
│                     │                                     [Row 1-3 of 847] │
├─────────────────────┴───────────────────────────────────────────────────────┤
│ > :connect prod                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Panel Layout

| Panel | Purpose | Focus Highlight |
|-------|---------|-----------------|
| **Tree Browser** (left) | Navigate database objects | Blue border |
| **Query Editor** (top-right) | Write and edit SQL | Green border |
| **Results Viewer** (bottom-right) | Display query results | Yellow border |
| **Command Bar** (bottom) | Quick commands, connection switching | Magenta border |

---

## Feature Summary

### Core Features
- **Tree Browser**: Hierarchical view of schemas, tables, views, functions, indexes
- **Query Editor**: Multi-line SQL editing with syntax awareness
- **Results Viewer**: Scrollable table with column resizing
- **Command Bar**: Quick commands with autocomplete
- **Connection Manager**: Save, load, switch between connections

### Quality of Life
- **SQL Formatting**: One-keystroke formatting (`Ctrl+Shift+F`)
- **SQL Fixer**: Auto-correct typos on execute (SELEC → SELECT, usres → users)
- **JSONB Inspector**: Pretty-printed JSON in popup panel
- **EXPLAIN Visualizer**: Query plan as navigable tree
- **Schema Autocomplete**: Context-aware suggestions
- **Cell Popup**: Inspect long/complex values in overlay

---

## Detailed Documentation

| Document | Description |
|----------|-------------|
| [01-architecture.md](./01-architecture.md) | Module structure, database abstraction, state management |
| [02-ui-layout.md](./02-ui-layout.md) | Panel system, focus management, visual design |
| [03-tree-browser.md](./03-tree-browser.md) | Database object tree, lazy loading, refresh |
| [04-query-editor.md](./04-query-editor.md) | Text editing, formatting, history |
| [05-results-viewer.md](./05-results-viewer.md) | Table rendering, scrolling, cell inspection |
| [06-command-bar.md](./06-command-bar.md) | Command palette, autocomplete, available commands |
| [07-keyboard-shortcuts.md](./07-keyboard-shortcuts.md) | Complete keybinding reference |
| [08-connections.md](./08-connections.md) | Configuration format, credential storage |
| [09-roadmap.md](./09-roadmap.md) | Implementation phases, future considerations |

---

## Quick Reference

### Essential Shortcuts

| Key | Action |
|-----|--------|
| `Tab` | Cycle focus between panels |
| `Ctrl+1/2/3` | Jump to Tree/Query/Results |
| `Ctrl+Enter` | Execute query |
| `Ctrl+Shift+F` | Format SQL |
| `Ctrl+E` | Show EXPLAIN for query |
| `Enter` (on cell) | Open cell inspector popup |
| `:` | Open command bar |
| `Ctrl+Q` | Quit |

### Common Commands

| Command | Action |
|---------|--------|
| `:connect <name>` | Switch to saved connection |
| `:disconnect` | Close current connection |
| `:save <name>` | Save current connection |
| `:refresh` | Refresh tree browser |
| `:export csv` | Export results to CSV |

---

## Technology Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Language | Rust | Performance, safety, excellent TUI ecosystem |
| TUI Framework | `ratatui` | Mature, actively maintained, flexible |
| Terminal Backend | `crossterm` | Cross-platform (Linux, macOS, Windows) |
| PostgreSQL Driver | `tokio-postgres` | Async, production-ready |
| SQL Formatter | `sqlformat` | Pure Rust, PostgreSQL-aware |
| Async Runtime | `tokio` | Industry standard, great ecosystem |
| Config Format | TOML | Human-readable, Rust-native support |

---

## Testing Strategy

> **Testing is critical.** This section outlines our approach to comprehensive testing.

See [01-architecture.md](./01-architecture.md) for detailed testing guidelines.

### Testing Layers

| Layer | Type | Tools | Coverage Goal |
|-------|------|-------|---------------|
| Database Abstraction | Unit + Integration | `tokio-test`, testcontainers | 90%+ |
| UI Components | Unit + Snapshot | `insta` for snapshots | 80%+ |
| State Management | Unit | Standard Rust tests | 95%+ |
| Key Bindings | Unit | Action mapping tests | 100% |
| SQL Formatting | Unit | Input/output pairs | 90%+ |
| End-to-End | Integration | Headless terminal simulation | Critical paths |

### Test Requirements

1. **All PRs must pass CI** - No merging with failing tests
2. **New features require tests** - Feature code must include test coverage
3. **Regression tests** - Bugs fixed must have corresponding test
4. **Snapshot tests for UI** - Visual regressions caught automatically

---

## Project Structure (Planned)

```
vizgres/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, arg parsing
│   ├── app.rs               # Application state, main loop
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── layout.rs        # Panel arrangement
│   │   ├── tree.rs          # Tree browser widget
│   │   ├── editor.rs        # Query editor widget
│   │   ├── results.rs       # Results table widget
│   │   ├── command_bar.rs   # Command input widget
│   │   ├── cell_popup.rs    # Cell inspector overlay
│   │   └── theme.rs         # Colors, borders, highlights
│   ├── db/
│   │   ├── mod.rs
│   │   ├── provider.rs      # DatabaseProvider trait
│   │   ├── postgres.rs      # PostgreSQL implementation
│   │   ├── schema.rs        # Schema introspection queries
│   │   └── types.rs         # Data type mappings
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── parser.rs        # Command parsing
│   │   └── handlers.rs      # Command execution
│   ├── config/
│   │   ├── mod.rs
│   │   ├── connections.rs   # Connection profiles
│   │   └── settings.rs      # User preferences
│   └── sql/
│       ├── mod.rs
│       ├── formatter.rs     # SQL formatting wrapper
│       └── completer.rs     # Autocomplete logic
├── tests/
│   ├── integration/         # End-to-end tests
│   ├── ui/                  # UI snapshot tests
│   └── db/                  # Database integration tests
└── docs/                    # This documentation
```

---

## Configuration Location

All configuration stored in `~/.vizgres/`:

```
~/.vizgres/
├── config.toml      # Global settings
├── connections.toml # Saved connection profiles
└── history.sql      # Query history (optional)
```

See [08-connections.md](./08-connections.md) for format details.

---

## License

MIT License - See LICENSE file in repository root.

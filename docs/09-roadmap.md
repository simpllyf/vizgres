# 09 - Roadmap

> Implementation phases, milestones, and future considerations.

---

## Development Philosophy

1. **Vertical slices**: Each phase delivers working, usable functionality
2. **Test-first**: Write tests alongside (or before) features
3. **Iterate**: Ship minimal, gather feedback, improve
4. **No premature optimization**: Make it work, make it right, then make it fast

---

## Phase 1: Foundation

**Goal**: Basic working application with connection and query execution.

### Deliverables

- [ ] Project setup (Cargo, dependencies, CI)
- [ ] Basic TUI scaffold with ratatui
- [ ] Connection configuration loading (`~/.vizgres/connections.toml`)
- [ ] PostgreSQL connection (tokio-postgres)
- [ ] Simple query editor (single-line initially)
- [ ] Query execution and raw result display
- [ ] Basic error handling and display
- [ ] Quit command

### Technical Tasks

```
src/
├── main.rs              # Entry point, terminal setup
├── app.rs               # Basic app state, event loop
├── ui/
│   ├── mod.rs
│   └── layout.rs        # Simple 2-panel layout
├── db/
│   ├── mod.rs
│   ├── provider.rs      # Trait definition
│   └── postgres.rs      # Basic implementation
└── config/
    └── connections.rs   # Load connections.toml
```

### Tests Required

- [ ] Connection config parsing
- [ ] PostgreSQL connection (integration test with testcontainers)
- [ ] Query execution returns results
- [ ] Basic UI renders without panic

### Exit Criteria

User can:
1. Start application
2. Connect to a PostgreSQL database
3. Type and execute a query
4. See results
5. Quit

---

## Phase 2: Core UI

**Goal**: Full panel layout with navigation.

### Deliverables

- [ ] Three-panel layout (tree, query, results)
- [ ] Panel focus system with visual highlighting
- [ ] Tab navigation between panels
- [ ] Command bar (basic)
- [ ] Status message display
- [ ] Keyboard shortcut: `:quit`, `:connect`

### Technical Tasks

```
src/ui/
├── tree.rs              # Tree browser placeholder
├── editor.rs            # Multi-line editor
├── results.rs           # Table display
├── command_bar.rs       # Command input
└── theme.rs             # Color definitions
```

### Tests Required

- [ ] Focus cycling works correctly
- [ ] Panel borders change on focus
- [ ] Command parsing for `:quit`, `:connect`
- [ ] Layout adapts to terminal size

### Exit Criteria

User can:
1. Navigate between all panels with Tab
2. See clear visual indication of focused panel
3. Use command bar for quit and connect
4. See status messages

---

## Phase 3: Tree Browser

**Goal**: Navigable database object tree.

### Deliverables

- [ ] Schema introspection queries
- [ ] Tree rendering with expand/collapse
- [ ] Lazy loading of tree nodes
- [ ] Keyboard navigation (j/k, h/l, Enter)
- [ ] Tree search/filter (/)
- [ ] Refresh command

### Technical Tasks

```
src/
├── db/
│   └── schema.rs        # Introspection queries
└── ui/
    └── tree.rs          # Full tree widget
```

### Tests Required

- [ ] Schema queries return expected structure
- [ ] Tree navigation (up/down/expand/collapse)
- [ ] Filter shows only matching nodes
- [ ] Lazy loading triggers on expand

### Exit Criteria

User can:
1. See database schemas, tables, columns
2. Expand/collapse tree nodes
3. Navigate with keyboard
4. Search/filter the tree
5. Refresh schema

---

## Phase 4: Query Editor

**Goal**: Full-featured SQL editor with intelligent error correction.

### Deliverables

- [ ] Multi-line text editing
- [ ] Cursor movement (arrows, Home, End, Page)
- [ ] Text insertion and deletion
- [ ] SQL formatting (Ctrl+Shift+F)
- [ ] Query execution (Ctrl+Enter)
- [ ] Query history (Ctrl+Up/Down)
- [ ] Basic syntax highlighting
- [ ] **SQL Fixer**: Auto-fix typos on execute
  - [ ] Keyword typo detection (SELEC → SELECT)
  - [ ] Identifier matching against schema (usres → users)
  - [ ] "Did you mean?" confirmation dialog
  - [ ] Configurable auto-accept threshold

### Technical Tasks

```
src/
├── ui/
│   ├── editor.rs           # Full editor implementation
│   └── fix_confirmation.rs # Fix preview dialog
└── sql/
    ├── formatter.rs        # sqlformat wrapper
    ├── highlighter.rs      # Syntax highlighting
    └── fixer/
        ├── mod.rs          # SqlFixer trait
        ├── rule_based.rs   # Built-in fixer pipeline
        ├── keyword.rs      # Keyword typo fixer
        ├── identifier.rs   # Schema-aware identifier fixer
        └── llm.rs          # Optional Ollama integration
```

### Tests Required

- [ ] Cursor movement in all directions
- [ ] Text insertion at cursor
- [ ] Line splitting on Enter
- [ ] SQL formatting produces expected output
- [ ] History navigation cycles correctly
- [ ] SQL fixer corrects common keyword typos
- [ ] SQL fixer matches identifiers against schema
- [ ] Confidence levels assigned correctly
- [ ] Fix confirmation accepts/rejects properly

### Exit Criteria

User can:
1. Write multi-line SQL queries
2. Navigate with all expected keys
3. Format SQL with one keystroke
4. Navigate query history
5. Execute with Ctrl+Enter
6. See "Did you mean?" for typos in SQL
7. Accept or reject suggested fixes

---

## Phase 5: Results Viewer

**Goal**: Scrollable, interactive results table.

### Deliverables

- [ ] Table rendering with headers
- [ ] Vertical scrolling
- [ ] Horizontal scrolling
- [ ] Column width auto-sizing
- [ ] Row/cell selection modes
- [ ] Cell value inspector popup
- [ ] JSONB pretty printing

### Technical Tasks

```
src/ui/
├── results.rs           # Full results table
└── cell_popup.rs        # Cell inspector overlay
```

### Tests Required

- [ ] Table renders correct number of rows/columns
- [ ] Scrolling stays within bounds
- [ ] Cell inspector shows full content
- [ ] JSONB is pretty-printed
- [ ] NULL values display correctly

### Exit Criteria

User can:
1. Scroll through large result sets
2. See truncated values in cells
3. Open cell inspector for full content
4. View JSONB in formatted tree
5. Copy cell values

---

## Phase 6: Polish & Integration

**Goal**: Seamless integrated experience.

### Deliverables

- [ ] Tree selection inserts SELECT into editor
- [ ] Error display with line highlighting
- [ ] EXPLAIN plan viewer
- [ ] Export functionality (CSV, JSON)
- [ ] Full command bar with autocomplete
- [ ] Help overlay (Ctrl+?)
- [ ] Settings/preferences loading

### Technical Tasks

```
src/
├── commands/
│   ├── parser.rs        # Full command parsing
│   └── handlers.rs      # Command execution
└── config/
    └── settings.rs      # User preferences
```

### Tests Required

- [ ] All commands parse correctly
- [ ] Export produces valid CSV/JSON
- [ ] EXPLAIN parses PostgreSQL output
- [ ] Autocomplete shows relevant suggestions

### Exit Criteria

User can:
1. Select table → see SELECT in editor
2. Export results to file
3. View query plans
4. Use full command palette
5. See help for shortcuts

---

## Phase 7: Autocomplete & Intelligence

**Goal**: Context-aware SQL assistance.

### Deliverables

- [ ] Schema-aware autocomplete
- [ ] Table/column suggestions after FROM/SELECT
- [ ] Alias tracking
- [ ] Keyword completion
- [ ] Function signature hints

### Technical Tasks

```
src/sql/
└── completer.rs         # Full autocomplete engine
```

### Tests Required

- [ ] After `FROM ` suggests tables
- [ ] After `tablename.` suggests columns
- [ ] Alias resolution works
- [ ] Keywords suggested appropriately

### Exit Criteria

User can:
1. Get table suggestions after FROM
2. Get column suggestions after table.
3. See function signatures
4. Complete SQL keywords

---

## Phase 8: Connection Management

**Goal**: Full connection lifecycle support.

### Deliverables

- [ ] New connection dialog
- [ ] Connection editing
- [ ] Password keychain integration
- [ ] SSH tunnel support
- [ ] SSL certificate configuration
- [ ] Connection testing

### Technical Tasks

```
src/
├── config/
│   └── credentials.rs   # Keychain integration
└── db/
    └── tunnel.rs        # SSH tunnel
```

### Tests Required

- [ ] Keychain store/retrieve works
- [ ] SSH tunnel establishes correctly
- [ ] SSL connections work
- [ ] Connection test validates

### Exit Criteria

User can:
1. Create new connections via UI
2. Store passwords securely
3. Connect through SSH tunnels
4. Use SSL certificates

---

## Future Phases

### Phase 9: Advanced Features

- [ ] Multiple result tabs
- [ ] Query bookmarks/snippets
- [ ] Data editing (INSERT/UPDATE helpers)
- [ ] Table data export with filters
- [ ] Dark/light theme switching
- [ ] Custom keybinding configuration

### Phase 10: Additional Databases

- [ ] Database provider abstraction finalization
- [ ] MySQL/MariaDB support
- [ ] SQLite support
- [ ] Potential: SQL Server, Oracle

### Phase 11: Productivity

- [ ] Query profiling integration
- [ ] Index suggestions
- [ ] Table relationship visualization
- [ ] Query plan comparison
- [ ] Performance metrics dashboard

---

## Testing Milestones

### By End of Phase 2

- [ ] CI pipeline running (GitHub Actions)
- [ ] Unit test coverage > 60%
- [ ] Integration tests with testcontainers
- [ ] Snapshot tests for UI components

### By End of Phase 5

- [ ] Unit test coverage > 75%
- [ ] All public APIs documented
- [ ] Performance benchmarks established
- [ ] Memory leak testing

### By End of Phase 8

- [ ] Unit test coverage > 85%
- [ ] End-to-end test suite
- [ ] Security audit of credential handling
- [ ] Cross-platform testing (Linux, macOS, Windows)

---

## Quality Gates

Each phase must pass before proceeding:

1. **All tests pass**: Zero failing tests
2. **No clippy warnings**: `cargo clippy -- -D warnings`
3. **Formatted code**: `cargo fmt --check`
4. **Documentation**: Public APIs documented
5. **No regressions**: Previous functionality still works

---

## Release Strategy

### Alpha (After Phase 5)

- Internal testing
- Basic functionality complete
- Known limitations documented

### Beta (After Phase 7)

- Public beta release
- Core features stable
- Gathering user feedback

### 1.0 (After Phase 8)

- Production ready
- Full documentation
- Stable API
- Security reviewed

---

## Dependencies by Phase

### Phase 1-2

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
ratatui = "0.28"
crossterm = "0.28"
tokio-postgres = "0.7"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
dirs = "5"
thiserror = "1"
anyhow = "1"
```

### Phase 3-5 (additions)

```toml
[dependencies]
sqlformat = "0.2"
serde_json = "1"
chrono = "0.4"
```

### Phase 6-7 (additions)

```toml
[dependencies]
async-trait = "0.1"
```

### Phase 8 (additions)

```toml
[dependencies]
keyring = "2"
```

### Development

```toml
[dev-dependencies]
testcontainers = "0.20"
insta = "1"
tokio-test = "0.4"
criterion = "0.5"
```

---

## Contributing

### Getting Started

1. Clone repository
2. Run `cargo build`
3. Run `cargo test`
4. Start PostgreSQL: `docker run -e POSTGRES_PASSWORD=postgres -p 5432:5432 postgres`
5. Run application: `cargo run`

### Pull Request Checklist

- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] `cargo fmt` run
- [ ] `cargo clippy` passes
- [ ] Changelog updated

---

## Success Metrics

### Usability

- User can complete common tasks faster than DataGrip startup time
- Keyboard-only operation for all features
- < 100ms response for all UI interactions

### Reliability

- Zero crashes in normal operation
- Graceful handling of connection drops
- No data loss in query history

### Performance

- < 50MB memory for typical usage
- < 5ms render time for result sets up to 10,000 rows
- < 100ms schema load time for databases with 500 tables

---

## Conclusion

This roadmap provides a structured path from zero to a fully-featured PostgreSQL TUI client. Each phase builds on the previous, delivering incremental value while maintaining code quality through comprehensive testing.

The key to success is discipline: ship each phase complete with tests before moving on. Resist the urge to add features outside the current phase scope.

Let's build something great.

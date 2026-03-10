# vizgres

A fast, keyboard-driven PostgreSQL client for the terminal.

## Features

- **Schema Browser**: Navigate schemas, tables, views, functions, indexes with expand/collapse
- **SQL Editor**: Multi-line editing with syntax highlighting, autocomplete, undo/redo
- **Query Execution**: Run queries with configurable timeout, cancel running queries
- **EXPLAIN ANALYZE**: One-key query plan analysis
- **Results Viewer**: Scrollable table with auto-pagination, cell-level navigation, and NULL styling
- **Inspector**: Full cell content viewer with JSON pretty-printing
- **Export**: Save results as CSV or JSON
- **Query History**: Navigate previous queries with Ctrl+Up/Down
- **Multi-Tab**: Work with multiple queries simultaneously
- **Connection Profiles**: Save and manage database connections
- **Large Schema Support**: Pagination for databases with 10k+ tables

## Install

```bash
cargo install --path .
```

Requires Rust 1.85+ (2024 edition).

## Usage

```bash
# Connect via URL
vizgres postgres://user:pass@localhost:5432/mydb

# Interactive connection dialog
vizgres

# Connect to saved profile
vizgres myprofile
```

## Keybindings

Press `?` in-app for full keybinding help. All keybindings are configurable in `~/.vizgres/config.toml`.

### Global

| Key | Action |
|-----|--------|
| Tab / Shift+Tab | Cycle focus between panels |
| Ctrl+Q | Quit |
| Ctrl+P | Open command bar |
| ? | Show help |
| Ctrl+T | New tab |
| Ctrl+W | Close tab |
| Ctrl+Tab | Next tab |

### Editor

| Key | Action |
|-----|--------|
| F5 / Ctrl+Enter | Execute query |
| Ctrl+E | EXPLAIN ANALYZE |
| Ctrl+L | Clear editor |
| Ctrl+Z | Undo |
| Ctrl+Shift+Z | Redo |
| Ctrl+Shift+F | Format SQL |
| Ctrl+Up/Down | Query history |
| Tab | Cycle autocomplete suggestions |
| Right | Accept autocomplete |
| Escape | Cancel running query |

### Schema Tree

| Key | Action |
|-----|--------|
| j/k or ↑/↓ | Navigate |
| Enter | Expand / Preview table data |
| Space | Toggle expand |
| h | Collapse / go to parent |
| / | Filter tree |
| y | Copy qualified name |
| d | Show table definition (DDL) |

### Results

| Key | Action |
|-----|--------|
| j/k or ↑/↓ | Navigate rows |
| h/l or ←/→ | Navigate columns |
| Enter | Open inspector |
| Ctrl+C | Copy cell |
| Ctrl+Shift+C | Copy row |
| Ctrl+S | Export as CSV |
| Ctrl+J | Export as JSON |
| g/G | Jump to first/last row |
| n | Next page |
| p | Previous page |

## Commands

Open the command bar with `Ctrl+P`:

| Command | Action |
|---------|--------|
| `/connect [url]` | Connect to database |
| `/refresh` | Reload schema |
| `/clear` | Clear editor |
| `/help` | Show help |
| `/quit` | Quit |

## Configuration

Settings are stored in `~/.vizgres/config.toml`:

```toml
# Query timeout in milliseconds (0 = no timeout)
query_timeout_ms = 30000

# Maximum rows to fetch (0 = unlimited)
max_result_rows = 10000

# Items per category in tree (0 = unlimited)
tree_category_limit = 500

# Query history size
history_size = 500

# Custom keybindings
[keybindings.editor]
"ctrl+enter" = "execute_query"

[keybindings.results]
"ctrl+c" = "copy_cell"
```

Connection profiles are stored in `~/.vizgres/connections.toml`.

## License

MIT

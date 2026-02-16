# vizgres

A fast, keyboard-driven PostgreSQL client for the terminal.

## Features

- **Tree Browser**: Navigate schemas, tables, and columns with expand/collapse
- **Query Editor**: Multi-line SQL editing with line numbers
- **Results Viewer**: Scrollable table with cell-level navigation
- **Inspector**: Full cell content viewer with JSON pretty-printing
- **Command Bar**: `:connect`, `:disconnect`, `:refresh`, `:quit`
- **Clipboard**: Copy cells (`y`) and rows (`Y`) to clipboard

## Install

```bash
cargo install --path .
```

Requires Rust 1.85+ (2024 edition).

## Usage

```bash
# Connect via URL
vizgres postgres://user:pass@localhost:5432/mydb

# Start without connection (use :connect later)
vizgres
```

## Keybindings

| Key | Context | Action |
|-----|---------|--------|
| Tab / Shift+Tab | Global | Cycle focus between panels |
| Ctrl+Q | Global | Quit |
| `:` | Tree, Results | Open command bar |
| j/k | Tree, Results | Move down/up |
| h/l | Tree | Collapse/expand |
| h/l | Results | Move left/right column |
| Enter | Results | Open inspector for selected cell |
| Escape | Inspector, Command bar | Close, return to previous panel |
| Ctrl+Enter | Editor | Execute query |
| y | Results, Inspector | Copy cell to clipboard |
| Y | Results | Copy row to clipboard |
| g/G | Results, Inspector | Jump to top/bottom |
| PageUp/PageDown | Results, Inspector | Scroll by page |

## Commands

| Command | Short | Action |
|---------|-------|--------|
| `:connect <url>` | `:c` | Connect to database |
| `:disconnect` | `:dc` | Disconnect |
| `:refresh` | `:r` | Reload schema |
| `:help` | `:h` | Show help |
| `:quit` | `:q` | Quit |

## Architecture

See [docs/design.md](./docs/design.md) for the design document.

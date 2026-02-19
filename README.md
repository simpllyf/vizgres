# vizgres

A fast, keyboard-driven PostgreSQL client for the terminal.

> **⚠️ Pre-1.0: Expect breaking changes between minor versions.**

## Features

- **Tree Browser**: Navigate schemas, tables, and columns with expand/collapse
- **Query Editor**: Multi-line SQL editing with line numbers
- **Results Viewer**: Scrollable table with cell-level navigation
- **Inspector**: Full cell content viewer with JSON pretty-printing
- **Command Bar**: `/refresh`, `/clear`, `/help`, `/quit` (via `Ctrl+P`)
- **Clipboard**: Copy cells (`y`) and rows (`Y`) to clipboard

## Install

```bash
cargo install --path .
```

Requires Rust 1.93+ (2024 edition).

## Usage

```bash
# Connect via URL
vizgres postgres://user:pass@localhost:5432/mydb

# Interactive prompt for URL
vizgres
```

## Keybindings

| Key | Context | Action |
|-----|---------|--------|
| Tab / Shift+Tab | Global | Cycle focus between panels |
| Ctrl+Q | Global | Quit |
| Ctrl+P | Global | Open command bar |
| F5 / Ctrl+Enter | Editor | Execute query |
| Ctrl+L | Editor | Clear editor |
| j/k | Tree, Results | Move down/up |
| h | Tree | Collapse / go to parent |
| Enter | Tree | Expand node |
| h/l | Results | Move left/right column |
| Enter | Results | Open inspector for selected cell |
| Escape | Inspector, Command bar | Close, return to previous panel |
| y | Results, Inspector | Copy cell to clipboard |
| Y | Results | Copy row to clipboard |
| g/G | Results, Inspector | Jump to top/bottom |
| PageUp/PageDown | Results, Inspector | Scroll by page |

## Commands

Open the command bar with `Ctrl+P`, then type a command:

| Command | Short | Action |
|---------|-------|--------|
| `/refresh` | `/r` | Reload schema |
| `/clear` | `/cl` | Clear query editor |
| `/help` | `/h` | Show help |
| `/quit` | `/q` | Quit |

## Architecture

See [docs/design.md](./docs/design.md) for the design document.

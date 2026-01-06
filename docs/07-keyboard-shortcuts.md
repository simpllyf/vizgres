# 07 - Keyboard Shortcuts

> Complete keyboard shortcut reference organized by context.

---

## Design Principles

1. **Consistent across panels**: Similar actions use same keys where possible
2. **Vim-inspired navigation**: `hjkl` for movement, `:` for commands
3. **Modifier conventions**:
   - `Ctrl+` for primary actions
   - `Shift+` for reverse/alternate
   - `Alt+` rarely used (terminal compatibility issues)
4. **Discoverable**: `Ctrl+?` shows help overlay

---

## Global Shortcuts

These work regardless of which panel is focused.

| Key | Action | Notes |
|-----|--------|-------|
| `Tab` | Cycle focus forward | Tree → Query → Results → Tree |
| `Shift+Tab` | Cycle focus backward | Reverse direction |
| `Ctrl+1` | Focus Tree Browser | Direct jump |
| `Ctrl+2` | Focus Query Editor | Direct jump |
| `Ctrl+3` | Focus Results Viewer | Direct jump |
| `:` | Open command bar | Enter command mode |
| `Ctrl+Q` | Quit application | With confirmation if unsaved |
| `Ctrl+?` | Show help overlay | Keyboard shortcut reference |
| `Escape` | Cancel / Close | Context-dependent |

### Connection Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+N` | New connection dialog |
| `Ctrl+D` | Disconnect current |
| `Ctrl+Shift+C` | Connection switcher popup |

---

## Tree Browser Shortcuts

When Tree Browser panel is focused.

### Navigation

| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `←` / `h` | Collapse node / Go to parent |
| `→` / `l` | Expand node / Enter children |
| `Enter` | Expand/collapse toggle |
| `Home` | Jump to first item |
| `End` | Jump to last item |
| `PageUp` | Scroll up one page |
| `PageDown` | Scroll down one page |
| `Ctrl+Home` | Collapse all, go to root |

### Actions

| Key | Action |
|-----|--------|
| `/` | Start search/filter |
| `Escape` | Clear search filter |
| `r` | Refresh selected node |
| `R` | Refresh entire tree |
| `y` | Copy node name to clipboard |
| `Ctrl+D` | Show DDL/definition |
| `Space` | Preview table (SELECT * LIMIT 100) |
| `Enter` (on table) | Insert SELECT into query editor |

### Tree Node Actions

| Node Type | `Enter` | `Space` | `Ctrl+D` |
|-----------|---------|---------|----------|
| Schema | Expand | — | — |
| Table | SELECT → editor | Preview | CREATE TABLE |
| View | SELECT → editor | Preview | CREATE VIEW |
| Column | Add to SELECT | — | — |
| Index | — | — | CREATE INDEX |
| Function | — | Call preview | Function source |

---

## Query Editor Shortcuts

When Query Editor panel is focused.

### Cursor Movement

| Key | Action |
|-----|--------|
| `←` / `→` | Move cursor left/right |
| `↑` / `↓` | Move cursor up/down |
| `Home` | Move to line start |
| `End` | Move to line end |
| `Ctrl+Home` | Move to document start |
| `Ctrl+End` | Move to document end |
| `Ctrl+←` | Move to previous word |
| `Ctrl+→` | Move to next word |
| `PageUp` | Move up one page |
| `PageDown` | Move down one page |

### Editing

| Key | Action |
|-----|--------|
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character at cursor |
| `Ctrl+Backspace` | Delete word before cursor |
| `Ctrl+Delete` | Delete word after cursor |
| `Enter` | Insert newline |
| `Tab` | Insert 2 spaces / Accept autocomplete |
| `Ctrl+V` | Paste from clipboard |
| `Ctrl+A` | Select all |
| `Ctrl+/` | Toggle line comment (`--`) |
| `Ctrl+L` | Clear editor |

### Execution

| Key | Action |
|-----|--------|
| `Ctrl+Enter` | Execute query |
| `Ctrl+E` | Execute with EXPLAIN ANALYZE |
| `Ctrl+Shift+Enter` | Execute and stay in editor |
| `F5` | Execute query (alternate) |

### Formatting & Utilities

| Key | Action |
|-----|--------|
| `Ctrl+Shift+F` | Format SQL |
| `Ctrl+Space` | Trigger autocomplete |
| `Ctrl+↑` | Previous query from history |
| `Ctrl+↓` | Next query from history |

### Autocomplete Active

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate suggestions |
| `Tab` / `Enter` | Accept suggestion |
| `Escape` | Dismiss autocomplete |
| Continue typing | Filter suggestions |

---

## Results Viewer Shortcuts

When Results Viewer panel is focused.

### Navigation

| Key | Action |
|-----|--------|
| `↑` / `k` | Move selection up |
| `↓` / `j` | Move selection down |
| `←` / `h` | Scroll left / Previous cell (cell mode) |
| `→` / `l` | Scroll right / Next cell (cell mode) |
| `Home` | Jump to first row |
| `End` | Jump to last row |
| `Ctrl+Home` | Jump to top-left cell |
| `Ctrl+End` | Jump to bottom-right cell |
| `PageUp` | Scroll up one page |
| `PageDown` | Scroll down one page |

### Selection Mode

| Key | Action |
|-----|--------|
| `Tab` | Toggle row/cell selection mode |
| `Enter` | Open cell inspector popup |

### Cell Inspector Active

| Key | Action |
|-----|--------|
| `Escape` / `Enter` / `q` | Close inspector |
| `↑` / `k` | Scroll content up |
| `↓` / `j` | Scroll content down |
| `PageUp` | Scroll up one page |
| `PageDown` | Scroll down one page |
| `Home` | Scroll to top |
| `End` | Scroll to bottom |
| `c` / `y` | Copy content to clipboard |

### Actions

| Key | Action |
|-----|--------|
| `c` | Copy cell value |
| `y` | Copy entire row (CSV format) |
| `Y` | Copy all results (CSV format) |
| `s` | Sort by current column (ascending) |
| `S` | Sort by current column (descending) |
| `Ctrl+Shift+S` | Clear sort |

---

## Command Bar Shortcuts

When Command Bar is active (`:` pressed).

### Input Editing

| Key | Action |
|-----|--------|
| `←` / `→` | Move cursor |
| `Home` | Move to start |
| `End` | Move to end |
| `Backspace` | Delete before cursor |
| `Delete` | Delete at cursor |
| `Ctrl+U` | Clear entire line |
| `Ctrl+W` | Delete word before cursor |
| `Ctrl+K` | Delete to end of line |

### Command Execution

| Key | Action |
|-----|--------|
| `Enter` | Execute command |
| `Escape` | Cancel, return to previous panel |
| `Tab` | Accept autocomplete suggestion |
| `↑` | Previous autocomplete / command history |
| `↓` | Next autocomplete / command history |

---

## Popup/Modal Shortcuts

### Help Overlay (`Ctrl+?`)

| Key | Action |
|-----|--------|
| `Escape` / `q` / `Ctrl+?` | Close help |
| `↑` / `↓` | Scroll content |
| `PageUp` / `PageDown` | Scroll by page |
| `/` | Search within help |

### Connection Switcher (`Ctrl+Shift+C`)

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate connections |
| `Enter` | Connect to selected |
| `Escape` | Cancel |
| Type | Filter connections |

### Confirmation Dialogs

| Key | Action |
|-----|--------|
| `y` / `Enter` | Confirm |
| `n` / `Escape` | Cancel |

---

## Resize Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl+Left` | Decrease tree panel width |
| `Ctrl+Right` | Increase tree panel width |
| `Ctrl+Up` | Decrease query editor height |
| `Ctrl+Down` | Increase query editor height |
| `Ctrl+0` | Reset to default layout |

---

## Quick Reference Card

```
╔═══════════════════════════════════════════════════════════════════════════╗
║                        VIZGRES QUICK REFERENCE                            ║
╠═════════════════════════════╦═════════════════════════════════════════════╣
║  GLOBAL                     ║  QUERY EDITOR                               ║
║  Tab         Cycle panels   ║  Ctrl+Enter  Execute query                  ║
║  Ctrl+1/2/3  Jump to panel  ║  Ctrl+E      EXPLAIN query                  ║
║  :           Command bar    ║  Ctrl+Shift+F Format SQL                    ║
║  Ctrl+Q      Quit           ║  Ctrl+Space  Autocomplete                   ║
║  Ctrl+?      Help           ║  Ctrl+↑/↓    Query history                  ║
╠═════════════════════════════╬═════════════════════════════════════════════╣
║  TREE BROWSER               ║  RESULTS VIEWER                             ║
║  ↑/↓ or j/k  Navigate       ║  ↑/↓ or j/k  Navigate rows                  ║
║  ←/→ or h/l  Collapse/Expand║  Tab         Row/Cell mode                  ║
║  /           Search         ║  Enter       Cell inspector                 ║
║  r           Refresh        ║  s/S         Sort asc/desc                  ║
║  Space       Preview table  ║  c           Copy cell                      ║
║  Enter       SELECT to query║  y           Copy row                       ║
╠═════════════════════════════╩═════════════════════════════════════════════╣
║  COMMAND BAR (:)                                                          ║
║  :connect <name>  Connect     :export csv   Export results                ║
║  :disconnect      Disconnect  :format       Format query                  ║
║  :refresh         Refresh     :quit         Exit                          ║
╚═══════════════════════════════════════════════════════════════════════════╝
```

---

## Customization (Future)

### Key Binding Configuration

```toml
# ~/.vizgres/keybindings.toml

[global]
quit = "Ctrl+Q"
help = "Ctrl+?"
command_bar = ":"
cycle_focus = "Tab"

[query_editor]
execute = "Ctrl+Enter"
explain = "Ctrl+E"
format = "Ctrl+Shift+F"

[tree_browser]
refresh = "r"
search = "/"

[results_viewer]
copy_cell = "c"
copy_row = "y"
sort = "s"
```

---

## Implementation

### Key Mapping Structure

```rust
pub struct KeyBindings {
    pub global: HashMap<KeyEvent, GlobalAction>,
    pub tree: HashMap<KeyEvent, TreeAction>,
    pub editor: HashMap<KeyEvent, EditorAction>,
    pub results: HashMap<KeyEvent, ResultsAction>,
    pub command_bar: HashMap<KeyEvent, CommandBarAction>,
}

pub enum GlobalAction {
    CycleFocus,
    CycleFocusReverse,
    FocusTree,
    FocusQuery,
    FocusResults,
    OpenCommandBar,
    ShowHelp,
    Quit,
}

pub enum TreeAction {
    MoveUp,
    MoveDown,
    Expand,
    Collapse,
    Search,
    Refresh,
    RefreshAll,
    CopyName,
    ShowDDL,
    Preview,
    SelectToQuery,
}

// ... etc
```

### Key Event Handling

```rust
impl App {
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Action> {
        // First check global shortcuts
        if let Some(action) = self.bindings.global.get(&key) {
            return self.handle_global_action(action);
        }

        // Then delegate to focused panel
        match self.focus {
            PanelFocus::TreeBrowser => {
                if let Some(action) = self.bindings.tree.get(&key) {
                    return self.tree.handle_action(action);
                }
            }
            PanelFocus::QueryEditor => {
                if let Some(action) = self.bindings.editor.get(&key) {
                    return self.editor.handle_action(action);
                }
            }
            PanelFocus::ResultsViewer => {
                if let Some(action) = self.bindings.results.get(&key) {
                    return self.results.handle_action(action);
                }
            }
            PanelFocus::CommandBar => {
                if let Some(action) = self.bindings.command_bar.get(&key) {
                    return self.command_bar.handle_action(action);
                }
            }
        }

        None
    }
}
```

---

## Testing Strategy

### Key Binding Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn test_tab_cycles_focus() {
        let mut app = App::new();
        app.focus = PanelFocus::TreeBrowser;

        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, PanelFocus::QueryEditor);

        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, PanelFocus::ResultsViewer);

        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, PanelFocus::TreeBrowser);
    }

    #[test]
    fn test_ctrl_1_2_3_jumps_to_panels() {
        let mut app = App::new();

        app.handle_key(ctrl(KeyCode::Char('2')));
        assert_eq!(app.focus, PanelFocus::QueryEditor);

        app.handle_key(ctrl(KeyCode::Char('1')));
        assert_eq!(app.focus, PanelFocus::TreeBrowser);

        app.handle_key(ctrl(KeyCode::Char('3')));
        assert_eq!(app.focus, PanelFocus::ResultsViewer);
    }

    #[test]
    fn test_colon_opens_command_bar() {
        let mut app = App::new();
        app.focus = PanelFocus::TreeBrowser;

        app.handle_key(key(KeyCode::Char(':')));

        assert_eq!(app.focus, PanelFocus::CommandBar);
        assert_eq!(app.command_bar.mode, CommandBarMode::Command);
    }

    #[test]
    fn test_escape_closes_command_bar() {
        let mut app = App::new();
        app.focus = PanelFocus::CommandBar;
        app.previous_focus = PanelFocus::QueryEditor;

        app.handle_key(key(KeyCode::Esc));

        assert_eq!(app.focus, PanelFocus::QueryEditor);
    }

    #[test]
    fn test_ctrl_enter_in_editor_executes_query() {
        let mut app = App::new();
        app.focus = PanelFocus::QueryEditor;
        app.editor.set_text("SELECT 1");

        let action = app.handle_key(ctrl(KeyCode::Enter));

        assert!(matches!(action, Some(Action::ExecuteQuery(_))));
    }

    #[test]
    fn test_vim_navigation_in_tree() {
        let mut app = App::new_with_schema(test_schema());
        app.focus = PanelFocus::TreeBrowser;
        let initial_selection = app.tree.selected.clone();

        app.handle_key(key(KeyCode::Char('j')));
        assert_ne!(app.tree.selected, initial_selection);

        let after_j = app.tree.selected.clone();
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(app.tree.selected, initial_selection);
    }
}
```

### All Actions Mapped Test

```rust
#[test]
fn test_all_documented_shortcuts_are_mapped() {
    let bindings = KeyBindings::default();

    // Global
    assert!(bindings.global.contains_key(&key(KeyCode::Tab)));
    assert!(bindings.global.contains_key(&ctrl(KeyCode::Char('q'))));
    assert!(bindings.global.contains_key(&key(KeyCode::Char(':'))));

    // Tree
    assert!(bindings.tree.contains_key(&key(KeyCode::Char('j'))));
    assert!(bindings.tree.contains_key(&key(KeyCode::Char('k'))));
    assert!(bindings.tree.contains_key(&key(KeyCode::Char('/'))));

    // Editor
    assert!(bindings.editor.contains_key(&ctrl(KeyCode::Enter)));
    assert!(bindings.editor.contains_key(&ctrl(KeyCode::Char('e'))));

    // Results
    assert!(bindings.results.contains_key(&key(KeyCode::Char('c'))));
    assert!(bindings.results.contains_key(&key(KeyCode::Char('s'))));
}
```

---

## Next Steps

See [08-connections.md](./08-connections.md) for connection configuration details.

# 02 - UI Layout

> Panel system, focus management, responsive layout, and visual design.

---

## Layout Structure

### Primary Layout

The interface uses a fixed panel arrangement optimized for database work:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Title Bar                                       │
├────────────────────┬────────────────────────────────────────────────────────┤
│                    │                                                        │
│                    │                   Query Editor                         │
│    Tree Browser    │                                                        │
│                    │                                                        │
│    (Schemas,       ├────────────────────────────────────────────────────────┤
│     Tables,        │                                                        │
│     Columns)       │                  Results Viewer                        │
│                    │                                                        │
│                    │                                                        │
├────────────────────┴────────────────────────────────────────────────────────┤
│                              Command Bar                                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Panel Dimensions

| Panel | Width | Height | Resizable |
|-------|-------|--------|-----------|
| Title Bar | 100% | 1 row | No |
| Tree Browser | 25% (min 20 cols) | 100% - 2 rows | Yes (horizontal) |
| Query Editor | 75% | 40% (min 5 rows) | Yes (vertical) |
| Results Viewer | 75% | 60% - 2 rows | Yes (vertical) |
| Command Bar | 100% | 1 row | No |

### Responsive Behavior

```
Terminal Width < 80:
┌──────────────────────────────────┐
│ [vizgres] localhost:5432/mydb    │
├──────────────────────────────────┤
│ ▼ public                         │  ← Full width tree
│   ▼ Tables                       │
│     ▶ users                      │
│     ▶ orders                     │
├──────────────────────────────────┤
│ SELECT * FROM users              │  ← Full width editor
│ WHERE active = true;             │
├──────────────────────────────────┤
│ id │ name  │ email               │  ← Full width results
│  1 │ Alice │ alice@test.com      │
├──────────────────────────────────┤
│ >                                │
└──────────────────────────────────┘

Note: In narrow mode, use Tab to toggle between
      Tree/Editor+Results views.
```

---

## Title Bar

### Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ [vizgres]                    localhost:5432/mydb                    Ctrl+? │
└─────────────────────────────────────────────────────────────────────────────┘
  ▲                              ▲                                       ▲
  │                              │                                       │
  App name                       Connection info                         Help hint
```

### States

| State | Display |
|-------|---------|
| Not connected | `[vizgres]                         No connection                   Ctrl+?` |
| Connected | `[vizgres]                    hostname:port/database              Ctrl+?` |
| Query running | `[vizgres]              ⣾ hostname:port/database                Ctrl+?` |

### Implementation

```rust
fn render_title_bar(frame: &mut Frame, area: Rect, app: &App) {
    let connection_text = match &app.connection {
        Some(conn) => format!("{}:{}/{}", conn.host, conn.port, conn.database),
        None => "No connection".to_string(),
    };

    let title = Line::from(vec![
        Span::styled("[vizgres]", Style::default().bold()),
        Span::raw("  "),
        Span::styled(connection_text, Style::default().fg(Color::Cyan)),
    ]);

    let help_hint = Span::styled("Ctrl+?", Style::default().fg(Color::DarkGray));

    // Render with proper alignment
}
```

---

## Panel Focus System

### Focus States

Each panel has three visual states:

| State | Border Style | Background |
|-------|--------------|------------|
| **Focused** | Bold, bright color | Slightly lighter |
| **Unfocused** | Dim, gray | Default terminal |
| **Disabled** | Dotted/dashed | Darker |

### Focus Colors by Panel

| Panel | Focused Border Color |
|-------|---------------------|
| Tree Browser | `Color::Blue` |
| Query Editor | `Color::Green` |
| Results Viewer | `Color::Yellow` |
| Command Bar | `Color::Magenta` |
| Cell Popup | `Color::Cyan` |

### Visual Example

```
Focused (Query Editor):
┌───────────────────┐ ╔═══════════════════════════════════════════╗
│ Tree Browser      │ ║ Query Editor                         [*] ║
│ (dim border)      │ ║ SELECT * FROM users;                     ║
│                   │ ╚═══════════════════════════════════════════╝
                      ▲
                      └── Bold green double border
```

### Focus Navigation

```rust
pub enum PanelFocus {
    TreeBrowser,
    QueryEditor,
    ResultsViewer,
    CommandBar,
    CellPopup,  // Modal overlay
}

impl App {
    fn cycle_focus(&mut self, direction: Direction) {
        // Tab cycles: Tree → Editor → Results → Tree
        // Shift+Tab: Reverse
        // Command bar opened with ':'
        // Cell popup opened with Enter on cell

        self.focus = match (self.focus, direction) {
            (PanelFocus::TreeBrowser, Direction::Forward) => PanelFocus::QueryEditor,
            (PanelFocus::QueryEditor, Direction::Forward) => PanelFocus::ResultsViewer,
            (PanelFocus::ResultsViewer, Direction::Forward) => PanelFocus::TreeBrowser,
            // ... reverse direction
        };
    }
}
```

---

## Cell Popup Overlay

### Purpose
Display full content of cells that are truncated or contain complex data (JSONB).

### Appearance

```
┌───────────────────┬─────────────────────────────────────────────────────────┐
│ Tree              │ Query Editor                                             │
│                   │                                                          │
│                   ├──────────────────────────────────────┬──────────────────┤
│                   │ Results                              │ Cell Inspector   │
│                   │ id │ name  │ metadata               ││                  │
│                   │  1 │ Alice │ {"role":"admin"...     ││ {                │
│                   │  2 │ Bob   │ {"role":"user",...█    ││   "role": "user",│
│                   │                                      ││   "permissions": │
│                   │                                      ││     ["read",     │
│                   │                                      ││      "write"]    │
│                   │                                      ││ }                │
│                   │                                      │└──────────────────┤
├───────────────────┴──────────────────────────────────────┴───────────────────┤
│ >                                                                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Behavior

- Opens with `Enter` key when a cell is selected
- Slides in from right (animated if terminal supports)
- Takes 30-40% of screen width
- Scrollable for large content
- JSON content is syntax highlighted and pretty-printed
- Close with `Escape` or `Enter` again

### Implementation

```rust
pub struct CellPopup {
    visible: bool,
    content: String,
    content_type: ContentType,
    scroll_offset: usize,
}

pub enum ContentType {
    PlainText,
    Json,
    Xml,
    Binary,
}

impl CellPopup {
    fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        // Calculate popup area (right 35% of screen)
        let popup_width = area.width * 35 / 100;
        let popup_area = Rect {
            x: area.width - popup_width,
            y: area.y + 1,
            width: popup_width,
            height: area.height - 2,
        };

        // Render content based on type
        let content = match self.content_type {
            ContentType::Json => self.render_json(),
            ContentType::PlainText => self.render_text(),
            // ...
        };

        let block = Block::default()
            .title("Cell Inspector")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        frame.render_widget(Paragraph::new(content).block(block), popup_area);
    }
}
```

---

## Status Messages

### Placement
Status messages appear in the command bar area when not in command mode.

### Types

| Type | Color | Icon | Duration |
|------|-------|------|----------|
| Success | Green | `✓` | 3 seconds |
| Error | Red | `✗` | 5 seconds (or until dismissed) |
| Info | Blue | `ℹ` | 3 seconds |
| Warning | Yellow | `⚠` | 4 seconds |

### Examples

```
Success:
├──────────────────────────────────────────────────────────────────────────────┤
│ ✓ Query executed successfully (0.023s, 150 rows)                             │
└──────────────────────────────────────────────────────────────────────────────┘

Error:
├──────────────────────────────────────────────────────────────────────────────┤
│ ✗ ERROR: relation "users" does not exist (LINE 1)                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## Loading States

### Query Execution

```
Results panel during query:
┌────────────────────────────────────────────────────────────────────────────┐
│                                                                            │
│                                                                            │
│                           ⣾ Executing query...                             │
│                                                                            │
│                                                                            │
└────────────────────────────────────────────────────────────────────────────┘
```

### Schema Loading

```
Tree browser during refresh:
┌─────────────────────┐
│ ⣾ Loading schema... │
│                     │
│                     │
│                     │
└─────────────────────┘
```

### Spinner Animation

```rust
const SPINNER_FRAMES: &[char] = &['⣾', '⣽', '⣻', '⢿', '⡿', '⣟', '⣯', '⣷'];

pub struct Spinner {
    frame: usize,
    last_update: Instant,
}

impl Spinner {
    fn tick(&mut self) -> char {
        if self.last_update.elapsed() > Duration::from_millis(80) {
            self.frame = (self.frame + 1) % SPINNER_FRAMES.len();
            self.last_update = Instant::now();
        }
        SPINNER_FRAMES[self.frame]
    }
}
```

---

## Borders and Box Drawing

### Character Set

Using Unicode box-drawing characters for crisp borders:

| Element | Character |
|---------|-----------|
| Horizontal | `─` (U+2500) |
| Vertical | `│` (U+2502) |
| Top-left | `┌` (U+250C) |
| Top-right | `┐` (U+2510) |
| Bottom-left | `└` (U+2514) |
| Bottom-right | `┘` (U+2518) |
| T-down | `┬` (U+252C) |
| T-up | `┴` (U+2534) |
| T-right | `├` (U+251C) |
| T-left | `┤` (U+2524) |
| Cross | `┼` (U+253C) |

### Focused Border (Double Line)

| Element | Character |
|---------|-----------|
| Horizontal | `═` (U+2550) |
| Vertical | `║` (U+2551) |
| Top-left | `╔` (U+2554) |
| Top-right | `╗` (U+2557) |
| Bottom-left | `╚` (U+255A) |
| Bottom-right | `╝` (U+255D) |

---

## Color Palette

### Base Colors (Dark Theme - Default)

| Element | Foreground | Background |
|---------|------------|------------|
| Default text | `#c0c0c0` | `#1e1e1e` |
| Focused panel | `#ffffff` | `#2d2d2d` |
| Selected item | `#ffffff` | `#264f78` |
| Cursor | `#000000` | `#ffffff` |
| Null value | `#808080` | — |
| Primary key | `#ffd700` | — |

### Syntax Colors

| Element | Color |
|---------|-------|
| SQL keyword | `#569cd6` (blue) |
| String literal | `#ce9178` (orange) |
| Number | `#b5cea8` (green) |
| Comment | `#6a9955` (dim green) |
| Table name | `#4ec9b0` (teal) |
| Column name | `#9cdcfe` (light blue) |

---

## Resize Behavior

### Horizontal Resize (Tree Width)

```rust
// Drag border between tree and editor/results
fn handle_resize_drag(&mut self, x: u16) {
    let min_tree_width = 20;
    let min_right_width = 40;
    let max_tree_width = self.terminal_width - min_right_width;

    self.tree_width = x.clamp(min_tree_width, max_tree_width);
}
```

### Vertical Resize (Editor/Results Split)

```rust
// Drag border between editor and results
fn handle_vertical_resize(&mut self, y: u16) {
    let min_editor_height = 5;
    let min_results_height = 5;
    let available = self.right_panel_height - min_results_height;

    self.editor_height = y.clamp(min_editor_height, available);
}
```

### Keyboard Resize

| Key | Action |
|-----|--------|
| `Ctrl+Left` | Decrease tree width |
| `Ctrl+Right` | Increase tree width |
| `Ctrl+Up` | Decrease editor height |
| `Ctrl+Down` | Increase editor height |

---

## Testing Considerations

### Layout Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn test_layout_minimum_terminal_size() {
        // Ensure layout works at minimum 80x24
        let layout = calculate_layout(80, 24);
        assert!(layout.tree.width >= 20);
        assert!(layout.editor.height >= 5);
        assert!(layout.results.height >= 5);
    }

    #[test]
    fn test_layout_snapshot_standard_size() {
        let frame = render_layout(120, 40, &mock_app());
        assert_snapshot!(frame_to_string(&frame));
    }

    #[test]
    fn test_focus_cycle_wraps_correctly() {
        let mut app = App::new();
        app.focus = PanelFocus::ResultsViewer;
        app.cycle_focus(Direction::Forward);
        assert_eq!(app.focus, PanelFocus::TreeBrowser);
    }

    #[test]
    fn test_cell_popup_positions_correctly() {
        let popup_area = calculate_popup_area(Rect::new(0, 0, 120, 40));
        assert_eq!(popup_area.x, 78);  // Right 35%
        assert_eq!(popup_area.width, 42);
    }
}
```

### Visual Regression Tests

```rust
#[test]
fn test_focused_panel_border_style() {
    let tree = TreeBrowser::new(test_schema());
    let frame = render_component(&tree, Rect::new(0, 0, 30, 20), true);

    // Verify double-line border for focused state
    assert!(frame.contains('╔'));
    assert!(frame.contains('═'));
}

#[test]
fn test_unfocused_panel_border_style() {
    let tree = TreeBrowser::new(test_schema());
    let frame = render_component(&tree, Rect::new(0, 0, 30, 20), false);

    // Verify single-line border for unfocused state
    assert!(frame.contains('┌'));
    assert!(frame.contains('─'));
}
```

---

## Accessibility Notes

1. **Color Blindness**: Don't rely solely on color. Use:
   - Different border styles (single vs double)
   - Icons/symbols alongside colors
   - Bold/dim text modifiers

2. **Screen Readers**: While TUI apps are challenging for screen readers, we:
   - Maintain logical tab order
   - Use clear, descriptive panel titles
   - Announce status changes (future enhancement)

3. **High Contrast**: Theme system allows custom high-contrast themes

---

## Next Steps

See [03-tree-browser.md](./03-tree-browser.md) for tree component details.

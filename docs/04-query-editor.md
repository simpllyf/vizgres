# 04 - Query Editor

> SQL text editing, formatting, execution, autocomplete, and history.

---

## Overview

The Query Editor is a multi-line text input area for writing and executing SQL queries. It supports basic text editing operations, SQL formatting, autocomplete, and query history navigation.

---

## Visual Layout

```
╔═══════════════════════════════════════════════════════════════════════════╗
║ Query                                                           [Ctrl+E] ║
╠═══════════════════════════════════════════════════════════════════════════╣
║  1 │ SELECT                                                               ║
║  2 │   u.id,                                                              ║
║  3 │   u.name,                                                            ║
║  4 │   u.email,                                                           ║
║  5 │   COUNT(o.id) AS order_count                                         ║
║  6 │ FROM users u                                                         ║
║  7 │ LEFT JOIN orders o ON o.user_id = u.id                              ║
║  8 │ WHERE u.active = true█                                               ║
║  9 │ GROUP BY u.id, u.name, u.email                                       ║
║ 10 │ ORDER BY order_count DESC                                            ║
║ 11 │ LIMIT 100;                                                           ║
╚═══════════════════════════════════════════════════════════════════════════╝
                    ▲
                    └── Cursor position (block cursor)
```

### Components

| Element | Description |
|---------|-------------|
| Title | "Query" with hint for EXPLAIN shortcut |
| Line numbers | Left gutter with line numbers |
| Text area | SQL content with cursor |
| Cursor | Block cursor showing current position |

---

## Text Editing

### Supported Operations

| Category | Operations |
|----------|------------|
| Navigation | Arrow keys, Home/End, Ctrl+Home/End, PageUp/Down |
| Editing | Insert, Delete, Backspace |
| Selection | Shift+arrows (future enhancement) |
| Clipboard | Paste (Ctrl+V), system clipboard integration |
| Undo/Redo | Ctrl+Z / Ctrl+Y (future enhancement) |

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `←` / `→` | Move cursor left/right |
| `↑` / `↓` | Move cursor up/down |
| `Home` | Move to start of line |
| `End` | Move to end of line |
| `Ctrl+Home` | Move to start of buffer |
| `Ctrl+End` | Move to end of buffer |
| `Ctrl+←` | Move to previous word |
| `Ctrl+→` | Move to next word |
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character at cursor |
| `Ctrl+Backspace` | Delete word before cursor |
| `Ctrl+Delete` | Delete word after cursor |
| `Enter` | Insert newline |
| `Tab` | Insert 2 spaces (or autocomplete) |
| `Ctrl+V` | Paste from clipboard |
| `Ctrl+A` | Select all |
| `Ctrl+Enter` | Execute query |
| `Ctrl+Shift+F` | Format SQL |
| `Ctrl+E` | Execute with EXPLAIN ANALYZE |
| `Ctrl+/` | Toggle comment on current line |
| `Ctrl+L` | Clear editor |
| `Ctrl+↑` | Previous query from history |
| `Ctrl+↓` | Next query from history |

---

## Data Model

### Editor State

```rust
pub struct QueryEditor {
    // Content
    lines: Vec<String>,

    // Cursor position
    cursor_row: usize,
    cursor_col: usize,

    // View state
    scroll_offset: usize,
    viewport_height: usize,

    // History
    history: Vec<String>,
    history_index: Option<usize>,
    draft: Option<String>,  // Current unsaved query

    // Autocomplete
    autocomplete: Option<AutocompleteState>,

    // Undo stack (future)
    undo_stack: Vec<EditorAction>,
    redo_stack: Vec<EditorAction>,
}

pub struct AutocompleteState {
    suggestions: Vec<Completion>,
    selected_index: usize,
    trigger_position: (usize, usize),
    filter_text: String,
}

pub struct Completion {
    pub text: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
}

pub enum CompletionKind {
    Keyword,
    Table,
    Column,
    Function,
    Schema,
}
```

---

## SQL Formatting

### Trigger
- Keyboard: `Ctrl+Shift+F`
- Command bar: `:format`

### Behavior

```rust
use sqlformat::{format, FormatOptions, QueryParams, Indent};

impl QueryEditor {
    pub fn format_query(&mut self) {
        let current_text = self.get_text();

        let options = FormatOptions {
            indent: Indent::Spaces(2),
            uppercase: true,
            lines_between_queries: 2,
        };

        let formatted = format(&current_text, &QueryParams::None, options);

        // Replace content
        self.set_text(&formatted);

        // Move cursor to end
        self.cursor_row = self.lines.len().saturating_sub(1);
        self.cursor_col = self.lines.last().map(|l| l.len()).unwrap_or(0);
    }
}
```

### Example

**Before:**
```sql
select id,name,email,created_at from users where active=true and role='admin' order by created_at desc limit 10
```

**After:**
```sql
SELECT
  id,
  name,
  email,
  created_at
FROM
  users
WHERE
  active = true
  AND role = 'admin'
ORDER BY
  created_at DESC
LIMIT
  10
```

---

## Query Execution

### Execute Query (`Ctrl+Enter`)

```rust
impl QueryEditor {
    pub fn execute(&self) -> Action {
        let query = self.get_selected_or_all();
        Action::ExecuteQuery(query)
    }

    fn get_selected_or_all(&self) -> String {
        // If there's a selection, execute only selected text
        // Otherwise, execute entire buffer
        self.get_text()
    }
}
```

### Execute with EXPLAIN (`Ctrl+E`)

```rust
impl QueryEditor {
    pub fn execute_explain(&self) -> Action {
        let query = self.get_selected_or_all();
        let explain_query = format!(
            "EXPLAIN (ANALYZE, COSTS, VERBOSE, BUFFERS, FORMAT JSON)\n{}",
            query.trim_end_matches(';')
        );
        Action::ExecuteExplain(explain_query)
    }
}
```

### Execution States

| State | Indicator | Editor Behavior |
|-------|-----------|-----------------|
| Idle | None | Fully editable |
| Fixing | "Fixing..." in title | Read-only |
| Confirming | Fix preview shown | Accept/reject only |
| Executing | Spinner in title | Read-only (or allow cancel) |
| Completed | Success message | Editable, results shown |
| Failed | Error message | Editable, error highlighted |

---

## SQL Fixer

> Automatically fix common SQL errors before execution.

When the user presses `Ctrl+Enter`, vizgres intercepts the query and attempts to fix common mistakes before running it. This provides a "did you mean?" experience for sloppy or fast typing.

### UX Flow

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  User types:  SELEC * FORM usres WEHRE actve = true                         │
│                                                                             │
│  User presses Ctrl+Enter                                                    │
│                          ↓                                                  │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │ Did you mean?                                              [Esc/Enter] │  │
│  ├───────────────────────────────────────────────────────────────────────┤  │
│  │ SELECT *                                                               │  │
│  │ FROM users                                                             │  │
│  │ WHERE active = true                                                    │  │
│  ├───────────────────────────────────────────────────────────────────────┤  │
│  │ Fixes applied:                                                         │  │
│  │  • SELEC → SELECT (keyword typo)                                       │  │
│  │  • FORM → FROM (keyword typo)                                          │  │
│  │  • usres → users (matched schema table)                                │  │
│  │  • WEHRE → WHERE (keyword typo)                                        │  │
│  │  • actve → active (matched schema column)                              │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  Enter = Accept & Execute    Esc = Edit Original    Tab = Accept & Edit    │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Keyboard Shortcuts (Fix Confirmation)

| Key | Action |
|-----|--------|
| `Enter` | Accept fixed SQL, format, and execute |
| `Esc` | Dismiss, return to editing original |
| `Tab` | Accept fixed SQL into editor (don't execute) |
| `Ctrl+Enter` | Skip fixer, execute original as-is |

### Configuration

```toml
# ~/.vizgres/config.toml

[sql_fixer]
enabled = true
engine = "builtin"              # "builtin" or "ollama"
auto_accept = "high"            # auto-accept fixes at this confidence or above
                                # "high" = auto-accept obvious fixes
                                # "always" = never show confirmation
                                # "never" = always show confirmation

[sql_fixer.ollama]              # only if engine = "ollama"
model = "qwen2.5-coder:1.5b"
endpoint = "http://localhost:11434"
```

### SqlFixer Trait

```rust
/// Trait for SQL fixing implementations.
/// Vizgres ships with RuleBasedFixer; users can configure LlmFixer.
pub trait SqlFixer: Send + Sync {
    /// Attempt to fix malformed SQL using schema context.
    fn fix(&self, raw_sql: &str, schema: &Schema) -> FixResult;
}

pub struct FixResult {
    /// The corrected SQL (or original if no fixes)
    pub fixed_sql: String,

    /// List of fixes that were applied
    pub fixes: Vec<Fix>,

    /// Confidence level for the overall fix
    pub confidence: Confidence,

    /// Whether any changes were made
    pub changed: bool,
}

pub struct Fix {
    /// What was wrong
    pub original: String,

    /// What it was changed to
    pub replacement: String,

    /// Type of fix applied
    pub kind: FixKind,

    /// Position in original SQL
    pub span: (usize, usize),
}

pub enum FixKind {
    KeywordTypo,       // SELEC → SELECT
    IdentifierTypo,    // usres → users (matched against schema)
    ClauseReorder,     // ORDER BY before GROUP BY → fixed
    MissingKeyword,    // SELECT id name → SELECT id, name
    QuoteBalance,      // unclosed string literal
}

pub enum Confidence {
    High,    // Very likely correct (exact schema match, common typo)
    Medium,  // Probably correct (fuzzy match, context-based)
    Low,     // Uncertain (multiple possibilities, guessing)
}
```

### Schema Context

The fixer uses the connected database's schema to validate identifiers:

```rust
/// Schema information passed to the fixer
pub struct Schema {
    pub tables: Vec<TableInfo>,
    pub current_schema: String,
}

pub struct TableInfo {
    pub schema: String,
    pub name: String,
    pub columns: Vec<ColumnInfo>,
}

pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
}
```

### Built-in Fixer (RuleBasedFixer)

The default fixer applies a pipeline of independent fixers in sequence:

```rust
pub struct RuleBasedFixer {
    fixers: Vec<Box<dyn SubFixer>>,
}

trait SubFixer: Send + Sync {
    fn fix(&self, sql: &str, schema: &Schema, fixes: &mut Vec<Fix>) -> String;
}

impl Default for RuleBasedFixer {
    fn default() -> Self {
        Self {
            fixers: vec![
                Box::new(KeywordTypoFixer::new()),
                Box::new(IdentifierFixer::new()),
                Box::new(ClauseOrderFixer::new()),
                Box::new(MissingKeywordFixer::new()),
                Box::new(QuoteBalanceFixer::new()),
            ],
        }
    }
}
```

#### Keyword Typo Fixer

Fixes common keyword misspellings using edit distance:

| Typo | Fixed | Distance |
|------|-------|----------|
| `SELEC` | `SELECT` | 1 |
| `FORM` | `FROM` | 1 |
| `WEHRE` | `WHERE` | 1 |
| `ODRER` | `ORDER` | 2 |
| `GROPU` | `GROUP` | 2 |
| `DISTICT` | `DISTINCT` | 1 |

```rust
impl KeywordTypoFixer {
    const KEYWORDS: &[&str] = &[
        "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "IN",
        "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "CROSS", "ON",
        "GROUP", "BY", "ORDER", "HAVING", "LIMIT", "OFFSET",
        "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE",
        "DISTINCT", "AS", "BETWEEN", "LIKE", "IS", "NULL",
        "ASC", "DESC", "UNION", "EXCEPT", "INTERSECT",
    ];

    fn find_closest_keyword(&self, word: &str) -> Option<(&str, usize)> {
        // Return keyword with smallest edit distance if distance <= 2
    }
}
```

#### Identifier Fixer

Matches misspelled table/column names against the schema:

```rust
impl IdentifierFixer {
    fn fix_identifier(&self, word: &str, schema: &Schema) -> Option<String> {
        // Check tables
        for table in &schema.tables {
            if edit_distance(word, &table.name) <= 2 {
                return Some(table.name.clone());
            }

            // Check columns
            for col in &table.columns {
                if edit_distance(word, &col.name) <= 2 {
                    return Some(col.name.clone());
                }
            }
        }
        None
    }
}
```

#### Clause Order Fixer

Reorders SQL clauses to correct sequence:

```rust
// Correct order: SELECT → FROM → WHERE → GROUP BY → HAVING → ORDER BY → LIMIT
impl ClauseOrderFixer {
    fn reorder_clauses(&self, sql: &str) -> String {
        // Parse clause positions
        // Reorder if out of sequence
    }
}
```

### LLM Fixer (Optional)

For users who want AI-powered fixing via local Ollama:

```rust
pub struct LlmFixer {
    endpoint: String,
    model: String,
    client: reqwest::Client,
}

impl SqlFixer for LlmFixer {
    fn fix(&self, raw_sql: &str, schema: &Schema) -> FixResult {
        let prompt = format!(
            "Fix this SQL query. Only fix syntax errors and typos. \
             Do not change the query's meaning. \
             Available tables: {}\n\nQuery: {}\n\nFixed query:",
            schema.table_list(),
            raw_sql
        );

        // Call Ollama API
        // Parse response
        // Diff to find fixes
    }
}
```

### Integration with Execution Flow

```rust
impl QueryEditor {
    pub fn execute(&mut self) -> Action {
        let query = self.get_selected_or_all();

        // Check if fixer is enabled
        if self.config.sql_fixer.enabled {
            return Action::FixAndConfirm(query);
        }

        Action::ExecuteQuery(query)
    }

    pub fn handle_fix_result(&mut self, result: FixResult) {
        if !result.changed {
            // No fixes needed, execute immediately
            self.execute_directly(result.fixed_sql);
            return;
        }

        match self.config.sql_fixer.auto_accept {
            AutoAccept::Always => {
                self.format_and_execute(result.fixed_sql);
            }
            AutoAccept::High if result.confidence == Confidence::High => {
                self.format_and_execute(result.fixed_sql);
            }
            _ => {
                // Show confirmation dialog
                self.show_fix_confirmation(result);
            }
        }
    }

    fn format_and_execute(&mut self, sql: String) {
        let formatted = format_sql(&sql);
        self.pending_action = Some(Action::ExecuteQuery(formatted));
    }
}
```

### Fix Confirmation Widget

```rust
pub struct FixConfirmation {
    result: FixResult,
    original: String,
}

impl FixConfirmation {
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Title bar
        let title = Line::from(vec![
            Span::raw("Did you mean?"),
            Span::styled(" [Enter] Accept  [Esc] Edit  [Tab] Accept & Edit",
                         Style::default().fg(Color::DarkGray)),
        ]);

        // Fixed SQL with syntax highlighting
        let sql_block = Paragraph::new(highlight_sql(&self.result.fixed_sql))
            .block(Block::default().borders(Borders::ALL));

        // List of fixes
        let fixes: Vec<ListItem> = self.result.fixes.iter()
            .map(|f| ListItem::new(format!("• {} → {} ({})",
                f.original, f.replacement, f.kind)))
            .collect();

        // Render
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<FixConfirmAction> {
        match key.code {
            KeyCode::Enter => Some(FixConfirmAction::AcceptAndExecute),
            KeyCode::Esc => Some(FixConfirmAction::Dismiss),
            KeyCode::Tab => Some(FixConfirmAction::AcceptToEditor),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(FixConfirmAction::ExecuteOriginal)
            }
            _ => None,
        }
    }
}

pub enum FixConfirmAction {
    AcceptAndExecute,   // Enter: use fixed, format, run
    AcceptToEditor,     // Tab: use fixed, put in editor, don't run
    Dismiss,            // Esc: back to original in editor
    ExecuteOriginal,    // Ctrl+Enter: skip fixer, run original
}
```

### Testing Strategy

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_typo_selec_to_select() {
        let fixer = RuleBasedFixer::default();
        let schema = Schema::empty();

        let result = fixer.fix("SELEC * FROM users", &schema);

        assert_eq!(result.fixed_sql, "SELECT * FROM users");
        assert_eq!(result.fixes.len(), 1);
        assert_eq!(result.fixes[0].kind, FixKind::KeywordTypo);
    }

    #[test]
    fn test_identifier_typo_uses_schema() {
        let fixer = RuleBasedFixer::default();
        let schema = Schema::with_table("users", &["id", "name", "email"]);

        let result = fixer.fix("SELECT * FROM usres", &schema);

        assert_eq!(result.fixed_sql, "SELECT * FROM users");
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_multiple_fixes_combined() {
        let fixer = RuleBasedFixer::default();
        let schema = Schema::with_table("users", &["id", "active"]);

        let result = fixer.fix("SELEC * FORM usres WEHRE actve = true", &schema);

        assert_eq!(result.fixed_sql, "SELECT * FROM users WHERE active = true");
        assert_eq!(result.fixes.len(), 5);
    }

    #[test]
    fn test_no_changes_returns_original() {
        let fixer = RuleBasedFixer::default();
        let schema = Schema::with_table("users", &["id", "name"]);

        let result = fixer.fix("SELECT id, name FROM users", &schema);

        assert!(!result.changed);
        assert!(result.fixes.is_empty());
    }

    #[test]
    fn test_confidence_high_for_exact_schema_match() {
        let fixer = RuleBasedFixer::default();
        let schema = Schema::with_table("users", &["id"]);

        let result = fixer.fix("SELECT * FROM usres", &schema);

        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_confidence_medium_for_fuzzy_match() {
        let fixer = RuleBasedFixer::default();
        let schema = Schema::with_table("user_accounts", &["id"]);

        let result = fixer.fix("SELECT * FROM usr_acounts", &schema);

        assert_eq!(result.confidence, Confidence::Medium);
    }
}
```

---

## Autocomplete

### Trigger Conditions

| Trigger | Context |
|---------|---------|
| `.` after identifier | Column suggestions for table |
| After `FROM` / `JOIN` | Table/view suggestions |
| After `SELECT` | Column/function suggestions |
| After `WHERE` / `AND` / `OR` | Column suggestions |
| `Ctrl+Space` | Force show suggestions |

### Autocomplete UI

```
│ SELECT u.na█                                                              │
│           ┌──────────────────┐                                            │
│           │ name       (col) │ ← selected                                 │
│           │ nationality(col) │                                            │
│           └──────────────────┘                                            │
```

### Autocomplete Logic

```rust
impl QueryEditor {
    pub fn trigger_autocomplete(&mut self, schema: &SchemaTree) {
        let context = self.get_completion_context();

        let suggestions = match context.context_type {
            ContextType::AfterDot(table_alias) => {
                // Find table for alias, return its columns
                self.get_columns_for_alias(&table_alias, schema)
            }
            ContextType::AfterFrom | ContextType::AfterJoin => {
                // Return all tables and views
                self.get_all_tables(schema)
            }
            ContextType::AfterSelect => {
                // Return columns from tables in query + functions
                self.get_select_suggestions(schema)
            }
            ContextType::General => {
                // Return keywords + tables
                self.get_general_suggestions(schema)
            }
        };

        if !suggestions.is_empty() {
            self.autocomplete = Some(AutocompleteState {
                suggestions,
                selected_index: 0,
                trigger_position: (self.cursor_row, self.cursor_col),
                filter_text: String::new(),
            });
        }
    }

    fn get_completion_context(&self) -> CompletionContext {
        // Parse tokens before cursor
        // Determine context based on preceding keywords
    }
}
```

### Autocomplete Keyboard

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate suggestions |
| `Tab` / `Enter` | Accept suggestion |
| `Escape` | Dismiss suggestions |
| Typing | Filter suggestions |

---

## Query History

### Storage

```rust
pub struct QueryHistory {
    queries: VecDeque<HistoryEntry>,
    max_size: usize,
}

pub struct HistoryEntry {
    pub query: String,
    pub executed_at: DateTime<Utc>,
    pub execution_time: Option<Duration>,
    pub row_count: Option<usize>,
    pub success: bool,
}
```

### Navigation

```rust
impl QueryEditor {
    pub fn history_previous(&mut self) {
        if self.history.is_empty() {
            return;
        }

        // Save current as draft if first navigation
        if self.history_index.is_none() {
            self.draft = Some(self.get_text());
        }

        let new_index = match self.history_index {
            None => self.history.len() - 1,
            Some(i) if i > 0 => i - 1,
            Some(i) => i,
        };

        self.history_index = Some(new_index);
        self.set_text(&self.history[new_index]);
    }

    pub fn history_next(&mut self) {
        if let Some(current) = self.history_index {
            if current < self.history.len() - 1 {
                self.history_index = Some(current + 1);
                self.set_text(&self.history[current + 1]);
            } else {
                // Restore draft
                self.history_index = None;
                if let Some(draft) = &self.draft {
                    self.set_text(draft);
                }
            }
        }
    }
}
```

### History Persistence

```toml
# ~/.vizgres/history.sql

-- 2024-01-15 10:30:45 (23ms, 150 rows)
SELECT * FROM users WHERE active = true LIMIT 100;

-- 2024-01-15 10:32:12 (45ms, 1 row)
SELECT COUNT(*) FROM orders WHERE created_at > '2024-01-01';
```

---

## Syntax Highlighting

### Token Types

| Token | Color | Example |
|-------|-------|---------|
| Keyword | Blue | `SELECT`, `FROM`, `WHERE` |
| Function | Yellow | `COUNT`, `SUM`, `NOW` |
| String | Orange | `'hello'` |
| Number | Green | `123`, `45.67` |
| Identifier | Cyan | `users`, `id` |
| Operator | White | `=`, `>`, `AND` |
| Comment | Gray | `-- comment` |
| Error | Red | Unclosed string |

### Highlighting Implementation

```rust
pub fn highlight_sql(line: &str) -> Vec<(String, Style)> {
    let mut spans = vec![];
    let mut chars = line.chars().peekable();
    let mut current_token = String::new();

    while let Some(ch) = chars.next() {
        // Tokenize and assign styles
        // Keywords: SELECT, FROM, WHERE, etc.
        // Strings: 'quoted text'
        // Numbers: digits
        // Identifiers: alphanumeric starting with letter
    }

    spans
}

fn is_keyword(word: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "IN",
        "JOIN", "LEFT", "RIGHT", "INNER", "OUTER", "ON",
        "GROUP", "BY", "ORDER", "HAVING", "LIMIT", "OFFSET",
        "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE",
        "CREATE", "ALTER", "DROP", "TABLE", "INDEX", "VIEW",
        "AS", "DISTINCT", "ALL", "UNION", "EXCEPT", "INTERSECT",
        "NULL", "TRUE", "FALSE", "CASE", "WHEN", "THEN", "ELSE", "END",
    ];
    KEYWORDS.contains(&word.to_uppercase().as_str())
}
```

---

## Line Numbers

### Rendering

```rust
fn render_line_numbers(&self, frame: &mut Frame, area: Rect) {
    let line_count = self.lines.len();
    let width = line_count.to_string().len().max(2);

    for (i, line_num) in (self.scroll_offset..self.scroll_offset + area.height as usize)
        .enumerate()
    {
        if line_num < line_count {
            let num_str = format!("{:>width$} │", line_num + 1, width = width);
            let style = if line_num == self.cursor_row {
                Style::default().fg(Color::Yellow).bold()
            } else {
                Style::default().fg(Color::DarkGray)
            };

            frame.render_widget(
                Paragraph::new(num_str).style(style),
                Rect::new(area.x, area.y + i as u16, width as u16 + 2, 1),
            );
        }
    }
}
```

---

## Multi-Query Support

### Detection

```rust
fn split_queries(text: &str) -> Vec<&str> {
    // Split on semicolons, respecting string literals
    // Return individual queries
}
```

### Execution Options

1. **Execute All**: Run all queries in sequence
2. **Execute Current**: Run query containing cursor
3. **Execute Selected**: Run highlighted text only

---

## Error Display

### Inline Error Highlighting

```
│  7 │ SELECT * FROM uusers                                                 │
│    │               ~~~~~~ ERROR: relation "uusers" does not exist        │
│  8 │ WHERE active = true;                                                 │
```

### Error Parsing

```rust
pub struct QueryError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub hint: Option<String>,
}

fn parse_postgres_error(error: &tokio_postgres::Error) -> QueryError {
    // Parse error message
    // Extract LINE and POSITION if present
}
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_movement_right_moves_within_line() {
        let mut editor = QueryEditor::new();
        editor.set_text("SELECT");
        editor.cursor_col = 0;

        editor.move_right();

        assert_eq!(editor.cursor_col, 1);
    }

    #[test]
    fn test_cursor_movement_right_at_eol_moves_to_next_line() {
        let mut editor = QueryEditor::new();
        editor.set_text("SELECT\nFROM");
        editor.cursor_row = 0;
        editor.cursor_col = 6;

        editor.move_right();

        assert_eq!(editor.cursor_row, 1);
        assert_eq!(editor.cursor_col, 0);
    }

    #[test]
    fn test_insert_character_at_cursor() {
        let mut editor = QueryEditor::new();
        editor.set_text("SELCT");
        editor.cursor_col = 3;

        editor.insert_char('E');

        assert_eq!(editor.get_text(), "SELECT");
        assert_eq!(editor.cursor_col, 4);
    }

    #[test]
    fn test_insert_newline_splits_line() {
        let mut editor = QueryEditor::new();
        editor.set_text("SELECT *");
        editor.cursor_col = 6;

        editor.insert_newline();

        assert_eq!(editor.lines, vec!["SELECT", " *"]);
        assert_eq!(editor.cursor_row, 1);
        assert_eq!(editor.cursor_col, 0);
    }

    #[test]
    fn test_backspace_joins_lines() {
        let mut editor = QueryEditor::new();
        editor.set_text("SELECT\nFROM");
        editor.cursor_row = 1;
        editor.cursor_col = 0;

        editor.backspace();

        assert_eq!(editor.lines, vec!["SELECTFROM"]);
        assert_eq!(editor.cursor_row, 0);
        assert_eq!(editor.cursor_col, 6);
    }

    #[test]
    fn test_format_query_uppercases_keywords() {
        let mut editor = QueryEditor::new();
        editor.set_text("select * from users");

        editor.format_query();

        assert!(editor.get_text().contains("SELECT"));
        assert!(editor.get_text().contains("FROM"));
    }

    #[test]
    fn test_history_navigation_cycles_through_queries() {
        let mut editor = QueryEditor::new();
        editor.history = vec![
            "SELECT 1".to_string(),
            "SELECT 2".to_string(),
            "SELECT 3".to_string(),
        ];
        editor.set_text("current");

        editor.history_previous();
        assert_eq!(editor.get_text(), "SELECT 3");

        editor.history_previous();
        assert_eq!(editor.get_text(), "SELECT 2");

        editor.history_next();
        assert_eq!(editor.get_text(), "SELECT 3");

        editor.history_next();
        assert_eq!(editor.get_text(), "current");
    }
}
```

### Autocomplete Tests

```rust
#[test]
fn test_autocomplete_after_dot_shows_columns() {
    let mut editor = QueryEditor::new();
    editor.set_text("SELECT u.");
    editor.cursor_col = 9;

    let schema = test_schema_with_table("users", &["id", "name", "email"]);
    editor.trigger_autocomplete(&schema);

    let suggestions: Vec<_> = editor.autocomplete.unwrap().suggestions
        .iter().map(|s| s.text.as_str()).collect();

    assert!(suggestions.contains(&"id"));
    assert!(suggestions.contains(&"name"));
    assert!(suggestions.contains(&"email"));
}

#[test]
fn test_autocomplete_after_from_shows_tables() {
    let mut editor = QueryEditor::new();
    editor.set_text("SELECT * FROM ");
    editor.cursor_col = 14;

    let schema = test_schema_with_tables(&["users", "orders", "products"]);
    editor.trigger_autocomplete(&schema);

    let suggestions: Vec<_> = editor.autocomplete.unwrap().suggestions
        .iter().map(|s| s.text.as_str()).collect();

    assert!(suggestions.contains(&"users"));
    assert!(suggestions.contains(&"orders"));
    assert!(suggestions.contains(&"products"));
}
```

### Syntax Highlighting Tests

```rust
#[test]
fn test_highlight_keyword_is_blue() {
    let spans = highlight_sql("SELECT");
    assert_eq!(spans[0].1.fg, Some(Color::Blue));
}

#[test]
fn test_highlight_string_is_orange() {
    let spans = highlight_sql("'hello'");
    assert_eq!(spans[0].1.fg, Some(Color::Rgb(206, 145, 120)));
}

#[test]
fn test_highlight_handles_mixed_content() {
    let spans = highlight_sql("SELECT 'value' FROM users");

    // Verify each token is correctly styled
    assert!(spans.iter().any(|(t, s)| t == "SELECT" && s.fg == Some(Color::Blue)));
    assert!(spans.iter().any(|(t, s)| t == "'value'" && s.fg == Some(Color::Rgb(206, 145, 120))));
    assert!(spans.iter().any(|(t, s)| t == "FROM" && s.fg == Some(Color::Blue)));
}
```

---

## Performance Considerations

1. **Large Queries**: For queries > 10KB, disable real-time syntax highlighting
2. **Autocomplete Debounce**: Wait 100ms after typing before triggering autocomplete
3. **History Limit**: Keep only last 1000 queries in history
4. **Lazy Rendering**: Only render visible lines

---

## Next Steps

See [05-results-viewer.md](./05-results-viewer.md) for results display details.

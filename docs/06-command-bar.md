# 06 - Command Bar

> Command palette, autocomplete, quick actions, and status display.

---

## Overview

The Command Bar is a single-line input at the bottom of the screen, inspired by vim's command mode and Claude CLI's interface. It provides quick access to commands, connection switching, and displays status messages when not in command mode.

---

## Visual Layout

### Command Mode

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ :connect pr█                                                                │
│ ┌───────────────────────────────────────────────────────────────────────┐   │
│ │ connect production    Connect to production database                  │   │
│ │ connect staging       Connect to staging database                     │   │
│ └───────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
    ▲           ▲                            ▲
    │           │                            │
  Prompt     User input               Autocomplete popup
```

### Status Mode (Default)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ ✓ Query executed: 150 rows in 23ms                      Press : for commands│
└─────────────────────────────────────────────────────────────────────────────┘
    ▲                                                            ▲
    │                                                            │
  Status message                                           Hint text
```

---

## Modes

### Mode States

| Mode | Trigger | Display |
|------|---------|---------|
| Status | Default | Status message or connection info |
| Command | `:` | Command input with prompt |
| Search | `/` | Search input (in tree/results) |
| Filter | `Ctrl+F` | Filter input |

### Mode Transitions

```
Status Mode
    │
    ├─── ':' ───▶ Command Mode ───▶ Enter/Esc ───▶ Status Mode
    │
    ├─── '/' ───▶ Search Mode ────▶ Enter/Esc ───▶ Status Mode
    │
    └─── Ctrl+F ▶ Filter Mode ────▶ Esc ─────────▶ Status Mode
```

---

## Commands

### Connection Commands

| Command | Aliases | Description | Arguments |
|---------|---------|-------------|-----------|
| `:connect` | `:c` | Connect to saved connection | `<name>` |
| `:disconnect` | `:dc` | Close current connection | — |
| `:save` | `:s` | Save current connection | `<name>` |
| `:connections` | `:conns` | List saved connections | — |
| `:new` | `:n` | New connection dialog | — |

### Query Commands

| Command | Aliases | Description | Arguments |
|---------|---------|-------------|-----------|
| `:format` | `:fmt` | Format current query | — |
| `:clear` | `:cl` | Clear query editor | — |
| `:history` | `:hist` | Show query history | `[count]` |

### Export Commands

| Command | Aliases | Description | Arguments |
|---------|---------|-------------|-----------|
| `:export` | `:e` | Export results | `csv\|json\|sql\|md [path]` |
| `:copy` | `:cp` | Copy results to clipboard | `csv\|json` |

### View Commands

| Command | Aliases | Description | Arguments |
|---------|---------|-------------|-----------|
| `:refresh` | `:r` | Refresh tree browser | — |
| `:focus` | `:f` | Focus specific panel | `tree\|query\|results` |
| `:theme` | — | Switch color theme | `dark\|light\|<name>` |

### Settings Commands

| Command | Aliases | Description | Arguments |
|---------|---------|-------------|-----------|
| `:set` | — | Set option | `<key> <value>` |
| `:toggle` | `:t` | Toggle boolean option | `<key>` |

### General Commands

| Command | Aliases | Description | Arguments |
|---------|---------|-------------|-----------|
| `:help` | `:h`, `:?` | Show help | `[command]` |
| `:quit` | `:q` | Quit application | — |
| `:version` | `:v` | Show version | — |

---

## Data Model

### Command Bar State

```rust
pub struct CommandBar {
    mode: CommandBarMode,
    input: String,
    cursor_position: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    autocomplete: Option<AutocompleteState>,
    status: Option<StatusMessage>,
}

pub enum CommandBarMode {
    Status,
    Command,
    Search,
    Filter,
}

pub struct StatusMessage {
    text: String,
    level: StatusLevel,
    timestamp: Instant,
}

pub enum StatusLevel {
    Success,
    Error,
    Info,
    Warning,
}

pub struct AutocompleteState {
    suggestions: Vec<CommandSuggestion>,
    selected_index: usize,
}

pub struct CommandSuggestion {
    command: String,
    description: String,
    arguments: Option<String>,
}
```

---

## Command Parsing

### Parser Implementation

```rust
pub fn parse_command(input: &str) -> Result<Command, ParseError> {
    let input = input.trim();

    // Remove leading ':'
    let input = input.strip_prefix(':').unwrap_or(input);

    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let args = parts.get(1).map(|s| s.trim());

    match cmd.as_str() {
        "connect" | "c" => {
            let name = args.ok_or(ParseError::MissingArgument("connection name"))?;
            Ok(Command::Connect(name.to_string()))
        }
        "disconnect" | "dc" => Ok(Command::Disconnect),
        "save" | "s" => {
            let name = args.ok_or(ParseError::MissingArgument("connection name"))?;
            Ok(Command::SaveConnection(name.to_string()))
        }
        "format" | "fmt" => Ok(Command::Format),
        "export" | "e" => {
            let format = args.ok_or(ParseError::MissingArgument("format"))?;
            parse_export_args(format)
        }
        "refresh" | "r" => Ok(Command::Refresh),
        "quit" | "q" => Ok(Command::Quit),
        "help" | "h" | "?" => Ok(Command::Help(args.map(String::from))),
        "set" => {
            let args = args.ok_or(ParseError::MissingArgument("key value"))?;
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            if parts.len() < 2 {
                return Err(ParseError::MissingArgument("value"));
            }
            Ok(Command::Set(parts[0].to_string(), parts[1].to_string()))
        }
        "" => Err(ParseError::EmptyCommand),
        _ => Err(ParseError::UnknownCommand(cmd)),
    }
}

pub enum Command {
    Connect(String),
    Disconnect,
    SaveConnection(String),
    Format,
    Clear,
    Export(ExportFormat, Option<PathBuf>),
    Copy(ExportFormat),
    Refresh,
    Focus(PanelFocus),
    Theme(String),
    Set(String, String),
    Toggle(String),
    Help(Option<String>),
    Quit,
    History(Option<usize>),
    Connections,
    Version,
}

#[derive(Debug)]
pub enum ParseError {
    EmptyCommand,
    UnknownCommand(String),
    MissingArgument(&'static str),
    InvalidArgument(String),
}
```

---

## Autocomplete

### Trigger Behavior

- Autocomplete activates as user types
- Shows matching commands and saved connections
- Updates with each keystroke

### Autocomplete Sources

| Context | Suggestions |
|---------|-------------|
| Empty input | All commands |
| `:c` | Commands starting with 'c' |
| `:connect ` | Saved connection names |
| `:export ` | Export format options |
| `:focus ` | Panel names |
| `:set ` | Setting keys |
| `:theme ` | Available themes |

### Autocomplete Implementation

```rust
impl CommandBar {
    pub fn update_autocomplete(&mut self, connections: &[String]) {
        let input = self.input.trim_start_matches(':');

        if input.is_empty() {
            self.autocomplete = Some(AutocompleteState {
                suggestions: self.get_all_commands(),
                selected_index: 0,
            });
            return;
        }

        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let cmd = parts[0];

        let suggestions = if parts.len() == 1 {
            // Command completion
            self.get_matching_commands(cmd)
        } else {
            // Argument completion
            match cmd.to_lowercase().as_str() {
                "connect" | "c" => {
                    let filter = parts[1].to_lowercase();
                    connections.iter()
                        .filter(|c| c.to_lowercase().starts_with(&filter))
                        .map(|c| CommandSuggestion {
                            command: format!("connect {}", c),
                            description: format!("Connect to {}", c),
                            arguments: None,
                        })
                        .collect()
                }
                "export" | "e" => {
                    vec!["csv", "json", "sql", "md"].iter()
                        .filter(|f| f.starts_with(parts[1]))
                        .map(|f| CommandSuggestion {
                            command: format!("export {}", f),
                            description: format!("Export as {}", f.to_uppercase()),
                            arguments: Some("[path]".to_string()),
                        })
                        .collect()
                }
                "focus" | "f" => {
                    vec!["tree", "query", "results"].iter()
                        .filter(|p| p.starts_with(parts[1]))
                        .map(|p| CommandSuggestion {
                            command: format!("focus {}", p),
                            description: format!("Focus {} panel", p),
                            arguments: None,
                        })
                        .collect()
                }
                _ => vec![],
            }
        };

        if suggestions.is_empty() {
            self.autocomplete = None;
        } else {
            self.autocomplete = Some(AutocompleteState {
                suggestions,
                selected_index: 0,
            });
        }
    }

    fn get_matching_commands(&self, prefix: &str) -> Vec<CommandSuggestion> {
        COMMANDS.iter()
            .filter(|(cmd, _, _)| cmd.starts_with(&prefix.to_lowercase()))
            .map(|(cmd, desc, args)| CommandSuggestion {
                command: cmd.to_string(),
                description: desc.to_string(),
                arguments: args.map(String::from),
            })
            .collect()
    }
}

const COMMANDS: &[(&str, &str, Option<&str>)] = &[
    ("connect", "Connect to saved connection", Some("<name>")),
    ("disconnect", "Close current connection", None),
    ("save", "Save current connection", Some("<name>")),
    ("format", "Format SQL in editor", None),
    ("clear", "Clear query editor", None),
    ("export", "Export results to file", Some("csv|json|sql|md [path]")),
    ("copy", "Copy results to clipboard", Some("csv|json")),
    ("refresh", "Refresh schema tree", None),
    ("focus", "Focus panel", Some("tree|query|results")),
    ("quit", "Exit application", None),
    ("help", "Show help", Some("[command]")),
];
```

### Autocomplete Rendering

```
│ :conn█                                                                      │
│ ┌─────────────────────────────────────────────────────────────────────────┐ │
│ │▶connect      Connect to saved connection                          <name>│ │
│ │ connections  List saved connections                                     │ │
│ └─────────────────────────────────────────────────────────────────────────┘ │
```

---

## Keyboard Controls

### Command Mode Keys

| Key | Action |
|-----|--------|
| `Enter` | Execute command |
| `Escape` | Cancel and return to status mode |
| `Tab` | Accept autocomplete suggestion |
| `↑` / `↓` | Navigate autocomplete / command history |
| `Ctrl+U` | Clear input line |
| `Ctrl+W` | Delete word before cursor |
| `←` / `→` | Move cursor |
| `Home` / `End` | Move to start/end of input |
| `Backspace` | Delete character before cursor |
| `Delete` | Delete character at cursor |

### Global Shortcuts to Command Bar

| Key | Action |
|-----|--------|
| `:` | Enter command mode |
| `/` | Enter search mode (context-dependent) |
| `Escape` | Return to status mode (from any mode) |

---

## Status Messages

### Display Behavior

```rust
impl CommandBar {
    pub fn set_status(&mut self, message: impl Into<String>, level: StatusLevel) {
        self.status = Some(StatusMessage {
            text: message.into(),
            level,
            timestamp: Instant::now(),
        });
    }

    pub fn clear_expired_status(&mut self) {
        if let Some(status) = &self.status {
            let expiry = match status.level {
                StatusLevel::Success => Duration::from_secs(3),
                StatusLevel::Info => Duration::from_secs(3),
                StatusLevel::Warning => Duration::from_secs(4),
                StatusLevel::Error => Duration::from_secs(5),
            };

            if status.timestamp.elapsed() > expiry {
                self.status = None;
            }
        }
    }
}
```

### Status Message Types

| Level | Icon | Color | Example |
|-------|------|-------|---------|
| Success | `✓` | Green | "Query executed: 150 rows in 23ms" |
| Error | `✗` | Red | "ERROR: relation 'users' does not exist" |
| Info | `ℹ` | Blue | "Connected to localhost:5432/mydb" |
| Warning | `⚠` | Yellow | "Query returned 10000+ rows, showing first 1000" |

### Status Rendering

```rust
fn render_status(&self, frame: &mut Frame, area: Rect) {
    let content = match &self.status {
        Some(status) => {
            let (icon, style) = match status.level {
                StatusLevel::Success => ("✓", Style::default().fg(Color::Green)),
                StatusLevel::Error => ("✗", Style::default().fg(Color::Red)),
                StatusLevel::Info => ("ℹ", Style::default().fg(Color::Blue)),
                StatusLevel::Warning => ("⚠", Style::default().fg(Color::Yellow)),
            };

            Line::from(vec![
                Span::styled(format!("{} ", icon), style),
                Span::raw(&status.text),
            ])
        }
        None => {
            // Show connection info or default hint
            Line::from(vec![
                Span::styled("Press ", Style::default().fg(Color::DarkGray)),
                Span::styled(":", Style::default().fg(Color::Cyan)),
                Span::styled(" for commands", Style::default().fg(Color::DarkGray)),
            ])
        }
    };

    frame.render_widget(Paragraph::new(content), area);
}
```

---

## Command Execution

### Execution Flow

```rust
impl App {
    pub fn execute_command(&mut self, cmd: Command) -> Result<(), AppError> {
        match cmd {
            Command::Connect(name) => {
                self.command_bar.set_status("Connecting...", StatusLevel::Info);

                match self.config.get_connection(&name) {
                    Some(config) => {
                        // Async connect
                        self.pending_action = Some(Action::Connect(config.clone()));
                    }
                    None => {
                        self.command_bar.set_status(
                            format!("Unknown connection: {}", name),
                            StatusLevel::Error,
                        );
                    }
                }
            }
            Command::Disconnect => {
                if self.connection.is_some() {
                    self.pending_action = Some(Action::Disconnect);
                    self.command_bar.set_status("Disconnected", StatusLevel::Info);
                }
            }
            Command::Format => {
                self.query_editor.format_query();
                self.command_bar.set_status("Query formatted", StatusLevel::Success);
            }
            Command::Export(format, path) => {
                let content = self.results_viewer.export(format);
                let path = path.unwrap_or_else(|| self.generate_export_path(format));
                std::fs::write(&path, content)?;
                self.command_bar.set_status(
                    format!("Exported to {}", path.display()),
                    StatusLevel::Success,
                );
            }
            Command::Quit => {
                self.running = false;
            }
            // ... other commands
        }
        Ok(())
    }
}
```

---

## Help System

### Help Command

```
:help

Available Commands:
──────────────────────────────────────────────────────────────────
  :connect <name>       Connect to saved connection
  :disconnect           Close current connection
  :save <name>          Save current connection
  :format               Format SQL in editor
  :export <format>      Export results (csv|json|sql|md)
  :refresh              Refresh schema tree
  :quit                 Exit application

Use :help <command> for detailed help on a specific command.
Press Escape to close.
```

### Contextual Help

```
:help export

:export <format> [path]

Export query results to a file.

Formats:
  csv     Comma-separated values
  json    JSON array of objects
  sql     SQL INSERT statements
  md      Markdown table

Examples:
  :export csv
  :export json /tmp/results.json
  :export sql ./insert_data.sql
```

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_connect_command() {
        let result = parse_command(":connect production");
        assert!(matches!(result, Ok(Command::Connect(name)) if name == "production"));
    }

    #[test]
    fn test_parse_connect_with_alias() {
        let result = parse_command(":c prod");
        assert!(matches!(result, Ok(Command::Connect(name)) if name == "prod"));
    }

    #[test]
    fn test_parse_connect_without_name_fails() {
        let result = parse_command(":connect");
        assert!(matches!(result, Err(ParseError::MissingArgument(_))));
    }

    #[test]
    fn test_parse_export_with_format() {
        let result = parse_command(":export csv");
        assert!(matches!(result, Ok(Command::Export(ExportFormat::Csv, None))));
    }

    #[test]
    fn test_parse_export_with_path() {
        let result = parse_command(":export json /tmp/out.json");
        match result {
            Ok(Command::Export(ExportFormat::Json, Some(path))) => {
                assert_eq!(path, PathBuf::from("/tmp/out.json"));
            }
            _ => panic!("Expected Export command with path"),
        }
    }

    #[test]
    fn test_parse_unknown_command() {
        let result = parse_command(":foobar");
        assert!(matches!(result, Err(ParseError::UnknownCommand(cmd)) if cmd == "foobar"));
    }

    #[test]
    fn test_parse_empty_command() {
        let result = parse_command(":");
        assert!(matches!(result, Err(ParseError::EmptyCommand)));
    }
}
```

### Autocomplete Tests

```rust
#[test]
fn test_autocomplete_shows_all_commands_for_empty() {
    let mut bar = CommandBar::new();
    bar.input = ":".to_string();
    bar.update_autocomplete(&[]);

    let ac = bar.autocomplete.unwrap();
    assert!(ac.suggestions.len() > 5);  // Multiple commands
}

#[test]
fn test_autocomplete_filters_by_prefix() {
    let mut bar = CommandBar::new();
    bar.input = ":co".to_string();
    bar.update_autocomplete(&[]);

    let ac = bar.autocomplete.unwrap();
    let cmds: Vec<_> = ac.suggestions.iter().map(|s| s.command.as_str()).collect();
    assert!(cmds.iter().all(|c| c.starts_with("co")));
    assert!(cmds.contains(&"connect"));
    assert!(cmds.contains(&"connections"));
    assert!(!cmds.contains(&"quit"));
}

#[test]
fn test_autocomplete_shows_connections_for_connect() {
    let mut bar = CommandBar::new();
    bar.input = ":connect pr".to_string();
    bar.update_autocomplete(&["production".to_string(), "staging".to_string()]);

    let ac = bar.autocomplete.unwrap();
    let cmds: Vec<_> = ac.suggestions.iter().map(|s| s.command.as_str()).collect();
    assert!(cmds.contains(&"connect production"));
    assert!(!cmds.contains(&"connect staging"));
}

#[test]
fn test_autocomplete_navigation() {
    let mut bar = CommandBar::new();
    bar.input = ":".to_string();
    bar.update_autocomplete(&[]);

    let initial = bar.autocomplete.as_ref().unwrap().selected_index;
    bar.autocomplete_next();
    assert_eq!(bar.autocomplete.as_ref().unwrap().selected_index, initial + 1);

    bar.autocomplete_prev();
    assert_eq!(bar.autocomplete.as_ref().unwrap().selected_index, initial);
}
```

### Status Message Tests

```rust
#[test]
fn test_status_message_expires() {
    let mut bar = CommandBar::new();
    bar.set_status("Test message", StatusLevel::Success);

    // Immediately after, should be present
    assert!(bar.status.is_some());

    // After expiry (mock time), should be cleared
    // In real test, use time mocking
}

#[test]
fn test_error_status_has_longer_expiry() {
    // Error messages should last 5 seconds vs 3 for success
    let mut bar = CommandBar::new();
    bar.set_status("Error", StatusLevel::Error);

    // At 4 seconds, should still be present
    // At 6 seconds, should be gone
}
```

---

## Performance Considerations

1. **Debounce Autocomplete**: Don't update on every keystroke, wait 50ms
2. **Limit Suggestions**: Show max 10 suggestions
3. **Lazy Load Connection List**: Only fetch when needed

---

## Next Steps

See [07-keyboard-shortcuts.md](./07-keyboard-shortcuts.md) for complete keybinding reference.

//! Command parsing
//!
//! Parses user input from the command bar into structured Command enums.

use crate::error::{CommandError, CommandResult};

/// Commands that can be executed from the command bar
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Connect to a saved connection profile
    Connect(String),

    /// Disconnect from current database
    Disconnect,

    /// Save current connection as a profile
    SaveConnection(String),

    /// Refresh database schema
    Refresh,

    /// Export query results to file
    Export(ExportFormat),

    /// Set a configuration value
    Set(String, String),

    /// Show help
    Help,

    /// Quit the application
    Quit,
}

/// Export file formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Json,
    Sql,
}

/// Parse a command string into a Command enum
///
/// # Examples
/// ```ignore
/// let cmd = parse_command(":connect prod")?;
/// assert!(matches!(cmd, Command::Connect(name) if name == "prod"));
/// ```
pub fn parse_command(input: &str) -> CommandResult<Command> {
    let input = input.trim();

    // Remove leading ':' if present
    let input = input.strip_prefix(':').unwrap_or(input);

    // Split into parts
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.is_empty() {
        return Err(CommandError::Unknown(String::new()));
    }

    match parts[0] {
        "connect" | "c" => {
            let name = parts
                .get(1)
                .ok_or(CommandError::MissingArgument)?
                .to_string();
            Ok(Command::Connect(name))
        }

        "disconnect" | "dc" => Ok(Command::Disconnect),

        "save" => {
            let name = parts
                .get(1)
                .ok_or(CommandError::MissingArgument)?
                .to_string();
            Ok(Command::SaveConnection(name))
        }

        "refresh" | "r" => Ok(Command::Refresh),

        "export" => {
            let format_str = parts.get(1).ok_or(CommandError::MissingArgument)?;
            let format = match *format_str {
                "csv" => ExportFormat::Csv,
                "json" => ExportFormat::Json,
                "sql" => ExportFormat::Sql,
                _ => {
                    return Err(CommandError::InvalidArgument(format!(
                        "Unknown export format: {}",
                        format_str
                    )))
                }
            };
            Ok(Command::Export(format))
        }

        "set" => {
            if parts.len() < 3 {
                return Err(CommandError::MissingArgument);
            }
            Ok(Command::Set(parts[1].to_string(), parts[2].to_string()))
        }

        "help" | "h" | "?" => Ok(Command::Help),

        "quit" | "q" | "exit" => Ok(Command::Quit),

        unknown => Err(CommandError::Unknown(unknown.to_string())),
    }
}

/// Command autocomplete helper
pub struct CommandCompleter {
    /// Available connection names
    connection_names: Vec<String>,
}

impl CommandCompleter {
    /// Create a new command completer
    pub fn new(connection_names: Vec<String>) -> Self {
        Self { connection_names }
    }

    /// Get completion suggestions for partial input
    pub fn complete(&self, _input: &str) -> Vec<String> {
        // TODO: Phase 6 - Implement smart autocomplete
        // 1. Parse partial input
        // 2. Determine what we're completing (command name, argument, etc.)
        // 3. Return relevant suggestions
        todo!("Command autocomplete not yet implemented")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_connect_command() {
        let cmd = parse_command(":connect prod").unwrap();
        assert_eq!(cmd, Command::Connect("prod".to_string()));
    }

    #[test]
    fn test_parse_connect_short() {
        let cmd = parse_command(":c mydb").unwrap();
        assert_eq!(cmd, Command::Connect("mydb".to_string()));
    }

    #[test]
    fn test_parse_disconnect() {
        let cmd = parse_command(":disconnect").unwrap();
        assert_eq!(cmd, Command::Disconnect);
    }

    #[test]
    fn test_parse_export_csv() {
        let cmd = parse_command(":export csv").unwrap();
        assert_eq!(cmd, Command::Export(ExportFormat::Csv));
    }

    #[test]
    fn test_parse_quit_variants() {
        assert_eq!(parse_command(":quit").unwrap(), Command::Quit);
        assert_eq!(parse_command(":q").unwrap(), Command::Quit);
        assert_eq!(parse_command(":exit").unwrap(), Command::Quit);
    }

    #[test]
    fn test_parse_missing_argument() {
        let result = parse_command(":connect");
        assert!(matches!(result, Err(CommandError::MissingArgument)));
    }

    #[test]
    fn test_parse_unknown_command() {
        let result = parse_command(":foobar");
        assert!(matches!(result, Err(CommandError::Unknown(_))));
    }

    #[test]
    fn test_parse_without_colon() {
        let cmd = parse_command("quit").unwrap();
        assert_eq!(cmd, Command::Quit);
    }
}

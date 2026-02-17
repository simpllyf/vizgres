//! Command parsing
//!
//! Parses user input from the command bar into structured Command enums.

use crate::error::{CommandError, CommandResult};

/// Commands that can be executed from the command bar
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Connect to a saved connection profile or URL
    Connect(String),

    /// Disconnect from current database
    Disconnect,

    /// Refresh database schema
    Refresh,

    /// Show help
    Help,

    /// Quit the application
    Quit,
}

/// Parse a command string into a Command enum
pub fn parse_command(input: &str) -> CommandResult<Command> {
    let input = input.trim();
    let input = input.strip_prefix(':').unwrap_or(input);
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
        "refresh" | "r" => Ok(Command::Refresh),
        "help" | "h" | "?" => Ok(Command::Help),
        "quit" | "q" | "exit" => Ok(Command::Quit),
        unknown => Err(CommandError::Unknown(unknown.to_string())),
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

//! Command parsing
//!
//! Parses user input from the command bar into structured Command enums.
//! Commands use `/` prefix (e.g., `/help`, `/quit`).

use crate::error::{CommandError, CommandResult};

/// Commands that can be executed from the command bar
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Refresh database schema
    Refresh,

    /// Clear the query editor
    Clear,

    /// Show help
    Help,

    /// Quit the application
    Quit,
}

/// Parse a command string into a Command enum
pub fn parse_command(input: &str) -> CommandResult<Command> {
    let input = input.trim();
    // Strip optional / or : prefix (accept both during transition)
    let input = input
        .strip_prefix('/')
        .or_else(|| input.strip_prefix(':'))
        .unwrap_or(input);
    let parts: Vec<&str> = input.split_whitespace().collect();

    if parts.is_empty() {
        return Err(CommandError::Unknown(String::new()));
    }

    match parts[0] {
        "refresh" | "r" => Ok(Command::Refresh),
        "clear" | "cl" => Ok(Command::Clear),
        "help" | "h" | "?" => Ok(Command::Help),
        "quit" | "q" | "exit" => Ok(Command::Quit),
        unknown => Err(CommandError::Unknown(unknown.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_refresh() {
        assert_eq!(parse_command("/refresh").unwrap(), Command::Refresh);
        assert_eq!(parse_command("/r").unwrap(), Command::Refresh);
    }

    #[test]
    fn test_parse_clear() {
        assert_eq!(parse_command("/clear").unwrap(), Command::Clear);
        assert_eq!(parse_command("/cl").unwrap(), Command::Clear);
    }

    #[test]
    fn test_parse_quit_variants() {
        assert_eq!(parse_command("/quit").unwrap(), Command::Quit);
        assert_eq!(parse_command("/q").unwrap(), Command::Quit);
        assert_eq!(parse_command("/exit").unwrap(), Command::Quit);
    }

    #[test]
    fn test_parse_help() {
        assert_eq!(parse_command("/help").unwrap(), Command::Help);
        assert_eq!(parse_command("/h").unwrap(), Command::Help);
        assert_eq!(parse_command("/?").unwrap(), Command::Help);
    }

    #[test]
    fn test_parse_unknown_command() {
        let result = parse_command("/foobar");
        assert!(matches!(result, Err(CommandError::Unknown(_))));
    }

    #[test]
    fn test_parse_without_prefix() {
        assert_eq!(parse_command("quit").unwrap(), Command::Quit);
    }

    #[test]
    fn test_parse_colon_prefix_still_works() {
        assert_eq!(parse_command(":quit").unwrap(), Command::Quit);
        assert_eq!(parse_command(":help").unwrap(), Command::Help);
    }
}

//! Command parsing and execution
//!
//! Handles the command bar system (commands starting with `:`)

pub mod parser;

pub use parser::{Command, parse_command};

//! Terminal UI components
//!
//! All UI widgets and rendering logic using ratatui.

pub mod command_bar;
pub mod editor;
pub mod inspector;
pub mod layout;
pub mod render;
pub mod results;
pub mod theme;
pub mod tree;

use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::ui::theme::Theme;

/// Actions a component can return to signal intent to the parent.
/// Components never mutate siblings — they declare what should happen,
/// and `App::process_component_action` decides how.
pub enum ComponentAction {
    /// Event consumed, no further action needed
    Consumed,
    /// Event not handled, parent should try
    Ignored,
    /// Open inspector with (value, column_name, data_type)
    OpenInspector(String, String, String),
    /// Close the inspector
    CloseInspector,
    /// Copy text to clipboard
    CopyToClipboard(String),
    /// Execute a command string
    ExecuteCommand(String),
    /// Dismiss command bar (cancel)
    DismissCommandBar,
}

/// Trait for UI components
pub trait Component {
    /// Handle a key event, returning an action for the parent to process
    fn handle_key(&mut self, key: KeyEvent) -> ComponentAction;

    /// Render the component to the frame
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, theme: &Theme);
}

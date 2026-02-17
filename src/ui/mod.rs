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
/// Most keybindings are resolved by `KeyMap` before reaching the component.
/// Components only return `Consumed` / `Ignored` for free-form text input.
pub enum ComponentAction {
    /// Event consumed, no further action needed
    Consumed,
    /// Event not handled, parent should try
    Ignored,
}

/// Trait for UI components
pub trait Component {
    /// Handle a key event, returning an action for the parent to process
    fn handle_key(&mut self, key: KeyEvent) -> ComponentAction;

    /// Render the component to the frame
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool, theme: &Theme);
}

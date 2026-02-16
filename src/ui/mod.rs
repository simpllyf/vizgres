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

/// Trait for UI components
pub trait Component {
    /// Handle a key event, return true if consumed
    fn handle_key(&mut self, key: KeyEvent) -> bool;

    /// Render the component to the frame
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool);
}

//! Terminal UI components
//!
//! All UI widgets and rendering logic using ratatui.

pub mod cell_popup;
pub mod command_bar;
pub mod editor;
pub mod layout;
pub mod results;
pub mod theme;
pub mod tree;

use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};

/// Trait for UI components
pub trait Component {
    /// Handle a key event, return true if consumed
    fn handle_key(&mut self, key: KeyEvent) -> bool;

    /// Render the component to the frame
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool);

    /// Get the minimum size this component needs (width, height)
    fn min_size(&self) -> (u16, u16) {
        (10, 3) // Reasonable default
    }
}

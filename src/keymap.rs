//! Data-driven keybinding configuration
//!
//! All keybindings are defined as data in `KeyMap::default()`, not as match arms
//! scattered across components. To add a new binding, add an entry to the
//! appropriate context in `KeyMap::default()` and handle the `KeyAction` in
//! `App::execute_key_action()`.

use crate::app::PanelFocus;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;

/// A key combination (code + modifiers)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyBind {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl From<KeyEvent> for KeyBind {
    fn from(event: KeyEvent) -> Self {
        Self {
            code: event.code,
            modifiers: event.modifiers,
        }
    }
}

/// Semantic key actions — what a key means, not what key it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    // Global
    Quit,
    OpenCommandBar,
    CycleFocus,
    CycleFocusReverse,

    // Navigation (shared by tree, results, inspector)
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    PageUp,
    PageDown,
    GoToTop,
    GoToBottom,
    Home,
    End,

    // Editor-specific
    ExecuteQuery,
    ClearEditor,

    // Results-specific
    OpenInspector,
    CopyCell,
    CopyRow,

    // Inspector-specific
    CopyContent,

    // Tree-specific
    ToggleExpand,
    Expand,
    Collapse,

    // Modal dismiss/submit
    Dismiss,
    Submit,
}

/// Keybinding configuration — maps key combos to semantic actions per context.
pub struct KeyMap {
    /// Bindings that apply regardless of focus (checked first)
    global: HashMap<KeyBind, KeyAction>,
    /// Per-panel bindings (checked after global)
    panels: HashMap<PanelFocus, HashMap<KeyBind, KeyAction>>,
}

impl KeyMap {
    /// Resolve a key event to a semantic action.
    /// Checks global bindings first, then panel-specific bindings.
    pub fn resolve(&self, focus: PanelFocus, key: KeyEvent) -> Option<KeyAction> {
        let bind = KeyBind::from(key);
        if let Some(action) = self.global.get(&bind) {
            return Some(*action);
        }
        self.panels
            .get(&focus)
            .and_then(|m| m.get(&bind))
            .copied()
    }
}

impl Default for KeyMap {
    fn default() -> Self {
        let mut global = HashMap::new();
        global.insert(
            KeyBind {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::Quit,
        );
        global.insert(
            KeyBind {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::OpenCommandBar,
        );
        global.insert(
            KeyBind {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::CycleFocus,
        );
        global.insert(
            KeyBind {
                code: KeyCode::BackTab,
                modifiers: KeyModifiers::SHIFT,
            },
            KeyAction::CycleFocusReverse,
        );

        let mut panels = HashMap::new();

        // ── Editor ───────────────────────────────────────────────
        let mut editor = HashMap::new();
        editor.insert(
            KeyBind {
                code: KeyCode::F(5),
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::ExecuteQuery,
        );
        editor.insert(
            KeyBind {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::ExecuteQuery,
        );
        editor.insert(
            KeyBind {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::ClearEditor,
        );
        panels.insert(PanelFocus::QueryEditor, editor);

        // ── Results ──────────────────────────────────────────────
        let mut results = HashMap::new();
        insert_vim_nav(&mut results);
        results.insert(
            KeyBind {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::OpenInspector,
        );
        results.insert(
            KeyBind {
                code: KeyCode::Char('y'),
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::CopyCell,
        );
        results.insert(
            KeyBind {
                code: KeyCode::Char('Y'),
                modifiers: KeyModifiers::SHIFT,
            },
            KeyAction::CopyRow,
        );
        panels.insert(PanelFocus::ResultsViewer, results);

        // ── Tree ─────────────────────────────────────────────────
        let mut tree = HashMap::new();
        tree.insert(
            KeyBind {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::MoveDown,
        );
        tree.insert(
            KeyBind {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::MoveDown,
        );
        tree.insert(
            KeyBind {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::MoveUp,
        );
        tree.insert(
            KeyBind {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::MoveUp,
        );
        tree.insert(
            KeyBind {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::Expand,
        );
        tree.insert(
            KeyBind {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::Collapse,
        );
        tree.insert(
            KeyBind {
                code: KeyCode::Char(' '),
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::ToggleExpand,
        );
        panels.insert(PanelFocus::TreeBrowser, tree);

        // ── Inspector ────────────────────────────────────────────
        let mut inspector = HashMap::new();
        inspector.insert(
            KeyBind {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::Dismiss,
        );
        inspector.insert(
            KeyBind {
                code: KeyCode::Char('y'),
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::CopyContent,
        );
        insert_scroll_nav(&mut inspector);
        panels.insert(PanelFocus::Inspector, inspector);

        // ── Command bar ──────────────────────────────────────────
        let mut command_bar = HashMap::new();
        command_bar.insert(
            KeyBind {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::Submit,
        );
        command_bar.insert(
            KeyBind {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::Dismiss,
        );
        panels.insert(PanelFocus::CommandBar, command_bar);

        Self { global, panels }
    }
}

/// Insert vim-style navigation bindings (arrows + hjkl + page + g/G + Home/End)
fn insert_vim_nav(map: &mut HashMap<KeyBind, KeyAction>) {
    insert_scroll_nav(map);

    map.insert(
        KeyBind {
            code: KeyCode::Right,
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::MoveRight,
    );
    map.insert(
        KeyBind {
            code: KeyCode::Char('l'),
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::MoveRight,
    );
    map.insert(
        KeyBind {
            code: KeyCode::Left,
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::MoveLeft,
    );
    map.insert(
        KeyBind {
            code: KeyCode::Char('h'),
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::MoveLeft,
    );
    map.insert(
        KeyBind {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::Home,
    );
    map.insert(
        KeyBind {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::End,
    );
}

/// Insert vertical navigation bindings (arrows + jk + page + g/G)
fn insert_scroll_nav(map: &mut HashMap<KeyBind, KeyAction>) {
    map.insert(
        KeyBind {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::MoveDown,
    );
    map.insert(
        KeyBind {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::MoveDown,
    );
    map.insert(
        KeyBind {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::MoveUp,
    );
    map.insert(
        KeyBind {
            code: KeyCode::Char('k'),
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::MoveUp,
    );
    map.insert(
        KeyBind {
            code: KeyCode::PageDown,
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::PageDown,
    );
    map.insert(
        KeyBind {
            code: KeyCode::PageUp,
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::PageUp,
    );
    map.insert(
        KeyBind {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::GoToTop,
    );
    map.insert(
        KeyBind {
            code: KeyCode::Char('G'),
            modifiers: KeyModifiers::SHIFT,
        },
        KeyAction::GoToBottom,
    );
    map.insert(
        KeyBind {
            code: KeyCode::Home,
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::GoToTop,
    );
    map.insert(
        KeyBind {
            code: KeyCode::End,
            modifiers: KeyModifiers::NONE,
        },
        KeyAction::GoToBottom,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_quit() {
        let km = KeyMap::default();
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert_eq!(km.resolve(PanelFocus::QueryEditor, key), Some(KeyAction::Quit));
        assert_eq!(km.resolve(PanelFocus::ResultsViewer, key), Some(KeyAction::Quit));
    }

    #[test]
    fn test_global_overrides_panel() {
        let km = KeyMap::default();
        // Tab is global CycleFocus, not panel-specific
        let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(
            km.resolve(PanelFocus::ResultsViewer, key),
            Some(KeyAction::CycleFocus)
        );
    }

    #[test]
    fn test_panel_specific_binding() {
        let km = KeyMap::default();
        // Enter in results = OpenInspector
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            km.resolve(PanelFocus::ResultsViewer, key),
            Some(KeyAction::OpenInspector)
        );
        // Enter in tree = Expand
        assert_eq!(
            km.resolve(PanelFocus::TreeBrowser, key),
            Some(KeyAction::Expand)
        );
    }

    #[test]
    fn test_unbound_key_returns_none() {
        let km = KeyMap::default();
        let key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        assert_eq!(km.resolve(PanelFocus::ResultsViewer, key), None);
    }

    #[test]
    fn test_vim_navigation_in_results() {
        let km = KeyMap::default();
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        let k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(km.resolve(PanelFocus::ResultsViewer, j), Some(KeyAction::MoveDown));
        assert_eq!(km.resolve(PanelFocus::ResultsViewer, k), Some(KeyAction::MoveUp));
    }

    #[test]
    fn test_editor_has_no_navigation_bindings() {
        let km = KeyMap::default();
        // j in editor should return None (editor handles text input directly)
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(km.resolve(PanelFocus::QueryEditor, j), None);
    }

    #[test]
    fn test_command_bar_submit_dismiss() {
        let km = KeyMap::default();
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(km.resolve(PanelFocus::CommandBar, enter), Some(KeyAction::Submit));
        assert_eq!(km.resolve(PanelFocus::CommandBar, esc), Some(KeyAction::Dismiss));
    }
}

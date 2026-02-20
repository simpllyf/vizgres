//! Data-driven keybinding configuration
//!
//! All keybindings are defined as data in `KeyMap::default()`, not as match arms
//! scattered across components. To add a new binding, add an entry to the
//! appropriate context in `KeyMap::default()` and handle the `KeyAction` in
//! `App::execute_key_action()`.
//!
//! User overrides from `~/.vizgres/config.toml` are merged on top via
//! `KeyMap::from_config()`.

use crate::app::PanelFocus;
use crate::config::settings::KeybindingsConfig;
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
    ExplainQuery,
    ClearEditor,
    HistoryBack,
    HistoryForward,
    Undo,
    Redo,
    FormatQuery,

    // Query cancellation (works from editor, results, tree)
    CancelQuery,

    // Results-specific
    OpenInspector,
    CopyCell,
    CopyRow,
    ExportCsv,
    ExportJson,

    // Inspector-specific
    CopyContent,

    // Tree-specific
    ToggleExpand,
    Expand,
    Collapse,

    // Completion
    NextCompletion,
    PrevCompletion,

    // Help
    ShowHelp,

    // Tabs
    NewTab,
    CloseTab,
    NextTab,

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
        self.panels.get(&focus).and_then(|m| m.get(&bind)).copied()
    }

    /// Build a KeyMap from defaults plus user overrides.
    /// Returns the keymap and a list of warning messages for invalid entries.
    pub fn from_config(config: &KeybindingsConfig) -> (Self, Vec<String>) {
        let mut km = Self::default();
        let mut warnings = Vec::new();

        apply_overrides(&mut km.global, &config.global, "global", &mut warnings);

        let panel_configs: &[(PanelFocus, &HashMap<String, String>)] = &[
            (PanelFocus::QueryEditor, &config.editor),
            (PanelFocus::ResultsViewer, &config.results),
            (PanelFocus::TreeBrowser, &config.tree),
        ];

        for &(focus, bindings) in panel_configs {
            let panel_map = km.panels.entry(focus).or_default();
            let label = match focus {
                PanelFocus::QueryEditor => "editor",
                PanelFocus::ResultsViewer => "results",
                PanelFocus::TreeBrowser => "tree",
                _ => "unknown",
            };
            apply_overrides(panel_map, bindings, label, &mut warnings);
        }

        (km, warnings)
    }

    /// Reverse lookup: find all keys bound to a given action.
    /// Searches global bindings plus the panel-specific map for the given focus.
    /// Returns formatted key strings joined for display.
    pub fn keys_for_action(&self, focus: Option<PanelFocus>, action: KeyAction) -> Vec<String> {
        let mut keys = Vec::new();

        // Search global bindings
        for (bind, act) in &self.global {
            if *act == action {
                keys.push(format_keybind(bind));
            }
        }

        // Search panel-specific bindings
        if let Some(focus) = focus
            && let Some(panel_map) = self.panels.get(&focus)
        {
            for (bind, act) in panel_map {
                if *act == action {
                    keys.push(format_keybind(bind));
                }
            }
        }

        // Sort for deterministic output
        keys.sort();
        keys
    }
}

/// Apply user overrides from a HashMap<String, String> into a binding map.
fn apply_overrides(
    map: &mut HashMap<KeyBind, KeyAction>,
    overrides: &HashMap<String, String>,
    context: &str,
    warnings: &mut Vec<String>,
) {
    for (key_str, action_str) in overrides {
        let bind = match parse_keybind(key_str) {
            Ok(b) => b,
            Err(e) => {
                warnings.push(format!(
                    "[keybindings.{}] invalid key \"{}\": {}",
                    context, key_str, e
                ));
                continue;
            }
        };
        let action = match parse_key_action(action_str) {
            Ok(a) => a,
            Err(e) => {
                warnings.push(format!(
                    "[keybindings.{}] invalid action \"{}\" for key \"{}\": {}",
                    context, action_str, key_str, e
                ));
                continue;
            }
        };
        map.insert(bind, action);
    }
}

/// Parse a key string like "ctrl+shift+z" into a KeyBind.
///
/// Format: modifier parts joined with `+`, key name last.
/// Modifiers: `ctrl`, `alt`, `shift` (case-insensitive).
/// Key names: `enter`, `esc`, `space`, `tab`, `backtab`, `up`, `down`, `left`, `right`,
/// `home`, `end`, `pageup`, `pagedown`, `f1`..`f12`, or a single character.
pub fn parse_keybind(s: &str) -> Result<KeyBind, String> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        return Err("empty key string".to_string());
    }

    let parts: Vec<&str> = s.split('+').collect();
    if parts.is_empty() {
        return Err("empty key string".to_string());
    }

    let mut modifiers = KeyModifiers::NONE;
    let key_part = parts.last().unwrap();

    // All parts except the last are modifiers
    for &part in &parts[..parts.len() - 1] {
        match part.trim() {
            "ctrl" => modifiers |= KeyModifiers::CONTROL,
            "alt" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            other => return Err(format!("unknown modifier: {}", other)),
        }
    }

    let code = parse_key_code(key_part.trim())?;

    // If shift is specified with a letter, uppercase it
    if modifiers.contains(KeyModifiers::SHIFT)
        && let KeyCode::Char(c) = code
    {
        return Ok(KeyBind {
            code: KeyCode::Char(c.to_ascii_uppercase()),
            modifiers,
        });
    }

    Ok(KeyBind { code, modifiers })
}

/// Parse a key name string into a KeyCode
fn parse_key_code(s: &str) -> Result<KeyCode, String> {
    match s {
        "enter" => Ok(KeyCode::Enter),
        "esc" | "escape" => Ok(KeyCode::Esc),
        "space" => Ok(KeyCode::Char(' ')),
        "tab" => Ok(KeyCode::Tab),
        "backtab" => Ok(KeyCode::BackTab),
        "backspace" => Ok(KeyCode::Backspace),
        "delete" | "del" => Ok(KeyCode::Delete),
        "up" => Ok(KeyCode::Up),
        "down" => Ok(KeyCode::Down),
        "left" => Ok(KeyCode::Left),
        "right" => Ok(KeyCode::Right),
        "home" => Ok(KeyCode::Home),
        "end" => Ok(KeyCode::End),
        "pageup" | "pgup" => Ok(KeyCode::PageUp),
        "pagedown" | "pgdn" | "pgdown" => Ok(KeyCode::PageDown),
        _ if s.starts_with('f') && s.len() >= 2 => {
            let num: u8 = s[1..]
                .parse()
                .map_err(|_| format!("invalid function key: {}", s))?;
            if (1..=12).contains(&num) {
                Ok(KeyCode::F(num))
            } else {
                Err(format!("function key out of range: {}", s))
            }
        }
        _ if s.chars().count() == 1 => Ok(KeyCode::Char(s.chars().next().unwrap())),
        _ => Err(format!("unknown key: {}", s)),
    }
}

/// Parse a snake_case action string into a KeyAction.
pub fn parse_key_action(s: &str) -> Result<KeyAction, String> {
    match s.trim() {
        "quit" => Ok(KeyAction::Quit),
        "command_bar" | "open_command_bar" => Ok(KeyAction::OpenCommandBar),
        "cycle_focus" => Ok(KeyAction::CycleFocus),
        "cycle_focus_reverse" => Ok(KeyAction::CycleFocusReverse),
        "move_up" => Ok(KeyAction::MoveUp),
        "move_down" => Ok(KeyAction::MoveDown),
        "move_left" => Ok(KeyAction::MoveLeft),
        "move_right" => Ok(KeyAction::MoveRight),
        "page_up" => Ok(KeyAction::PageUp),
        "page_down" => Ok(KeyAction::PageDown),
        "go_to_top" => Ok(KeyAction::GoToTop),
        "go_to_bottom" => Ok(KeyAction::GoToBottom),
        "home" => Ok(KeyAction::Home),
        "end" => Ok(KeyAction::End),
        "execute_query" => Ok(KeyAction::ExecuteQuery),
        "explain_query" => Ok(KeyAction::ExplainQuery),
        "clear_editor" => Ok(KeyAction::ClearEditor),
        "history_back" => Ok(KeyAction::HistoryBack),
        "history_forward" => Ok(KeyAction::HistoryForward),
        "undo" => Ok(KeyAction::Undo),
        "redo" => Ok(KeyAction::Redo),
        "format_query" => Ok(KeyAction::FormatQuery),
        "cancel_query" => Ok(KeyAction::CancelQuery),
        "open_inspector" => Ok(KeyAction::OpenInspector),
        "copy_cell" => Ok(KeyAction::CopyCell),
        "copy_row" => Ok(KeyAction::CopyRow),
        "export_csv" => Ok(KeyAction::ExportCsv),
        "export_json" => Ok(KeyAction::ExportJson),
        "copy_content" => Ok(KeyAction::CopyContent),
        "toggle_expand" => Ok(KeyAction::ToggleExpand),
        "expand" => Ok(KeyAction::Expand),
        "collapse" => Ok(KeyAction::Collapse),
        "next_completion" => Ok(KeyAction::NextCompletion),
        "prev_completion" => Ok(KeyAction::PrevCompletion),
        "show_help" => Ok(KeyAction::ShowHelp),
        "new_tab" => Ok(KeyAction::NewTab),
        "close_tab" => Ok(KeyAction::CloseTab),
        "next_tab" => Ok(KeyAction::NextTab),
        "dismiss" => Ok(KeyAction::Dismiss),
        "submit" => Ok(KeyAction::Submit),
        other => Err(format!("unknown action: {}", other)),
    }
}

/// Format a KeyBind as a human-readable string like "Ctrl+Shift+Z"
pub fn format_keybind(bind: &KeyBind) -> String {
    let mut parts = Vec::new();

    if bind.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("Ctrl".to_string());
    }
    if bind.modifiers.contains(KeyModifiers::ALT) {
        parts.push("Alt".to_string());
    }
    if bind.modifiers.contains(KeyModifiers::SHIFT) {
        // Don't show Shift for uppercase letters — they imply it
        if !matches!(bind.code, KeyCode::Char(c) if c.is_ascii_uppercase()) {
            parts.push("Shift".to_string());
        }
    }

    let key_name = match bind.code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "Shift+Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PgUp".to_string(),
        KeyCode::PageDown => "PgDn".to_string(),
        KeyCode::F(n) => format!("F{}", n),
        _ => "?".to_string(),
    };

    parts.push(key_name);
    parts.join("+")
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
        global.insert(
            KeyBind {
                code: KeyCode::F(1),
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::ShowHelp,
        );
        global.insert(
            KeyBind {
                code: KeyCode::Char('t'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::NewTab,
        );
        global.insert(
            KeyBind {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::CloseTab,
        );
        global.insert(
            KeyBind {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::NextTab,
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
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::ExplainQuery,
        );
        editor.insert(
            KeyBind {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::ClearEditor,
        );
        editor.insert(
            KeyBind {
                code: KeyCode::Up,
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::HistoryBack,
        );
        editor.insert(
            KeyBind {
                code: KeyCode::Down,
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::HistoryForward,
        );
        editor.insert(
            KeyBind {
                code: KeyCode::Char('z'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::Undo,
        );
        editor.insert(
            KeyBind {
                code: KeyCode::Char('Z'),
                modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            },
            KeyAction::Redo,
        );
        editor.insert(
            KeyBind {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::CONTROL | KeyModifiers::ALT,
            },
            KeyAction::FormatQuery,
        );
        editor.insert(
            KeyBind {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::CancelQuery,
        );
        editor.insert(
            KeyBind {
                code: KeyCode::Down,
                modifiers: KeyModifiers::ALT,
            },
            KeyAction::NextCompletion,
        );
        editor.insert(
            KeyBind {
                code: KeyCode::Up,
                modifiers: KeyModifiers::ALT,
            },
            KeyAction::PrevCompletion,
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
        results.insert(
            KeyBind {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::ExportCsv,
        );
        results.insert(
            KeyBind {
                code: KeyCode::Char('j'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyAction::ExportJson,
        );
        results.insert(
            KeyBind {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::CancelQuery,
        );
        results.insert(
            KeyBind {
                code: KeyCode::Char('?'),
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::ShowHelp,
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
        tree.insert(
            KeyBind {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::CancelQuery,
        );
        tree.insert(
            KeyBind {
                code: KeyCode::Char('?'),
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::ShowHelp,
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

        // ── Help overlay ─────────────────────────────────────────
        let mut help = HashMap::new();
        help.insert(
            KeyBind {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
            },
            KeyAction::Dismiss,
        );
        insert_scroll_nav(&mut help);
        panels.insert(PanelFocus::Help, help);

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

/// Insert vertical navigation bindings (arrows + jk + page + g/G).
/// Home/End map to GoToTop/GoToBottom here (vertical-only contexts like inspector).
/// `insert_vim_nav` overwrites these with Home/End for horizontal contexts.
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
    fn test_cancel_query_binding() {
        let km = KeyMap::default();
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        // Escape should resolve to CancelQuery in editor, results, tree
        assert_eq!(
            km.resolve(PanelFocus::QueryEditor, esc),
            Some(KeyAction::CancelQuery)
        );
        assert_eq!(
            km.resolve(PanelFocus::ResultsViewer, esc),
            Some(KeyAction::CancelQuery)
        );
        assert_eq!(
            km.resolve(PanelFocus::TreeBrowser, esc),
            Some(KeyAction::CancelQuery)
        );
        // In modals, Escape is still Dismiss
        assert_eq!(
            km.resolve(PanelFocus::Inspector, esc),
            Some(KeyAction::Dismiss)
        );
        assert_eq!(
            km.resolve(PanelFocus::CommandBar, esc),
            Some(KeyAction::Dismiss)
        );
    }

    #[test]
    fn test_global_quit() {
        let km = KeyMap::default();
        let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert_eq!(
            km.resolve(PanelFocus::QueryEditor, key),
            Some(KeyAction::Quit)
        );
        assert_eq!(
            km.resolve(PanelFocus::ResultsViewer, key),
            Some(KeyAction::Quit)
        );
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
        assert_eq!(
            km.resolve(PanelFocus::ResultsViewer, j),
            Some(KeyAction::MoveDown)
        );
        assert_eq!(
            km.resolve(PanelFocus::ResultsViewer, k),
            Some(KeyAction::MoveUp)
        );
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
        assert_eq!(
            km.resolve(PanelFocus::CommandBar, enter),
            Some(KeyAction::Submit)
        );
        assert_eq!(
            km.resolve(PanelFocus::CommandBar, esc),
            Some(KeyAction::Dismiss)
        );
    }

    #[test]
    fn test_tree_specific_bindings() {
        let km = KeyMap::default();
        let space = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        let h = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            km.resolve(PanelFocus::TreeBrowser, space),
            Some(KeyAction::ToggleExpand)
        );
        assert_eq!(
            km.resolve(PanelFocus::TreeBrowser, h),
            Some(KeyAction::Collapse)
        );
        assert_eq!(
            km.resolve(PanelFocus::TreeBrowser, enter),
            Some(KeyAction::Expand)
        );
    }

    #[test]
    fn test_inspector_bindings() {
        let km = KeyMap::default();
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let y = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(PanelFocus::Inspector, esc),
            Some(KeyAction::Dismiss)
        );
        assert_eq!(
            km.resolve(PanelFocus::Inspector, y),
            Some(KeyAction::CopyContent)
        );
        assert_eq!(
            km.resolve(PanelFocus::Inspector, j),
            Some(KeyAction::MoveDown)
        );
    }

    #[test]
    fn test_results_home_end_are_horizontal() {
        let km = KeyMap::default();
        let home = KeyEvent::new(KeyCode::Home, KeyModifiers::NONE);
        let end = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
        // In results viewer, Home/End navigate columns (horizontal)
        assert_eq!(
            km.resolve(PanelFocus::ResultsViewer, home),
            Some(KeyAction::Home)
        );
        assert_eq!(
            km.resolve(PanelFocus::ResultsViewer, end),
            Some(KeyAction::End)
        );
    }

    #[test]
    fn test_inspector_home_end_are_vertical() {
        let km = KeyMap::default();
        let home = KeyEvent::new(KeyCode::Home, KeyModifiers::NONE);
        let end = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
        // In inspector, Home/End navigate vertically (scroll to top/bottom)
        assert_eq!(
            km.resolve(PanelFocus::Inspector, home),
            Some(KeyAction::GoToTop)
        );
        assert_eq!(
            km.resolve(PanelFocus::Inspector, end),
            Some(KeyAction::GoToBottom)
        );
    }

    #[test]
    fn test_explain_keybinding_resolves() {
        let km = KeyMap::default();
        let ctrl_e = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
        assert_eq!(
            km.resolve(PanelFocus::QueryEditor, ctrl_e),
            Some(KeyAction::ExplainQuery)
        );
        // Should not resolve in other panels
        assert_eq!(km.resolve(PanelFocus::ResultsViewer, ctrl_e), None);
    }

    #[test]
    fn test_history_keybindings_resolve() {
        let km = KeyMap::default();
        let ctrl_up = KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL);
        let ctrl_down = KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL);
        assert_eq!(
            km.resolve(PanelFocus::QueryEditor, ctrl_up),
            Some(KeyAction::HistoryBack)
        );
        assert_eq!(
            km.resolve(PanelFocus::QueryEditor, ctrl_down),
            Some(KeyAction::HistoryForward)
        );
    }

    #[test]
    fn test_help_keybinding_resolves() {
        let km = KeyMap::default();
        let f1 = KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE);
        let question = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);

        // F1 resolves to ShowHelp from every panel (global)
        assert_eq!(
            km.resolve(PanelFocus::QueryEditor, f1),
            Some(KeyAction::ShowHelp)
        );
        assert_eq!(
            km.resolve(PanelFocus::ResultsViewer, f1),
            Some(KeyAction::ShowHelp)
        );
        assert_eq!(
            km.resolve(PanelFocus::TreeBrowser, f1),
            Some(KeyAction::ShowHelp)
        );
        assert_eq!(
            km.resolve(PanelFocus::Inspector, f1),
            Some(KeyAction::ShowHelp)
        );
        assert_eq!(
            km.resolve(PanelFocus::CommandBar, f1),
            Some(KeyAction::ShowHelp)
        );

        // ? resolves in results and tree (non-text-input panels)
        assert_eq!(
            km.resolve(PanelFocus::ResultsViewer, question),
            Some(KeyAction::ShowHelp)
        );
        assert_eq!(
            km.resolve(PanelFocus::TreeBrowser, question),
            Some(KeyAction::ShowHelp)
        );
        // ? should NOT resolve in editor (it's a text character)
        assert_eq!(km.resolve(PanelFocus::QueryEditor, question), None);
    }

    #[test]
    fn test_help_panel_bindings() {
        let km = KeyMap::default();
        let esc = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        let k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        let g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);

        assert_eq!(km.resolve(PanelFocus::Help, esc), Some(KeyAction::Dismiss));
        assert_eq!(km.resolve(PanelFocus::Help, j), Some(KeyAction::MoveDown));
        assert_eq!(km.resolve(PanelFocus::Help, k), Some(KeyAction::MoveUp));
        assert_eq!(km.resolve(PanelFocus::Help, g), Some(KeyAction::GoToTop));
    }

    #[test]
    fn test_format_query_binding() {
        let km = KeyMap::default();
        let ctrl_alt_f = KeyEvent::new(
            KeyCode::Char('f'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        );
        assert_eq!(
            km.resolve(PanelFocus::QueryEditor, ctrl_alt_f),
            Some(KeyAction::FormatQuery)
        );
        // Should not resolve in other panels
        assert_eq!(km.resolve(PanelFocus::ResultsViewer, ctrl_alt_f), None);
    }

    #[test]
    fn test_export_keybindings_resolve() {
        let km = KeyMap::default();
        let ctrl_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL);
        let ctrl_j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL);

        // Ctrl+S → ExportCsv in results only
        assert_eq!(
            km.resolve(PanelFocus::ResultsViewer, ctrl_s),
            Some(KeyAction::ExportCsv)
        );
        assert_eq!(km.resolve(PanelFocus::QueryEditor, ctrl_s), None);
        assert_eq!(km.resolve(PanelFocus::TreeBrowser, ctrl_s), None);

        // Ctrl+J → ExportJson in results only
        assert_eq!(
            km.resolve(PanelFocus::ResultsViewer, ctrl_j),
            Some(KeyAction::ExportJson)
        );
        assert_eq!(km.resolve(PanelFocus::QueryEditor, ctrl_j), None);
        assert_eq!(km.resolve(PanelFocus::TreeBrowser, ctrl_j), None);
    }

    #[test]
    fn test_history_keybindings_only_in_editor() {
        let km = KeyMap::default();
        let ctrl_up = KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL);
        let ctrl_down = KeyEvent::new(KeyCode::Down, KeyModifiers::CONTROL);
        // Should not resolve in other panels
        assert_eq!(km.resolve(PanelFocus::ResultsViewer, ctrl_up), None);
        assert_eq!(km.resolve(PanelFocus::ResultsViewer, ctrl_down), None);
        assert_eq!(km.resolve(PanelFocus::TreeBrowser, ctrl_up), None);
        assert_eq!(km.resolve(PanelFocus::TreeBrowser, ctrl_down), None);
    }

    #[test]
    fn test_tab_keybindings_resolve() {
        let km = KeyMap::default();
        let ctrl_t = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
        let ctrl_w = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        let ctrl_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL);

        // All three should resolve as global bindings from any non-modal panel
        for panel in [
            PanelFocus::QueryEditor,
            PanelFocus::ResultsViewer,
            PanelFocus::TreeBrowser,
        ] {
            assert_eq!(km.resolve(panel, ctrl_t), Some(KeyAction::NewTab));
            assert_eq!(km.resolve(panel, ctrl_w), Some(KeyAction::CloseTab));
            assert_eq!(km.resolve(panel, ctrl_n), Some(KeyAction::NextTab));
        }
    }

    // ── Key string parsing tests ──────────────────────────────

    #[test]
    fn test_parse_keybind_ctrl_q() {
        let bind = parse_keybind("ctrl+q").unwrap();
        assert_eq!(bind.code, KeyCode::Char('q'));
        assert_eq!(bind.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_keybind_ctrl_shift_z() {
        let bind = parse_keybind("ctrl+shift+z").unwrap();
        assert_eq!(bind.code, KeyCode::Char('Z'));
        assert!(bind.modifiers.contains(KeyModifiers::CONTROL));
        assert!(bind.modifiers.contains(KeyModifiers::SHIFT));
    }

    #[test]
    fn test_parse_keybind_bare_char() {
        let bind = parse_keybind("j").unwrap();
        assert_eq!(bind.code, KeyCode::Char('j'));
        assert_eq!(bind.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_keybind_f5() {
        let bind = parse_keybind("f5").unwrap();
        assert_eq!(bind.code, KeyCode::F(5));
        assert_eq!(bind.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_keybind_special_keys() {
        assert_eq!(parse_keybind("enter").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_keybind("esc").unwrap().code, KeyCode::Esc);
        assert_eq!(parse_keybind("space").unwrap().code, KeyCode::Char(' '));
        assert_eq!(parse_keybind("tab").unwrap().code, KeyCode::Tab);
        assert_eq!(parse_keybind("backtab").unwrap().code, KeyCode::BackTab);
        assert_eq!(parse_keybind("up").unwrap().code, KeyCode::Up);
        assert_eq!(parse_keybind("down").unwrap().code, KeyCode::Down);
        assert_eq!(parse_keybind("left").unwrap().code, KeyCode::Left);
        assert_eq!(parse_keybind("right").unwrap().code, KeyCode::Right);
        assert_eq!(parse_keybind("home").unwrap().code, KeyCode::Home);
        assert_eq!(parse_keybind("end").unwrap().code, KeyCode::End);
        assert_eq!(parse_keybind("pageup").unwrap().code, KeyCode::PageUp);
        assert_eq!(parse_keybind("pagedown").unwrap().code, KeyCode::PageDown);
    }

    #[test]
    fn test_parse_keybind_ctrl_enter() {
        let bind = parse_keybind("ctrl+enter").unwrap();
        assert_eq!(bind.code, KeyCode::Enter);
        assert_eq!(bind.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_keybind_ctrl_alt_f() {
        let bind = parse_keybind("ctrl+alt+f").unwrap();
        assert_eq!(bind.code, KeyCode::Char('f'));
        assert!(bind.modifiers.contains(KeyModifiers::CONTROL));
        assert!(bind.modifiers.contains(KeyModifiers::ALT));
    }

    #[test]
    fn test_parse_keybind_case_insensitive() {
        let bind = parse_keybind("Ctrl+Q").unwrap();
        assert_eq!(bind.code, KeyCode::Char('q'));
        assert_eq!(bind.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_keybind_invalid() {
        assert!(parse_keybind("").is_err());
        assert!(parse_keybind("magic+q").is_err());
        assert!(parse_keybind("f99").is_err());
    }

    // ── Action string parsing tests ──────────────────────────

    #[test]
    fn test_parse_key_action_all_variants() {
        assert_eq!(parse_key_action("quit").unwrap(), KeyAction::Quit);
        assert_eq!(
            parse_key_action("execute_query").unwrap(),
            KeyAction::ExecuteQuery
        );
        assert_eq!(
            parse_key_action("explain_query").unwrap(),
            KeyAction::ExplainQuery
        );
        assert_eq!(parse_key_action("copy_cell").unwrap(), KeyAction::CopyCell);
        assert_eq!(parse_key_action("show_help").unwrap(), KeyAction::ShowHelp);
        assert_eq!(parse_key_action("new_tab").unwrap(), KeyAction::NewTab);
        assert_eq!(parse_key_action("dismiss").unwrap(), KeyAction::Dismiss);
    }

    #[test]
    fn test_parse_key_action_aliases() {
        assert_eq!(
            parse_key_action("command_bar").unwrap(),
            KeyAction::OpenCommandBar
        );
        assert_eq!(
            parse_key_action("open_command_bar").unwrap(),
            KeyAction::OpenCommandBar
        );
    }

    #[test]
    fn test_parse_key_action_invalid() {
        assert!(parse_key_action("nonexistent").is_err());
    }

    // ── Format keybind tests ──────────────────────────────────

    #[test]
    fn test_format_keybind_ctrl_q() {
        let bind = KeyBind {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::CONTROL,
        };
        assert_eq!(format_keybind(&bind), "Ctrl+q");
    }

    #[test]
    fn test_format_keybind_f5() {
        let bind = KeyBind {
            code: KeyCode::F(5),
            modifiers: KeyModifiers::NONE,
        };
        assert_eq!(format_keybind(&bind), "F5");
    }

    #[test]
    fn test_format_keybind_ctrl_shift_z() {
        let bind = KeyBind {
            code: KeyCode::Char('Z'),
            modifiers: KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        };
        assert_eq!(format_keybind(&bind), "Ctrl+Z");
    }

    // ── from_config tests ─────────────────────────────────────

    #[test]
    fn test_from_config_overrides_default() {
        let mut config = KeybindingsConfig::default();
        config
            .editor
            .insert("f6".to_string(), "execute_query".to_string());

        let (km, warnings) = KeyMap::from_config(&config);
        assert!(warnings.is_empty(), "warnings: {:?}", warnings);

        let f6 = KeyEvent::new(KeyCode::F(6), KeyModifiers::NONE);
        assert_eq!(
            km.resolve(PanelFocus::QueryEditor, f6),
            Some(KeyAction::ExecuteQuery)
        );
    }

    #[test]
    fn test_from_config_invalid_key_warns() {
        let mut config = KeybindingsConfig::default();
        config
            .global
            .insert("magic+q".to_string(), "quit".to_string());

        let (_, warnings) = KeyMap::from_config(&config);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("invalid key"));
    }

    #[test]
    fn test_from_config_invalid_action_warns() {
        let mut config = KeybindingsConfig::default();
        config
            .editor
            .insert("f6".to_string(), "nonexistent".to_string());

        let (_, warnings) = KeyMap::from_config(&config);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("invalid action"));
    }

    #[test]
    fn test_from_config_empty_preserves_defaults() {
        let config = KeybindingsConfig::default();
        let (km, warnings) = KeyMap::from_config(&config);
        assert!(warnings.is_empty());

        let ctrl_q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        assert_eq!(
            km.resolve(PanelFocus::QueryEditor, ctrl_q),
            Some(KeyAction::Quit)
        );
    }

    // ── keys_for_action tests ────────────────────────────────

    #[test]
    fn test_keys_for_action_finds_global() {
        let km = KeyMap::default();
        let keys = km.keys_for_action(None, KeyAction::Quit);
        assert!(!keys.is_empty());
        assert!(keys.iter().any(|k| k.contains("Ctrl") && k.contains("q")));
    }

    #[test]
    fn test_keys_for_action_finds_panel_specific() {
        let km = KeyMap::default();
        let keys = km.keys_for_action(Some(PanelFocus::QueryEditor), KeyAction::ExecuteQuery);
        assert!(keys.len() >= 2); // F5 and Ctrl+Enter
    }

    #[test]
    fn test_keys_for_action_empty_for_unbound() {
        let km = KeyMap::default();
        let keys = km.keys_for_action(Some(PanelFocus::QueryEditor), KeyAction::CopyCell);
        assert!(keys.is_empty());
    }
}

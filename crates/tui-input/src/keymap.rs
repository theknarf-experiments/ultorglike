//! Keymap management with mode support.
//! Combines mode-based mappings (canvas-tui style) with string commands (mbp-info style).

use std::collections::HashMap;

use crate::{parse_key_sequence, KeyToken};

/// Predefined mappable commands that can be bound to keys.
/// These are the "built-in" commands that are always available.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MappableCommand {
    // Navigation
    CursorDown,
    CursorUp,
    JumpToFirstRow,
    JumpToLastRow,
    PageDown,
    PageUp,

    // Panel navigation
    PanelNavigateLeft,
    PanelNavigateRight,
    PanelNavigateUp,
    PanelNavigateDown,

    // Viewport
    PanLeft,
    PanDown,
    PanUp,
    PanRight,
    ZoomIn,
    ZoomOut,
    CenterViewport,
    JumpTop,
    JumpBottom,
    JumpLeft,
    JumpRight,

    // Mode switching
    EnterNormalMode,
    EnterVisualMode,
    EnterInsertMode,
    EnterCommandMode,

    // File operations
    Quit,
    Help,

    // Custom command (allows binding to arbitrary string commands)
    Custom(String),
}

impl MappableCommand {
    pub fn name(&self) -> &str {
        match self {
            MappableCommand::CursorDown => "cursor_down",
            MappableCommand::CursorUp => "cursor_up",
            MappableCommand::JumpToFirstRow => "jump_to_first_row",
            MappableCommand::JumpToLastRow => "jump_to_last_row",
            MappableCommand::PageDown => "page_down",
            MappableCommand::PageUp => "page_up",
            MappableCommand::PanelNavigateLeft => "panel_navigate_left",
            MappableCommand::PanelNavigateRight => "panel_navigate_right",
            MappableCommand::PanelNavigateUp => "panel_navigate_up",
            MappableCommand::PanelNavigateDown => "panel_navigate_down",
            MappableCommand::PanLeft => "pan_left",
            MappableCommand::PanDown => "pan_down",
            MappableCommand::PanUp => "pan_up",
            MappableCommand::PanRight => "pan_right",
            MappableCommand::ZoomIn => "zoom_in",
            MappableCommand::ZoomOut => "zoom_out",
            MappableCommand::CenterViewport => "center_viewport",
            MappableCommand::JumpTop => "jump_top",
            MappableCommand::JumpBottom => "jump_bottom",
            MappableCommand::JumpLeft => "jump_left",
            MappableCommand::JumpRight => "jump_right",
            MappableCommand::EnterNormalMode => "enter_normal_mode",
            MappableCommand::EnterVisualMode => "enter_visual_mode",
            MappableCommand::EnterInsertMode => "enter_insert_mode",
            MappableCommand::EnterCommandMode => "enter_command_mode",
            MappableCommand::Quit => "quit",
            MappableCommand::Help => "help",
            MappableCommand::Custom(s) => s,
        }
    }
}

/// Mode-based key mapping.
/// Maps (Mode, KeySequence) -> MappableCommand.
#[derive(Clone, Debug)]
pub struct KeyMap {
    /// Mode -> (KeySequence -> Command)
    mappings: HashMap<Mode, HashMap<Vec<KeyToken>, MappableCommand>>,
}

/// Modes for key mappings (canvas-tui style).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    #[default]
    Normal,
    Visual,
    Insert,
    Command(String), // The command being typed
    Help,
    Other(String), // Custom modes
}

impl Mode {
    pub fn name(&self) -> &str {
        match self {
            Mode::Normal => "normal",
            Mode::Visual => "visual",
            Mode::Insert => "insert",
            Mode::Command(cmd) => cmd,
            Mode::Help => "help",
            Mode::Other(name) => name,
        }
    }
}

impl KeyMap {
    /// Create a new KeyMap with default vim-like keybindings.
    pub fn new() -> Self {
        let mut km = Self {
            mappings: HashMap::new(),
        };
        km.load_defaults();
        km
    }

    /// Load default keybindings (vim-inspired).
    fn load_defaults(&mut self) {
        let normal = Mode::Normal;

        // Navigation - vim h/j/k/l style
        self.bind_tokens(&normal, vec![KeyToken::Char('h')], MappableCommand::PanLeft);
        self.bind_tokens(&normal, vec![KeyToken::Char('j')], MappableCommand::PanDown);
        self.bind_tokens(&normal, vec![KeyToken::Char('k')], MappableCommand::PanUp);
        self.bind_tokens(
            &normal,
            vec![KeyToken::Char('l')],
            MappableCommand::PanRight,
        );

        // Arrow keys as alternative
        self.bind_tokens(&normal, vec![KeyToken::Left], MappableCommand::PanLeft);
        self.bind_tokens(&normal, vec![KeyToken::Down], MappableCommand::PanDown);
        self.bind_tokens(&normal, vec![KeyToken::Up], MappableCommand::PanUp);
        self.bind_tokens(&normal, vec![KeyToken::Right], MappableCommand::PanRight);

        // Jump to first/last line - vim gg/G style
        self.bind_tokens(
            &normal,
            vec![KeyToken::Char('g'), KeyToken::Char('g')],
            MappableCommand::JumpToFirstRow,
        );
        self.bind_tokens(
            &normal,
            vec![KeyToken::Char('G')],
            MappableCommand::JumpToLastRow,
        );

        for (sequence, action) in [
            (":", "enter_command_mode"),
            ("q", "quit"),
            ("<C-c>", "quit"),
            ("?", "help"),
            ("H", "help"),
            ("<Esc>", "escape"),
            ("<C-d>", "page_down"),
            ("<C-u>", "page_up"),
            ("<Enter>", "expand"),
            ("zo", "expand"),
            ("zc", "collapse"),
            ("za", "toggle_relation"),
            ("/", "search"),
            ("n", "search_next"),
            ("N", "search_previous"),
            ("F", "filter_prompt"),
            ("=", "formula_prompt"),
            ("K", "inspect"),
            ("]w", "wider"),
            ("[w", "narrower"),
            ("f", "fields"),
            ("t", "tables"),
            ("s", "sort"),
            ("S", "sort_desc"),
            ("x", "hide"),
            ("u", "undo"),
            ("<C-r>", "redo"),
            ("]p", "nextpage"),
            ("[p", "prevpage"),
            ("+", "more"),
            ("gt", "next_view"),
            ("gT", "previous_view"),
            ("0", "first_column"),
            ("$", "last_column"),
        ] {
            self.bind(&normal, sequence, MappableCommand::Custom(action.into()))
                .expect("valid default binding");
        }
    }

    /// Bind a single character key in a mode (convenience wrapper).
    pub fn bind_simple(&mut self, mode: &Mode, ch: char, command: MappableCommand) {
        self.bind_tokens(mode, vec![KeyToken::Char(ch)], command);
    }

    /// Bind a control key in a mode (convenience wrapper).
    pub fn bind_ctrl(&mut self, mode: &Mode, ch: char, command: MappableCommand) {
        self.bind_tokens(mode, vec![KeyToken::Ctrl(ch)], command);
    }

    /// Bind an arrow key in a mode (convenience wrapper).
    pub fn bind_arrow(&mut self, mode: &Mode, arrow: KeyToken, command: MappableCommand) {
        self.bind_tokens(mode, vec![arrow], command);
    }

    /// Bind a key sequence to a command.
    pub fn bind_tokens(&mut self, mode: &Mode, tokens: Vec<KeyToken>, command: MappableCommand) {
        self.mappings
            .entry(mode.clone())
            .or_default()
            .insert(tokens, command);
    }

    /// Bind a string command (custom handler).
    pub fn bind(
        &mut self,
        mode: &Mode,
        sequence: &str,
        command: MappableCommand,
    ) -> Result<(), String> {
        let tokens = parse_key_sequence(sequence)?;
        self.bind_tokens(mode, tokens, command);
        Ok(())
    }

    /// Remove a binding.
    pub fn unbind(&mut self, mode: &Mode, sequence: &str) -> Result<(), String> {
        let tokens = parse_key_sequence(sequence)?;
        if let Some(mode_map) = self.mappings.get_mut(mode) {
            if mode_map.remove(&tokens).is_none() {
                return Err(format!(
                    "No mapping for '{}' in mode '{}'",
                    sequence,
                    mode.name()
                ));
            }
        }
        Ok(())
    }

    /// Look up a command for a key sequence in a mode.
    /// Returns the first (shortest) matching command.
    pub fn get(&self, mode: &Mode, tokens: &[KeyToken]) -> Option<&MappableCommand> {
        let mode_map = self.mappings.get(mode)?;
        mode_map.get(tokens)
    }

    /// Get all mappings for a mode.
    pub fn mappings_for_mode(&self, mode: &Mode) -> Vec<(String, MappableCommand)> {
        let Some(mode_map) = self.mappings.get(mode) else {
            return vec![];
        };
        let mut entries: Vec<_> = mode_map
            .iter()
            .map(|(tokens, cmd)| (format_tokens(tokens), cmd.clone()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }

    /// Get all modes that have mappings.
    pub fn modes(&self) -> Vec<&str> {
        self.mappings.keys().map(|m| m.name()).collect()
    }

    /// Check if a mode has any mappings.
    pub fn has_mappings(&self, mode: &Mode) -> bool {
        self.mappings.contains_key(mode)
    }

    /// Clear all mappings for a mode.
    pub fn clear_mode(&mut self, mode: &Mode) {
        self.mappings.remove(mode);
    }

    /// Reset to defaults.
    pub fn reset(&mut self) {
        self.mappings.clear();
        self.load_defaults();
    }
}

fn format_tokens(tokens: &[KeyToken]) -> String {
    tokens.iter().map(|t| t.to_string()).collect()
}

impl Default for KeyMap {
    fn default() -> Self {
        Self::new()
    }
}

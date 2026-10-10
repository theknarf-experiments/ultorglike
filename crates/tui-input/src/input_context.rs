//! The main input context that ties everything together.
//! Manages mode, leader key, pending keys, and command resolution.

use std::collections::HashMap;

use crossterm::event::{Event, KeyEvent, KeyEventKind};

use crate::{
    KeyMap, KeyToken, Mode, parse_key_sequence,
    key_event_to_token,
};

/// The input context manages mode, leader key, pending keys,
/// and resolves key events to commands.
pub struct InputContext {
    keymap: KeyMap,
    mode: Mode,
    /// The leader key character (e.g., ',' or '\\'). None disables leader.
    leader: Option<char>,
    /// Pending key tokens waiting for completion.
    pending: Vec<KeyToken>,
    /// Custom command handlers registered by the application.
    /// Maps command name to a handler function.
    handlers: HashMap<String, Box<dyn FnOnce(&mut InputState) -> bool + Send>>,
    /// Input buffer for command mode.
    command_buffer: String,
    /// Command history for up/down navigation.
    command_history: Vec<String>,
    history_index: Option<usize>,
}

/// State that command handlers can modify.
pub struct InputState {
    pub should_quit: bool,
    pub message: Option<String>,
    pub error: Option<String>,
}

impl Default for InputContext {
    fn default() -> Self {
        Self::new()
    }
}

impl InputContext {
    /// Create a new input context with default keybindings.
    pub fn new() -> Self {
        Self {
            keymap: KeyMap::new(),
            mode: Mode::Normal,
            leader: Some(','),  // Default leader key like mbp-info
            pending: Vec::new(),
            handlers: HashMap::new(),
            command_buffer: String::new(),
            command_history: Vec::new(),
            history_index: None,
        }
    }

    /// Set the leader key. Pass `None` to disable.
    pub fn set_leader(&mut self, leader: Option<char>) {
        self.leader = leader;
        self.pending.clear();
    }

    /// Get the current mode.
    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    /// Switch to a new mode.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.pending.clear();
        self.command_buffer.clear();
        self.history_index = None;
    }

    /// Get the current mode as a mutable reference.
    pub fn mode_mut(&mut self) -> &mut Mode {
        &mut self.mode
    }

    /// Get the current mode name.
    pub fn mode_name(&self) -> &str {
        self.mode.name()
    }

    /// Register a handler for a command.
    /// The handler returns true if the app should quit.
    pub fn register_handler<F>(&mut self, command: &str, handler: F)
    where
        F: FnOnce(&mut InputState) -> bool + Send + 'static,
    {
        self.handlers.insert(command.to_string(), Box::new(handler));
    }

    /// Get command handler names for completion.
    pub fn command_names(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    /// Enter command mode.
    pub fn enter_command_mode(&mut self) {
        self.command_buffer.clear();
        self.history_index = None;
        self.pending.clear();
        self.mode = Mode::Command(String::new());
    }

    /// Exit command mode.
    pub fn exit_command_mode(&mut self) {
        self.command_buffer.clear();
        self.history_index = None;
        self.mode = Mode::Normal;
    }

    /// Add to command buffer.
    pub fn push_command_char(&mut self, ch: char) {
        self.command_buffer.push(ch);
        self.history_index = None;
    }

    /// Remove last char from command buffer.
    pub fn pop_command_char(&mut self) {
        self.command_buffer.pop();
        self.history_index = None;
    }

    /// Get the current command buffer.
    pub fn command_buffer(&self) -> &str {
        &self.command_buffer
    }

    /// Execute the current command buffer.
    pub fn execute_command(&mut self) -> bool {
        self.execute_command_internal()
    }

    fn execute_command_internal(&mut self) -> bool {
        let cmd = self.command_buffer.clone();
        self.command_buffer.clear();
        self.history_index = None;

        if cmd.is_empty() {
            self.mode = Mode::Normal;
            return false;
        }

        // Add to history
        if !self.command_history.last().map(|h| h == &cmd).unwrap_or(true) {
            self.command_history.push(cmd.clone());
        }
        self.history_index = None;

        // Execute the command
        let should_quit = self.run_command(&cmd);
        self.mode = Mode::Normal;
        should_quit
    }

    /// Navigate command history (up).
    pub fn history_prev(&mut self) {
        if self.command_history.is_empty() {
            return;
        }
        let next_index = match self.history_index {
            Some(idx) => idx.saturating_sub(1),
            None => {
                self.history_index = Some(self.command_history.len() - 1);
                self.command_history.len() - 1
            }
        };
        if let Some(cmd) = self.command_history.get(next_index) {
            self.command_buffer = cmd.clone();
        }
    }

    /// Navigate command history (down).
    pub fn history_next(&mut self) {
        let Some(idx) = self.history_index else {
            return;
        };
        if idx + 1 < self.command_history.len() {
            self.history_index = Some(idx + 1);
            if let Some(cmd) = self.command_history.get(idx + 1) {
                self.command_buffer = cmd.clone();
            }
        } else {
            self.history_index = None;
            self.command_buffer.clear();
        }
    }

    /// Handle a crossterm Event and return an optional command.
    /// Returns the resolved command name, or None if the key was consumed but didn't resolve.
    pub fn handle_event(&mut self, event: &Event) -> Option<String> {
        let Event::Key(key) = event else {
            return None;
        };

        // Only handle press events (not repeat)
        if key.kind != KeyEventKind::Press {
            return None;
        }

        match &self.mode {
            Mode::Command(_) => self.handle_command_key(key),
            _ => self.handle_normal_key(key),
        }
    }

    /// Handle a key event when in command mode.
    fn handle_command_key(&mut self, key: &KeyEvent) -> Option<String> {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Enter => {
                let quit = self.execute_command_internal();
                if quit {
                    Some("quit".to_string())
                } else {
                    None
                }
            }
            KeyCode::Esc | KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.exit_command_mode();
                None
            }
            KeyCode::Backspace if self.command_buffer.is_empty() => {
                self.exit_command_mode();
                None
            }
            KeyCode::Backspace => {
                self.pop_command_char();
                None
            }
            KeyCode::Up => {
                self.history_prev();
                None
            }
            KeyCode::Down => {
                self.history_next();
                None
            }
            KeyCode::Tab => {
                self.tab_complete();
                None
            }
            KeyCode::Char(ch) => {
                self.push_command_char(ch);
                None
            }
            _ => None,
        }
    }

    /// Handle a key event in normal/visual/insert mode.
    fn handle_normal_key(&mut self, key: &KeyEvent) -> Option<String> {
        let Some(token) = key_event_to_token(key) else {
            self.pending.clear();
            return None;
        };

        // Check for leader key press
        if let Some(leader) = self.leader {
            if token == KeyToken::Char(leader) && self.pending.is_empty() {
                self.pending.push(KeyToken::Leader);
                return None;
            }
        }

        let mut attempted = self.pending.clone();
        attempted.push(token.clone());

        // Try to resolve the sequence
        if let Some(cmd) = self.keymap.get(&self.mode, &attempted) {
            if !self.has_pending_longer(&attempted) {
                let command = cmd.name().to_string();
                self.pending.clear();
                return Some(command);
            }
        }

        // If this is a prefix of a longer sequence, keep pending
        if self.has_pending(&attempted) {
            self.pending = attempted;
            return None;
        }

        // No match - clear pending and try single token
        self.pending.clear();
        if let Some(cmd) = self.keymap.get(&self.mode, &[token]) {
            Some(cmd.name().to_string())
        } else {
            None
        }
    }

    /// Check if there's a longer mapping that has the current tokens as prefix.
    fn has_pending_longer(&self, tokens: &[KeyToken]) -> bool {
        self.keymap
            .mappings_for_mode(&self.mode)
            .iter()
            .any(|(seq, _)| {
                seq.len() > tokens.len() && is_sequence_prefix(tokens, &seq_to_tokens(seq))
            })
    }

    /// Check if current tokens match any mapping prefix.
    fn has_pending(&self, tokens: &[KeyToken]) -> bool {
        self.keymap
            .mappings_for_mode(&self.mode)
            .iter()
            .any(|(seq, _)| is_sequence_prefix(tokens, &seq_to_tokens(seq)))
    }

    /// Execute a command by name.
    fn run_command(&mut self, command: &str) -> bool {
        let canonical = self.canonicalize_command(command);
        let Some(handler) = self.handlers.remove(canonical.as_str()) else {
            // Check if it's a built-in mappable command
            if self.is_mappable_command(&canonical) {
                return self.execute_mappable_command(&canonical);
            }
            return false;
        };

        let mut state = InputState {
            should_quit: false,
            message: None,
            error: None,
        };
        handler(&mut state)
    }

    /// Execute a mappable (built-in) command.
    fn execute_mappable_command(&mut self, command: &str) -> bool {
        match command {
            "quit" => true,
            "enter_command_mode" => {
                self.enter_command_mode();
                false
            }
            "help" => {
                // Just toggle help mode - app handles rendering
                self.set_mode(Mode::Help);
                false
            }
            "enter_normal_mode" => {
                self.set_mode(Mode::Normal);
                false
            }
            "enter_visual_mode" => {
                self.set_mode(Mode::Visual);
                false
            }
            "enter_insert_mode" => {
                self.set_mode(Mode::Insert);
                false
            }
            "cursor_up" => false,
            "cursor_down" => false,
            "pan_up" => false,
            "pan_down" => false,
            "pan_left" => false,
            "pan_right" => false,
            "zoom_in" => false,
            "zoom_out" => false,
            "center_viewport" => false,
            "jump_to_first_row" => false,
            "jump_to_last_row" => false,
            "jump_top" => false,
            "jump_bottom" => false,
            _ => false,
        }
    }

    /// Canonicalize a command name (handle aliases).
    fn canonicalize_command(&self, command: &str) -> String {
        match command {
            "q" | "quit" => "quit".to_string(),
            "h" | "help" => "help".to_string(),
            "j" | "cursor_down" | "pan_down" => "cursor_down".to_string(),
            "k" | "cursor_up" | "pan_up" => "cursor_up".to_string(),
            "gg" | "jump_to_first_row" | "jump_top" => "jump_to_first_row".to_string(),
            "G" | "jump_to_last_row" | "jump_bottom" => "jump_to_last_row".to_string(),
            "v" | "enter_visual_mode" => "enter_visual_mode".to_string(),
            "i" | "enter_insert_mode" => "enter_insert_mode".to_string(),
            ":" | "enter_command_mode" => "enter_command_mode".to_string(),
            _ => command.to_string(),
        }
    }

    /// Check if a command is a known mappable command.
    fn is_mappable_command(&self, command: &str) -> bool {
        matches!(
            command,
            "quit" | "help" | "cursor_up" | "cursor_down"
                | "pan_up" | "pan_down" | "pan_left" | "pan_right"
                | "jump_to_first_row" | "jump_to_last_row"
                | "jump_top" | "jump_bottom"
                | "zoom_in" | "zoom_out"
                | "center_viewport"
                | "enter_visual_mode" | "enter_insert_mode" | "enter_normal_mode"
                | "enter_command_mode"
        )
    }

    /// Tab completion for command mode.
    fn tab_complete(&mut self) {
        let partial = self.command_buffer.clone();
        let matches: Vec<String> = self
            .handlers
            .keys()
            .filter(|cmd| cmd.starts_with(&partial))
            .cloned()
            .collect();

        if matches.is_empty() {
            return;
        }

        if matches.len() == 1 {
            self.command_buffer = format!("{}{}", partial, matches[0].strip_prefix(&partial).unwrap_or(&matches[0]));
        } else if partial.is_empty() {
            // Show candidates as status (app can read these)
            self.command_buffer = matches.join(" ");
        } else {
            // Find common prefix
            let common: String = matches
                .iter()
                .fold(None, |acc, s| {
                    acc.map(|common: String| {
                        common
                            .chars()
                            .zip(s.chars())
                            .take_while(|(a, b)| a == b)
                            .map(|(a, _)| a)
                            .collect()
                    })
                    .or_else(|| Some(s[..s.len().min(common_len(&matches))].to_string()))
                })
                .unwrap_or_default();
            if common.len() > partial.len() {
                self.command_buffer = common;
            }
        }
    }

    /// Get mappings display rows (for :maps command).
    pub fn mapping_rows(&self, mode: Option<&Mode>) -> Vec<Vec<String>> {
        let mode = mode.unwrap_or(&self.mode);
        let mut entries = self.keymap.mappings_for_mode(mode);
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut rows = Vec::with_capacity(entries.len() + 1);
        rows.push(vec!["Keys".to_string(), "Command".to_string()]);
        rows.extend(entries.into_iter().map(|(seq, cmd)| vec![seq, cmd.name().to_string()]));
        rows
    }
}

/// Trait for extending Event handling with additional context.
pub trait KeyEventExt {
    /// Convert a KeyEvent to a KeyToken.
    fn to_token(&self) -> Option<KeyToken>;
}

impl KeyEventExt for KeyEvent {
    fn to_token(&self) -> Option<KeyToken> {
        key_event_to_token(self)
    }
}

fn is_sequence_prefix(short: &[KeyToken], long: &[KeyToken]) -> bool {
    if long.len() < short.len() {
        return false;
    }
    long[..short.len()].iter().zip(short.iter()).all(|(a, b)| a == b)
}

fn seq_to_tokens(seq: &str) -> Vec<KeyToken> {
    parse_key_sequence(seq).unwrap_or_default()
}

fn common_len(strings: &[String]) -> usize {
    if strings.is_empty() {
        return 0;
    }
    let first = &strings[0];
    let chars: Vec<(char, Vec<char>)> = first
        .chars()
        .zip(strings.iter().skip(1).map(|s| s.chars().collect::<Vec<_>>()))
        .collect();

    chars
        .iter()
        .take_while(|(c, others)| others.iter().all(|oc| oc == c))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_context() {
        let ctx = InputContext::new();
        assert_eq!(ctx.mode_name(), "normal");
    }

    #[test]
    fn test_set_leader() {
        let mut ctx = InputContext::new();
        ctx.set_leader(None);
        ctx.set_leader(Some('\\'));
        assert_eq!(ctx.leader, Some('\\'));
    }

    #[test]
    fn test_mode_switching() {
        let mut ctx = InputContext::new();
        ctx.set_mode(Mode::Visual);
        assert_eq!(ctx.mode_name(), "visual");
    }

    #[test]
    fn test_command_mode() {
        let mut ctx = InputContext::new();
        ctx.enter_command_mode();
        assert!(matches!(ctx.mode(), Mode::Command(_)));
        ctx.push_command_char('q');
        ctx.push_command_char('u');
        ctx.push_command_char('i');
        ctx.push_command_char('t');
        assert_eq!(ctx.command_buffer(), "quit");
    }

    #[test]
    fn test_tab_completion() {
        let mut ctx = InputContext::new();
        ctx.register_handler("quit", |_| false);
        ctx.register_handler("help", |_| false);
        ctx.register_handler("hello", |_| false);

        ctx.enter_command_mode();
        ctx.push_command_char('h');
        ctx.tab_complete();
        // Should complete to 'help' or 'hello' (first match)
        assert!(ctx.command_buffer().starts_with('h'));
    }

    #[test]
    fn test_history_navigation() {
        let mut ctx = InputContext::new();
        ctx.command_history = vec!["quit".to_string(), "help".to_string()];

        ctx.history_prev();
        assert_eq!(ctx.command_buffer(), "help");

        ctx.history_next();
        assert_eq!(ctx.command_buffer(), "");
    }
}

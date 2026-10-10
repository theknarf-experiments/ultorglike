//! Key tokens represent atomic key events that can be composed into sequences.
//! Uses the same bracket syntax as mbp-info for readability.

use std::fmt;

/// Represents a single key token in a key sequence.
/// These can be composed to form multi-key sequences (e.g., `<leader> gg`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum KeyToken {
    /// A regular character (case-sensitive)
    Char(char),
    /// Control character (e.g., `<C-a>`)
    Ctrl(char),
    /// The leader key placeholder (e.g., `<leader> gg`)
    Leader,
    /// Arrow keys
    Up,
    Down,
    Left,
    Right,
    /// Special keys
    Esc,
    Enter,
    Tab,
    Backspace,
    Space,
    /// Function keys
    F(u8),
}

impl fmt::Display for KeyToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyToken::Char(c) => write!(f, "{}", c),
            KeyToken::Ctrl(c) => write!(f, "<C-{}>", c),
            KeyToken::Leader => write!(f, "<leader>"),
            KeyToken::Up => write!(f, "<up>"),
            KeyToken::Down => write!(f, "<down>"),
            KeyToken::Left => write!(f, "<left>"),
            KeyToken::Right => write!(f, "<right>"),
            KeyToken::Esc => write!(f, "<esc>"),
            KeyToken::Enter => write!(f, "<enter>"),
            KeyToken::Tab => write!(f, "<tab>"),
            KeyToken::Backspace => write!(f, "<bs>"),
            KeyToken::Space => write!(f, "<space>"),
            KeyToken::F(n) => write!(f, "<F{}>", n),
        }
    }
}

impl KeyToken {
    /// Format a key token for display in help text or mappings.
    pub fn display(&self) -> String {
        self.to_string()
    }
}

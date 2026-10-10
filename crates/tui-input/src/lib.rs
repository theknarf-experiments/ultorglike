//! # tui-input
//!
//! A reusable, vim-like input handling system for Rust TUI applications.
//! Combines the best features from mbp-info and canvas-tui:
//!
//! - **Mode support** (Normal, Visual, Insert, Command) - like canvas-tui
//! - **Leader key** with configurable prefix - like mbp-info
//! - **Bracket key syntax** (`<C-a>`, `<Esc>`, `<leader>`) - like mbp-info
//! - **Partial matching** for multi-key sequences - like mbp-info
//! - **String-based commands** for flexible handler registration
//! - **Default vim keybindings** (j/k, gg/G, navigation)
//!
//! ## Quick Example
//!
//! ```rust,no_run
//! use tui_input::{InputContext, KeyToken};
//! use crossterm::event::{Event, KeyEvent};
//!
//! let mut ctx = InputContext::new();
//!
//! // Handle a key event
//! let event = Event::Key(KeyEvent::new(crossterm::event::KeyCode::Char('j'), crossterm::event::KeyModifiers::empty()));
//! if let Some(command) = ctx.handle_event(&event) {
//!     println!("Command: {}", command);
//! }
//! ```

mod key_token;
mod input_context;
mod parse;
mod keymap;

pub use key_token::KeyToken;
pub use input_context::{InputContext, KeyEventExt};
pub use parse::parse_key_sequence;
pub use keymap::{KeyMap, MappableCommand, Mode};
pub use parse::key_event_to_token;

// Re-export commonly used types from crossterm
pub use crossterm::event::{KeyEvent, KeyModifiers, KeyCode};

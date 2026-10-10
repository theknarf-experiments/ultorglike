//! # tui-widgets
//!
//! Reusable TUI widget components for Ratatui-based applications.
//!
//! ## Widgets
//!
//! - `status_bar` - Status line with mode indicator
//! - `help_popup` - Modal help overlay
//! - `command_bar` - Command line with input and history

mod command_bar;
mod help_popup;
mod status_bar;

pub use command_bar::command_bar;
pub use help_popup::help_popup;
pub use status_bar::status_bar;

// Re-export common types
pub use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

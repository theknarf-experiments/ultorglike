//! # tui-widgets
//!
//! Reusable TUI widget components for Ratatui-based applications.
//!
//! ## Widgets
//!
//! - `status_bar` - Status line with mode indicator
//! - `help_popup` - Modal help overlay
//! - `command_bar` - Command line with input and history

mod status_bar;
mod help_popup;
mod command_bar;

pub use status_bar::status_bar;
pub use help_popup::help_popup;
pub use command_bar::command_bar;

// Re-export common types
pub use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

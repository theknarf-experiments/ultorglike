//! Input handling utilities for the application.
//!
//! This module handles special key processing (like in-notice mode)
//! that should happen before the main input context.

use tui_input::KeyEventExt;

/// Handle keys when a notice is showing.
/// Returns true if the key was consumed.
pub fn handle_notice_input(key: &crossterm::event::KeyEvent, notice_selected: &mut usize, rows_count: usize) -> bool {
    match key.to_token() {
        Some(tui_input::KeyToken::Esc) => {
            return true;
        }
        Some(tui_input::KeyToken::Char('q')) => {
            return true;
        }
        Some(tui_input::KeyToken::Down) | Some(tui_input::KeyToken::Char('j')) => {
            if *notice_selected + 1 < rows_count {
                *notice_selected += 1;
            }
            return true;
        }
        Some(tui_input::KeyToken::Up) | Some(tui_input::KeyToken::Char('k')) => {
            if *notice_selected > 0 {
                *notice_selected -= 1;
            }
            return true;
        }
        _ => false,
    }
}

/// Handle keys in a popup/modal context.
pub fn handle_popup_input(key: &crossterm::event::KeyEvent, pending: &mut Vec<tui_input::KeyToken>) -> bool {
    match key.to_token() {
        Some(tui_input::KeyToken::Esc) => {
            pending.clear();
            true
        }
        _ => false,
    }
}

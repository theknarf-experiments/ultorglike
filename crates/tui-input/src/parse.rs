//! Key sequence parsing from string format.
//! Supports both bracket syntax (`<C-a>`, `<leader>`) and plain characters.

use crate::KeyToken;

/// Parse a key sequence string into a vector of `KeyToken`s.
///
/// # Examples
///
/// ```
/// use tui_input::{parse_key_sequence, KeyToken};
///
/// // Plain characters
/// assert_eq!(parse_key_sequence("gg").unwrap(), vec![KeyToken::Char('g'), KeyToken::Char('g')]);
///
/// // Bracket syntax
/// assert_eq!(parse_key_sequence("<C-w> h").unwrap(), vec![KeyToken::Ctrl('w'), KeyToken::Char('h')]);
///
/// // Leader key
/// assert_eq!(parse_key_sequence("<leader> q").unwrap(), vec![KeyToken::Leader, KeyToken::Char('q')]);
///
/// // Arrow keys
/// assert_eq!(parse_key_sequence("<up>").unwrap(), vec![KeyToken::Up]);
/// ```
pub fn parse_key_sequence(input: &str) -> Result<Vec<KeyToken>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '<' {
            let mut token = String::new();
            let mut closed = false;
            for next in chars.by_ref() {
                if next == '>' {
                    closed = true;
                    break;
                }
                token.push(next);
            }
            if !closed {
                return Err("Unclosed <...> token in key sequence.".to_string());
            }
            tokens.push(parse_bracket_token(&token)?);
        } else if ch.is_whitespace() {
            // Skip whitespace between tokens
            continue;
        } else {
            tokens.push(KeyToken::Char(ch));
        }
    }

    if tokens.is_empty() {
        return Err("Key sequence is empty.".to_string());
    }

    Ok(tokens)
}

fn parse_bracket_token(token: &str) -> Result<KeyToken, String> {
    let lower = token.to_lowercase();

    // Control key: c-x
    if let Some(rest) = lower.strip_prefix("c-") {
        let mut chars = rest.chars();
        let Some(ch) = chars.next() else {
            return Err("Invalid ctrl token.".to_string());
        };
        if chars.next().is_some() {
            return Err("Ctrl token must be a single character.".to_string());
        }
        return Ok(KeyToken::Ctrl(ch));
    }

    // Alt key: m-x (mbp-info style)
    if let Some(rest) = lower.strip_prefix("m-") {
        let mut chars = rest.chars();
        let Some(ch) = chars.next() else {
            return Err("Invalid alt token.".to_string());
        };
        if chars.next().is_some() {
            return Err("Alt token must be a single character.".to_string());
        }
        // Store as a separate Alt variant or handle via modifiers
        return Ok(KeyToken::Char(ch.to_ascii_uppercase()));
    }

    match lower.as_str() {
        "leader" => Ok(KeyToken::Leader),
        "up" => Ok(KeyToken::Up),
        "down" => Ok(KeyToken::Down),
        "left" => Ok(KeyToken::Left),
        "right" => Ok(KeyToken::Right),
        "esc" | "escape" => Ok(KeyToken::Esc),
        "enter" | "cr" => Ok(KeyToken::Enter),
        "tab" => Ok(KeyToken::Tab),
        "bs" | "backspace" => Ok(KeyToken::Backspace),
        "space" | "spc" => Ok(KeyToken::Space),
        // Function keys
        f if f.starts_with("f") && f.len() > 1 => {
            let num: u8 = f[1..].parse().map_err(|_| format!("Unknown key token: <{}>", token))?;
            if num < 1 || num > 24 {
                return Err(format!("F key must be 1-24: <{}>", token));
            }
            Ok(KeyToken::F(num))
        }
        _ => Err(format!("Unknown key token: <{}>", token)),
    }
}

/// Convert a crossterm KeyEvent to a KeyToken.
pub fn key_event_to_token(event: &crossterm::event::KeyEvent) -> Option<KeyToken> {
    use crossterm::event::{KeyCode, KeyModifiers};

    match event.code {
        KeyCode::Char(ch) => {
            if event.modifiers.contains(KeyModifiers::CONTROL) {
                Some(KeyToken::Ctrl(ch.to_ascii_lowercase()))
            } else if event.modifiers.contains(KeyModifiers::SHIFT) {
                Some(KeyToken::Char(ch.to_ascii_uppercase()))
            } else {
                Some(KeyToken::Char(ch))
            }
        }
        KeyCode::Up => Some(KeyToken::Up),
        KeyCode::Down => Some(KeyToken::Down),
        KeyCode::Left => Some(KeyToken::Left),
        KeyCode::Right => Some(KeyToken::Right),
        KeyCode::Esc => Some(KeyToken::Esc),
        KeyCode::Enter => Some(KeyToken::Enter),
        KeyCode::Tab => Some(KeyToken::Tab),
        KeyCode::Backspace => Some(KeyToken::Backspace),
        KeyCode::F(f) => Some(KeyToken::F(f)),
        _ => None,
    }
}

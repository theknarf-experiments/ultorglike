//! Status bar widget.
//! Shows the current mode and any status messages.

use ratatui::{Frame, layout::Rect, style::{Color, Modifier, Style}, text::{Line, Span}, widgets::Paragraph};

/// Draw a status bar at the given area.
///
/// # Arguments
/// * `frame` - The render frame
/// * `area` - The area to draw in
/// * `mode` - The current mode name
/// * `status` - Optional status message
/// * `command` - Optional command being typed (for command mode)
pub fn status_bar(
    frame: &mut Frame,
    area: Rect,
    mode: &str,
    status: Option<&str>,
    command: Option<&str>,
) {
    let mut parts = Vec::new();

    // Mode indicator
    let mode_style = match mode {
        "normal" => Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD),
        "visual" => Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD),
        "insert" => Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        "command" => Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
        "help" => Style::default().fg(Color::Black).bg(Color::Blue).add_modifier(Modifier::BOLD),
        _ => Style::default().fg(Color::White).bg(Color::DarkGray).add_modifier(Modifier::BOLD),
    };
    parts.push(Span::styled(format!("[{}]", mode), mode_style));

    // Separator
    if status.is_some() || command.is_some() {
        parts.push(Span::raw(" "));
    }

    // Status message
    if let Some(s) = status {
        parts.push(Span::raw(s));
    }

    // Command buffer
    if let Some(cmd) = command {
        if !cmd.is_empty() {
            parts.push(Span::styled(
                format!(" :{}", cmd),
                Style::default().fg(Color::Yellow),
            ));
        }
    }

    let text = Line::from(parts);
    let widget = Paragraph::new(text)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    frame.render_widget(widget, area);
}

/// Draw a mode indicator only (no status or command).
pub fn mode_indicator(frame: &mut Frame, area: Rect, mode: &str) {
    status_bar(frame, area, mode, None, None);
}

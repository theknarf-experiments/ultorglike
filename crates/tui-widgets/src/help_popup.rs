//! Help popup widget.
//! Shows keybindings and help text in a modal overlay.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

/// Draw a help popup centered on screen.
///
/// # Arguments
/// * `frame` - The render frame
/// * `size` - The terminal size
/// * `title` - Title for the popup
/// * `lines` - Help text lines
/// * `mappings` - Key mappings to display (optional)
pub fn help_popup(
    frame: &mut Frame,
    size: Rect,
    title: &str,
    lines: Vec<String>,
    mappings: Option<Vec<Vec<String>>>,
) {
    // Calculate popup size (70% of terminal, min 40x10, max width/height - 4)
    let width = ((size.width as f32) * 0.7) as u16;
    let height = ((size.height as f32) * 0.6) as u16;
    let max_width = size.width.saturating_sub(4);
    let max_height = size.height.saturating_sub(4);
    let width = width.max(40).min(max_width);
    let height = height.max(10).min(max_height);

    let popup_area = Rect {
        x: size.x + (size.width.saturating_sub(width)) / 2,
        y: size.y + (size.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    // Clear the area behind the popup
    frame.render_widget(ratatui::widgets::Clear, popup_area);

    // Build block with title
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // Build content
    let mut content_lines: Vec<Line> = vec![Line::from("")];

    // Add help text lines
    for line in &lines {
        content_lines.push(Line::from(line.as_str()));
    }

    // Add mappings if provided
    if let Some(mappings) = mappings {
        if !mappings.is_empty() {
            content_lines.push(Line::from(""));
            content_lines.push(Line::from(Span::styled(
                "Key Bindings:",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            for mapping in mappings {
                if mapping.len() >= 2 {
                    content_lines.push(Line::from(format!(
                        "  {:15} {}",
                        mapping[0],
                        mapping[1]
                    )));
                }
            }
        }
    }

    content_lines.push(Line::from(""));
    content_lines.push(Line::from(
        Span::styled("Press any key to close", Style::default().fg(Color::DarkGray)),
    ));

    let widget = Paragraph::new(content_lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });

    frame.render_widget(widget, popup_area);
}

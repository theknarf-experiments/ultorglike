//! Command bar widget.
//! Shows command input with completion candidates.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

/// Draw a command bar at the bottom of the screen.
///
/// # Arguments
/// * `frame` - The render frame
/// * `area` - The area to draw in
/// * `buffer` - The current command buffer text
/// * `candidates` - Optional completion candidates with current index
pub fn command_bar(
    frame: &mut Frame,
    area: Rect,
    buffer: &str,
    candidates: Option<(&[String], usize)>,
) {
    let mut spans = Vec::new();

    // Command prefix
    spans.push(Span::raw(":"));

    // Command buffer
    let _cursor_pos = buffer.len();
    spans.push(Span::raw(buffer));

    // Blinking cursor (we can't do real blinking in ratatui without state,
    // but we can show position)
    if !buffer.is_empty() {
        // No-op - cursor would need animation frame tracking
    }

    // Completion candidates
    if let Some((items, index)) = candidates {
        if !items.is_empty() {
            spans.push(Span::raw("  "));
            for (i, item) in items.iter().enumerate() {
                let style = if i == index {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                spans.push(Span::styled(item.clone(), style));
                if i + 1 < items.len() {
                    spans.push(Span::raw(" "));
                }
            }
        }
    }

    let widget = Paragraph::new(Line::from(spans));
    frame.render_widget(widget, area);
}

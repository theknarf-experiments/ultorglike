//! UI rendering for the application.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::{App, FocusPanel};

pub fn draw(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Layout: dashboard bar (1 row), body (flexible), command bar (1 row)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(size);

    draw_dashboard_bar(frame, chunks[0]);
    draw_body(frame, app, chunks[1]);
    draw_command_bar(frame, app, chunks[2]);

    // Draw notice popup if showing
    if app.has_notice() {
        draw_notice_popup(frame, size, app);
    }

    // Draw help overlay if in help mode
    if matches!(app.input.mode(), tui_input::Mode::Help) {
        draw_help_overlay(frame, size, app);
    }
}

fn draw_dashboard_bar(frame: &mut Frame, area: Rect) {
    // Simple dashboard indicator
    let text = Line::from(vec![
        Span::styled(" Dashboard ", Style::default().fg(Color::Black).bg(Color::Blue).add_modifier(Modifier::BOLD)),
        Span::raw(" ultorglike demo "),
    ]);
    let widget = Paragraph::new(text);
    frame.render_widget(widget, area);
}

fn draw_body(frame: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let blocks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    draw_metrics_list(frame, app, blocks[0]);
    draw_info_panel(frame, app, blocks[1]);
}

fn draw_metrics_list(frame: &mut Frame, app: &App, area: Rect) {
    let mut block = Block::default()
        .title(" Metrics ")
        .borders(Borders::ALL);

    if matches!(app.focus, FocusPanel::Metrics) {
        block = block.border_style(Style::default().fg(Color::Cyan));
    }

    let items: Vec<ListItem> = app
        .data_lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let style = if i == app.selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(line.clone())).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, area, &mut ratatui::widgets::ListState::default());
}

fn draw_info_panel(frame: &mut Frame, _app: &App, area: Rect) {
    let block = Block::default()
        .title(" System Info ")
        .borders(Borders::ALL);

    let info_lines = vec![
        "Host:      MacMini (M2)".to_string(),
        "OS:        macOS Sonoma 14.5".to_string(),
        "Kernel:    23.5.0".to_string(),
        "Shell:     zsh 5.9".to_string(),
        "Terminal:  iTerm2".to_string(),
        "CPU:       Apple M2 (8 cores)".to_string(),
        "Memory:    32 GB LPDDR5".to_string(),
        "Storage:   512 GB NVMe".to_string(),
    ];

    let text = info_lines
        .iter()
        .map(|l| Line::from(l.as_str()))
        .collect::<Vec<_>>();

    let widget = Paragraph::new(text).block(block);
    frame.render_widget(widget, area);
}

fn draw_command_bar(frame: &mut Frame, app: &App, area: Rect) {
    use tui_widgets::command_bar;

    // Get completion info
    let mut completion_candidates: Option<Vec<String>> = None;
    let completion = if app.input.command_buffer().starts_with(':') {
        let partial = app.input.command_buffer().trim_start_matches(':');
        let matches: Vec<String> = app
            .input
            .command_names()
            .into_iter()
            .filter(|c| c.starts_with(partial))
            .collect();
        if !matches.is_empty() {
            completion_candidates = Some(matches);
            completion_candidates.as_ref().map(|c| (c.as_slice(), 0))
        } else {
            None
        }
    } else {
        None
    };

    command_bar(frame, area, app.input.command_buffer(), completion);
}

fn draw_notice_popup(frame: &mut Frame, size: Rect, app: &App) {
    let Some(rows) = app.notice_rows() else {
        return;
    };
    let title = app.notice_title().unwrap_or("Notice");

    // Popup dimensions (60% of screen)
    let width = ((size.width as f32) * 0.6) as u16;
    let height = ((size.height as f32) * 0.5) as u16;
    let width = width.max(40).min(size.width.saturating_sub(4));
    let height = height.max(8).min(size.height.saturating_sub(4));

    let popup_area = Rect {
        x: size.x + (size.width.saturating_sub(width)) / 2,
        y: size.y + (size.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    frame.render_widget(ratatui::widgets::Clear, popup_area);

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let items: Vec<ListItem> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let text = row.join("  ");
            let style = if i == app.notice.as_ref().unwrap().selected {
                Style::default()
                    .bg(Color::Cyan)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(text)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, popup_area, &mut ratatui::widgets::ListState::default());
}

fn draw_help_overlay(frame: &mut Frame, size: Rect, _app: &App) {

    let width = ((size.width as f32) * 0.7) as u16;
    let height = ((size.height as f32) * 0.7) as u16;
    let width = width.max(50).min(size.width.saturating_sub(4));
    let height = height.max(15).min(size.height.saturating_sub(4));

    let popup_area = Rect {
        x: size.x + (size.width.saturating_sub(width)) / 2,
        y: size.y + (size.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    frame.render_widget(ratatui::widgets::Clear, popup_area);

    let block = Block::default()
        .title(" Help - Press any key to close ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let help_text = vec![
        "Navigation:".to_string(),
        "  j / down      Move down".to_string(),
        "  k / up        Move up".to_string(),
        "  gg            Jump to first row".to_string(),
        "  G             Jump to last row".to_string(),
        "".to_string(),
        "Pan viewport:".to_string(),
        "  h / left      Pan left".to_string(),
        "  l / right     Pan right".to_string(),
        "".to_string(),
        "Zoom:".to_string(),
        "  + / =         Zoom in".to_string(),
        "  -             Zoom out".to_string(),
        "  c             Center viewport".to_string(),
        "".to_string(),
        "Modes:".to_string(),
        "  i             Enter insert mode".to_string(),
        "  v             Enter visual mode".to_string(),
        "  :             Enter command mode".to_string(),
        "".to_string(),
        "Commands:".to_string(),
        "  :quit or :q   Quit the app".to_string(),
        "  :help or :h   Toggle help".to_string(),
        "".to_string(),
        "Leader key: , (comma)".to_string(),
        "  , q           Quit".to_string(),
        "  , h           Help".to_string(),
    ];

    let content_lines: Vec<ratatui::text::Line> = help_text
        .iter()
        .map(|l| ratatui::text::Line::from(l.as_str()))
        .collect();

    let widget = Paragraph::new(content_lines)
        .block(block)
        .wrap(ratatui::widgets::Wrap { trim: true });

    frame.render_widget(widget, popup_area);
}

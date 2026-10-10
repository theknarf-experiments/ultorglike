use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use tui_input::Mode;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub const HELP:&str="NAVIGATION\nh j k l / arrows: move cells     10j: count     gg / G: first / last\nCtrl-d / Ctrl-u: half page       0 / $: first / last column\n/ search loaded values          n / N: next / previous match\nEnter / zo: expand              zc: collapse     za: toggle\n\nQUERY\nt: tables   f: fields & relationships   s / S: sort asc / desc   F: filter\nK: full value   =: formula editor   ]w / [w: widen / narrow\nx: hide field   u: undo   Ctrl-r: redo   gt / gT: switch views\nPicker: j/k select, / search, Enter toggle, Tab enter child, Esc close\n:filter orders.status = Shipped      :unfilter orders.status\n:match orders  (require matching children)   :keep orders (keep parents)\n:show orders.freight   :sort_desc orders.order_date   :unsort field\n:join orders customers   :fields customers.orders   :open customers\n:save views/customers.json    :load views/customers.json    :view 2\n:formula orders.line_items.amount = [quantity] * [unit_price]\n:formula orders.total = sum([line_items.amount]) + [freight]\n:unformula orders.total   (remove formula; x only hides it)\n]p / [p: next / previous collection page   +: load more\n:limit 200    :refresh    :help    :quit\n\n:query / :sql / :schema: inspect query, SQL, or table\n:session_save workspace.json   :session_load workspace.json\n:views   :duplicate   :close   :reconnect\n:move orders.total 1   :width orders.total 24\n:export json results.json   :export csv results.csv (loaded grid)\nEsc cancels input/popups or a running query. q closes help; q quits browsing.\nFilters affect children by default; :match propagates matching to the root.\nData is read-only. Saved views contain query settings, not credentials.";

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if area.width < 12 || area.height < 6 {
        return;
    }
    let title = app
        .query
        .as_ref()
        .map(|q| q.root.table.qualified())
        .unwrap_or_else(|| "PostgreSQL".into());
    line(
        frame,
        Rect::new(0, 0, area.width, 1),
        &format!(
            " ultorglike  {title}   view {}/{}   ? help   t tables   f fields",
            app.active_view + 1,
            app.views.len()
        ),
        Style::default().fg(Color::White).bg(Color::Blue),
    );
    let hidden = app
        .query
        .as_ref()
        .map(|q| {
            q.root
                .filters
                .iter()
                .filter(|f| !q.root.columns.contains(&f.column))
                .count()
                + q.root
                    .sort
                    .iter()
                    .filter(|s| !q.root.columns.contains(&s.column))
                    .count()
        })
        .unwrap_or(0);
    let mode = match app.input.mode() {
        Mode::Normal => "NORMAL",
        Mode::Command(_) => "COMMAND",
        Mode::Help => "HELP",
        Mode::Other(s) => s.as_str(),
        _ => "NORMAL",
    };
    line(
        frame,
        Rect::new(0, 1, area.width, 1),
        &format!(
            " {mode} {}  {}  {}  {}{}",
            app.count,
            app.location(),
            app.range(),
            if app.busy { "Loading…  " } else { "" },
            if hidden > 0 {
                format!("[{hidden} hidden dependencies; f to inspect]")
            } else {
                String::new()
            }
        ),
        Style::default().fg(Color::Cyan),
    );
    let header_height = app
        .grid
        .header_height
        .max(1)
        .min((area.height - 5) as usize) as u16;
    let body = Rect::new(
        0,
        2 + header_height,
        area.width,
        area.height - 5 - header_height,
    );
    app.viewport_width = area.width as usize;
    app.viewport_height = body.height as usize;
    app.clamp();
    let header = Rect::new(0, 2, area.width, header_height);
    grid(frame, app, header, body);
    let value = app
        .grid
        .cell_at(app.column, app.y)
        .map(|c| c.text.as_str())
        .unwrap_or("");
    line(
        frame,
        Rect::new(0, area.height - 3, area.width, 1),
        &format!(" {}:{}  {}", app.y + 1, app.column + 1, value),
        Style::default().fg(Color::Gray),
    );
    line(
        frame,
        Rect::new(0, area.height - 2, area.width, 1),
        &format!(" {}", app.message),
        Style::default().fg(if app.error {
            Color::LightRed
        } else {
            Color::Gray
        }),
    );
    let prompt = match app.input.mode() {
        Mode::Command(_) => format!(":{}", app.input.command_buffer()),
        Mode::Other(s) if s == "search" => format!("/{}", app.search),
        _ => " : command   / search   Enter expand   zc collapse   u undo".into(),
    };
    let cursor = if matches!(app.input.mode(), Mode::Command(_)) {
        1 + app.input.command_buffer()[..app.input.command_cursor()].width()
    } else {
        prompt.width()
    };
    let skip = cursor.saturating_sub(area.width.saturating_sub(2) as usize);
    clipped(
        frame,
        Rect::new(0, area.height - 1, area.width, 1),
        0,
        skip,
        prompt.width() + 1,
        &prompt,
        Style::default().fg(Color::Yellow),
    );
    if matches!(app.input.mode(), Mode::Command(_))
        || app.input.mode() == &Mode::Other("search".into())
    {
        frame.set_cursor_position((
            (cursor - skip).min(area.width.saturating_sub(1) as usize) as u16,
            area.height - 1,
        ));
    }
    if let Some(panel) = &app.panel {
        let rect = popup(
            area,
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        );
        frame.render_widget(Clear, rect);
        frame.render_widget(
            Paragraph::new(panel.text.as_str())
                .scroll((
                    panel.y.min(u16::MAX as usize) as u16,
                    panel.x.min(u16::MAX as usize) as u16,
                ))
                .block(
                    Block::default()
                        .title(format!(
                            " {} · hjkl scroll · g/G ends · q/Esc close ",
                            panel.title
                        ))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                ),
            rect,
        );
    }
    if let Some(picker) = &app.picker {
        let rect = popup(area, 100, 22);
        frame.render_widget(Clear, rect);
        let title = picker
            .node
            .and_then(|id| app.query.as_ref()?.root.path(id))
            .map(|p| format!("Fields: {}", p.join(" → ")))
            .unwrap_or_else(|| "Tables".into());
        frame.render_widget(
            Block::default()
                .title(format!(" {title} "))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
            rect,
        );
        if rect.height < 5 || rect.width < 4 {
            return;
        }
        line(
            frame,
            Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, 1),
            &format!(
                "/{}{}",
                picker.text,
                if picker.searching { " ▏" } else { "" }
            ),
            Style::default().fg(Color::Yellow),
        );
        let height = rect.height.saturating_sub(4) as usize;
        let start = picker.selected.saturating_sub(height.saturating_sub(1));
        for (index, entry) in picker.entries.iter().enumerate().skip(start).take(height) {
            line(
                frame,
                Rect::new(
                    rect.x + 1,
                    rect.y + 2 + (index - start) as u16,
                    rect.width - 2,
                    1,
                ),
                &entry.label,
                if index == picker.selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::White)
                },
            );
        }
        line(
            frame,
            Rect::new(rect.x + 1, rect.bottom() - 2, rect.width - 2, 1),
            "j/k move · / search · Enter toggle · Tab child · Esc close",
            Style::default().fg(Color::Gray),
        );
    }
}

fn grid(frame: &mut Frame, app: &App, header: Rect, body: Rect) {
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::DarkGray)),
        header,
    );
    for heading in &app.grid.headings {
        let x = app.grid.x(heading.first_column);
        let width: usize = app
            .grid
            .widths
            .iter()
            .skip(heading.first_column)
            .take(heading.columns)
            .sum();
        if heading.depth >= header.height as usize {
            continue;
        }
        let style = Style::default()
            .fg(if heading.columns > 1 {
                Color::Cyan
            } else {
                Color::White
            })
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD);
        clipped(
            frame,
            Rect::new(header.x, header.y + heading.depth as u16, header.width, 1),
            x,
            app.scroll_x,
            width,
            &format!(" {}", heading.label),
            style,
        );
    }
    if app.document.rows.is_empty() {
        line(
            frame,
            body,
            if app.busy {
                " Loading data…"
            } else {
                " No matching rows. Headers and query actions remain available; use u to undo or t to open a table."
            },
            Style::default().fg(Color::Gray),
        );
        return;
    }
    let selected = app.address();
    for cell in &app.grid.cells {
        let top = cell.y.max(app.scroll_y);
        let bottom = (cell.y + cell.height).min(app.scroll_y + body.height as usize);
        if top >= bottom {
            continue;
        }
        let x = app.grid.x(cell.column);
        let width = app.grid.widths[cell.column];
        let current = selected.as_ref() == Some(&cell.address);
        let style = if current {
            Style::default().fg(Color::Black).bg(Color::Yellow)
        } else if cell.text == "NULL" || cell.text == "∅" {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };
        for y in top..bottom {
            // Repeat a spanning parent's label at the top of the viewport for context.
            let text = if y == top {
                cell.text.replace(['\n', '\r', '\t'], " ")
            } else {
                String::new()
            };
            let row = Rect::new(body.x, body.y + (y - app.scroll_y) as u16, body.width, 1);
            clipped(
                frame,
                row,
                x,
                app.scroll_x,
                width.saturating_sub(1),
                &format!(" {text}"),
                style,
            );
            clipped(
                frame,
                row,
                x + width - 1,
                app.scroll_x,
                1,
                "│",
                Style::default().fg(Color::DarkGray),
            );
        }
    }
}
fn line(frame: &mut Frame, area: Rect, text: &str, style: Style) {
    frame.render_widget(Paragraph::new(text).style(style), area);
}
fn clipped(
    frame: &mut Frame,
    area: Rect,
    x: usize,
    scroll: usize,
    width: usize,
    text: &str,
    style: Style,
) {
    let start = x.max(scroll);
    let end = (x + width).min(scroll + area.width as usize);
    if start >= end || area.height == 0 {
        return;
    }
    let skip = start - x;
    let available = end - start;
    let mut at = 0;
    let mut used = 0;
    let mut output = String::new();
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if w == 0 {
            continue;
        }
        if at + w <= skip {
            at += w;
            continue;
        }
        if at < skip {
            let pad = at + w - skip;
            if pad <= available {
                output.push_str(&" ".repeat(pad));
                used += pad;
            }
            at += w;
            continue;
        }
        if used + w > available {
            break;
        }
        output.push(ch);
        used += w;
        at += w;
    }
    frame.render_widget(
        Paragraph::new(output).style(style),
        Rect::new(
            area.x + (start - scroll) as u16,
            area.y,
            available as u16,
            1,
        ),
    );
}
fn popup(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

mod app;
#[cfg(test)]
mod tests;
mod ui;
use app::App;
use crossterm::{
    event, execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| query_engine::executor::DEFAULT_URL.into());
    let mut view = None;
    let mut session = None;
    let mut editable = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--editable" => editable = true,
            "--database-url" => url = args.next().ok_or("--database-url requires a value")?,
            "--session" => session = Some(args.next().ok_or("--session requires a path")?),
            "--view" => view = Some(args.next().ok_or("--view requires a path")?),
            "--help" | "-h" => {
                println!("ultorglike [--database-url URL] [--view FILE | --session FILE] [--editable]\nUses DATABASE_URL or local Compose PostgreSQL on port 55432.\nStart test data: docker compose up -d --wait\nInside the app: ? help, t tables, f fields, : commands.");
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    if view.is_some() && session.is_some() {
        return Err("Choose --view or --session".into());
    }
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        previous(info);
    }));
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let mut app = App::new(Some(url), view);
    app.initial_session = session;
    app.editable = editable;
    while !app.should_quit {
        app.poll();
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        if event::poll(std::time::Duration::from_millis(40))? {
            app.handle_event(event::read()?);
        }
    }
    Ok(())
}

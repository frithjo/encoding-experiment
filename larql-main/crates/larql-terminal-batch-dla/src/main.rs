use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{event, execute};
use larql_terminal_batch_dla::{ui, App, Cli};
use larql_terminal_renderer::{draw_frame, AtomicGraphicsBackend};
use ratatui::Terminal;
use std::io;
use tokio::runtime::Runtime;
use clap::Parser;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let rt = Runtime::new()?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = AtomicGraphicsBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(cli.server.clone());
    app.renderer.clear_all_graphics()?;

    loop {
        draw_frame(&mut terminal, &app.renderer, |f, graphics_layer| {
            ui(f, &app, graphics_layer);
        })?;

        if event::poll(std::time::Duration::from_millis(16))? {
            if let event::Event::Key(key) = event::read()? {
                if app.handle_key(key, &rt) {
                    break;
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

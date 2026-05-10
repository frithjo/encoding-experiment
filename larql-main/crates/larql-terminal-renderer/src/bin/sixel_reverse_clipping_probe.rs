use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use larql_terminal_renderer::{
    draw_frame, AtomicGraphicsBackend, BackendType, GraphicsLayer, Image, ImageWidget, Renderer,
    Rgb,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::env;
use std::io;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), String> {
    let z_index = parse_z_index(env::args().collect());
    let renderer = Renderer::new_with_backend(BackendType::Sixel);
    let image = gradient_image(256, 192);

    enable_raw_mode().map_err(|e| e.to_string())?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(|e| e.to_string())?;
    let backend = AtomicGraphicsBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| e.to_string())?;

    for tick in 0..6 {
        draw_frame(&mut terminal, &renderer, |frame, graphics_layer| {
            ui(frame, graphics_layer, &renderer, &image, z_index, tick)
        })?;
        thread::sleep(Duration::from_millis(80));
    }
    disable_raw_mode().map_err(|e| e.to_string())?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(|e| e.to_string())?;

    Ok(())
}

fn parse_z_index(args: Vec<String>) -> i32 {
    args.windows(2)
        .find(|pair| pair[0] == "--z-index")
        .and_then(|pair| pair[1].parse().ok())
        .unwrap_or(-1)
}

fn gradient_image(width: usize, height: usize) -> Image {
    let mut image = Image::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let r = ((x * 255) / width.max(1)) as u8;
            let g = ((y * 255) / height.max(1)) as u8;
            let b = (((x + y) * 255) / (width + height).max(1)) as u8;
            image.set(x, y, Rgb::new(r, g, b));
        }
    }
    image
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(area.height.saturating_sub(height) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(area.width.saturating_sub(width) / 2),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(vertical[1]);
    horizontal[1]
}

fn ui(
    frame: &mut ratatui::terminal::Frame,
    graphics_layer: &mut GraphicsLayer,
    renderer: &Renderer,
    image: &Image,
    z_index: i32,
    tick: usize,
) {
    let root = Block::default()
        .borders(Borders::ALL)
        .title(format!("sixel probe z-index={z_index} tick={tick}"));
    let area = frame.size();
    let inner = root.inner(area);
    frame.render_widget(root, area);

    frame.render_widget(
        ImageWidget::new(image, renderer)
            .offset(0.0, 0.0)
            .zoom(1.0)
            .z_index(z_index),
        inner,
    );
    graphics_layer.add_with_view(image, inner, 0.0, 0.0, 1.0, z_index);

    let popup = centered_rect(inner, 30, 7);
    let popup_text = Paragraph::new("OCCLUDER\nreverse clipping proof")
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("overlay {tick}")),
        );
    frame.render_widget(popup_text, popup);
}

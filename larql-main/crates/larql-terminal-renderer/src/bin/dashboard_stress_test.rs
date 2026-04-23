use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use larql_terminal_renderer::{
    AtomicGraphicsBackend, BackendType, GraphicsLayer, Image, ImageWidget, Renderer, Rgb,
    draw_frame,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::env;
use std::io;
use std::time::{Duration, Instant};

struct App {
    renderer: Renderer,
    heatmaps: Vec<Image>,
    list_state: ListState,
    image_size_percent: u16,
    zoom: f32,
    offset_x: f32,
    offset_y: f32,
    z_index: i32,
}

impl App {
    fn new(z_index: i32) -> Self {
        let mut heatmaps = Vec::new();
        for i in 0..10 {
            heatmaps.push(create_mock_heatmap(i));
        }

        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            renderer: Renderer::new_auto(),
            heatmaps,
            list_state,
            image_size_percent: 50,
            zoom: 1.0,
            offset_x: 0.5,
            offset_y: 0.5,
            z_index,
        }
    }
}

fn parse_args() -> (i32, Option<Duration>) {
    let args: Vec<String> = env::args().collect();
    let mut z_index = 0;
    let mut quit_after = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--z-index" => {
                if i + 1 < args.len() {
                    z_index = args[i + 1].parse().unwrap_or(0);
                    i += 1;
                }
            }
            "--quit-after-ms" => {
                if i + 1 < args.len() {
                    if let Ok(ms) = args[i + 1].parse::<u64>() {
                        quit_after = Some(Duration::from_millis(ms));
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    (z_index, quit_after)
}

fn create_mock_heatmap(seed: usize) -> Image {
    let size = 256; // Larger size to test zoom/pan
    let mut img = Image::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let val = ((x + y + seed * 10) % 256) as f32 / 256.0;
            let intensity = (val * 255.0) as u8;
            img.set(
                x,
                y,
                Rgb::new(intensity, 255 - intensity, (seed * 25) as u8),
            );
        }
    }
    img
}

fn ui(frame: &mut Frame, app: &App, graphics_layer: &mut GraphicsLayer) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(frame.size());

    // 1. Sidebar with controls
    let controls = Paragraph::new(
        "Controls:\n\
         j/k: Select Image\n\
         z/x: Zoom In/Out\n\
         w/a/s/d: Pan\n\
         +/-: Widget Size\n\
         b: Cycle Backend\n\
         l: Toggle z-index\n\
         q: Quit\n\n\
         Backend: "
            .to_string()
            + &format!("{:?}", app.renderer.backend_type)
            + &format!("\nZoom: {:.1}x", app.zoom)
            + &format!("\nOffset: ({:.1}, {:.1})", app.offset_x, app.offset_y)
            + &format!("\nz-index: {}", app.z_index),
    )
    .block(Block::default().borders(Borders::ALL).title("Controls"));
    frame.render_widget(controls, chunks[0]);

    // 2. Main content: Image inside a nested layout
    let main_block = Block::default()
        .borders(Borders::ALL)
        .title("Dashboard Stress Test");
    let inner_area = main_block.inner(chunks[1]);
    frame.render_widget(main_block, chunks[1]);

    // Add some text *under* where the image will be to test z-index < 0
    let bg_text = Paragraph::new("LAYER TEST BACKGROUND TEXT - IF Z < 0 YOU SEE THIS")
        .style(ratatui::style::Style::default().fg(ratatui::style::Color::Yellow));
    frame.render_widget(bg_text, inner_area);

    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(app.image_size_percent),
            Constraint::Percentage(100 - app.image_size_percent),
        ])
        .split(inner_area);

    // Selected image display
    if let Some(selected) = app.list_state.selected() {
        let img = &app.heatmaps[selected];

        let img_block = Block::default()
            .borders(Borders::ALL)
            .title(format!("Image {}", selected));
        let img_inner = img_block.inner(content_chunks[0]);
        frame.render_widget(img_block, content_chunks[0]);

        // Ratatui Widget Pass
        let widget = ImageWidget::new(img, &app.renderer)
            .zoom(app.zoom)
            .offset(app.offset_x, app.offset_y)
            .z_index(app.z_index);
        frame.render_widget(widget, img_inner);

        // Graphics Layer Pass
        graphics_layer.add_with_view(
            img,
            img_inner,
            app.offset_x,
            app.offset_y,
            app.zoom,
            app.z_index,
        );
    }

    // Bottom list for "scrolling" feel
    let items: Vec<ListItem> = app
        .heatmaps
        .iter()
        .enumerate()
        .map(|(i, _)| ListItem::new(format!("Heatmap Data Layer {}", i)))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Layers"))
        .highlight_style(ratatui::style::Style::default().bg(ratatui::style::Color::DarkGray));

    // We don't render actual images in the list yet for simplicity,
    // but we verify the list scrolls correctly below the main image.
    let mut state = app.list_state.clone();
    frame.render_stateful_widget(list, content_chunks[1], &mut state);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (initial_z_index, quit_after) = parse_args();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = AtomicGraphicsBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(initial_z_index);
    app.renderer.clear_all_graphics()?;
    let started_at = Instant::now();

    let tick_rate = Duration::from_millis(16); // ~60fps for stress test

    loop {
        draw_frame(&mut terminal, &app.renderer, |f, graphics_layer| {
            ui(f, &app, graphics_layer)
        })?;

        if let Some(limit) = quit_after {
            if started_at.elapsed() >= limit {
                break;
            }
        }

        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('w') => {
                        app.offset_y = (app.offset_y - 0.1 / app.zoom).max(0.0);
                    }
                    KeyCode::Char('s') => {
                        app.offset_y = (app.offset_y + 0.1 / app.zoom).min(1.0);
                    }
                    KeyCode::Char('a') => {
                        app.offset_x = (app.offset_x - 0.1 / app.zoom).max(0.0);
                    }
                    KeyCode::Char('d') => {
                        app.offset_x = (app.offset_x + 0.1 / app.zoom).min(1.0);
                    }
                    KeyCode::Char('z') => {
                        app.zoom *= 1.2;
                    }
                    KeyCode::Char('x') => {
                        app.zoom = (app.zoom / 1.2).max(1.0);
                    }
                    KeyCode::Char('l') => {
                        app.z_index = if app.z_index == 1 { -1 } else { 1 };
                    }
                    KeyCode::Char('q') => break,
                    KeyCode::Char('j') => {
                        let i = match app.list_state.selected() {
                            Some(i) => {
                                if i >= app.heatmaps.len() - 1 {
                                    0
                                } else {
                                    i + 1
                                }
                            }
                            None => 0,
                        };
                        app.list_state.select(Some(i));
                    }
                    KeyCode::Char('k') => {
                        let i = match app.list_state.selected() {
                            Some(i) => {
                                if i == 0 {
                                    app.heatmaps.len() - 1
                                } else {
                                    i - 1
                                }
                            }
                            None => 0,
                        };
                        app.list_state.select(Some(i));
                    }
                    KeyCode::Char('+') => {
                        app.image_size_percent = (app.image_size_percent + 5).min(90);
                    }
                    KeyCode::Char('-') => {
                        app.image_size_percent = (app.image_size_percent - 5).max(10);
                    }
                    KeyCode::Char('b') => {
                        app.renderer.backend_type = match app.renderer.backend_type {
                            BackendType::Sixel => BackendType::Kitty,
                            BackendType::Kitty => BackendType::ITerm2,
                            BackendType::ITerm2 => BackendType::Ansi,
                            BackendType::Ansi => BackendType::Sixel,
                        };
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

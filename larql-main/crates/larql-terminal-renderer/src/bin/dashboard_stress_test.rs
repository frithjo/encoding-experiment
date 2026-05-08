use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use larql_terminal_renderer::{
    draw_frame, AtomicGraphicsBackend, GraphicsLayer, Image, ImageWidget, Renderer, Rgb,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use std::env;
use std::io;
use std::time::{Duration, Instant};

#[derive(PartialEq)]
enum Focus {
    Controls,
    LayerList,
    Image,
}

struct App {
    renderer: Renderer,
    heatmaps: Vec<Image>,
    list_state: ListState,
    image_size_percent: u16,
    zoom: f32,
    offset_x: f32,
    offset_y: f32,
    cursor_x: usize,
    cursor_y: usize,
    z_index: i32,
    focus: Focus,
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
            cursor_x: 0,
            cursor_y: 0,
            z_index,
            focus: Focus::Controls,
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
         Tab: Cycle Focus\n\
         j/k: Select Image (List Focus)\n\
         z/x: Zoom In/Out\n\
         w/a/s/d: Pan (Image Focus)\n\
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

        let img_style = if app.focus == Focus::Image {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let img_block = Block::default()
            .borders(Borders::ALL)
            .title(format!("Image {}", selected))
            .border_style(img_style);
        let img_inner = img_block.inner(content_chunks[0]);
        frame.render_widget(img_block, content_chunks[0]);

        let viewport_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // Image
                Constraint::Length(1), // Cursor readout
            ])
            .split(img_inner);

        // Ratatui Widget Pass
        use larql_terminal_renderer::BackendType;
        if app.renderer.backend_type == BackendType::Ansi {
            let widget = ImageWidget::new(img, &app.renderer)
                .zoom(app.zoom)
                .offset(app.offset_x, app.offset_y)
                .z_index(app.z_index);
            frame.render_widget(widget, viewport_chunks[0]);
        } else {
            frame.render_widget(Block::default(), viewport_chunks[0]);
        }

        // Graphics Layer Pass
        graphics_layer.add_with_view(
            img,
            viewport_chunks[0],
            app.offset_x,
            app.offset_y,
            app.zoom,
            app.z_index,
        );

        let default_rgb = Rgb::new(0, 0, 0);
        let cursor_color = img
            .get(
                app.cursor_x.min(img.width.saturating_sub(1)),
                app.cursor_y.min(img.height.saturating_sub(1)),
            )
            .unwrap_or(&default_rgb);
        let val_approx = cursor_color.r as f32 / 255.0; // rudimentary approximation for now
        let readout = Paragraph::new(format!(
            "Cursor: ({}, {}) | Val: {:.2} | Color: RGB({},{},{}) | Legend: [Low: Dark Blue -> High: Bright Red]",
            app.cursor_x, app.cursor_y, val_approx, cursor_color.r, cursor_color.g, cursor_color.b
        ));
        frame.render_widget(readout, viewport_chunks[1]);
    }

    // Bottom list for "scrolling" feel
    let items: Vec<ListItem> = app
        .heatmaps
        .iter()
        .enumerate()
        .map(|(i, _)| ListItem::new(format!("Heatmap Data Layer {}", i)))
        .collect();

    let list_style = if app.focus == Focus::LayerList {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Layers")
                .border_style(list_style),
        )
        .highlight_style(ratatui::style::Style::default().bg(ratatui::style::Color::DarkGray));

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
                    KeyCode::Tab => {
                        app.focus = match app.focus {
                            Focus::Controls => Focus::LayerList,
                            Focus::LayerList => Focus::Image,
                            Focus::Image => Focus::Controls,
                        };
                    }
                    KeyCode::Char('q') => break,
                    KeyCode::Char('+') => {
                        app.image_size_percent = (app.image_size_percent + 5).min(90);
                    }
                    KeyCode::Char('-') => {
                        app.image_size_percent = (app.image_size_percent - 5).max(10);
                    }
                    KeyCode::Char('z') => {
                        app.zoom *= 1.2;
                    }
                    KeyCode::Char('x') => {
                        app.zoom = (app.zoom / 1.2).max(1.0);
                    }
                    KeyCode::Char('b') => {
                        use larql_terminal_renderer::BackendType;
                        app.renderer.backend_type = match app.renderer.backend_type {
                            BackendType::Sixel => BackendType::Kitty,
                            BackendType::Kitty => BackendType::ITerm2,
                            BackendType::ITerm2 => BackendType::Ansi,
                            BackendType::Ansi => BackendType::Sixel,
                        };
                    }
                    KeyCode::Char('l') => {
                        app.z_index = if app.z_index == 1 { -1 } else { 1 };
                    }
                    _ => match app.focus {
                        Focus::Controls => {}
                        Focus::LayerList => match key.code {
                            KeyCode::Char('j') | KeyCode::Down => {
                                let i = match app.list_state.selected() {
                                    Some(i) => {
                                        if i >= app.heatmaps.len().saturating_sub(1) {
                                            0
                                        } else {
                                            i + 1
                                        }
                                    }
                                    None => 0,
                                };
                                app.list_state.select(Some(i));
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                let i = match app.list_state.selected() {
                                    Some(i) => {
                                        if i == 0 {
                                            app.heatmaps.len().saturating_sub(1)
                                        } else {
                                            i - 1
                                        }
                                    }
                                    None => 0,
                                };
                                app.list_state.select(Some(i));
                            }
                            _ => {}
                        },
                        Focus::Image => match key.code {
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
                            KeyCode::Up => {
                                app.cursor_y = app.cursor_y.saturating_sub(1);
                            }
                            KeyCode::Down => {
                                if let Some(selected) = app.list_state.selected() {
                                    if let Some(img) = app.heatmaps.get(selected) {
                                        app.cursor_y =
                                            (app.cursor_y + 1).min(img.height.saturating_sub(1));
                                    }
                                }
                            }
                            KeyCode::Left => {
                                app.cursor_x = app.cursor_x.saturating_sub(1);
                            }
                            KeyCode::Right => {
                                if let Some(selected) = app.list_state.selected() {
                                    if let Some(img) = app.heatmaps.get(selected) {
                                        app.cursor_x =
                                            (app.cursor_x + 1).min(img.width.saturating_sub(1));
                                    }
                                }
                            }
                            _ => {}
                        },
                    },
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

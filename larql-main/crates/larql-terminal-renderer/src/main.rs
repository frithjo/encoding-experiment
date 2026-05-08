use larql_terminal_renderer::{
    fit_to_pixels, get_terminal_size_with_pixel_query, resize_nearest, BackendType, FfmpegDecoder,
    Image, Renderer, VideoProbe,
};
use std::env;
use std::path::PathBuf;
use std::thread;
#[cfg(all(feature = "ratatui", feature = "crossterm"))]
use std::time::{Duration, Instant};

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!(
            "Usage: termimg <image_path> [--fit] [--width <w>] [--backend <sixel|kitty|iterm2|ansi>] [--interactive] [--video <path>] [--fps <n>] [--loop]"
        );
        return Ok(());
    }

    let mut image_path: Option<PathBuf> = None;
    let mut video_path: Option<PathBuf> = None;
    let mut fit = false;
    let mut interactive = false;
    let mut width: Option<usize> = None;
    let mut force_backend: Option<BackendType> = None;
    let mut fps_cap: Option<f32> = None;
    let mut loop_video = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--fit" => fit = true,
            "--interactive" => interactive = true,
            "--loop" => loop_video = true,
            "--width" => {
                if i + 1 < args.len() {
                    width = args[i + 1].parse().ok();
                    i += 1;
                }
            }
            "--fps" => {
                if i + 1 < args.len() {
                    fps_cap = args[i + 1].parse().ok();
                    i += 1;
                }
            }
            "--video" => {
                if i + 1 < args.len() {
                    video_path = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--backend" => {
                if i + 1 < args.len() {
                    match args[i + 1].as_str() {
                        "sixel" => force_backend = Some(BackendType::Sixel),
                        "kitty" => force_backend = Some(BackendType::Kitty),
                        "iterm2" => force_backend = Some(BackendType::ITerm2),
                        "ansi" => force_backend = Some(BackendType::Ansi),
                        _ => {}
                    }
                    i += 1;
                }
            }
            value if !value.starts_with("--") && image_path.is_none() => {
                image_path = Some(PathBuf::from(value));
            }
            _ => {}
        }
        i += 1;
    }

    let renderer = if let Some(bt) = force_backend {
        Renderer::new_with_backend(bt)
    } else {
        Renderer::new_auto()
    };

    if let Some(video_path) = video_path {
        return if interactive {
            run_interactive_video(&video_path, renderer, fps_cap, loop_video)
        } else {
            run_video(&video_path, renderer, fps_cap, loop_video)
        };
    }

    let path = image_path.ok_or("missing image path")?;
    let mut image = Image::from_path(path)?;

    if interactive {
        return run_interactive_image(image, renderer);
    }

    if fit {
        if let Some(ts) = get_terminal_size_with_pixel_query() {
            if ts.pixel_width > 0 {
                image = fit_to_pixels(&image, ts.pixel_width as usize, ts.pixel_height as usize);
            } else {
                // Cell-based fallback if pixels aren't detected
                let h = if matches!(renderer.backend_type, BackendType::Ansi) {
                    ts.rows as usize * 2
                } else {
                    ts.rows as usize * 20 // guess 20px per cell
                };
                image = fit_to_pixels(&image, ts.cols as usize * 10, h);
            }
        }
    } else if let Some(w_cells) = width {
        // Assume 10px per cell if width is given in cells
        let w_px = w_cells * 10;
        let h_px = (image.height as f32 * (w_px as f32 / image.width as f32)) as usize;
        image = resize_nearest(&image, w_px, h_px);
    }

    renderer.render(&image)?;

    Ok(())
}

#[cfg(all(feature = "ratatui", feature = "crossterm"))]
fn run_interactive_image(image: Image, mut renderer: Renderer) -> Result<(), String> {
    use crossterm::{
        event::{self, Event, KeyCode},
        execute,
        style::force_color_output,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use larql_terminal_renderer::{draw_frame, AtomicGraphicsBackend, ImageWidget};
    use ratatui::{
        layout::{Constraint, Direction, Layout},
        widgets::{Block, Borders, Paragraph},
        Terminal,
    };
    use std::io;
    use std::time::Duration;

    struct TerminalRestore {
        raw_enabled: bool,
        alternate_screen: bool,
    }

    impl TerminalRestore {
        fn new() -> Self {
            Self {
                raw_enabled: false,
                alternate_screen: false,
            }
        }

        fn mark_raw_enabled(&mut self) {
            self.raw_enabled = true;
        }

        fn mark_alternate_screen(&mut self) {
            self.alternate_screen = true;
        }
    }

    impl Drop for TerminalRestore {
        fn drop(&mut self) {
            if self.alternate_screen {
                let _ = execute!(io::stdout(), LeaveAlternateScreen);
            }
            if self.raw_enabled {
                let _ = disable_raw_mode();
            }
        }
    }

    let mut zoom = 1.0f32;
    let mut offset_x = 0.0f32;
    let mut offset_y = 0.0f32;
    let mut show_inspect = true;

    // This path is itself an image renderer; suppressing color would make it non-functional.
    force_color_output(true);

    let mut terminal_restore = TerminalRestore::new();
    enable_raw_mode().map_err(|e| e.to_string())?;
    terminal_restore.mark_raw_enabled();
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(|e| e.to_string())?;
    terminal_restore.mark_alternate_screen();
    let backend = AtomicGraphicsBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| e.to_string())?;
    renderer.clear_all_graphics()?;

    loop {
        draw_frame(&mut terminal, &renderer, |frame, graphics_layer| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3)])
                .split(frame.size());
            let image_area = chunks[0];
            let block = Block::default().borders(Borders::ALL).title("termimg");
            let inner = block.inner(image_area);
            frame.render_widget(block, image_area);
            frame.render_widget(
                ImageWidget::new(&image, &renderer)
                    .zoom(zoom)
                    .offset(offset_x, offset_y)
                    .z_index(-1),
                inner,
            );
            graphics_layer.add_with_view(&image, inner, offset_x, offset_y, zoom, -1);

            let help = if show_inspect {
                format!(
                        "backend={:?} zoom={:.2} pan=({:.2},{:.2}) | arrows pan | +/- zoom | b backend | i inspect | q quit",
                        renderer.backend_type, zoom, offset_x, offset_y
                    )
            } else {
                "arrows pan | +/- zoom | b backend | i inspect | q quit".to_string()
            };
            frame.render_widget(
                Paragraph::new(help).block(Block::default().borders(Borders::ALL)),
                chunks[1],
            );
        })?;

        if event::poll(Duration::from_millis(100)).map_err(|e| e.to_string())? {
            if let Event::Key(key) = event::read().map_err(|e| e.to_string())? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('+') | KeyCode::Char('=') => zoom = (zoom * 1.25).min(16.0),
                    KeyCode::Char('-') => zoom = (zoom / 1.25).max(1.0),
                    KeyCode::Left => offset_x = (offset_x - 0.05).max(0.0),
                    KeyCode::Right => offset_x = (offset_x + 0.05).min(1.0),
                    KeyCode::Up => offset_y = (offset_y - 0.05).max(0.0),
                    KeyCode::Down => offset_y = (offset_y + 0.05).min(1.0),
                    KeyCode::Char('i') => show_inspect = !show_inspect,
                    KeyCode::Char('b') => {
                        renderer = Renderer::new_with_backend(match renderer.backend_type {
                            BackendType::Ansi => BackendType::Sixel,
                            BackendType::Sixel => BackendType::Kitty,
                            BackendType::Kitty => BackendType::ITerm2,
                            BackendType::ITerm2 => BackendType::Ansi,
                        });
                        renderer.clear_all_graphics()?;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

#[cfg(not(all(feature = "ratatui", feature = "crossterm")))]
fn run_interactive_image(_image: Image, _renderer: Renderer) -> Result<(), String> {
    Err("termimg --interactive requires ratatui and crossterm features".to_string())
}

fn run_video(
    path: &std::path::Path,
    renderer: Renderer,
    fps_cap: Option<f32>,
    loop_video: bool,
) -> Result<(), String> {
    let probe = VideoProbe::open(path)?;
    let mut decoder = FfmpegDecoder::spawn(path, probe, fps_cap)?;
    let frame_duration = decoder.frame_duration();

    loop {
        match decoder.next_frame()? {
            Some(frame) => {
                print!("\x1b[H\x1b[2J");
                if renderer.backend_type != BackendType::Ansi {
                    renderer.clear_all_graphics()?;
                }
                renderer.render(&frame.image)?;
                thread::sleep(frame_duration);
            }
            None if loop_video => decoder.restart()?,
            None => break,
        }
    }

    Ok(())
}

#[cfg(all(feature = "ratatui", feature = "crossterm"))]
fn run_interactive_video(
    path: &std::path::Path,
    mut renderer: Renderer,
    fps_cap: Option<f32>,
    loop_video: bool,
) -> Result<(), String> {
    use crossterm::{
        event::{self, Event, KeyCode},
        execute,
        style::force_color_output,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use larql_terminal_renderer::{draw_frame, AtomicGraphicsBackend, ImageWidget};
    use ratatui::{
        layout::{Constraint, Direction, Layout},
        widgets::{Block, Borders, Paragraph},
        Terminal,
    };
    use std::io;

    struct TerminalRestore {
        raw_enabled: bool,
        alternate_screen: bool,
    }

    impl TerminalRestore {
        fn new() -> Self {
            Self {
                raw_enabled: false,
                alternate_screen: false,
            }
        }
    }

    impl Drop for TerminalRestore {
        fn drop(&mut self) {
            if self.alternate_screen {
                let _ = execute!(io::stdout(), LeaveAlternateScreen);
            }
            if self.raw_enabled {
                let _ = disable_raw_mode();
            }
        }
    }

    let probe = VideoProbe::open(path)?;
    let mut decoder = FfmpegDecoder::spawn(path, probe, fps_cap)?;
    let frame_duration = decoder.frame_duration();
    let mut current = decoder
        .next_frame()?
        .ok_or("video contains no decodable frames")?;
    let mut paused = false;
    let mut terminal_restore = TerminalRestore::new();

    force_color_output(true);
    enable_raw_mode().map_err(|e| e.to_string())?;
    terminal_restore.raw_enabled = true;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(|e| e.to_string())?;
    terminal_restore.alternate_screen = true;
    let backend = AtomicGraphicsBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| e.to_string())?;
    renderer.clear_all_graphics()?;
    let mut next_deadline = Instant::now() + frame_duration;

    loop {
        draw_frame(&mut terminal, &renderer, |frame, graphics_layer| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3)])
                .split(frame.size());
            let image_area = chunks[0];
            let block = Block::default()
                .borders(Borders::ALL)
                .title("termimg video");
            let inner = block.inner(image_area);
            frame.render_widget(block, image_area);
            frame.render_widget(
                ImageWidget::new(&current.image, &renderer).z_index(-1),
                inner,
            );
            graphics_layer.add_with_view(&current.image, inner, 0.0, 0.0, 1.0, -1);

            let help = format!(
                "backend={:?} frame={} pts={:.2}s paused={} | space pause | r restart | b backend | q quit",
                renderer.backend_type,
                current.index,
                current.pts.as_secs_f32(),
                paused
            );
            frame.render_widget(
                Paragraph::new(help).block(Block::default().borders(Borders::ALL)),
                chunks[1],
            );
        })?;

        let timeout = if paused {
            Duration::from_millis(250)
        } else {
            next_deadline.saturating_duration_since(Instant::now())
        };
        if event::poll(timeout).map_err(|e| e.to_string())? {
            if let Event::Key(key) = event::read().map_err(|e| e.to_string())? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char(' ') => {
                        paused = !paused;
                        if !paused {
                            next_deadline = Instant::now() + frame_duration;
                        }
                    }
                    KeyCode::Char('r') => {
                        decoder.restart()?;
                        current = decoder
                            .next_frame()?
                            .ok_or("video contains no decodable frames after restart")?;
                        next_deadline = Instant::now() + frame_duration;
                    }
                    KeyCode::Char('b') => {
                        renderer = Renderer::new_with_backend(match renderer.backend_type {
                            BackendType::Ansi => BackendType::Sixel,
                            BackendType::Sixel => BackendType::Kitty,
                            BackendType::Kitty => BackendType::ITerm2,
                            BackendType::ITerm2 => BackendType::Ansi,
                        });
                        renderer.clear_all_graphics()?;
                    }
                    _ => {}
                }
            }
        }

        if !paused && Instant::now() >= next_deadline {
            match decoder.next_frame()? {
                Some(frame) => current = frame,
                None if loop_video => {
                    decoder.restart()?;
                    current = decoder
                        .next_frame()?
                        .ok_or("video contains no decodable frames after loop restart")?;
                }
                None => break,
            }
            next_deadline += frame_duration;
        }
    }

    Ok(())
}

#[cfg(not(all(feature = "ratatui", feature = "crossterm")))]
fn run_interactive_video(
    _path: &std::path::Path,
    _renderer: Renderer,
    _fps_cap: Option<f32>,
    _loop_video: bool,
) -> Result<(), String> {
    Err("termimg --interactive requires ratatui and crossterm features".to_string())
}

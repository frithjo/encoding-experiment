use larql_terminal_renderer::{
    fit_to_pixels, get_terminal_size, resize_nearest, BackendType, Image, Renderer,
};
use std::env;

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Usage: termimg <image_path> [--fit] [--width <w>] [--backend <sixel|kitty|iterm2|ansi>]");
        return Ok(());
    }

    let path = &args[1];
    let mut fit = false;
    let mut width: Option<usize> = None;
    let mut force_backend: Option<BackendType> = None;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--fit" => fit = true,
            "--width" => {
                if i + 1 < args.len() {
                    width = args[i + 1].parse().ok();
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
            _ => {}
        }
        i += 1;
    }

    let mut image = Image::from_path(path)?;

    let renderer = if let Some(bt) = force_backend {
        Renderer::new_with_backend(bt)
    } else {
        Renderer::new_auto()
    };

    if fit {
        if let Some(ts) = get_terminal_size() {
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

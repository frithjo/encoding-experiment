use larql_terminal_renderer::{
    dither_floyd_steinberg, fit_to_pixels, quantize, Image, KittyRenderer, RenderBackend,
    SixelRenderer,
};
use std::time::Instant;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let width = parse_arg(&args, "--width").unwrap_or(1920);
    let height = parse_arg(&args, "--height").unwrap_or(1080);
    let target_width = parse_arg(&args, "--target-width").unwrap_or(960);
    let target_height = parse_arg(&args, "--target-height").unwrap_or(540);

    let image = gradient_image(width, height);
    emit_case("input", width, height, 0, 0, 0);

    let (resized, resize_ms) = timed(|| fit_to_pixels(&image, target_width, target_height));
    emit_case("resize_fit", resized.width, resized.height, resize_ms, 0, 0);

    let (palette, quantize_ms) = timed(|| quantize(&resized, 256));
    emit_case(
        "quantize_oklab_median_cut",
        resized.width,
        resized.height,
        quantize_ms,
        palette.colors.len(),
        0,
    );

    let (indices, dither_ms) = timed(|| dither_floyd_steinberg(&resized, &palette));
    emit_case(
        "dither_floyd_steinberg",
        resized.width,
        resized.height,
        dither_ms,
        palette.colors.len(),
        indices.len(),
    );

    let sixel = SixelRenderer;
    SixelRenderer::clear_cache();
    let (first_sixel, sixel_miss_ms) = timed(|| sixel.encode(&resized));
    let first_sixel = first_sixel?;
    emit_case(
        "sixel_encode_cache_miss",
        resized.width,
        resized.height,
        sixel_miss_ms,
        palette.colors.len(),
        first_sixel.len(),
    );

    let (second_sixel, sixel_hit_ms) = timed(|| sixel.encode(&resized));
    let second_sixel = second_sixel?;
    emit_case(
        "sixel_encode_cache_hit",
        resized.width,
        resized.height,
        sixel_hit_ms,
        palette.colors.len(),
        second_sixel.len(),
    );

    let kitty = KittyRenderer;
    let (kitty_command, kitty_ms) = timed(|| kitty.render_command_at(&resized, 0, 0, -1));
    emit_case(
        "kitty_command_upload_or_place",
        resized.width,
        resized.height,
        kitty_ms,
        0,
        kitty_command?.len(),
    );

    Ok(())
}

fn parse_arg(args: &[String], name: &str) -> Option<usize> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .and_then(|pair| pair[1].parse().ok())
}

fn timed<T>(f: impl FnOnce() -> T) -> (T, u128) {
    let start = Instant::now();
    let value = f();
    (value, start.elapsed().as_millis())
}

fn emit_case(
    name: &str,
    width: usize,
    height: usize,
    elapsed_ms: u128,
    palette_or_colors: usize,
    bytes_or_items: usize,
) {
    println!(
        "{{\"case\":\"{}\",\"width\":{},\"height\":{},\"elapsed_ms\":{},\"rayon_threads\":{},\"palette_or_colors\":{},\"bytes_or_items\":{}}}",
        name,
        width,
        height,
        elapsed_ms,
        rayon::current_num_threads(),
        palette_or_colors,
        bytes_or_items
    );
}

fn gradient_image(width: usize, height: usize) -> Image {
    let mut rgb = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            rgb.push(((x * 255) / width.max(1)) as u8);
            rgb.push(((y * 255) / height.max(1)) as u8);
            rgb.push((((x + y) * 255) / (width + height).max(1)) as u8);
        }
    }
    Image::from_rgb(width, height, rgb)
}

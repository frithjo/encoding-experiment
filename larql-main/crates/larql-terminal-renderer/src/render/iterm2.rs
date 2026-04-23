use crate::image_buffer::Image;
use crate::render::protocol::RenderBackend;

pub struct ITerm2Renderer;

impl RenderBackend for ITerm2Renderer {
    fn render(&self, image: &Image) -> Result<(), String> {
        self.render_at(image, 0, 0, 0)
    }

    fn render_at(&self, image: &Image, x: u16, y: u16, _z_index: i32) -> Result<(), String> {
        // Convert our Image to PNG bytes
        #[cfg(any(feature = "decode-png", feature = "decode-jpeg"))]
        {
            use base64::{engine::general_purpose, Engine as _};
            use image::RgbImage;
            use std::io::{self, Cursor, Write};

            let mut stdout = io::stdout().lock();

            let mut rgb_img = RgbImage::new(image.width as u32, image.height as u32);
            for y in 0..image.height {
                for x in 0..image.width {
                    let p = image.get(x, y).unwrap();
                    rgb_img.put_pixel(x as u32, y as u32, image::Rgb([p.r, p.g, p.b]));
                }
            }

            let mut png_bytes = Vec::new();
            let mut cursor = Cursor::new(&mut png_bytes);
            rgb_img
                .write_to(&mut cursor, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;

            let b64_data = general_purpose::STANDARD.encode(&png_bytes);

            // Move cursor
            write!(stdout, "\x1b[{};{}H", y + 1, x + 1).map_err(|e| e.to_string())?;

            // iTerm2 inline image sequence
            // OSC 1337 ; File = [args] : [base64] ST
            write!(
                stdout,
                "\x1b]1337;File=width={};height={};inline=1;size={}:{}\x07",
                image.width,
                image.height,
                png_bytes.len(),
                b64_data
            )
            .map_err(|e| e.to_string())?;
            stdout.flush().map_err(|e| e.to_string())?;
            Ok(())
        }

        #[cfg(not(any(feature = "decode-png", feature = "decode-jpeg")))]
        {
            // Fallback to Sixel if we can't encode PNG
            crate::render::SixelRenderer.render_at(image, x, y, _z_index)
        }
    }
}

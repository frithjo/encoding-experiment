use crate::image_buffer::Image;
use crate::render::protocol::RenderBackend;

pub struct ITerm2Renderer;

impl RenderBackend for ITerm2Renderer {
    fn render_command(&self, image: &Image) -> Result<Vec<u8>, String> {
        self.render_command_at(image, 0, 0, 0)
    }

    fn render_command_at(
        &self,
        image: &Image,
        x: u16,
        y: u16,
        _z_index: i32,
    ) -> Result<Vec<u8>, String> {
        #[cfg(any(feature = "decode-png", feature = "decode-jpeg"))]
        {
            use base64::{engine::general_purpose, Engine as _};
            use image::RgbImage;
            use rayon::prelude::*;
            use std::io::Cursor;
            use std::io::Write;

            let rgb_bytes: Vec<u8> = image
                .pixels
                .par_iter()
                .flat_map_iter(|p| [p.r, p.g, p.b])
                .collect();
            let rgb_img = RgbImage::from_raw(image.width as u32, image.height as u32, rgb_bytes)
                .ok_or("Failed to create iTerm2 RGB image")?;

            let mut png_bytes = Vec::new();
            let mut cursor = Cursor::new(&mut png_bytes);
            rgb_img
                .write_to(&mut cursor, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;

            let b64_data = general_purpose::STANDARD.encode(&png_bytes);

            let mut output = Vec::new();
            write!(output, "\x1b[{};{}H", y + 1, x + 1).map_err(|e| e.to_string())?;

            write!(
                output,
                "\x1b]1337;File=width={};height={};inline=1;size={}:{}\x07",
                image.width,
                image.height,
                png_bytes.len(),
                b64_data
            )
            .map_err(|e| e.to_string())?;
            Ok(output)
        }

        #[cfg(not(any(feature = "decode-png", feature = "decode-jpeg")))]
        {
            crate::render::SixelRenderer.render_command_at(image, x, y, _z_index)
        }
    }
}

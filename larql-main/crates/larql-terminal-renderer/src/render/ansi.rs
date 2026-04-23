use crate::image_buffer::{Image, Rgb};
use crate::render::protocol::RenderBackend;
use std::io::{self, Write};

pub struct AnsiRenderer;

impl AnsiRenderer {
    fn render_string(image: &Image) -> Result<String, String> {
        let mut output = Vec::new();

        for y in (0..image.height).step_by(2) {
            for x in 0..image.width {
                let top = image.get(x, y).cloned().unwrap_or(Rgb::new(0, 0, 0));
                let bottom = if y + 1 < image.height {
                    image.get(x, y + 1).cloned().unwrap_or(Rgb::new(0, 0, 0))
                } else {
                    Rgb::new(0, 0, 0)
                };

                write!(
                    output,
                    "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
                    top.r, top.g, top.b, bottom.r, bottom.g, bottom.b
                )
                .map_err(|e| e.to_string())?;
            }
            write!(output, "\x1b[0m\n").map_err(|e| e.to_string())?;
        }

        String::from_utf8(output).map_err(|e| e.to_string())
    }
}

impl RenderBackend for AnsiRenderer {
    fn render(&self, image: &Image) -> Result<(), String> {
        let mut stdout = io::stdout().lock();
        write!(stdout, "{}", Self::render_string(image)?).map_err(|e| e.to_string())?;

        stdout.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn render_at(&self, image: &Image, x: u16, y: u16, _z_index: i32) -> Result<(), String> {
        // ANSI half-blocks always render at cursor, z-index not supported natively
        print!("\x1b[{};{}H", y + 1, x + 1);
        self.render(image)
    }
}

#[cfg(test)]
mod tests {
    use super::AnsiRenderer;
    use crate::image_buffer::{Image, Rgb};

    #[test]
    fn ansi_half_block_output_matches_top_and_bottom_pixels() {
        let mut image = Image::new(1, 2);
        image.set(0, 0, Rgb::new(10, 20, 30));
        image.set(0, 1, Rgb::new(40, 50, 60));

        let rendered = AnsiRenderer::render_string(&image).unwrap();
        assert_eq!(rendered, "\x1b[38;2;10;20;30m\x1b[48;2;40;50;60m▀\x1b[0m\n");
    }
}

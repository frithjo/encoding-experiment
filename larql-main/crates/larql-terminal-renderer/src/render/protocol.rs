use crate::image_buffer::Image;
use std::io::{self, Write};
use std::path::Path;

pub trait RenderBackend {
    fn render_command(&self, image: &Image) -> Result<Vec<u8>, String>;
    fn render_command_at(
        &self,
        image: &Image,
        x: u16,
        y: u16,
        _z_index: i32,
    ) -> Result<Vec<u8>, String> {
        let mut output = Vec::new();
        write!(output, "\x1b[{};{}H", y + 1, x + 1).map_err(|e| e.to_string())?;
        output.extend_from_slice(&self.render_command(image)?);
        Ok(output)
    }

    fn render(&self, image: &Image) -> Result<(), String> {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(&self.render_command(image)?)
            .map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn render_at(&self, image: &Image, x: u16, y: u16, _z_index: i32) -> Result<(), String> {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(&self.render_command_at(image, x, y, _z_index)?)
            .map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn render_file(&self, path: &Path) -> Result<(), String> {
        let image = Image::from_path(path)?;
        self.render(&image)
    }
}

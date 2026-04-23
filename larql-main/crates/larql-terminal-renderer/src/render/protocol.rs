use crate::image_buffer::Image;
use std::path::Path;

pub trait RenderBackend {
    fn render(&self, image: &Image) -> Result<(), String>;
    fn render_at(&self, image: &Image, x: u16, y: u16, _z_index: i32) -> Result<(), String> {
        // Default implementation: move cursor and render
        print!("\x1b[{};{}H", y + 1, x + 1);
        self.render(image)
    }
    fn render_file(&self, path: &Path) -> Result<(), String> {
        // Default implementation: load image and render
        let image = Image::from_path(path)?;
        self.render(&image)
    }
}

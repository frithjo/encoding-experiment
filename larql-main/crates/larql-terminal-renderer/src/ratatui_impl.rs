use crate::image_buffer::Image;
use crate::render::{BackendType, Renderer};
use crate::resize::{fit_to_pixels, resize_bilinear};
use crate::terminal::get_terminal_size;
#[cfg(feature = "ratatui")]
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

#[cfg(feature = "ratatui")]
pub struct ImageWidget<'a> {
    image: &'a Image,
    renderer: &'a Renderer,
    offset_x: f32,
    offset_y: f32,
    zoom: f32,
    z_index: i32,
}

#[cfg(feature = "ratatui")]
impl<'a> ImageWidget<'a> {
    pub fn new(image: &'a Image, renderer: &'a Renderer) -> Self {
        Self {
            image,
            renderer,
            offset_x: 0.0,
            offset_y: 0.0,
            zoom: 1.0,
            z_index: 0,
        }
    }

    pub fn z_index(mut self, z: i32) -> Self {
        self.z_index = z;
        self
    }

    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset_x = x;
        self.offset_y = y;
        self
    }

    pub fn zoom(mut self, zoom: f32) -> Self {
        self.zoom = zoom;
        self
    }

    fn backend_type(&self) -> BackendType {
        self.renderer.backend_type
    }

    fn get_derivative(&self) -> Image {
        if self.zoom <= 1.0 {
            self.image.clone_shallow()
        } else {
            let view_w = (self.image.width as f32 / self.zoom) as usize;
            let view_h = (self.image.height as f32 / self.zoom) as usize;
            let x = (self.offset_x * (self.image.width - view_w) as f32) as usize;
            let y = (self.offset_y * (self.image.height - view_h) as f32) as usize;
            self.image.crop(x, y, view_w, view_h)
        }
    }

    fn render_ansi_to_buffer(&self, area: Rect, buf: &mut Buffer) {
        use ratatui::style::Color;

        let derivative = self.get_derivative();

        // For ANSI half-blocks, we assume 1 cell width = 2 pixels height.
        let fitted = resize_bilinear(&derivative, area.width as usize, area.height as usize * 2);

        for y in (0..fitted.height).step_by(2) {
            for x in 0..fitted.width {
                if (area.left() as usize + x) < area.right() as usize
                    && (area.top() as usize + y / 2) < area.bottom() as usize
                {
                    let top = fitted.get(x, y).unwrap();
                    let bottom = if y + 1 < fitted.height {
                        fitted.get(x, y + 1).unwrap()
                    } else {
                        &crate::image_buffer::Rgb::new(0, 0, 0)
                    };

                    let cell = buf.get_mut(area.left() + (x as u16), area.top() + (y / 2) as u16);
                    cell.set_char('▀');
                    cell.set_fg(Color::Rgb(top.r, top.g, top.b));
                    cell.set_bg(Color::Rgb(bottom.r, bottom.g, bottom.b));
                }
            }
        }
    }
}

#[cfg(feature = "ratatui")]
impl<'a> Widget for ImageWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.backend_type() {
            BackendType::Ansi => {
                self.render_ansi_to_buffer(area, buf);
            }
            _ => {
                // If z-index >= 0 (foreground), mark area as skipped so Ratatui doesn't overwrite it.
                // If z-index < 0 (background), don't mark as skipped, allowing text to render on top.
                if self.z_index >= 0 {
                    for y in area.top()..area.bottom() {
                        for x in area.left()..area.right() {
                            buf.get_mut(x, y).set_skip(true);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(feature = "ratatui")]
pub struct GraphicsLayer<'a> {
    items: Vec<GraphicItem<'a>>,
    renderer: &'a Renderer,
}

#[cfg(feature = "ratatui")]
struct GraphicItem<'a> {
    image: &'a Image,
    area: Rect,
    offset_x: f32,
    offset_y: f32,
    zoom: f32,
    z_index: i32,
}

#[cfg(feature = "ratatui")]
impl<'a> GraphicsLayer<'a> {
    pub fn new(renderer: &'a Renderer) -> Self {
        Self {
            items: Vec::new(),
            renderer,
        }
    }

    pub fn add(&mut self, image: &'a Image, area: Rect) {
        self.items.push(GraphicItem {
            image,
            area,
            offset_x: 0.0,
            offset_y: 0.0,
            zoom: 1.0,
            z_index: 0,
        });
    }

    pub fn add_with_view(
        &mut self,
        image: &'a Image,
        area: Rect,
        offset_x: f32,
        offset_y: f32,
        zoom: f32,
        z_index: i32,
    ) {
        self.items.push(GraphicItem {
            image,
            area,
            offset_x,
            offset_y,
            zoom,
            z_index,
        });
    }

    pub fn flush(&self) -> Result<(), String> {
        if self.items.is_empty() {
            return Ok(());
        }

        let term_size = get_terminal_size().ok_or("Failed to get terminal size")?;
        let (cell_w_px, cell_h_px) = term_size.cell_pixel_size();

        for item in &self.items {
            match self.renderer.backend_type {
                BackendType::Ansi => continue,
                _ => {
                    let derivative = if item.zoom <= 1.0 {
                        item.image.clone_shallow()
                    } else {
                        let view_w = (item.image.width as f32 / item.zoom) as usize;
                        let view_h = (item.image.height as f32 / item.zoom) as usize;
                        let x = (item.offset_x * (item.image.width - view_w) as f32) as usize;
                        let y = (item.offset_y * (item.image.height - view_h) as f32) as usize;
                        item.image.crop(x, y, view_w, view_h)
                    };

                    let fitted = fit_to_pixels(
                        &derivative,
                        item.area.width as usize * cell_w_px as usize,
                        item.area.height as usize * cell_h_px as usize,
                    );

                    self.renderer.render_at(
                        &fitted,
                        item.area.left(),
                        item.area.top(),
                        item.z_index,
                    )?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ImageWidget;
    use crate::image_buffer::{Image, Rgb};
    use crate::render::{BackendType, Renderer};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use ratatui::widgets::Widget;

    #[test]
    fn ansi_widget_renders_expected_half_block_colors() {
        let renderer = Renderer::new_with_backend(BackendType::Ansi);
        let mut image = Image::new(1, 2);
        image.set(0, 0, Rgb::new(1, 2, 3));
        image.set(0, 1, Rgb::new(4, 5, 6));
        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);

        ImageWidget::new(&image, &renderer).render(area, &mut buf);

        let cell = buf.get(0, 0);
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Rgb(1, 2, 3));
        assert_eq!(cell.bg, Color::Rgb(4, 5, 6));
    }

    #[test]
    fn positive_z_index_marks_cells_skipped_for_graphics_backends() {
        let renderer = Renderer::new_with_backend(BackendType::Sixel);
        let image = Image::new(1, 1);
        let area = Rect::new(0, 0, 2, 1);
        let mut buf = Buffer::empty(area);

        ImageWidget::new(&image, &renderer)
            .z_index(1)
            .render(area, &mut buf);

        assert!(buf.get(0, 0).skip);
        assert!(buf.get(1, 0).skip);
    }

    #[test]
    fn negative_z_index_leaves_cells_unskipped_for_background_graphics() {
        let renderer = Renderer::new_with_backend(BackendType::Sixel);
        let image = Image::new(1, 1);
        let area = Rect::new(0, 0, 2, 1);
        let mut buf = Buffer::empty(area);

        ImageWidget::new(&image, &renderer)
            .z_index(-1)
            .render(area, &mut buf);

        assert!(!buf.get(0, 0).skip);
        assert!(!buf.get(1, 0).skip);
    }
}

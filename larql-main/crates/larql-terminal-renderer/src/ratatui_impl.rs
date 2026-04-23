use crate::image_buffer::Image;
use crate::render::{BackendType, Renderer};
use crate::resize::{fit_to_pixels, resize_bilinear};
use crate::terminal::get_terminal_size;
#[cfg(feature = "ratatui")]
use ratatui::{
    backend::{Backend, ClearType, CrosstermBackend, WindowSize},
    buffer::Buffer,
    layout::Rect,
    terminal::Frame,
    widgets::Widget,
    Terminal,
};
#[cfg(feature = "ratatui")]
use std::cell::RefCell;
#[cfg(feature = "ratatui")]
use std::io::{self, Write};

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
pub struct GraphicsLayer {
    _backend_type: BackendType,
}

#[cfg(feature = "ratatui")]
struct GraphicItem {
    image: Image,
    logical_area: Rect,
    visible_area: Rect,
    offset_x: f32,
    offset_y: f32,
    zoom: f32,
    z_index: i32,
}

#[cfg(feature = "ratatui")]
struct PendingGraphicsFrame {
    backend_type: BackendType,
    items: Vec<GraphicItem>,
    buffer: Option<Buffer>,
}

#[cfg(feature = "ratatui")]
thread_local! {
    static PENDING_GRAPHICS_FRAME: RefCell<Option<PendingGraphicsFrame>> = const { RefCell::new(None) };
}

#[cfg(feature = "ratatui")]
pub struct AtomicGraphicsBackend<W: Write> {
    inner: CrosstermBackend<W>,
}

#[cfg(feature = "ratatui")]
impl<W: Write> AtomicGraphicsBackend<W> {
    pub fn new(writer: W) -> Self {
        Self {
            inner: CrosstermBackend::new(writer),
        }
    }

    fn emit_pending_graphics(&mut self) -> io::Result<()> {
        let pending = PENDING_GRAPHICS_FRAME.with(|slot| slot.borrow_mut().take());
        let Some(pending) = pending else {
            return Ok(());
        };
        let Some(buffer) = pending.buffer.as_ref() else {
            return Ok(());
        };

        let frame_commands =
            build_frame_commands(pending.backend_type, &pending.items, buffer).map_err(io_other)?;
        self.inner.write_all(&frame_commands)?;
        Ok(())
    }
}

#[cfg(feature = "ratatui")]
impl<W: Write> Write for AtomicGraphicsBackend<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        std::io::Write::flush(&mut self.inner)
    }
}

#[cfg(feature = "ratatui")]
impl<W: Write> Backend for AtomicGraphicsBackend<W> {
    fn draw<'b, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'b ratatui::buffer::Cell)>,
    {
        self.inner.draw(content)?;
        self.emit_pending_graphics()
    }

    fn append_lines(&mut self, n: u16) -> io::Result<()> {
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor(&mut self) -> io::Result<(u16, u16)> {
        self.inner.get_cursor()
    }

    fn set_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
        self.inner.set_cursor(x, y)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Rect> {
        self.inner.size()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}

#[cfg(feature = "ratatui")]
pub fn draw_frame<W, F>(
    terminal: &mut Terminal<AtomicGraphicsBackend<W>>,
    renderer: &Renderer,
    f: F,
) -> Result<(), String>
where
    W: Write,
    F: FnOnce(&mut Frame, &mut GraphicsLayer),
{
    PENDING_GRAPHICS_FRAME.with(|slot| {
        *slot.borrow_mut() = Some(PendingGraphicsFrame {
            backend_type: renderer.backend_type,
            items: Vec::new(),
            buffer: None,
        });
    });

    let draw_result = terminal.draw(|frame| {
        let mut graphics_layer = GraphicsLayer::new(renderer);
        f(frame, &mut graphics_layer);
        PENDING_GRAPHICS_FRAME.with(|slot| {
            if let Some(pending) = slot.borrow_mut().as_mut() {
                pending.buffer = Some(frame.buffer_mut().clone());
            }
        });
    });

    if draw_result.is_err() {
        PENDING_GRAPHICS_FRAME.with(|slot| {
            slot.borrow_mut().take();
        });
    }

    draw_result.map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(feature = "ratatui")]
impl GraphicsLayer {
    pub fn new(renderer: &Renderer) -> Self {
        Self {
            _backend_type: renderer.backend_type,
        }
    }

    pub fn add(&mut self, image: &Image, area: Rect) {
        self.push(GraphicItem {
            image: image.clone_shallow(),
            logical_area: area,
            visible_area: area,
            offset_x: 0.0,
            offset_y: 0.0,
            zoom: 1.0,
            z_index: 0,
        });
    }

    pub fn add_with_view(
        &mut self,
        image: &Image,
        area: Rect,
        offset_x: f32,
        offset_y: f32,
        zoom: f32,
        z_index: i32,
    ) {
        self.push(GraphicItem {
            image: image.clone_shallow(),
            logical_area: area,
            visible_area: area,
            offset_x,
            offset_y,
            zoom,
            z_index,
        });
    }

    pub fn add_clipped(
        &mut self,
        image: &Image,
        logical_area: Rect,
        visible_area: Rect,
        offset_x: f32,
        offset_y: f32,
        zoom: f32,
        z_index: i32,
    ) {
        self.push(GraphicItem {
            image: image.clone_shallow(),
            logical_area,
            visible_area,
            offset_x,
            offset_y,
            zoom,
            z_index,
        });
    }

    fn push(&mut self, item: GraphicItem) {
        PENDING_GRAPHICS_FRAME.with(|slot| {
            if let Some(pending) = slot.borrow_mut().as_mut() {
                pending.items.push(item);
            }
        });
    }
}

#[cfg(feature = "ratatui")]
fn build_frame_commands(
    backend_type: BackendType,
    items: &[GraphicItem],
    buffer: &Buffer,
) -> Result<Vec<u8>, String> {
    if items.is_empty() || backend_type == BackendType::Ansi {
        return Ok(Vec::new());
    }

    let term_size = get_terminal_size().ok_or("Failed to get terminal size")?;
    let (cell_w_px, cell_h_px) = term_size.cell_pixel_size();
    let renderer = Renderer::new_with_backend(backend_type);
    let mut frame_commands = Vec::new();

    for item in items {
        let Some(visible) = intersect_rect(item.logical_area, item.visible_area) else {
            continue;
        };
        let derivative = viewed_image(&item.image, item.offset_x, item.offset_y, item.zoom);
        let fragments = if backend_type == BackendType::Sixel && item.z_index < 0 {
            subtract_occlusion(visible, buffer)
        } else {
            vec![visible]
        };

        for fragment in fragments {
            let cropped = crop_for_visible(&derivative, visible, fragment);
            let fitted = fit_to_pixels(
                &cropped,
                fragment.width as usize * cell_w_px as usize,
                fragment.height as usize * cell_h_px as usize,
            );
            frame_commands.extend_from_slice(&renderer.render_command_at(
                &fitted,
                fragment.left(),
                fragment.top(),
                item.z_index,
            )?);
        }
    }

    Ok(frame_commands)
}

#[cfg(feature = "ratatui")]
fn viewed_image(image: &Image, offset_x: f32, offset_y: f32, zoom: f32) -> Image {
    if zoom <= 1.0 {
        image.clone_shallow()
    } else {
        let view_w = (image.width as f32 / zoom) as usize;
        let view_h = (image.height as f32 / zoom) as usize;
        let x = (offset_x * (image.width - view_w) as f32) as usize;
        let y = (offset_y * (image.height - view_h) as f32) as usize;
        image.crop(x, y, view_w, view_h)
    }
}

#[cfg(feature = "ratatui")]
fn subtract_occlusion(area: Rect, buffer: &Buffer) -> Vec<Rect> {
    let mut runs_by_row = Vec::new();
    for y in area.top()..area.bottom() {
        let mut row_runs = Vec::new();
        let mut run_start = None;
        for x in area.left()..area.right() {
            let visible = !is_occluded(buffer.get(x, y));
            if visible && run_start.is_none() {
                run_start = Some(x);
            } else if !visible {
                if let Some(start) = run_start.take() {
                    row_runs.push((start, x));
                }
            }
        }
        if let Some(start) = run_start {
            row_runs.push((start, area.right()));
        }
        runs_by_row.push((y, row_runs));
    }

    let mut rects: Vec<Rect> = Vec::new();
    for (y, runs) in runs_by_row {
        for (start, end) in runs {
            if let Some(existing) = rects
                .iter_mut()
                .find(|rect| rect.left() == start && rect.right() == end && rect.bottom() == y)
            {
                existing.height += 1;
            } else {
                rects.push(Rect::new(start, y, end - start, 1));
            }
        }
    }
    rects
}

#[cfg(feature = "ratatui")]
fn is_occluded(cell: &ratatui::buffer::Cell) -> bool {
    cell.skip || cell.symbol() != " "
}

#[cfg(feature = "ratatui")]
fn io_other(err: String) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

#[cfg(feature = "ratatui")]
fn intersect_rect(a: Rect, b: Rect) -> Option<Rect> {
    let left = a.left().max(b.left());
    let top = a.top().max(b.top());
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());

    if right <= left || bottom <= top {
        None
    } else {
        Some(Rect::new(left, top, right - left, bottom - top))
    }
}

#[cfg(feature = "ratatui")]
fn crop_for_visible(image: &Image, logical_area: Rect, visible_area: Rect) -> Image {
    if logical_area == visible_area || logical_area.width == 0 || logical_area.height == 0 {
        return image.clone_shallow();
    }

    let left_frac = (visible_area.left().saturating_sub(logical_area.left())) as f32
        / logical_area.width as f32;
    let top_frac =
        (visible_area.top().saturating_sub(logical_area.top())) as f32 / logical_area.height as f32;
    let width_frac = visible_area.width as f32 / logical_area.width as f32;
    let height_frac = visible_area.height as f32 / logical_area.height as f32;

    let x = (left_frac * image.width as f32).floor() as usize;
    let y = (top_frac * image.height as f32).floor() as usize;
    let width = (width_frac * image.width as f32).ceil() as usize;
    let height = (height_frac * image.height as f32).ceil() as usize;

    image.crop(
        x.min(image.width),
        y.min(image.height),
        width.min(image.width.saturating_sub(x)).max(1),
        height.min(image.height.saturating_sub(y)).max(1),
    )
}

#[cfg(test)]
mod tests {
    use super::{crop_for_visible, subtract_occlusion, ImageWidget};
    use crate::image_buffer::{Image, Rgb};
    use crate::render::{BackendType, Renderer};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Style};
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

    #[test]
    fn crop_for_visible_maps_cell_clip_to_source_pixels() {
        let mut image = Image::new(4, 2);
        image.set(0, 0, Rgb::new(1, 0, 0));
        image.set(1, 0, Rgb::new(2, 0, 0));
        image.set(2, 0, Rgb::new(3, 0, 0));
        image.set(3, 0, Rgb::new(4, 0, 0));
        image.set(0, 1, Rgb::new(5, 0, 0));
        image.set(1, 1, Rgb::new(6, 0, 0));
        image.set(2, 1, Rgb::new(7, 0, 0));
        image.set(3, 1, Rgb::new(8, 0, 0));

        let cropped = crop_for_visible(&image, Rect::new(0, 0, 4, 2), Rect::new(1, 0, 2, 1));

        assert_eq!(cropped.width, 2);
        assert_eq!(cropped.height, 1);
        assert_eq!(cropped.get(0, 0), Some(&Rgb::new(2, 0, 0)));
        assert_eq!(cropped.get(1, 0), Some(&Rgb::new(3, 0, 0)));
    }

    #[test]
    fn reverse_clipping_splits_background_around_occluded_cells() {
        let area = Rect::new(0, 0, 4, 2);
        let mut buf = Buffer::empty(area);
        buf.get_mut(1, 0).set_char('X');
        buf.get_mut(2, 0).set_char('X');
        buf.get_mut(1, 1).set_char('X');
        buf.get_mut(2, 1).set_char('X');

        let fragments = subtract_occlusion(area, &buf);

        assert_eq!(fragments, vec![Rect::new(0, 0, 1, 2), Rect::new(3, 0, 1, 2)]);
    }

    #[test]
    fn reverse_clipping_ignores_style_only_empty_cells() {
        let area = Rect::new(0, 0, 4, 2);
        let mut buf = Buffer::empty(area);
        for x in area.left()..area.right() {
            buf.get_mut(x, 0).set_style(Style::default().fg(Color::Yellow));
        }

        assert_eq!(subtract_occlusion(area, &buf), vec![area]);
    }
}

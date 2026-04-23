use crate::image_buffer::Image;
use crate::render::cache::LruCache;
use crate::render::protocol::RenderBackend;
use base64::{engine::general_purpose, Engine as _};
use rayon::prelude::*;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref LOADED_IMAGES: Mutex<LruCache<(u64, usize, usize), u32>> = Mutex::new(LruCache::new(128));
}

static NEXT_KITTY_ID: AtomicU32 = AtomicU32::new(1);

pub struct KittyRenderer;

impl KittyRenderer {
    fn upload_command(image: &Image, kitty_id: u32) -> Result<String, String> {
        if let Some(png_bytes) = image.original_png_bytes() {
            return Self::upload_png_command(image.width, image.height, png_bytes, kitty_id);
        }

        Self::upload_rgb_command(image, kitty_id)
    }

    fn upload_png_command(
        width: usize,
        height: usize,
        png_bytes: &[u8],
        kitty_id: u32,
    ) -> Result<String, String> {
        let mut output = Vec::new();
        let b64_data = general_purpose::STANDARD.encode(png_bytes);
        let chunks = b64_data.as_bytes().chunks(4096);
        let num_chunks = chunks.len();

        for (i, chunk) in chunks.enumerate() {
            let m = if i == num_chunks - 1 { 0 } else { 1 };
            if i == 0 {
                write!(
                    output,
                    "\x1b_Ga=t,f=100,s={},v={},i={},m={};",
                    width, height, kitty_id, m
                )
                .map_err(|e| e.to_string())?;
            } else {
                write!(output, "\x1b_Gm={};", m).map_err(|e| e.to_string())?;
            }
            output.write_all(chunk).map_err(|e| e.to_string())?;
            write!(output, "\x1b\\").map_err(|e| e.to_string())?;
        }

        String::from_utf8(output).map_err(|e| e.to_string())
    }

    fn upload_rgb_command(image: &Image, kitty_id: u32) -> Result<String, String> {
        let mut output = Vec::new();

        let rgb_data: Vec<u8> = image
            .pixels
            .par_iter()
            .flat_map_iter(|p| [p.r, p.g, p.b])
            .collect();

        let b64_data = general_purpose::STANDARD.encode(&rgb_data);
        let chunks = b64_data.as_bytes().chunks(4096);
        let num_chunks = chunks.len();

        for (i, chunk) in chunks.enumerate() {
            let m = if i == num_chunks - 1 { 0 } else { 1 };
            if i == 0 {
                write!(
                    output,
                    "\x1b_Ga=t,f=24,s={},v={},i={},m={};",
                    image.width, image.height, kitty_id, m
                )
                .map_err(|e| e.to_string())?;
            } else {
                write!(output, "\x1b_Gm={};", m).map_err(|e| e.to_string())?;
            }
            output.write_all(chunk).map_err(|e| e.to_string())?;
            write!(output, "\x1b\\").map_err(|e| e.to_string())?;
        }

        String::from_utf8(output).map_err(|e| e.to_string())
    }

    fn delete_image_command(kitty_id: u32) -> String {
        format!("\x1b_Ga=d,i={}\x1b\\", kitty_id)
    }

    fn upload_image(&self, image: &Image) -> Result<(Option<String>, u32), String> {
        let mut loaded = LOADED_IMAGES.lock().unwrap();
        let key = (image.content_key, image.width, image.height);
        if let Some(kitty_id) = loaded.get(&key) {
            return Ok((None, kitty_id));
        }
        let kitty_id = NEXT_KITTY_ID.fetch_add(1, Ordering::Relaxed);

        let evicted = loaded.insert(key, kitty_id);
        drop(loaded);

        let mut command = String::new();
        if let Some((_, evicted_id)) = evicted {
            command.push_str(&Self::delete_image_command(evicted_id));
        }
        command.push_str(&Self::upload_command(image, kitty_id)?);
        Ok((Some(command), kitty_id))
    }

    fn place_command(image_id: u32, x_cell: u16, y_cell: u16, z_index: i32) -> String {
        format!(
            "\x1b[{};{}H\x1b_Ga=p,i={},z={}\x1b\\",
            y_cell + 1,
            x_cell + 1,
            image_id,
            z_index
        )
    }

    fn render_command(
        &self,
        image: &Image,
        x_cell: u16,
        y_cell: u16,
        z_index: i32,
    ) -> Result<String, String> {
        let mut output = String::new();
        let (upload, kitty_id) = self.upload_image(image)?;
        if let Some(upload) = upload {
            output.push_str(&upload);
        }
        output.push_str(&Self::place_command(kitty_id, x_cell, y_cell, z_index));
        Ok(output)
    }

    fn clear_all_images_command() -> String {
        "\x1b_Ga=d\x1b\\".to_string()
    }

    pub fn clear_all_images() -> Result<(), String> {
        let mut loaded = LOADED_IMAGES.lock().unwrap();
        loaded.clear();
        let mut stdout = io::stdout().lock();
        write!(stdout, "{}", Self::clear_all_images_command()).map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(test)]
    fn reset_for_tests() {
        LOADED_IMAGES.lock().unwrap().clear();
        NEXT_KITTY_ID.store(1, Ordering::Relaxed);
    }
}

impl RenderBackend for KittyRenderer {
    fn render_command(&self, image: &Image) -> Result<Vec<u8>, String> {
        Ok(self.render_command(image, 0, 0, 0)?.into_bytes())
    }

    fn render_command_at(
        &self,
        image: &Image,
        x: u16,
        y: u16,
        z_index: i32,
    ) -> Result<Vec<u8>, String> {
        Ok(self.render_command(image, x, y, z_index)?.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::KittyRenderer;
    use crate::image_buffer::{Image, Rgb};
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn one_pixel(r: u8, g: u8, b: u8) -> Image {
        let mut image = Image::new(1, 1);
        image.set(0, 0, Rgb::new(r, g, b));
        image
    }

    #[test]
    fn kitty_rerender_same_content_reuses_upload() {
        let _guard = TEST_LOCK.lock().unwrap();
        KittyRenderer::reset_for_tests();
        let renderer = KittyRenderer;
        let image = one_pixel(1, 2, 3);

        let first = renderer.render_command(&image, 0, 0, 0).unwrap();
        let second = renderer.render_command(&image, 4, 5, 7).unwrap();

        assert!(first.contains("a=t,f=24"));
        assert!(first.contains("a=p,i=1,z=0"));
        assert!(!second.contains("a=t,f=24"));
        assert_eq!(second, "\x1b[6;5H\x1b_Ga=p,i=1,z=7\x1b\\");
    }

    #[test]
    fn kitty_untouched_png_uses_png_passthrough() {
        let _guard = TEST_LOCK.lock().unwrap();
        KittyRenderer::reset_for_tests();
        let renderer = KittyRenderer;
        let png_bytes = vec![137, 80, 78, 71, 13, 10, 26, 10];
        let image = Image::from_png_bytes(1, 1, vec![1, 2, 3], png_bytes);

        let output = renderer.render_command(&image, 0, 0, 0).unwrap();

        assert!(output.contains("a=t,f=100"));
        assert!(output.contains("a=p,i=1,z=0"));
    }

    #[test]
    fn kitty_transformed_png_falls_back_to_rgb_upload() {
        let _guard = TEST_LOCK.lock().unwrap();
        KittyRenderer::reset_for_tests();
        let renderer = KittyRenderer;
        let image = Image::from_png_bytes(2, 1, vec![1, 2, 3, 4, 5, 6], vec![1, 2, 3, 4]);
        let cropped = image.crop(0, 0, 1, 1);

        let output = renderer.render_command(&cropped, 0, 0, 0).unwrap();

        assert!(output.contains("a=t,f=24"));
        assert!(!output.contains("a=t,f=100"));
    }

    #[test]
    fn kitty_same_size_different_content_gets_new_upload() {
        let _guard = TEST_LOCK.lock().unwrap();
        KittyRenderer::reset_for_tests();
        let renderer = KittyRenderer;
        let left = one_pixel(255, 0, 0);
        let right = one_pixel(0, 255, 0);

        let first = renderer.render_command(&left, 0, 0, 0).unwrap();
        let second = renderer.render_command(&right, 0, 0, 0).unwrap();

        assert!(first.contains("i=1"));
        assert!(second.contains("a=t,f=24"));
        assert!(second.contains("i=2"));
    }

    #[test]
    fn kitty_clear_forces_reupload() {
        let _guard = TEST_LOCK.lock().unwrap();
        KittyRenderer::reset_for_tests();
        let renderer = KittyRenderer;
        let image = one_pixel(9, 8, 7);

        let first = renderer.render_command(&image, 0, 0, 0).unwrap();
        assert!(first.contains("i=1"));

        KittyRenderer::reset_for_tests();
        let second = renderer.render_command(&image, 0, 0, 0).unwrap();

        assert!(second.contains("a=t,f=24"));
        assert!(second.contains("i=1"));
    }
}

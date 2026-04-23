use crate::dither::dither_floyd_steinberg;
use crate::image_buffer::Image;
use crate::quantize::quantize;
use crate::render::protocol::RenderBackend;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref SIXEL_CACHE: Mutex<HashMap<(u64, usize, usize), String>> = Mutex::new(HashMap::new());
}

pub struct SixelRenderer;

impl SixelRenderer {
    fn encode_band_payload(
        indices: &[usize],
        width: usize,
        height: usize,
        num_colors: usize,
    ) -> Result<String, String> {
        let mut output = Vec::new();

        let mut color_masks = vec![vec![0u8; width]; num_colors];
        let mut color_active = vec![false; num_colors];
        let mut active_indices = Vec::with_capacity(num_colors);

        for y_band in (0..height).step_by(6) {
            for &idx in &active_indices {
                let mask: &mut Vec<u8> = &mut color_masks[idx];
                mask.fill(0u8);
                color_active[idx] = false;
            }
            active_indices.clear();

            for bit in 0..6 {
                let y = y_band + bit;
                if y >= height {
                    break;
                }

                let row_offset = y * width;
                for x in 0..width {
                    let c = indices[row_offset + x];
                    color_masks[c][x] |= 1 << bit;
                    if !color_active[c] {
                        color_active[c] = true;
                        active_indices.push(c);
                    }
                }
            }

            for &color_idx in &active_indices {
                let band_data = &color_masks[color_idx];
                write!(output, "#{}", color_idx).map_err(|e| e.to_string())?;

                let mut x = 0;
                while x < width {
                    let current_mask = band_data[x];
                    let mut count = 1;
                    while x + count < width && band_data[x + count] == current_mask {
                        count += 1;
                    }
                    let sixel_char = (63u8 + current_mask) as char;

                    if count > 3 {
                        write!(output, "!{}{}", count, sixel_char).map_err(|e| e.to_string())?;
                    } else {
                        for _ in 0..count {
                            output.push(63u8 + current_mask);
                        }
                    }
                    x += count;
                }
                write!(output, "$").map_err(|e| e.to_string())?;
            }
            write!(output, "-").map_err(|e| e.to_string())?;
        }

        String::from_utf8(output).map_err(|e| e.to_string())
    }

    pub fn encode(&self, image: &Image) -> Result<String, String> {
        let key = (image.content_key, image.width, image.height);
        {
            let cache = SIXEL_CACHE.lock().unwrap();
            if let Some(encoded) = cache.get(&key) {
                return Ok(encoded.clone());
            }
        }

        let mut output = Vec::new();

        // 1. Quantize
        let palette = quantize(image, 256);

        // 2. Dither
        let indices = dither_floyd_steinberg(image, &palette);

        // 3. Sixel Header: DCS q
        write!(output, "\x1bP0;0;8q").map_err(|e| e.to_string())?;

        // 4. Set aspect ratio
        write!(output, "\"1;1").map_err(|e| e.to_string())?;

        // 5. Define Palette
        for (i, color) in palette.colors.iter().enumerate() {
            let r = (color.r as u32 * 100 / 255) as u8;
            let g = (color.g as u32 * 100 / 255) as u8;
            let b = (color.b as u32 * 100 / 255) as u8;
            write!(output, "#{};2;{};{};{}", i, r, g, b).map_err(|e| e.to_string())?;
        }

        // 6. Optimized Encoding: Single pass over each 6-pixel band
        output.extend_from_slice(
            Self::encode_band_payload(&indices, image.width, image.height, palette.colors.len())?
                .as_bytes(),
        );

        // 7. Sixel Footer: ST
        write!(output, "\x1b\\").map_err(|e| e.to_string())?;

        let encoded = String::from_utf8(output).map_err(|e| e.to_string())?;

        let mut cache = SIXEL_CACHE.lock().unwrap();
        if cache.len() > 100 {
            cache.clear();
        }
        cache.insert(key, encoded.clone());

        Ok(encoded)
    }

    pub fn clear_cache() {
        SIXEL_CACHE.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::SixelRenderer;
    use crate::image_buffer::{Image, Rgb};

    fn reference_band_payload(
        indices: &[usize],
        width: usize,
        height: usize,
        num_colors: usize,
    ) -> String {
        let mut out = String::new();

        for y_band in (0..height).step_by(6) {
            let mut active_indices = Vec::new();
            let mut seen = vec![false; num_colors];
            for bit in 0..6 {
                let y = y_band + bit;
                if y >= height {
                    break;
                }
                let row_offset = y * width;
                for x in 0..width {
                    let idx = indices[row_offset + x];
                    if !seen[idx] {
                        seen[idx] = true;
                        active_indices.push(idx);
                    }
                }
            }

            for color_idx in active_indices {
                let mut band = Vec::with_capacity(width);
                for x in 0..width {
                    let mut mask = 0u8;
                    for bit in 0..6 {
                        let y = y_band + bit;
                        if y >= height {
                            break;
                        }
                        if indices[y * width + x] == color_idx {
                            mask |= 1 << bit;
                        }
                    }
                    band.push(mask);
                }

                out.push('#');
                out.push_str(&color_idx.to_string());

                let mut x = 0;
                while x < width {
                    let current = band[x];
                    let mut count = 1;
                    while x + count < width && band[x + count] == current {
                        count += 1;
                    }

                    if count > 3 {
                        out.push('!');
                        out.push_str(&count.to_string());
                        out.push((63u8 + current) as char);
                    } else {
                        for _ in 0..count {
                            out.push((63u8 + current) as char);
                        }
                    }
                    x += count;
                }
                out.push('$');
            }
            out.push('-');
        }

        out
    }

    #[test]
    fn sixel_single_pixel_sets_first_bit_not_full_mask() {
        SixelRenderer::clear_cache();
        let renderer = SixelRenderer;

        let mut image = Image::new(1, 1);
        image.set(0, 0, Rgb::new(255, 0, 0));

        let encoded = renderer.encode(&image).unwrap();
        assert!(
            encoded.contains("#0@"),
            "expected one-bit sixel, got {encoded:?}"
        );
        assert!(
            !encoded.contains("#0?"),
            "unexpected full-mask sixel in {encoded:?}"
        );
    }

    #[test]
    fn sixel_cache_distinguishes_same_size_different_crops() {
        SixelRenderer::clear_cache();
        let renderer = SixelRenderer;

        let mut base = Image::new(3, 1);
        base.set(0, 0, Rgb::new(255, 0, 0));
        base.set(1, 0, Rgb::new(0, 255, 0));
        base.set(2, 0, Rgb::new(0, 0, 255));

        let left = base.crop(0, 0, 2, 1);
        let right = base.crop(1, 0, 2, 1);

        let left_encoded = renderer.encode(&left).unwrap();
        let right_encoded = renderer.encode(&right).unwrap();

        assert_ne!(
            left_encoded, right_encoded,
            "cache collapsed distinct crops"
        );
    }

    #[test]
    fn sixel_resets_band_masks_between_six_row_chunks() {
        SixelRenderer::clear_cache();
        let renderer = SixelRenderer;

        let mut image = Image::new(1, 7);
        image.set(0, 0, Rgb::new(255, 0, 0));
        image.set(0, 6, Rgb::new(255, 0, 0));

        let encoded = renderer.encode(&image).unwrap();
        assert_eq!(
            encoded.matches("@$").count(),
            2,
            "expected one-bit output in both bands, got {encoded:?}"
        );
    }

    #[test]
    fn sixel_band_payload_matches_reference_encoder() {
        let indices = vec![
            0, 1, 0, 1, 0, 1, 1, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 0, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0,
        ];

        let actual = SixelRenderer::encode_band_payload(&indices, 4, 7, 2).unwrap();
        let expected = reference_band_payload(&indices, 4, 7, 2);

        assert_eq!(actual, expected);
    }
}

impl RenderBackend for SixelRenderer {
    fn render(&self, image: &Image) -> Result<(), String> {
        let encoded = self.encode(image)?;
        let mut stdout = io::stdout().lock();
        write!(stdout, "{}", encoded).map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn render_at(&self, image: &Image, x: u16, y: u16, _z_index: i32) -> Result<(), String> {
        let encoded = self.encode(image)?;
        let mut stdout = io::stdout().lock();
        write!(stdout, "\x1b[{};{}H{}", y + 1, x + 1, encoded).map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
        Ok(())
    }
}

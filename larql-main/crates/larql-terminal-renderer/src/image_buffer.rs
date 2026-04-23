use rayon::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

static NEXT_IMAGE_ID: AtomicU32 = AtomicU32::new(1);

pub fn get_next_id() -> u32 {
    NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed)
}

use crate::oklab::Oklab;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn to_oklab(&self) -> Oklab {
        Oklab::from_rgb(self)
    }
}

pub struct Image {
    pub id: u32,
    pub revision: u32,
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Rgb>,
    pub(crate) content_key: u64,
    pub(crate) oklab_cache: Mutex<Option<Vec<Oklab>>>,
    pub(crate) source: ImageSourceKind,
}

#[derive(Clone)]
pub enum ImageSourceKind {
    Generated,
    OriginalPng {
        bytes: Arc<Vec<u8>>,
    },
}

impl Image {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            id: NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed),
            revision: 0,
            width,
            height,
            pixels: vec![Rgb::new(0, 0, 0); width * height],
            content_key: 0,
            oklab_cache: Mutex::new(None),
            source: ImageSourceKind::Generated,
        }
    }

    pub fn new_derivative(&self, width: usize, height: usize) -> Self {
        Self {
            id: self.id,
            revision: self.revision,
            width,
            height,
            pixels: vec![Rgb::new(0, 0, 0); width * height],
            content_key: 0,
            oklab_cache: Mutex::new(None),
            source: ImageSourceKind::Generated,
        }
    }

    pub fn from_rgb(width: usize, height: usize, rgb: Vec<u8>) -> Self {
        let mut pixels = Vec::with_capacity(width * height);
        for i in (0..rgb.len()).step_by(3) {
            if i + 2 < rgb.len() {
                pixels.push(Rgb::new(rgb[i], rgb[i + 1], rgb[i + 2]));
            }
        }
        let content_key = content_key_for_pixels(&pixels);
        Self {
            id: NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed),
            revision: 0,
            width,
            height,
            pixels,
            content_key,
            oklab_cache: Mutex::new(None),
            source: ImageSourceKind::Generated,
        }
    }

    pub fn from_png_bytes(width: usize, height: usize, rgb: Vec<u8>, png_bytes: Vec<u8>) -> Self {
        let mut image = Self::from_rgb(width, height, rgb);
        image.source = ImageSourceKind::OriginalPng {
            bytes: Arc::new(png_bytes),
        };
        image
    }

    pub fn get(&self, x: usize, y: usize) -> Option<&Rgb> {
        if x < self.width && y < self.height {
            Some(&self.pixels[y * self.width + x])
        } else {
            None
        }
    }

    pub fn ensure_oklab(&self) {
        let mut cache = self.oklab_cache.lock().unwrap();
        if cache.is_none() {
            *cache = Some(self.pixels.par_iter().map(|p| p.to_oklab()).collect());
        }
    }

    /// Sets both RGB and Oklab values for a pixel. Used for efficient resizing.
    pub fn set_both(&mut self, x: usize, y: usize, rgb: Rgb, lab: Oklab) {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            let old = self.pixels[idx];
            self.pixels[idx] = rgb;
            self.content_key ^= pixel_entry_key(idx, old);
            self.content_key ^= pixel_entry_key(idx, rgb);

            // Populate cache directly
            let cache = self.oklab_cache.get_mut().unwrap();
            if cache.is_none() {
                *cache = Some(vec![
                    Oklab {
                        l: 0.0,
                        a: 0.0,
                        b: 0.0
                    };
                    self.width * self.height
                ]);
            }
            cache.as_mut().unwrap()[idx] = lab;
        }
    }

    pub fn set(&mut self, x: usize, y: usize, color: Rgb) {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            let old = self.pixels[idx];
            self.pixels[idx] = color;
            self.content_key ^= pixel_entry_key(idx, old);
            self.content_key ^= pixel_entry_key(idx, color);
            self.revision += 1;
            *self.oklab_cache.get_mut().unwrap() = None;
        }
    }

    pub fn set_internal(&mut self, x: usize, y: usize, color: Rgb) {
        if x < self.width && y < self.height {
            let idx = y * self.width + x;
            let old = self.pixels[idx];
            self.pixels[idx] = color;
            self.content_key ^= pixel_entry_key(idx, old);
            self.content_key ^= pixel_entry_key(idx, color);
            // Only clear cache if it exists; if it's already None (new image), don't touch it.
            let cache = self.oklab_cache.get_mut().unwrap();
            if cache.is_some() {
                *cache = None;
            }
        }
    }

    pub fn clone_shallow(&self) -> Self {
        let cache = self.oklab_cache.lock().unwrap();
        Self {
            id: self.id,
            revision: self.revision,
            width: self.width,
            height: self.height,
            pixels: self.pixels.clone(),
            content_key: self.content_key,
            oklab_cache: Mutex::new(cache.clone()),
            source: self.source.clone(),
        }
    }

    pub fn crop(&self, x: usize, y: usize, width: usize, height: usize) -> Self {
        let mut new_img = self.new_derivative(width, height);
        for dy in 0..height {
            for dx in 0..width {
                if let Some(p) = self.get(x + dx, y + dy) {
                    new_img.set_internal(dx, dy, *p);
                }
            }
        }
        new_img
    }

    pub fn original_png_bytes(&self) -> Option<&[u8]> {
        match &self.source {
            ImageSourceKind::OriginalPng { bytes } => Some(bytes.as_slice()),
            ImageSourceKind::Generated => None,
        }
    }
}

pub(crate) fn content_key_for_pixels(pixels: &[Rgb]) -> u64 {
    pixels
        .iter()
        .enumerate()
        .fold(0, |acc, (idx, rgb)| acc ^ pixel_entry_key(idx, *rgb))
}

fn pixel_entry_key(idx: usize, rgb: Rgb) -> u64 {
    if rgb == Rgb::new(0, 0, 0) {
        return 0;
    }

    let color = ((rgb.r as u64) << 16) | ((rgb.g as u64) << 8) | (rgb.b as u64);
    let mut x = (idx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ color;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

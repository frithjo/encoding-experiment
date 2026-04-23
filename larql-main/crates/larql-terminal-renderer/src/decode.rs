use crate::image_buffer::Image;
#[cfg(any(feature = "decode-png", feature = "decode-jpeg"))]
use crate::image_buffer::{content_key_for_pixels, Rgb};
#[cfg(feature = "decode-png")]
use image::ImageFormat;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

#[cfg(any(feature = "decode-png", feature = "decode-jpeg"))]
use image::GenericImageView;

pub enum ImageSource {
    RawRgb {
        width: usize,
        height: usize,
        data: Vec<u8>,
    },
    #[cfg(any(feature = "decode-png", feature = "decode-jpeg"))]
    Path(String),
}

impl Image {
    #[cfg(any(feature = "decode-png", feature = "decode-jpeg"))]
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let p = path.as_ref();
        if let Some(ext) = p.extension() {
            if ext == "ppm" {
                return Self::from_ppm(p);
            }
        }

        #[cfg(feature = "decode-png")]
        if p.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("png"))
            .unwrap_or(false)
        {
            let png_bytes = std::fs::read(p).map_err(|e| e.to_string())?;
            let cursor = std::io::Cursor::new(&png_bytes);
            let reader = image::ImageReader::with_format(cursor, ImageFormat::Png);
            let img = reader.decode().map_err(|e| e.to_string())?;
            let (width, height) = img.dimensions();
            let rgb_img = img.to_rgb8();
            let rgb_bytes = rgb_img.into_raw();
            return Ok(Self::from_png_bytes(
                width as usize,
                height as usize,
                rgb_bytes,
                png_bytes,
            ));
        }

        let img = image::open(p).map_err(|e| e.to_string())?;
        let (width, height) = img.dimensions();
        let rgb_img = img.to_rgb8();

        let mut pixels = Vec::with_capacity(width as usize * height as usize);
        for pixel in rgb_img.pixels() {
            pixels.push(Rgb::new(pixel[0], pixel[1], pixel[2]));
        }

        let content_key = content_key_for_pixels(&pixels);

        Ok(Self {
            id: crate::image_buffer::get_next_id(),
            revision: 0,
            width: width as usize,
            height: height as usize,
            pixels,
            content_key,
            oklab_cache: std::sync::Mutex::new(None),
            source: crate::image_buffer::ImageSourceKind::Generated,
        })
    }

    #[cfg(not(any(feature = "decode-png", feature = "decode-jpeg")))]
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        Self::from_ppm(path)
    }

    pub fn from_ppm<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(file);

        let mut line = String::new();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        if line.trim() != "P6" {
            return Err("Only P6 PPM (binary) is supported".to_string());
        }

        loop {
            line.clear();
            reader.read_line(&mut line).map_err(|e| e.to_string())?;
            if !line.starts_with('#') {
                break;
            }
        }

        let dims: Vec<usize> = line
            .split_whitespace()
            .map(|s| s.parse().unwrap_or(0))
            .collect();
        if dims.len() < 2 {
            return Err("Invalid dimensions in PPM".to_string());
        }
        let (width, height) = (dims[0], dims[1]);

        line.clear();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        let max_val: u16 = line
            .trim()
            .parse()
            .map_err(|e| format!("Invalid maxval: {}", e))?;
        if max_val != 255 {
            return Err(format!("Only maxval 255 is supported, got {}", max_val));
        }

        let mut data = Vec::with_capacity(width * height * 3);
        reader.read_to_end(&mut data).map_err(|e| e.to_string())?;

        Ok(Self::from_rgb(width, height, data))
    }
}

use crate::image_buffer::Image;
use crate::oklab::Oklab;

pub fn resize_nearest(image: &Image, new_width: usize, new_height: usize) -> Image {
    let mut new_image = image.new_derivative(new_width, new_height);

    if image.width == 0 || image.height == 0 || new_width == 0 || new_height == 0 {
        return new_image;
    }

    let x_ratio = image.width as f32 / new_width as f32;
    let y_ratio = image.height as f32 / new_height as f32;

    for y in 0..new_height {
        for x in 0..new_width {
            let px = (x as f32 * x_ratio).floor() as usize;
            let py = (y as f32 * y_ratio).floor() as usize;

            let px = px.min(image.width - 1);
            let py = py.min(image.height - 1);

            if let Some(pixel) = image.get(px, py) {
                new_image.set_internal(x, y, *pixel);
            }
        }
    }

    new_image
}

pub fn resize_bilinear(image: &Image, new_width: usize, new_height: usize) -> Image {
    let mut new_image = image.new_derivative(new_width, new_height);

    if image.width < 2 || image.height < 2 || new_width == 0 || new_height == 0 {
        return resize_nearest(image, new_width, new_height);
    }

    // Pre-calculate Oklab pixels for the entire source image once.
    image.ensure_oklab();
    let src_cache_lock = image.oklab_cache.lock().unwrap();
    let src_oklab = src_cache_lock.as_ref().unwrap();

    let x_ratio = (image.width - 1) as f32 / new_width as f32;
    let y_ratio = (image.height - 1) as f32 / new_height as f32;

    for y in 0..new_height {
        for x in 0..new_width {
            let px = x_ratio * x as f32;
            let py = y_ratio * y as f32;

            let x_l = px.floor() as usize;
            let y_l = py.floor() as usize;
            let x_h = (x_l + 1).min(image.width - 1);
            let y_h = (y_l + 1).min(image.height - 1);

            let x_weight = px - x_l as f32;
            let y_weight = py - y_l as f32;

            let a = src_oklab[y_l * image.width + x_l];
            let b = src_oklab[y_l * image.width + x_h];
            let c = src_oklab[y_h * image.width + x_l];
            let d = src_oklab[y_h * image.width + x_h];

            let lab_top = Oklab::lerp(&a, &b, x_weight);
            let lab_bottom = Oklab::lerp(&c, &d, x_weight);
            let lab_final = Oklab::lerp(&lab_top, &lab_bottom, y_weight);

            // OPTIMIZATION: Store both values to avoid re-conversion later.
            new_image.set_both(x, y, lab_final.to_rgb(), lab_final);
        }
    }

    new_image
}

pub fn fit_to_terminal(
    image: &Image,
    term_width: usize,
    term_height: usize,
    is_half_block: bool,
) -> Image {
    let available_height = if is_half_block {
        term_height * 2
    } else {
        term_height
    };

    let width_ratio = term_width as f32 / image.width as f32;
    let height_ratio = available_height as f32 / image.height as f32;

    let ratio = width_ratio.min(height_ratio);

    let new_width = (image.width as f32 * ratio).floor() as usize;
    let new_height = (image.height as f32 * ratio).floor() as usize;

    resize_bilinear(image, new_width.max(1), new_height.max(1))
}

pub fn fit_to_pixels(image: &Image, width_px: usize, height_px: usize) -> Image {
    let width_ratio = width_px as f32 / image.width as f32;
    let height_ratio = height_px as f32 / image.height as f32;

    let ratio = width_ratio.min(height_ratio);

    let new_width = (image.width as f32 * ratio).floor() as usize;
    let new_height = (image.height as f32 * ratio).floor() as usize;

    resize_bilinear(image, new_width.max(1), new_height.max(1))
}

#[cfg(test)]
mod tests {
    use super::resize_bilinear;
    use crate::image_buffer::{Image, Rgb};
    use crate::oklab::Oklab;

    fn reference_resize_bilinear(image: &Image, new_width: usize, new_height: usize) -> Vec<Rgb> {
        image.ensure_oklab();
        let src_cache_lock = image.oklab_cache.lock().unwrap();
        let src_oklab = src_cache_lock.as_ref().unwrap();
        let x_ratio = (image.width - 1) as f32 / new_width as f32;
        let y_ratio = (image.height - 1) as f32 / new_height as f32;
        let mut out = Vec::with_capacity(new_width * new_height);

        for y in 0..new_height {
            for x in 0..new_width {
                let px = x_ratio * x as f32;
                let py = y_ratio * y as f32;

                let x_l = px.floor() as usize;
                let y_l = py.floor() as usize;
                let x_h = (x_l + 1).min(image.width - 1);
                let y_h = (y_l + 1).min(image.height - 1);

                let x_weight = px - x_l as f32;
                let y_weight = py - y_l as f32;

                let a = src_oklab[y_l * image.width + x_l];
                let b = src_oklab[y_l * image.width + x_h];
                let c = src_oklab[y_h * image.width + x_l];
                let d = src_oklab[y_h * image.width + x_h];

                let lab_top = Oklab::lerp(&a, &b, x_weight);
                let lab_bottom = Oklab::lerp(&c, &d, x_weight);
                let lab_final = Oklab::lerp(&lab_top, &lab_bottom, y_weight);
                out.push(lab_final.to_rgb());
            }
        }

        out
    }

    #[test]
    fn bilinear_resize_matches_reference_pixels() {
        let mut image = Image::new(2, 2);
        image.set(0, 0, Rgb::new(255, 0, 0));
        image.set(1, 0, Rgb::new(0, 255, 0));
        image.set(0, 1, Rgb::new(0, 0, 255));
        image.set(1, 1, Rgb::new(255, 255, 255));

        let resized = resize_bilinear(&image, 3, 3);
        let expected = reference_resize_bilinear(&image, 3, 3);

        assert_eq!(resized.pixels, expected);
    }

    #[test]
    fn bilinear_resize_populates_oklab_cache_for_all_pixels() {
        let mut image = Image::new(2, 2);
        image.set(0, 0, Rgb::new(255, 0, 0));
        image.set(1, 0, Rgb::new(0, 255, 0));
        image.set(0, 1, Rgb::new(0, 0, 255));
        image.set(1, 1, Rgb::new(255, 255, 255));

        let resized = resize_bilinear(&image, 3, 3);
        let cache = resized.oklab_cache.lock().unwrap();
        let labs = cache.as_ref().expect("resize should prefill oklab cache");

        assert_eq!(labs.len(), 9);
        for (idx, rgb) in resized.pixels.iter().enumerate() {
            let roundtrip = labs[idx].to_rgb();
            assert_eq!(&roundtrip, rgb);
        }
    }
}

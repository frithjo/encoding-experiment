use crate::image_buffer::Image;
use crate::oklab::Oklab;
use crate::quantize::Palette;

pub fn dither_floyd_steinberg(image: &Image, palette: &Palette) -> Vec<usize> {
    let mut indices = vec![0; image.width * image.height];

    // Copy image pixels to Oklab for error diffusion
    // Use the cache to avoid re-calculating Oklab conversions
    image.ensure_oklab();
    let cache_lock = image.oklab_cache.lock().unwrap();
    let oklab_pixels = cache_lock.as_ref().unwrap();

    let mut pixels: Vec<[f32; 3]> = oklab_pixels
        .iter()
        .map(|lab| [lab.l, lab.a, lab.b])
        .collect();

    for y in 0..image.height {
        for x in 0..image.width {
            let idx = y * image.width + x;
            let current = pixels[idx];

            let lab = Oklab {
                l: current[0],
                a: current[1],
                b: current[2],
            };
            let palette_idx = palette.find_closest_lab(&lab);
            indices[idx] = palette_idx;

            let p_lab = palette.oklab_colors[palette_idx];
            let err_l = lab.l - p_lab.l;
            let err_a = lab.a - p_lab.a;
            let err_b = lab.b - p_lab.b;

            // Distribute error:
            //   [ ] [7]
            //   [3] [5] [1]  / 16

            let err = [err_l, err_a, err_b];

            if x + 1 < image.width {
                distribute_error(&mut pixels, x + 1, y, image.width, err, 7.0 / 16.0);
            }
            if y + 1 < image.height {
                if x > 0 {
                    distribute_error(&mut pixels, x - 1, y + 1, image.width, err, 3.0 / 16.0);
                }
                distribute_error(&mut pixels, x, y + 1, image.width, err, 5.0 / 16.0);
                if x + 1 < image.width {
                    distribute_error(&mut pixels, x + 1, y + 1, image.width, err, 1.0 / 16.0);
                }
            }
        }
    }

    indices
}

fn distribute_error(
    pixels: &mut Vec<[f32; 3]>,
    x: usize,
    y: usize,
    width: usize,
    err: [f32; 3],
    weight: f32,
) {
    let idx = y * width + x;
    pixels[idx][0] += err[0] * weight;
    pixels[idx][1] += err[1] * weight;
    pixels[idx][2] += err[2] * weight;
}

#[cfg(test)]
mod tests {
    use super::dither_floyd_steinberg;
    use crate::image_buffer::{Image, Rgb};
    use crate::oklab::Oklab;
    use crate::quantize::Palette;

    fn brute_force_closest(target: &Oklab, palette: &Palette) -> usize {
        palette
            .oklab_colors
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                a.distance_sq(target)
                    .partial_cmp(&b.distance_sq(target))
                    .unwrap()
            })
            .map(|(idx, _)| idx)
            .unwrap()
    }

    fn reference_dither(image: &Image, palette: &Palette) -> Vec<usize> {
        image.ensure_oklab();
        let cache_lock = image.oklab_cache.lock().unwrap();
        let mut pixels: Vec<[f32; 3]> = cache_lock
            .as_ref()
            .unwrap()
            .iter()
            .map(|lab| [lab.l, lab.a, lab.b])
            .collect();
        let mut indices = vec![0; image.width * image.height];

        for y in 0..image.height {
            for x in 0..image.width {
                let idx = y * image.width + x;
                let current = pixels[idx];
                let lab = Oklab {
                    l: current[0],
                    a: current[1],
                    b: current[2],
                };
                let palette_idx = brute_force_closest(&lab, palette);
                indices[idx] = palette_idx;

                let p_lab = palette.oklab_colors[palette_idx];
                let err = [lab.l - p_lab.l, lab.a - p_lab.a, lab.b - p_lab.b];

                if x + 1 < image.width {
                    let idx = y * image.width + (x + 1);
                    pixels[idx][0] += err[0] * (7.0 / 16.0);
                    pixels[idx][1] += err[1] * (7.0 / 16.0);
                    pixels[idx][2] += err[2] * (7.0 / 16.0);
                }
                if y + 1 < image.height {
                    if x > 0 {
                        let idx = (y + 1) * image.width + (x - 1);
                        pixels[idx][0] += err[0] * (3.0 / 16.0);
                        pixels[idx][1] += err[1] * (3.0 / 16.0);
                        pixels[idx][2] += err[2] * (3.0 / 16.0);
                    }
                    let idx = (y + 1) * image.width + x;
                    pixels[idx][0] += err[0] * (5.0 / 16.0);
                    pixels[idx][1] += err[1] * (5.0 / 16.0);
                    pixels[idx][2] += err[2] * (5.0 / 16.0);
                    if x + 1 < image.width {
                        let idx = (y + 1) * image.width + (x + 1);
                        pixels[idx][0] += err[0] * (1.0 / 16.0);
                        pixels[idx][1] += err[1] * (1.0 / 16.0);
                        pixels[idx][2] += err[2] * (1.0 / 16.0);
                    }
                }
            }
        }

        indices
    }

    #[test]
    fn floyd_steinberg_matches_reference_on_small_gradient() {
        let palette = Palette::new(vec![
            Rgb::new(0, 0, 0),
            Rgb::new(255, 255, 255),
            Rgb::new(255, 0, 0),
        ]);
        let mut image = Image::new(3, 2);
        image.set(0, 0, Rgb::new(10, 10, 10));
        image.set(1, 0, Rgb::new(120, 120, 120));
        image.set(2, 0, Rgb::new(250, 250, 250));
        image.set(0, 1, Rgb::new(200, 10, 10));
        image.set(1, 1, Rgb::new(90, 80, 80));
        image.set(2, 1, Rgb::new(240, 200, 200));

        let actual = dither_floyd_steinberg(&image, &palette);
        let expected = reference_dither(&image, &palette);

        assert_eq!(actual, expected);
    }
}

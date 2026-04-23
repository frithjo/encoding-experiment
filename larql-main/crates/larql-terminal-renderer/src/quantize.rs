use crate::image_buffer::{Image, Rgb};
use crate::oklab::Oklab;

pub struct Palette {
    pub colors: Vec<Rgb>,
    pub oklab_colors: Vec<Oklab>,
}

impl Palette {
    pub fn new(colors: Vec<Rgb>) -> Self {
        // Sort by Luminance to allow early rejection in find_closest_lab
        let mut oklab_with_rgb: Vec<(Rgb, Oklab)> =
            colors.into_iter().map(|c| (c, c.to_oklab())).collect();
        oklab_with_rgb.sort_by(|a, b| a.1.l.partial_cmp(&b.1.l).unwrap());

        let (colors, oklab_colors): (Vec<Rgb>, Vec<Oklab>) = oklab_with_rgb.into_iter().unzip();
        Self {
            colors,
            oklab_colors,
        }
    }

    pub fn find_closest(&self, color: &Rgb) -> usize {
        self.find_closest_lab(&color.to_oklab())
    }

    pub fn find_closest_lab(&self, target: &Oklab) -> usize {
        let mut min_dist = f32::MAX;
        let mut closest_idx = 0;

        for (i, p_oklab) in self.oklab_colors.iter().enumerate() {
            let dl = target.l - p_oklab.l;
            let dl2 = dl * dl;

            if dl2 >= min_dist {
                if p_oklab.l > target.l {
                    break;
                }
                continue;
            }

            let da = target.a - p_oklab.a;
            let db = target.b - p_oklab.b;
            let dist = dl2 + da * da + db * db;
            if dist < min_dist {
                min_dist = dist;
                closest_idx = i;
            }
        }

        closest_idx
    }
}

/// Simple median cut quantization using Oklab for more perceptually accurate results
pub fn quantize(image: &Image, max_colors: usize) -> Palette {
    image.ensure_oklab();
    let cache_lock = image.oklab_cache.lock().unwrap();
    let oklab_pixels = cache_lock.as_ref().unwrap();

    let data: Vec<(Rgb, Oklab)> = image
        .pixels
        .iter()
        .zip(oklab_pixels.iter())
        .map(|(rgb, lab)| (*rgb, *lab))
        .collect();

    if data.is_empty() {
        return Palette::new(vec![Rgb::new(0, 0, 0)]);
    }

    let mut buckets: Vec<Vec<(Rgb, Oklab)>> = vec![data];

    while buckets.len() < max_colors {
        let mut split_idx = None;
        let mut max_spread = -1.0;

        for (i, bucket) in buckets.iter().enumerate() {
            if bucket.len() < 2 {
                continue;
            }

            let mut min_l: f32 = 1.0;
            let mut max_l: f32 = 0.0;
            let mut min_a: f32 = 1.0;
            let mut max_a: f32 = -1.0;
            let mut min_b: f32 = 1.0;
            let mut max_b: f32 = -1.0;

            for (_, lab) in bucket {
                min_l = min_l.min(lab.l);
                max_l = max_l.max(lab.l);
                min_a = min_a.min(lab.a);
                max_a = max_a.max(lab.a);
                min_b = min_b.min(lab.b);
                max_b = max_b.max(lab.b);
            }

            let spread: f32 = (max_l - min_l).max(max_a - min_a).max(max_b - min_b);
            if spread > max_spread {
                max_spread = spread;
                split_idx = Some(i);
            }
        }

        if let Some(idx) = split_idx {
            let mut bucket = buckets.remove(idx);

            let mut min_l: f32 = 1.0;
            let mut max_l: f32 = 0.0;
            let mut min_a: f32 = 1.0;
            let mut max_a: f32 = -1.0;
            let mut min_b: f32 = 1.0;
            let mut max_b: f32 = -1.0;
            for (_, lab) in &bucket {
                min_l = min_l.min(lab.l);
                max_l = max_l.max(lab.l);
                min_a = min_a.min(lab.a);
                max_a = max_a.max(lab.a);
                min_b = min_b.min(lab.b);
                max_b = max_b.max(lab.b);
            }

            let l_spread = max_l - min_l;
            let a_spread = max_a - min_a;
            let b_spread = max_b - min_b;

            if l_spread >= a_spread && l_spread >= b_spread {
                bucket.sort_by(|a, b| a.1.l.partial_cmp(&b.1.l).unwrap());
            } else if a_spread >= l_spread && a_spread >= b_spread {
                bucket.sort_by(|a, b| a.1.a.partial_cmp(&b.1.a).unwrap());
            } else {
                bucket.sort_by(|a, b| a.1.b.partial_cmp(&b.1.b).unwrap());
            }

            let median = bucket.len() / 2;
            let right = bucket.split_off(median);
            buckets.push(bucket);
            buckets.push(right);
        } else {
            break;
        }
    }

    let palette_colors = buckets
        .into_iter()
        .map(|bucket: Vec<(Rgb, Oklab)>| {
            let sum_r: u32 = bucket.iter().map(|(rgb, _)| rgb.r as u32).sum();
            let sum_g: u32 = bucket.iter().map(|(rgb, _)| rgb.g as u32).sum();
            let sum_b: u32 = bucket.iter().map(|(rgb, _)| rgb.b as u32).sum();
            let len = bucket.len() as u32;
            Rgb::new(
                (sum_r / len) as u8,
                (sum_g / len) as u8,
                (sum_b / len) as u8,
            )
        })
        .collect();

    Palette::new(palette_colors)
}

#[cfg(test)]
mod tests {
    use super::Palette;
    use crate::image_buffer::Rgb;
    use crate::oklab::Oklab;

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

    #[test]
    fn luminance_sorted_palette_matches_bruteforce_search() {
        let palette = Palette::new(vec![
            Rgb::new(250, 240, 20),
            Rgb::new(10, 10, 240),
            Rgb::new(240, 20, 20),
            Rgb::new(20, 240, 20),
            Rgb::new(128, 128, 128),
        ]);

        let targets = [
            Rgb::new(255, 32, 32),
            Rgb::new(32, 255, 32),
            Rgb::new(32, 32, 255),
            Rgb::new(200, 200, 30),
            Rgb::new(120, 120, 120),
            Rgb::new(100, 140, 80),
        ];

        for target in targets {
            let lab = target.to_oklab();
            assert_eq!(
                palette.find_closest_lab(&lab),
                brute_force_closest(&lab, &palette)
            );
        }
    }
}

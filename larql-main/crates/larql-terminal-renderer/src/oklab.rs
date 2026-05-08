use crate::image_buffer::Rgb;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

lazy_static::lazy_static! {
    static ref LINEAR_SRGB: [f32; 256] = {
        let mut table = [0.0; 256];
        for i in 0..256 {
            let f = i as f32 / 255.0;
            table[i] = if f <= 0.04045 {
                f / 12.92
            } else {
                ((f + 0.055) / 1.055).powf(2.4)
            };
        }
        table
    };

    static ref DELINEAR_TABLE: [u8; 4096] = {
        let mut table = [0; 4096];
        for i in 0..4096 {
            let f = i as f32 / 4095.0;
            let delin = if f <= 0.0031308 {
                f * 12.92
            } else {
                1.055 * f.powf(1.0 / 2.4) - 0.055
            };
            table[i] = (delin * 255.0).clamp(0.0, 255.0) as u8;
        }
        table
    };
}

impl Oklab {
    pub fn from_rgb(rgb: &Rgb) -> Self {
        let r = LINEAR_SRGB[rgb.r as usize];
        let g = LINEAR_SRGB[rgb.g as usize];
        let b = LINEAR_SRGB[rgb.b as usize];

        let l_lms = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
        let m_lms = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
        let s_lms = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;

        let l_prime = l_lms.cbrt();
        let m_prime = m_lms.cbrt();
        let s_prime = s_lms.cbrt();

        Self {
            l: 0.2104542553 * l_prime + 0.7936177850 * m_prime - 0.0040720468 * s_prime,
            a: 1.9779984951 * l_prime - 2.4285922050 * m_prime + 0.4505937099 * s_prime,
            b: 0.0259040371 * l_prime + 0.7827717662 * m_prime - 0.8086757660 * s_prime,
        }
    }

    pub fn to_rgb(&self) -> Rgb {
        let l_prime = self.l + 0.3963377774 * self.a + 0.2158037573 * self.b;
        let m_prime = self.l - 0.1055613458 * self.a - 0.0638541728 * self.b;
        let s_prime = self.l - 0.0894841775 * self.a - 1.2914855480 * self.b;

        let l_lms = l_prime * l_prime * l_prime;
        let m_lms = m_prime * m_prime * m_prime;
        let s_lms = s_prime * s_prime * s_prime;

        let r = 4.0767416621 * l_lms - 3.3077115913 * m_lms + 0.2309699292 * s_lms;
        let g = -1.2684380046 * l_lms + 2.6097574011 * m_lms - 0.3413193965 * s_lms;
        let b = -0.0041960863 * l_lms - 0.7034186147 * m_lms + 1.7076147010 * s_lms;

        Rgb::new(
            fast_delinearize(r),
            fast_delinearize(g),
            fast_delinearize(b),
        )
    }

    pub fn distance_sq(&self, other: &Self) -> f32 {
        let dl = self.l - other.l;
        let da = self.a - other.a;
        let db = self.b - other.b;
        dl * dl + da * da + db * db
    }

    pub fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        Self {
            l: a.l * (1.0 - t) + b.l * t,
            a: a.a * (1.0 - t) + b.a * t,
            b: a.b * (1.0 - t) + b.b * t,
        }
    }
}

#[inline]
fn fast_delinearize(f: f32) -> u8 {
    let idx = (f.clamp(0.0, 1.0) * 4095.0) as usize;
    DELINEAR_TABLE[idx]
}

#[cfg(test)]
mod tests {
    use super::Oklab;
    use crate::image_buffer::Rgb;

    fn assert_channel_close(actual: u8, expected: u8) {
        let delta = actual.abs_diff(expected);
        assert!(
            delta <= 1,
            "channel drift too large: actual={actual} expected={expected}"
        );
    }

    #[test]
    fn rgb_oklab_round_trip_stays_within_one_lsb_on_key_samples() {
        let samples = [
            Rgb::new(0, 0, 0),
            Rgb::new(255, 255, 255),
            Rgb::new(255, 0, 0),
            Rgb::new(0, 255, 0),
            Rgb::new(0, 0, 255),
            Rgb::new(12, 34, 56),
            Rgb::new(18, 18, 18),
            Rgb::new(128, 64, 32),
            Rgb::new(254, 1, 127),
        ];

        for rgb in samples {
            let roundtrip = Oklab::from_rgb(&rgb).to_rgb();
            assert_channel_close(roundtrip.r, rgb.r);
            assert_channel_close(roundtrip.g, rgb.g);
            assert_channel_close(roundtrip.b, rgb.b);
        }
    }
}

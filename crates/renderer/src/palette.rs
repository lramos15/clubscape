//! The original 65536-entry HSL palette (`fh.ae`) built for a brightness (gamma) value.
//!
//! Colors are 16-bit HSL indices: `hue6 << 10 | sat3 << 7 | lum7`. The runtime builds
//! the table in double precision and shifts each channel through `pow(c/256, brightness)`.

use crate::chunk::Chunks;
use crate::error::RenderError;

#[derive(Clone)]
pub struct Palette {
    pub brightness: f32,
    pub rgb: Vec<i32>,
}

impl Palette {
    pub fn from_chunks(data: &[u8]) -> Result<Self, RenderError> {
        let chunks = Chunks::parse(data)?;
        let brightness = f32::from_bits(chunks.ints("BRGT")?[0] as u32);
        let rgb = chunks.ints("PLTE")?;
        if rgb.len() != 65536 {
            return Err(RenderError::InvalidAsset(format!(
                "palette has {} entries",
                rgb.len()
            )));
        }
        Ok(Self { brightness, rgb })
    }

    /// Exact port of the original palette builder (`fh.bk(brightness, 0, 512)`).
    pub fn build(brightness: f64) -> Self {
        let mut rgb = vec![0i32; 65536];
        let mut index = 0usize;
        for hs in 0..512i32 {
            let hue = f64::from(hs >> 3) / 64.0 + 0.0078125;
            let sat = f64::from(hs & 7) / 8.0 + 0.0625;
            for l in 0..128i32 {
                let lum = f64::from(l) / 128.0;
                let mut r = lum;
                let mut g = lum;
                let mut b = lum;
                if sat != 0.0 {
                    let q = if lum < 0.5 {
                        lum * (1.0 + sat)
                    } else {
                        lum + sat - lum * sat
                    };
                    let p = 2.0 * lum - q;
                    let mut hr = hue + 0.3333333333333333;
                    if hr > 1.0 {
                        hr -= 1.0;
                    }
                    let mut hb = hue - 0.3333333333333333;
                    if hb < 0.0 {
                        hb += 1.0;
                    }
                    let channel = |t: f64| -> f64 {
                        if 6.0 * t < 1.0 {
                            p + (q - p) * 6.0 * t
                        } else if 2.0 * t < 1.0 {
                            q
                        } else if 3.0 * t < 2.0 {
                            p + (q - p) * (0.6666666666666666 - t) * 6.0
                        } else {
                            p
                        }
                    };
                    r = channel(hr);
                    g = channel(hue);
                    b = channel(hb);
                }
                let ri = (r * 256.0) as i32;
                let gi = (g * 256.0) as i32;
                let bi = (b * 256.0) as i32;
                let mut value = (ri << 16) + (gi << 8) + bi;
                value = adjust_brightness(value, brightness);
                if value == 0 {
                    value = 1;
                }
                rgb[index] = value;
                index += 1;
            }
        }
        Self {
            brightness: brightness as f32,
            rgb,
        }
    }

    #[inline]
    pub fn lookup(&self, hsl: i32) -> i32 {
        self.rgb[(hsl & 0xffff) as usize]
    }
}

/// `fh.ag(rgb, brightness)`: per-channel `pow(c / 256, brightness) * 256` truncated.
pub fn adjust_brightness(rgb: i32, brightness: f64) -> i32 {
    let r = f64::from(rgb >> 16) / 256.0;
    let g = f64::from(rgb >> 8 & 0xff) / 256.0;
    let b = f64::from(rgb & 0xff) / 256.0;
    let r = r.powf(brightness);
    let g = g.powf(brightness);
    let b = b.powf(brightness);
    let ri = (r * 256.0) as i32;
    let gi = (g * 256.0) as i32;
    let bi = (b * 256.0) as i32;
    (ri << 16) + (gi << 8) + bi
}

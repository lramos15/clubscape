//! Original textures: 128x128 RGB texel arrays produced by the runtime's texture provider at
//! the approved brightness, with the source average color, opacity flag and scroll animation.

use crate::chunk::Chunks;
use crate::error::RenderError;

#[derive(Clone, Debug)]
pub struct Texture {
    pub id: i32,
    pub size: i32,
    /// `fu.ax`: average RGB used when the texture is unavailable or for low-detail fills.
    pub average_rgb: i32,
    /// `fu.ac`: when true the scanline writes every texel; when false texel 0 is skipped.
    pub opaque: bool,
    /// `fu.aa`: 0 none, 1/2 vertical scroll, 3/4 horizontal scroll.
    pub animation_direction: i32,
    /// `fu.ao`: texels scrolled per client cycle.
    pub animation_speed: i32,
    pub pixels: Vec<i32>,
}

impl Texture {
    pub fn from_chunks(data: &[u8]) -> Result<Self, RenderError> {
        let chunks = Chunks::parse(data)?;
        let header = chunks.ints("TXHD")?;
        if header.len() < 6 {
            return Err(RenderError::Format("texture header".into()));
        }
        let pixels = chunks.ints("TXPX")?;
        let size = header[1];
        if size != 128 || pixels.len() != (size * size) as usize {
            return Err(RenderError::InvalidAsset(format!(
                "texture {} size {}",
                header[0],
                pixels.len()
            )));
        }
        Ok(Self {
            id: header[0],
            size,
            average_rgb: header[2],
            opaque: header[3] != 0,
            animation_direction: header[4],
            animation_speed: header[5],
            pixels,
        })
    }

    /// Exact port of `fu.ab(cycles)`: scrolls the texel array by direction/speed.
    pub fn animate(&mut self, cycles: i32) {
        if !(1..=4).contains(&self.animation_direction) {
            return;
        }
        let len = self.pixels.len();
        let row: usize = if len == 4096 { 64 } else { 128 };
        let mut out = vec![0i32; len];
        // Original int arithmetic: n5 = row * cycles * speed (wrapping), masked into range.
        let base = cycles.wrapping_mul(self.animation_speed);
        if self.animation_direction == 1 || self.animation_direction == 2 {
            let mut shift = (row as i32).wrapping_mul(base);
            if self.animation_direction == 1 {
                shift = shift.wrapping_neg();
            }
            let mask = len as i32 - 1;
            for (i, slot) in out.iter_mut().enumerate() {
                *slot = self.pixels[((i as i32).wrapping_add(shift) & mask) as usize];
            }
        } else {
            let mut shift = base;
            if self.animation_direction == 3 {
                shift = shift.wrapping_neg();
            }
            let mask = row as i32 - 1;
            for y in (0..len).step_by(row) {
                for x in 0..row {
                    out[y + x] = self.pixels[y + ((x as i32).wrapping_add(shift) & mask) as usize];
                }
            }
        }
        self.pixels = out;
    }
}

/// Loaded textures keyed by original texture id; implements the rasterizer's texture source.
#[derive(Default, Clone)]
pub struct TextureSet {
    textures: std::collections::HashMap<i32, Texture>,
}

impl TextureSet {
    /// `ec.ab`: the texture's source average colour, 0 for an unknown texture (the original
    /// returns 0 for a texture it has not loaded).
    pub fn average_rgb(&self, id: i32) -> i32 {
        self.textures.get(&id).map(|t| t.average_rgb).unwrap_or(0)
    }

    pub fn insert(&mut self, texture: Texture) {
        self.textures.insert(texture.id, texture);
    }

    pub fn get(&self, id: i32) -> Option<&Texture> {
        self.textures.get(&id)
    }

    pub fn ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.textures.keys().copied()
    }

    pub fn len(&self) -> usize {
        self.textures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    /// Advances every animated texture by `cycles` client cycles (`ec.an`).
    pub fn animate(&mut self, cycles: i32) {
        for texture in self.textures.values_mut() {
            texture.animate(cycles);
        }
    }
}

impl crate::raster::software::TextureSource for TextureSet {
    fn texels(&self, id: i32) -> Option<&[i32]> {
        self.textures.get(&id).map(|t| t.pixels.as_slice())
    }

    fn opaque(&self, id: i32) -> bool {
        self.textures.get(&id).is_some_and(|t| t.opaque)
    }

    fn average(&self, id: i32) -> i32 {
        self.average_rgb(id)
    }
}

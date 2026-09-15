//! CPU packing of the triangle stream into the GPU layout plus 16x8 pixel binning.

use crate::raster::software::merge_texture_shade;
use crate::raster::{Fill, RasterState, Tri};
use crate::texture::TextureSet;

pub const BIN_W: i32 = 16;
pub const BIN_H: i32 = 8;
pub const TRI_STRIDE: usize = 20;

pub const KIND_GOURAUD: i32 = 0;
pub const KIND_FLAT: i32 = 1;
pub const KIND_TEX_MODEL: i32 = 2;
pub const KIND_TEX_TILE: i32 = 3;

#[derive(Default, Debug, Clone)]
pub struct PackedFrame {
    /// `TRI_STRIDE` i32 per triangle.
    pub tris: Vec<i32>,
    /// `bins_x * bins_y + 1` offsets into `bin_tris`.
    pub bin_offsets: Vec<u32>,
    pub bin_tris: Vec<u32>,
    pub bins_x: u32,
    pub bins_y: u32,
    pub triangle_count: usize,
    /// Triangles referenced a texture that was not loaded; they were drawn with the original
    /// average-color Gouraud fallback rather than dropped.
    pub texture_fallbacks: usize,
}

/// Packs and bins triangles in draw order. Textured faces whose texture is not loaded are
/// rewritten into the original Gouraud fallback (`fq.af`), exactly as the CPU path does.
pub fn pack_frame(state: &RasterState, tris: &[Tri], textures: &TextureSet) -> PackedFrame {
    let width = state.width;
    let height = state.height;
    let bins_x = ((width + BIN_W - 1) / BIN_W) as u32;
    let bins_y = ((height + BIN_H - 1) / BIN_H) as u32;
    let bin_count = (bins_x * bins_y) as usize;
    let mut packed = Vec::with_capacity(tris.len() * TRI_STRIDE);
    let mut counts = vec![0u32; bin_count];
    let mut boxes: Vec<(i32, i32, i32, i32)> = Vec::with_capacity(tris.len());
    let mut fallbacks = 0usize;
    for tri in tris {
        let mut colors = [0i32; 3];
        let mut kind = KIND_GOURAUD;
        let mut texture = -1;
        let mut plane = [[0i32; 3]; 3];
        match tri.fill {
            Fill::Gouraud { colors: c } => colors = c,
            Fill::Flat { rgb } => {
                kind = KIND_FLAT;
                colors[0] = rgb;
            }
            Fill::Textured { colors: c, px, py, pz, texture: t, model_variant } => {
                if textures.get(t).is_some() {
                    kind = if model_variant { KIND_TEX_MODEL } else { KIND_TEX_TILE };
                    colors = c;
                    texture = t;
                    plane = [px, py, pz];
                } else {
                    fallbacks += 1;
                    let avg = textures.get(t).map(|x| x.average_rgb).unwrap_or(0);
                    colors = [merge_texture_shade(avg, c[0]), merge_texture_shade(avg, c[1]), merge_texture_shade(avg, c[2])];
                }
            }
        }
        let flags = kind | (if tri.clip_x { 4 } else { 0 }) | ((tri.alpha & 255) << 8);
        packed.extend_from_slice(&[tri.y[0], tri.y[1], tri.y[2], tri.x[0], tri.x[1], tri.x[2]]);
        packed.extend_from_slice(&colors);
        packed.push(flags);
        packed.push(texture);
        packed.extend_from_slice(&plane[0]);
        packed.extend_from_slice(&plane[1]);
        packed.extend_from_slice(&plane[2]);
        // Conservative screen bounds: rows [min y, max y), columns [min x, max x).
        let min_y = tri.y.iter().copied().min().unwrap().max(0);
        let max_y = tri.y.iter().copied().max().unwrap().min(height);
        let mut min_x = tri.x.iter().copied().min().unwrap();
        let mut max_x = tri.x.iter().copied().max().unwrap();
        if tri.clip_x {
            min_x = min_x.max(0);
            max_x = max_x.min(width);
        } else {
            min_x = min_x.max(0);
            max_x = max_x.min(width);
        }
        if min_y >= max_y || min_x >= max_x {
            boxes.push((0, -1, 0, -1));
            continue;
        }
        let bx0 = min_x / BIN_W;
        let bx1 = (max_x - 1) / BIN_W;
        let by0 = min_y / BIN_H;
        let by1 = (max_y - 1) / BIN_H;
        boxes.push((bx0, bx1, by0, by1));
        for by in by0..=by1 {
            for bx in bx0..=bx1 {
                counts[(by as u32 * bins_x + bx as u32) as usize] += 1;
            }
        }
    }
    let mut offsets = vec![0u32; bin_count + 1];
    for i in 0..bin_count {
        offsets[i + 1] = offsets[i] + counts[i];
    }
    let mut cursor = offsets[..bin_count].to_vec();
    let mut bin_tris = vec![0u32; offsets[bin_count] as usize];
    for (index, &(bx0, bx1, by0, by1)) in boxes.iter().enumerate() {
        if bx1 < bx0 || by1 < by0 {
            continue;
        }
        for by in by0..=by1 {
            for bx in bx0..=bx1 {
                let bin = (by as u32 * bins_x + bx as u32) as usize;
                bin_tris[cursor[bin] as usize] = index as u32;
                cursor[bin] += 1;
            }
        }
    }
    PackedFrame { tris: packed, bin_offsets: offsets, bin_tris, bins_x, bins_y, triangle_count: tris.len(), texture_fallbacks: fallbacks }
}

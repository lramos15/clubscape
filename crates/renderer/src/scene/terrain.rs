//! Exact port of the original terrain colour pass run when a scene is built (`rl4.ad`, with
//! `ez.bn` and the shaped-tile constructor `fn`): height-normal lighting with the scenery
//! shadow map, the running 5-tile HSL box blend of the floor underlays, the underlay/overlay
//! definitions, and the tile paints / shaped tile models the pass hands to the scene.
//!
//! The live client stores only the tiles of its own 104×104 scene (`rl4.xl` keeps `0 <= x <
//! 104`; at the stock draw distance `rl4.fn` lists no extra squares), so the blend, the light
//! normals and the shadows at the scene's outer tiles see nothing beyond the scene edge. Block
//! exports carry each square's raw tiles (`BTER`) and the shadows its own scenery casts
//! (`BSHD`); rebuilding the terrain from those at assembly time reproduces the live colours of
//! every tile — including the outer five — for whichever base the scene is assembled at, where
//! the exported lit colours (blended once with the square's real neighbours) could not.

use std::collections::HashMap;

use super::tile::HIDDEN_COLOR;
use super::{SceneData, TileModel, TilePaint};
use crate::chunk::Chunks;
use crate::error::RenderError;
use crate::palette::Palette;

/// One floor underlay definition (`ph`) as the blend sums it: hue, saturation, lightness and
/// the hue multiplier (`pk`, `eb`, `fs`, `bq`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Underlay {
    pub hue: i32,
    pub saturation: i32,
    pub lightness: i32,
    pub hue_multiplier: i32,
}

/// One floor overlay definition (`ow`): texture (`ab`), primary colour (`qa`) with its HSL
/// (`bk`/`ge`/`jc`), secondary colour (`gy`, −1 none) with its HSL (`lf`/`th`/`cs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overlay {
    pub texture: i32,
    pub rgb: i32,
    pub hue: i32,
    pub saturation: i32,
    pub lightness: i32,
    pub secondary_rgb: i32,
    pub secondary_hue: i32,
    pub secondary_saturation: i32,
    pub secondary_lightness: i32,
}

/// `terrain/floors.bin`: every underlay and overlay definition of the source cache.
#[derive(Clone, Debug, Default)]
pub struct FloorDefs {
    pub underlays: HashMap<i32, Underlay>,
    pub overlays: HashMap<i32, Overlay>,
}

impl FloorDefs {
    pub fn from_chunks(data: &[u8]) -> Result<Self, RenderError> {
        let chunks = Chunks::parse(data)?;
        let underlays = chunks.ints("FUND")?;
        let overlays = chunks.ints("FOVL")?;
        if underlays.len() % 5 != 0 || overlays.len() % 10 != 0 {
            return Err(RenderError::InvalidAsset(
                "terrain floor definitions require complete FUND/FOVL records".into(),
            ));
        }
        let mut defs = FloorDefs::default();
        for r in underlays.as_chunks::<5>().0 {
            if r[0] < 0 || !valid_hsl(r[1], r[2], r[3]) || !(1..=256).contains(&r[4]) {
                return Err(RenderError::InvalidAsset(format!(
                    "FUND floor {} has invalid HSL or hue multiplier",
                    r[0]
                )));
            }
            if defs
                .underlays
                .insert(
                    r[0],
                    Underlay {
                        hue: r[1],
                        saturation: r[2],
                        lightness: r[3],
                        hue_multiplier: r[4],
                    },
                )
                .is_some()
            {
                return Err(RenderError::InvalidAsset(format!(
                    "duplicate FUND floor {}",
                    r[0]
                )));
            }
        }
        for r in overlays.as_chunks::<10>().0 {
            if r[0] < 0
                || !(-1..=i32::from(u16::MAX)).contains(&r[1])
                || !(0..=0xFF_FFFF).contains(&r[2])
                || !valid_hsl(r[3], r[4], r[5])
                || !(-1..=0xFF_FFFF).contains(&r[6])
                || !valid_hsl(r[7], r[8], r[9])
            {
                return Err(RenderError::InvalidAsset(format!(
                    "FOVL floor {} has invalid texture, colour or HSL",
                    r[0]
                )));
            }
            if defs
                .overlays
                .insert(
                    r[0],
                    Overlay {
                        texture: r[1],
                        rgb: r[2],
                        hue: r[3],
                        saturation: r[4],
                        lightness: r[5],
                        secondary_rgb: r[6],
                        secondary_hue: r[7],
                        secondary_saturation: r[8],
                        secondary_lightness: r[9],
                    },
                )
                .is_some()
            {
                return Err(RenderError::InvalidAsset(format!(
                    "duplicate FOVL floor {}",
                    r[0]
                )));
            }
        }
        if defs.underlays.is_empty() || defs.overlays.is_empty() {
            return Err(RenderError::InvalidAsset(
                "terrain/floors.bin lists no floor definitions".into(),
            ));
        }
        Ok(defs)
    }
}

fn valid_hsl(hue: i32, saturation: i32, lightness: i32) -> bool {
    (-256..=256).contains(&hue) && (0..=255).contains(&saturation) && (0..=255).contains(&lightness)
}

/// Raw terrain of one 64×64 map square as the live loader stores it (`rl4.xl`), per plane:
/// underlay id + 1 (0 none), overlay id + 1 (0 none), overlay shape path, overlay rotation,
/// settings and the tile's south-west corner height; plus the shadow values (`rl4.bf`) the
/// square's own scenery writes (`ci.aq`), in tiles relative to the square origin (they may
/// spill past its edges).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawTerrain {
    pub underlay: Vec<i16>,
    pub overlay: Vec<i16>,
    pub overlay_path: Vec<i8>,
    pub overlay_rotation: Vec<i8>,
    pub settings: Vec<i8>,
    pub heights: Vec<i32>,
    /// `(plane, origin x, origin y, tile x, tile y, value)`: one `bf` write of the location whose
    /// origin tile is (origin x, origin y); tiles relative to the square origin (writes may
    /// spill past the square).
    pub shadows: Vec<(i32, i32, i32, i32, i32, i32)>,
}

pub const SQUARE: i32 = 64;
const PLANES: i32 = 4;

impl RawTerrain {
    /// Parses the paired `BTER` (6 ints per plane × tile) and `BSHD` (6 ints per entry)
    /// chunks. Legacy buffers with neither can be inspected, but cannot build a live scene.
    pub fn from_block_chunks(chunks: &Chunks) -> Result<Option<Self>, RenderError> {
        let Some(terrain) = chunks.ints_opt("BTER")? else {
            if chunks.has("BSHD") {
                return Err(RenderError::InvalidAsset(
                    "BSHD shadows require the matching BTER raw terrain".into(),
                ));
            }
            return Ok(None);
        };
        let shadows = chunks.ints("BSHD")?;
        let tiles = (PLANES * SQUARE * SQUARE) as usize;
        if terrain.len() != tiles * 6 {
            return Err(RenderError::InvalidAsset(format!(
                "BTER holds {} ints, expected {}",
                terrain.len(),
                tiles * 6
            )));
        }
        let mut raw = RawTerrain {
            underlay: Vec::with_capacity(tiles),
            overlay: Vec::with_capacity(tiles),
            overlay_path: Vec::with_capacity(tiles),
            overlay_rotation: Vec::with_capacity(tiles),
            settings: Vec::with_capacity(tiles),
            heights: Vec::with_capacity(tiles),
            shadows: Vec::new(),
        };
        for (tile, r) in terrain.as_chunks::<6>().0.iter().enumerate() {
            let (Ok(underlay), Ok(overlay), Ok(path), Ok(rotation), Ok(settings)) = (
                i16::try_from(r[0]),
                i16::try_from(r[1]),
                i8::try_from(r[2]),
                i8::try_from(r[3]),
                i8::try_from(r[4]),
            ) else {
                return Err(RenderError::InvalidAsset(format!(
                    "BTER tile {tile} contains an out-of-range short or byte"
                )));
            };
            // rl4.xl accumulates at most four unsigned-byte height deltas, in units of 8.
            if !(0..=11).contains(&path)
                || !(0..=3).contains(&rotation)
                || !(-PLANES * 255 * 8..=0).contains(&r[5])
                || r[5] % 8 != 0
            {
                return Err(RenderError::InvalidAsset(format!(
                    "BTER tile {tile} has an invalid shape, rotation or height"
                )));
            }
            raw.underlay.push(underlay);
            raw.overlay.push(overlay);
            raw.overlay_path.push(path);
            raw.overlay_rotation.push(rotation);
            raw.settings.push(settings);
            raw.heights.push(r[5]);
        }
        if shadows.len() % 6 != 0 {
            return Err(RenderError::InvalidAsset(format!(
                "BSHD holds {} ints, not a multiple of 6 (plane, origin x/y, tile x/y, value)",
                shadows.len()
            )));
        }
        for (entry, r) in shadows.as_chunks::<6>().0.iter().enumerate() {
            if !(0..PLANES).contains(&r[0])
                || !(0..SQUARE).contains(&r[1])
                || !(0..SQUARE).contains(&r[2])
                || !(-SQUARE..2 * SQUARE).contains(&r[3])
                || !(-SQUARE..2 * SQUARE).contains(&r[4])
                || !(0..=50).contains(&r[5])
            {
                return Err(RenderError::InvalidAsset(format!(
                    "BSHD entry {entry} has an invalid plane, origin, coordinate or shadow value"
                )));
            }
            raw.shadows.push((r[0], r[1], r[2], r[3], r[4], r[5]));
        }
        Ok(Some(raw))
    }

    #[inline]
    pub fn index(plane: i32, bx: i32, by: i32) -> usize {
        ((plane * SQUARE + bx) * SQUARE + by) as usize
    }
}

/// Grid of the live loader's arrays: 104 scene tiles inside a 184 margin frame (`vj = 40`),
/// heights and shadows one row/column larger (`xa + 1`).
pub const GRID: usize = 184;
pub const MARGIN: i32 = 40;
pub const MAIN: i32 = 104;

/// The terrain pass inputs of a whole scene, laid out exactly like `rl4`'s arrays: only tiles
/// of the 104×104 scene are filled; the margin stays zero like the live loader leaves it.
pub struct SceneTerrain {
    underlay: Vec<i16>,
    overlay: Vec<i16>,
    overlay_path: Vec<i8>,
    overlay_rotation: Vec<i8>,
    settings: Vec<i8>,
    heights: Vec<i32>,
    shadows: Vec<i8>,
}

const STRIDE: usize = GRID + 1;

impl SceneTerrain {
    pub fn empty() -> Self {
        let tiles = PLANES as usize * GRID * GRID;
        let corners = PLANES as usize * STRIDE * STRIDE;
        SceneTerrain {
            underlay: vec![0; tiles],
            overlay: vec![0; tiles],
            overlay_path: vec![0; tiles],
            overlay_rotation: vec![0; tiles],
            settings: vec![0; tiles],
            heights: vec![0; corners],
            shadows: vec![0; corners],
        }
    }

    #[inline]
    fn t(plane: i32, ex: i32, ey: i32) -> usize {
        (plane as usize * GRID + ex as usize) * GRID + ey as usize
    }

    #[inline]
    fn c(plane: i32, ex: i32, ey: i32) -> usize {
        (plane as usize * STRIDE + ex as usize) * STRIDE + ey as usize
    }

    /// Copies one square tile (`bx, by` of `raw`) to scene tile (`sx, sy`, 0..104) on
    /// `dest_plane`, turned by `quarter_turns` (the overlay rotation turns with it, as the
    /// original instance chunk copy does).
    #[allow(clippy::too_many_arguments)]
    pub fn set_tile(
        &mut self,
        raw: &RawTerrain,
        source_plane: i32,
        bx: i32,
        by: i32,
        dest_plane: i32,
        sx: i32,
        sy: i32,
        quarter_turns: i32,
    ) {
        if !(0..MAIN).contains(&sx) || !(0..MAIN).contains(&sy) {
            return;
        }
        let from = RawTerrain::index(source_plane, bx, by);
        let to = Self::t(dest_plane, sx + MARGIN, sy + MARGIN);
        self.underlay[to] = raw.underlay[from];
        self.overlay[to] = raw.overlay[from];
        self.overlay_path[to] = raw.overlay_path[from];
        self.overlay_rotation[to] =
            ((i32::from(raw.overlay_rotation[from]) + quarter_turns) & 3) as i8;
        self.settings[to] = raw.settings[from];
    }

    /// Sets a corner height of the scene (`sx, sy` in 0..=104 — the live loader never fills the
    /// far row/column, callers pass only loaded corners).
    pub fn set_height(&mut self, plane: i32, sx: i32, sy: i32, height: i32) {
        if !(0..MAIN).contains(&sx) || !(0..MAIN).contains(&sy) {
            return;
        }
        self.heights[Self::c(plane, sx + MARGIN, sy + MARGIN)] = height;
    }

    /// Whether the live loader places a location whose origin is scene tile (`sx, sy`)
    /// (`rl4.ws`: strictly inside the scene, 1..=102 on both axes).
    pub fn places_location(sx: i32, sy: i32) -> bool {
        sx > 0 && sy > 0 && sx < MAIN - 1 && sy < MAIN - 1
    }

    /// Merges a shadow value at scene tile (`sx, sy`) the way `ci.aq` writes them: scenery
    /// shadows only ever raise a tile's value (objects take the max, walls write 50, the largest
    /// value the pass uses), so the merge order does not matter.
    pub fn add_shadow(&mut self, plane: i32, sx: i32, sy: i32, value: i32) {
        let ex = sx + MARGIN;
        let ey = sy + MARGIN;
        if !(0..STRIDE as i32).contains(&ex) || !(0..STRIDE as i32).contains(&ey) {
            return;
        }
        let slot = &mut self.shadows[Self::c(plane, ex, ey)];
        if value as i8 > *slot {
            *slot = value as i8;
        }
    }

    pub fn settings(&self, plane: i32, sx: i32, sy: i32) -> i8 {
        self.settings[Self::t(plane, sx + MARGIN, sy + MARGIN)]
    }

    /// Corner height at scene corner (`sx, sy` in 0..=104): 0 where the live loader stores none
    /// (the far row/column, undeclared chunks).
    pub fn height(&self, plane: i32, sx: i32, sy: i32) -> i32 {
        self.heights[Self::c(plane, sx + MARGIN, sy + MARGIN)]
    }

    fn validate_floors(&self, defs: &FloorDefs) -> Result<(), RenderError> {
        for (&underlay, &overlay) in self.underlay.iter().zip(&self.overlay) {
            let underlay = i32::from(underlay) & 32767;
            let overlay = i32::from(overlay) & 32767;
            if underlay > 0 && !defs.underlays.contains_key(&(underlay - 1)) {
                return Err(RenderError::MissingAsset(format!(
                    "terrain underlay {} has no floor definition",
                    underlay - 1
                )));
            }
            if overlay > 0 && !defs.overlays.contains_key(&(overlay - 1)) {
                return Err(RenderError::MissingAsset(format!(
                    "terrain overlay {} has no floor definition",
                    overlay - 1
                )));
            }
        }
        Ok(())
    }
}

/// `rl4.qr`: packs hue/saturation/lightness into the 16-bit HSL index.
pub fn pack_hsl(hue: i32, mut saturation: i32, lightness: i32) -> i32 {
    if lightness > 179 {
        saturation /= 2;
    }
    if lightness > 192 {
        saturation /= 2;
    }
    if lightness > 217 {
        saturation /= 2;
    }
    if lightness > 243 {
        saturation /= 2;
    }
    ((saturation / 32) << 7) + ((hue / 4) << 10) + lightness / 2
}

/// `rl4.hs`: an underlay HSL lit by `light` (−1 → the hidden marker).
pub fn light_underlay(hsl: i32, mut light: i32) -> i32 {
    if hsl == -1 {
        return HIDDEN_COLOR;
    }
    light = (hsl & 127) * light / 128;
    light = light.clamp(2, 126);
    (hsl & 65408) + light
}

/// `rl4.he`: an overlay HSL lit by `light` (−2 → hidden marker, −1 → the light alone).
pub fn light_overlay(hsl: i32, mut light: i32) -> i32 {
    if hsl == -2 {
        return HIDDEN_COLOR;
    }
    if hsl == -1 {
        return light.clamp(2, 126);
    }
    light = (hsl & 127) * light / 128;
    light = light.clamp(2, 126);
    (hsl & 65408) + light
}

/// One built tile: paint or shaped model at a scene tile.
pub enum BuiltTile {
    Paint(TilePaint),
    Model(Box<TileModel>),
}

/// Statistics of a terrain rebuild.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerrainStats {
    pub paints: usize,
    pub tile_models: usize,
    /// Kept for diagnostic ABI compatibility; a successful pass has no missing definitions.
    pub missing_overlays: usize,
    pub missing_underlays: usize,
}

/// Runs the original pass over `terrain` and hands every built tile to `emit(plane, x, y, tile)`
/// with x/y in scene tiles (0..104). `texture_average(id)` is `ec.ab` (the texture's average
/// colour, 0 when unknown).
pub fn build(
    terrain: &SceneTerrain,
    defs: &FloorDefs,
    palette: &Palette,
    texture_average: &dyn Fn(i32) -> i32,
    mut emit: impl FnMut(i32, i32, i32, BuiltTile),
) -> Result<TerrainStats, RenderError> {
    terrain.validate_floors(defs)?;
    let mut stats = TerrainStats::default();
    let xa = GRID as i32;
    let ch = GRID as i32;
    // (int)Math.sqrt(5100.0) * 768 >> 8
    let light_scale = ((5100.0f64.sqrt() as i32) * 768) >> 8;
    let mut light = vec![0i32; STRIDE * STRIDE];
    let mut sum_hue = vec![0i32; ch as usize];
    let mut sum_sat = vec![0i32; ch as usize];
    let mut sum_light = vec![0i32; ch as usize];
    let mut sum_mult = vec![0i32; ch as usize];
    let mut count = vec![0i32; ch as usize];
    let height = |plane: i32, ex: i32, ey: i32| terrain.heights[SceneTerrain::c(plane, ex, ey)];
    let shadow =
        |plane: i32, ex: i32, ey: i32| i32::from(terrain.shadows[SceneTerrain::c(plane, ex, ey)]);
    for plane in 0..PLANES {
        // Height normals against the (−50, −10, −50) light, darkened by the scenery shadows.
        for ey in 1..ch - 1 {
            for ex in 1..xa - 1 {
                let dx = height(plane, ex + 1, ey) - height(plane, ex - 1, ey);
                let dz = height(plane, ex, ey + 1) - height(plane, ex, ey - 1);
                let len = (f64::from(dz * dz + dx * dx + 65536)).sqrt() as i32;
                let nx = (dx << 8) / len;
                let ny = 65536 / len;
                let nz = (dz << 8) / len;
                let lit = (nz * -50 + nx * -50 + ny * -10) / light_scale + 96;
                let shade = (shadow(plane, ex, ey + 1) >> 3)
                    + (shadow(plane, ex - 1, ey) >> 2)
                    + (shadow(plane, ex, ey - 1) >> 2)
                    + (shadow(plane, ex + 1, ey) >> 3)
                    + (shadow(plane, ex, ey) >> 1);
                light[ex as usize * STRIDE + ey as usize] = lit - shade;
            }
        }
        for v in [
            &mut sum_hue,
            &mut sum_sat,
            &mut sum_light,
            &mut sum_mult,
            &mut count,
        ] {
            v.iter_mut().for_each(|x| *x = 0);
        }
        let underlay_of = |ex: i32, ey: i32| -> Option<&Underlay> {
            let id = i32::from(terrain.underlay[SceneTerrain::t(plane, ex, ey)]) & 32767;
            (id > 0).then(|| defs.underlays.get(&(id - 1))).flatten()
        };
        for x in -5..xa + 5 {
            for y in 0..ch {
                let add = x + 5;
                if (0..xa).contains(&add)
                    && let Some(u) = underlay_of(add, y)
                {
                    let y = y as usize;
                    sum_hue[y] += u.hue;
                    sum_sat[y] += u.saturation;
                    sum_light[y] += u.lightness;
                    sum_mult[y] += u.hue_multiplier;
                    count[y] += 1;
                }
                let remove = x - 5;
                if (0..xa).contains(&remove)
                    && let Some(u) = underlay_of(remove, y)
                {
                    let y = y as usize;
                    sum_hue[y] -= u.hue;
                    sum_sat[y] -= u.saturation;
                    sum_light[y] -= u.lightness;
                    sum_mult[y] -= u.hue_multiplier;
                    count[y] -= 1;
                }
            }
            if x < 1 || x >= xa - 1 {
                continue;
            }
            let (mut hue, mut sat, mut lig, mut mult, mut n) = (0i32, 0i32, 0i32, 0i32, 0i32);
            for y in -5..ch + 5 {
                let add = y + 5;
                if (0..ch).contains(&add) {
                    let a = add as usize;
                    hue += sum_hue[a];
                    sat += sum_sat[a];
                    lig += sum_light[a];
                    mult += sum_mult[a];
                    n += count[a];
                }
                let remove = y - 5;
                if (0..ch).contains(&remove) {
                    let r = remove as usize;
                    hue -= sum_hue[r];
                    sat -= sum_sat[r];
                    lig -= sum_light[r];
                    mult -= sum_mult[r];
                    n -= count[r];
                }
                if y < 1 || y >= ch - 1 {
                    continue;
                }
                let t = SceneTerrain::t(plane, x, y);
                let underlay = i32::from(terrain.underlay[t]) & 32767;
                let overlay = i32::from(terrain.overlay[t]) & 32767;
                if underlay == 0 && overlay == 0 {
                    continue;
                }
                let h = [
                    height(plane, x, y),
                    height(plane, x + 1, y),
                    height(plane, x + 1, y + 1),
                    height(plane, x, y + 1),
                ];
                let l = [
                    light[x as usize * STRIDE + y as usize],
                    light[(x + 1) as usize * STRIDE + y as usize],
                    light[(x + 1) as usize * STRIDE + (y + 1) as usize],
                    light[x as usize * STRIDE + (y + 1) as usize],
                ];
                let mut under_hsl = -1;
                if underlay > 0 {
                    under_hsl = pack_hsl(hue * 256 / mult, sat / n, lig / n);
                }
                let under_rgb = if under_hsl != -1 {
                    palette.lookup(light_underlay(under_hsl, 96))
                } else {
                    0
                };
                let under = l.map(|light| light_underlay(under_hsl, light));
                let (sx, sy) = (x - MARGIN, y - MARGIN);
                if overlay == 0 {
                    stats.paints += 1;
                    emit(
                        plane,
                        sx,
                        sy,
                        BuiltTile::Paint(TilePaint {
                            sw: under[0],
                            se: under[1],
                            ne: under[2],
                            nw: under[3],
                            texture: -1,
                            flat: false,
                            rgb: under_rgb,
                        }),
                    );
                    continue;
                }
                let def = defs.overlays.get(&(overlay - 1)).ok_or_else(|| {
                    RenderError::MissingAsset(format!(
                        "terrain overlay {} has no floor definition",
                        overlay - 1
                    ))
                })?;
                let shape = i32::from(terrain.overlay_path[t]) + 1;
                let rotation = i32::from(terrain.overlay_rotation[t]);
                let mut texture = def.texture;
                let (over_hsl, mut over_key);
                if texture >= 0 {
                    over_key = texture_average(texture);
                    over_hsl = -1;
                } else if def.rgb == 0xFF00FF {
                    over_hsl = -2;
                    texture = -1;
                    over_key = -2;
                } else {
                    over_hsl = pack_hsl(def.hue, def.saturation, def.lightness);
                    over_key = over_hsl;
                }
                let mut over_rgb = 0;
                if over_key != -2 {
                    over_rgb = palette.lookup(light_overlay(over_key, 96));
                }
                if def.secondary_rgb != -1 {
                    over_key = pack_hsl(
                        def.secondary_hue,
                        def.secondary_saturation,
                        def.secondary_lightness,
                    );
                    over_rgb = palette.lookup(light_overlay(over_key, 96));
                }
                let over = l.map(|light| light_overlay(over_hsl, light));
                if shape == 1 {
                    stats.paints += 1;
                    emit(
                        plane,
                        sx,
                        sy,
                        BuiltTile::Paint(TilePaint {
                            sw: over[0],
                            se: over[1],
                            ne: over[2],
                            nw: over[3],
                            texture,
                            flat: h[1] == h[0] && h[0] == h[2] && h[3] == h[0],
                            rgb: over_rgb,
                        }),
                    );
                } else {
                    stats.tile_models += 1;
                    emit(
                        plane,
                        sx,
                        sy,
                        BuiltTile::Model(Box::new(shaped_tile(
                            shape,
                            rotation,
                            texture,
                            sx,
                            sy,
                            h,
                            under,
                            over,
                            under_rgb,
                            over_rgb.max(1),
                        ))),
                    );
                }
            }
        }
    }
    Ok(stats)
}

/// `fn.af`: per shape, the face list as (overlay flag, vertex a, vertex b, vertex c).
const SHAPE_FACES: [&[i32]; 13] = [
    &[0, 1, 2, 3, 0, 0, 1, 3],
    &[1, 1, 2, 3, 1, 0, 1, 3],
    &[0, 1, 2, 3, 1, 0, 1, 3],
    &[0, 0, 1, 2, 0, 0, 2, 4, 1, 0, 4, 3],
    &[0, 0, 1, 4, 0, 0, 4, 3, 1, 1, 2, 4],
    &[0, 0, 4, 3, 1, 0, 1, 2, 1, 0, 2, 4],
    &[0, 1, 2, 4, 1, 0, 1, 4, 1, 0, 4, 3],
    &[0, 4, 1, 2, 0, 4, 2, 5, 1, 0, 4, 5, 1, 0, 5, 3],
    &[0, 4, 1, 2, 0, 4, 2, 3, 0, 4, 3, 5, 1, 0, 4, 5],
    &[0, 0, 4, 5, 1, 4, 1, 2, 1, 4, 2, 3, 1, 4, 3, 5],
    &[
        0, 0, 1, 5, 0, 1, 4, 5, 0, 1, 2, 4, 1, 0, 5, 3, 1, 5, 4, 3, 1, 4, 2, 3,
    ],
    &[
        1, 0, 1, 5, 1, 1, 4, 5, 1, 1, 2, 4, 0, 0, 5, 3, 0, 5, 4, 3, 0, 4, 2, 3,
    ],
    &[
        1, 0, 5, 4, 1, 0, 1, 5, 0, 0, 4, 3, 0, 4, 5, 3, 0, 5, 2, 3, 0, 1, 2, 5,
    ],
];

/// `fn.az`: per shape, the vertex kinds (1..16: corners, edge midpoints, inner points).
const SHAPE_VERTICES: [&[i32]; 13] = [
    &[1, 3, 5, 7],
    &[1, 3, 5, 7],
    &[1, 3, 5, 7],
    &[1, 3, 5, 7, 6],
    &[1, 3, 5, 7, 6],
    &[1, 3, 5, 7, 6],
    &[1, 3, 5, 7, 6],
    &[1, 3, 5, 7, 2, 6],
    &[1, 3, 5, 7, 2, 8],
    &[1, 3, 5, 7, 2, 8],
    &[1, 3, 5, 7, 11, 12],
    &[1, 3, 5, 7, 11, 12],
    &[1, 3, 5, 7, 13, 14],
];

/// The shaped tile model constructor (`fn(shape, rotation, texture, x, y, heights, underlay
/// colours, overlay colours, underlay rgb, overlay rgb)`), vertices in scene units.
#[allow(clippy::too_many_arguments)]
pub fn shaped_tile(
    shape: i32,
    rotation: i32,
    texture: i32,
    tile_x: i32,
    tile_y: i32,
    h: [i32; 4],
    under: [i32; 4],
    over: [i32; 4],
    underlay_rgb: i32,
    overlay_rgb: i32,
) -> TileModel {
    let flat = h[0] == h[1] && h[0] == h[2] && h[0] == h[3];
    let size = 128;
    let half = size / 2;
    let quarter = size / 4;
    let three_quarter = size * 3 / 4;
    let kinds = SHAPE_VERTICES[shape as usize];
    let n = kinds.len();
    let mut xs = Vec::with_capacity(n);
    let mut ys = Vec::with_capacity(n);
    let mut zs = Vec::with_capacity(n);
    let mut under_v = Vec::with_capacity(n);
    let mut over_v = Vec::with_capacity(n);
    let x0 = tile_x * size;
    let z0 = tile_y * size;
    for &kind in kinds {
        let mut k = kind;
        if k & 1 == 0 && k <= 8 {
            k = ((k - rotation - rotation - 1) & 7) + 1;
        }
        if k > 8 && k <= 12 {
            k = ((k - 9 - rotation) & 3) + 9;
        }
        if k > 12 && k <= 16 {
            k = ((k - 13 - rotation) & 3) + 13;
        }
        let (x, z, y, u, o) = match k {
            1 => (x0, z0, h[0], under[0], over[0]),
            2 => (
                x0 + half,
                z0,
                (h[0] + h[1]) >> 1,
                (under[0] + under[1]) >> 1,
                (over[0] + over[1]) >> 1,
            ),
            3 => (x0 + size, z0, h[1], under[1], over[1]),
            4 => (
                x0 + size,
                z0 + half,
                (h[1] + h[2]) >> 1,
                (under[1] + under[2]) >> 1,
                (over[1] + over[2]) >> 1,
            ),
            5 => (x0 + size, z0 + size, h[2], under[2], over[2]),
            6 => (
                x0 + half,
                z0 + size,
                (h[2] + h[3]) >> 1,
                (under[2] + under[3]) >> 1,
                (over[2] + over[3]) >> 1,
            ),
            7 => (x0, z0 + size, h[3], under[3], over[3]),
            8 => (
                x0,
                z0 + half,
                (h[3] + h[0]) >> 1,
                (under[3] + under[0]) >> 1,
                (over[3] + over[0]) >> 1,
            ),
            9 => (
                x0 + half,
                z0 + quarter,
                (h[0] + h[1]) >> 1,
                (under[0] + under[1]) >> 1,
                (over[0] + over[1]) >> 1,
            ),
            10 => (
                x0 + three_quarter,
                z0 + half,
                (h[1] + h[2]) >> 1,
                (under[1] + under[2]) >> 1,
                (over[1] + over[2]) >> 1,
            ),
            11 => (
                x0 + half,
                z0 + three_quarter,
                (h[2] + h[3]) >> 1,
                (under[2] + under[3]) >> 1,
                (over[2] + over[3]) >> 1,
            ),
            12 => (
                x0 + quarter,
                z0 + half,
                (h[3] + h[0]) >> 1,
                (under[3] + under[0]) >> 1,
                (over[3] + over[0]) >> 1,
            ),
            13 => (x0 + quarter, z0 + quarter, h[0], under[0], over[0]),
            14 => (x0 + three_quarter, z0 + quarter, h[1], under[1], over[1]),
            15 => (
                x0 + three_quarter,
                z0 + three_quarter,
                h[2],
                under[2],
                over[2],
            ),
            _ => (x0 + quarter, z0 + three_quarter, h[3], under[3], over[3]),
        };
        xs.push(x);
        ys.push(y);
        zs.push(z);
        under_v.push(u);
        over_v.push(o);
    }
    let faces = SHAPE_FACES[shape as usize];
    let count = faces.len() / 4;
    let mut face_a = Vec::with_capacity(count);
    let mut face_b = Vec::with_capacity(count);
    let mut face_c = Vec::with_capacity(count);
    let mut color_a = Vec::with_capacity(count);
    let mut color_b = Vec::with_capacity(count);
    let mut color_c = Vec::with_capacity(count);
    let mut textures = (texture != -1).then(|| Vec::with_capacity(count));
    for f in 0..count {
        let overlay_face = faces[f * 4];
        let mut a = faces[f * 4 + 1];
        let mut b = faces[f * 4 + 2];
        let mut c = faces[f * 4 + 3];
        if a < 4 {
            a = (a - rotation) & 3;
        }
        if b < 4 {
            b = (b - rotation) & 3;
        }
        if c < 4 {
            c = (c - rotation) & 3;
        }
        face_a.push(a);
        face_b.push(b);
        face_c.push(c);
        let source = if overlay_face == 0 { &under_v } else { &over_v };
        color_a.push(source[a as usize]);
        color_b.push(source[b as usize]);
        color_c.push(source[c as usize]);
        if let Some(t) = textures.as_mut() {
            t.push(if overlay_face == 0 { -1 } else { texture });
        }
    }
    TileModel {
        shape,
        rotation,
        flat,
        underlay_rgb,
        overlay_rgb,
        xs,
        ys,
        zs,
        face_a,
        face_b,
        face_c,
        color_a,
        color_b,
        color_c,
        textures,
    }
}

/// Replaces the scene's tile paints / models over the 104×104 main area with a fresh run of
/// the original pass; also refreshes the tile flags the pass sets (`256` paint present, `512`
/// paint visible, `1024` shaped model). Tiles the pass produces nothing for are cleared.
pub fn apply(
    scene: &mut SceneData,
    terrain: &SceneTerrain,
    defs: &FloorDefs,
    palette: &Palette,
    texture_average: &dyn Fn(i32) -> i32,
) -> Result<TerrainStats, RenderError> {
    let offset = scene.offset;
    let mut built: Vec<Option<BuiltTile>> =
        (0..(PLANES * MAIN * MAIN) as usize).map(|_| None).collect();
    let slot = |plane: i32, sx: i32, sy: i32| ((plane * MAIN + sx) * MAIN + sy) as usize;
    let stats = build(
        terrain,
        defs,
        palette,
        texture_average,
        |plane, sx, sy, tile| {
            built[slot(plane, sx, sy)] = Some(tile);
        },
    )?;
    for plane in 0..PLANES {
        for sx in 0..MAIN {
            for sy in 0..MAIN {
                let index = scene.tile_index(plane, sx + scene.offset, sy + scene.offset);
                scene.paints.remove(&index);
                scene.tile_models.remove(&index);
                scene.flags[index] &= !(256 | 512 | 1024);
            }
        }
        // The drawn corner heights follow the live loader too: the far row/column and any
        // undeclared chunk hold no height (0).
        for sx in 0..=MAIN {
            for sy in 0..=MAIN {
                scene.set_height(
                    plane,
                    sx + scene.offset,
                    sy + scene.offset,
                    terrain.height(plane, sx, sy),
                );
            }
        }
    }
    // `rl4.ad` → `ez.bm`/`ez.xe`: on a bridge tile (plane-1 setting bit 2) the tile stack turns —
    // plane 0 (the ground under the bridge) moves to plane 3 and planes 1..3 move down one.
    for sx in 0..MAIN {
        for sy in 0..MAIN {
            if terrain.settings(1, sx, sy) & 2 == 0 {
                continue;
            }
            let ground = built[slot(0, sx, sy)].take();
            for plane in 0..3 {
                let above = built[slot(plane + 1, sx, sy)].take();
                built[slot(plane, sx, sy)] = above;
            }
            built[slot(3, sx, sy)] = ground;
        }
    }
    for plane in 0..PLANES {
        for sx in 0..MAIN {
            for sy in 0..MAIN {
                let Some(tile) = built[slot(plane, sx, sy)].take() else {
                    continue;
                };
                let index = scene.tile_index(plane, sx + offset, sy + offset);
                match tile {
                    BuiltTile::Paint(paint) => {
                        // `ez.bn`: bit 512 follows the paint's north-east colour (`fj.ae`).
                        scene.flags[index] |= 256 | if paint.ne != HIDDEN_COLOR { 512 } else { 0 };
                        scene.paints.insert(index, paint);
                    }
                    BuiltTile::Model(model) => {
                        scene.flags[index] |= 1024;
                        scene.tile_models.insert(index, *model);
                    }
                }
            }
        }
    }
    Ok(stats)
}

//! Exact port of the original minimap raster (`client.bm`): per-tile minimap flags
//! (`client.ed`), the tile sweep (`client.pv`), terrain fills (`client.nq -> client.wc` with the
//! 8x8 tile-shape masks of `client.gq`), wall/object/floor-decoration marks (`client.xx`) and
//! map-scene sprites (`client.ga` -> `yz.as`), on the 2D raster primitives of `yw`
//! (`ed`, `es`, `dj`, `el`). The result is the same 512x512, 4 px/tile, 48 px margin surface the
//! native HUD capture produced (`client.bm(world, sprite, 4.0, plane, 0, 0, 48, 48)`).
//!
//! Inputs are the assembled scene (paints, tile models, settings, walls with their placement
//! config, game objects, floor decorations), the object-definition minimap fields and the
//! original map-scene sprites exported by `tools/render-assets --profile minimap`.

use std::collections::HashMap;

use crate::chunk::Chunks;
use crate::error::RenderError;
use crate::scene::{SceneData, Wall, tag_non_interactive, tag_object_id};

/// Minimap flag bits of `ez.rc` (`client.ed`).
pub mod flag {
    pub const TILE: i64 = 1 << 54;
    pub const TILE_ABOVE: i64 = 1 << 55;
    pub const WALL: i64 = 1 << 56;
    pub const WALL_MAP_SCENE: i64 = 1 << 57;
    pub const GAME_OBJECT: i64 = 1 << 58;
    pub const FLOOR_DECORATION: i64 = 1 << 59;
    pub const BRIDGE: i64 = 1 << 60;
    /// `1080863910568919040L`: the four candidate bits every tile starts with.
    pub const CANDIDATES: i64 = WALL | WALL_MAP_SCENE | GAME_OBJECT | FLOOR_DECORATION;
}

/// Wall mark colours. The original jitters both by a per-session `Math.random()` (+-10 per
/// channel around 0xEE0000 / 0xEEEEEE at client init); the approved native minimap captures
/// were drawn with the values below, so the renderer holds them fixed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WallColours {
    /// `client.wc`: interactive walls (doors, gates).
    pub interactive: i32,
    /// `client.kn`: plain walls.
    pub plain: i32,
}

impl WallColours {
    pub const REFERENCE: WallColours = WallColours {
        interactive: 0xEF0000,
        plain: 0xF2E8F0,
    };
    /// The un-jittered seeds the original randomises around.
    pub const NOMINAL: WallColours = WallColours {
        interactive: 0xEE0000,
        plain: 0xEEEEEE,
    };
}

/// One original indexed sprite (`yz`): trimmed pixel indices into a shared palette.
#[derive(Clone, Debug)]
pub struct IndexedSprite {
    pub original_width: i32,
    pub original_height: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub width: i32,
    pub height: i32,
    pub indices: Vec<u8>,
}

/// One original map-element minimap sprite (`ps.as(false)`): `0xRRGGBB` pixels as the sprite
/// loader produced them; the original blit (`ym.af`) skips pixels whose value is 0 (transparent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapIconSprite {
    pub element: i32,
    pub width: i32,
    pub height: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub max_width: i32,
    pub max_height: i32,
    pub category: i32,
    /// `0xRRGGBB` per pixel, 0 = transparent (never drawn by the original).
    pub argb: Vec<i32>,
}

/// `minimap/mapicons.bin`: every map element the original shows on the minimap (`ps.ay`) with
/// its sprite. Drawn by the HUD over the minimap surface with the original `bo.as` rule (see
/// [`MapIcons::DRAW_RULE`]), never baked into the surface (the original paints them per frame).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapIcons {
    pub map_elements: i32,
    pub sprites: Vec<MapIconSprite>,
}

impl MapIcons {
    /// Original placement (`client.zr` → `bo.as`): for each icon, `dx = (tileX << 7) + 64 −
    /// playerX`, `dy = (tileY << 7) + 64 − playerY` (source units), both scaled by the minimap
    /// zoom, rotated by the map angle (`x' = dx·cos + dy·sin`, `y' = dy·cos − dx·sin`, >> 16 of
    /// the 65536-scaled trig), drawn with its top-left at `(cx + x' − width/2, cy − y' −
    /// height/2)` relative to the widget centre; skipped when `dx² + dy² > 6400` (80 units),
    /// clipped to the widget when beyond 2500 (50 units).
    pub const DRAW_RULE: &'static str = "client.zr/bo.as";

    pub fn from_chunks(data: &[u8]) -> Result<Self, RenderError> {
        let chunks = Chunks::parse(data)?;
        let h = chunks.ints("MIHD")?;
        if h.len() < 3 || h[2] != 9 {
            return Err(RenderError::Format("mapicons header".into()));
        }
        let header = chunks.ints("MIEL")?;
        let pixels = chunks.bytes("MIPX")?;
        if header.len() != h[1] as usize * 9 {
            return Err(RenderError::Format("mapicons element table".into()));
        }
        let mut sprites = Vec::with_capacity(h[1] as usize);
        let mut cursor = 0usize;
        for r in header.as_chunks::<9>().0 {
            let count = r[8] as usize;
            let end = cursor + count * 4;
            if end > pixels.len() {
                return Err(RenderError::Format("mapicons pixel data truncated".into()));
            }
            if r[1] < 0 || r[2] < 0 || (r[1] * r[2]) as usize != count {
                return Err(RenderError::InvalidAsset(format!(
                    "map icon {} sprite size {}x{} vs {count} pixels",
                    r[0], r[1], r[2]
                )));
            }
            let argb = pixels[cursor..end]
                .as_chunks::<4>()
                .0
                .iter()
                .map(|b| i32::from_be_bytes([b[0] as u8, b[1] as u8, b[2] as u8, b[3] as u8]))
                .collect();
            sprites.push(MapIconSprite {
                element: r[0],
                width: r[1],
                height: r[2],
                offset_x: r[3],
                offset_y: r[4],
                max_width: r[5],
                max_height: r[6],
                category: r[7],
                argb,
            });
            cursor = end;
        }
        Ok(MapIcons {
            map_elements: h[0],
            sprites,
        })
    }

    pub fn sprite(&self, element: i32) -> Option<&MapIconSprite> {
        self.sprites.iter().find(|s| s.element == element)
    }
}

/// The original map-scene sprite set (`oy.aq`) and tile-shape masks (`client.zw`).
#[derive(Clone, Debug)]
pub struct MapScenes {
    pub sprite_group: i32,
    pub shapes: [i64; 16],
    pub sprites: Vec<IndexedSprite>,
    pub palette: Vec<i32>,
}

impl MapScenes {
    pub fn from_chunks(data: &[u8]) -> Result<Self, RenderError> {
        let chunks = Chunks::parse(data)?;
        let h = chunks.ints("MSHD")?;
        if h.len() < 3 {
            return Err(RenderError::Format("mapscenes header".into()));
        }
        let shapes_vec = chunks.longs("MSHP")?;
        let shapes: [i64; 16] = shapes_vec
            .as_slice()
            .try_into()
            .map_err(|_| RenderError::Format("mapscenes needs 16 tile-shape masks".into()))?;
        if shapes[0] != 0 || shapes[1] != -1 {
            return Err(RenderError::InvalidAsset(
                "tile-shape masks do not start with the empty/full shapes".into(),
            ));
        }
        let header = chunks.ints("MSPR")?;
        let palette = chunks.ints("MSPL")?;
        let indices: Vec<u8> = chunks.bytes("MSPX")?.into_iter().map(|b| b as u8).collect();
        if header.len() != h[0] as usize * 7 {
            return Err(RenderError::Format("mapscenes sprite table".into()));
        }
        let mut sprites = Vec::with_capacity(h[0] as usize);
        let mut cursor = 0usize;
        for r in header.as_chunks::<7>().0 {
            let len = r[6] as usize;
            let end = cursor + len;
            if end > indices.len() {
                return Err(RenderError::Format("mapscenes pixel data truncated".into()));
            }
            if r[4] < 0 || r[5] < 0 || (r[4] * r[5]) as usize != len {
                return Err(RenderError::InvalidAsset("mapscene sprite size".into()));
            }
            if indices[cursor..end]
                .iter()
                .any(|&i| i as usize >= palette.len())
            {
                return Err(RenderError::InvalidAsset("mapscene palette index".into()));
            }
            sprites.push(IndexedSprite {
                original_width: r[0],
                original_height: r[1],
                offset_x: r[2],
                offset_y: r[3],
                width: r[4],
                height: r[5],
                indices: indices[cursor..end].to_vec(),
            });
            cursor = end;
        }
        Ok(MapScenes {
            sprite_group: h[1],
            shapes,
            sprites,
            palette,
        })
    }
}

/// The 2D raster target (`ym` + `yw` clip state) the minimap is drawn into. Pixels hold the
/// original int values: `1` where nothing was drawn, terrain colours without alpha, and
/// `0xFF000000 | rgb` for wall marks and sprite pixels, exactly like the native sprite.
#[derive(Clone, Debug)]
pub struct Raster {
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<i32>,
}

impl Raster {
    pub fn new(width: i32, height: i32) -> Self {
        Raster {
            width,
            height,
            pixels: vec![1; (width.max(0) * height.max(0)) as usize],
        }
    }

    /// `yw.ed`: vertical line.
    fn vertical(&mut self, x: i32, mut y: i32, mut len: i32, colour: i32) {
        if x < 0 || x >= self.width {
            return;
        }
        if y < 0 {
            len -= -y;
            y = 0;
        }
        if y + len > self.height {
            len = self.height - y;
        }
        let mut i = x + y * self.width;
        for _ in 0..len {
            self.pixels[i as usize] = colour | 0xFF00_0000u32 as i32;
            i += self.width;
        }
    }

    /// `yw.es`: horizontal line.
    fn horizontal(&mut self, mut x: i32, y: i32, mut len: i32, colour: i32) {
        if y < 0 || y >= self.height {
            return;
        }
        if x < 0 {
            len -= -x;
            x = 0;
        }
        if x + len > self.width {
            len = self.width - x;
        }
        let start = (x + y * self.width) as usize;
        for i in 0..len.max(0) as usize {
            self.pixels[start + i] = colour | 0xFF00_0000u32 as i32;
        }
    }

    /// `yw.dj`: filled rectangle.
    fn fill(&mut self, mut x: i32, mut y: i32, mut w: i32, mut h: i32, colour: i32) {
        if x < 0 {
            w -= -x;
            x = 0;
        }
        if y < 0 {
            h -= -y;
            y = 0;
        }
        if x + w > self.width {
            w = self.width - x;
        }
        if y + h > self.height {
            h = self.height - y;
        }
        if w <= 0 || h <= 0 {
            return;
        }
        for row in y..y + h {
            let start = (x + row * self.width) as usize;
            for p in &mut self.pixels[start..start + w as usize] {
                *p = colour | 0xFF00_0000u32 as i32;
            }
        }
    }

    /// `yw.el`: Bresenham-style line in 16.16 fixed point.
    fn line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, colour: i32) {
        let mut dx = x1 - x0;
        let mut dy = y1 - y0;
        if dy == 0 {
            if dx >= 0 {
                self.horizontal(x0, y0, dx + 1, colour);
            } else {
                self.horizontal(x0 + dx, y0, -dx + 1, colour);
            }
            return;
        }
        if dx == 0 {
            if dy >= 0 {
                self.vertical(x0, y0, dy + 1, colour);
            } else {
                self.vertical(x0, y0 + dy, -dy + 1, colour);
            }
            return;
        }
        if dx + dy < 0 {
            x0 += dx;
            dx = -dx;
            y0 += dy;
            dy = -dy;
        }
        let argb = colour | 0xFF00_0000u32 as i32;
        if dx > dy {
            let mut y = (y0 << 16) + 32768;
            let dyf = dy << 16;
            let step = (f64::from(dyf) / f64::from(dx) + 0.5).floor() as i32;
            let mut xe = dx + x0;
            let mut x = x0;
            if x < 0 {
                y += step * (0 - x);
                x = 0;
            }
            if xe >= self.width {
                xe = self.width - 1;
            }
            while x <= xe {
                let yy = y >> 16;
                if yy >= 0 && yy < self.height {
                    self.pixels[(x + yy * self.width) as usize] = argb;
                }
                y += step;
                x += 1;
            }
        } else {
            let mut x = (x0 << 16) + 32768;
            let dxf = dx << 16;
            let step = (f64::from(dxf) / f64::from(dy) + 0.5).floor() as i32;
            let mut ye = dy + y0;
            let mut y = y0;
            if y < 0 {
                x += step * (0 - y);
                y = 0;
            }
            if ye >= self.height {
                ye = self.height - 1;
            }
            while y <= ye {
                let xx = x >> 16;
                if xx >= 0 && xx < self.width {
                    self.pixels[(xx + y * self.width) as usize] = argb;
                }
                x += step;
                y += 1;
            }
        }
    }

    /// `yz.as` -> `yz.af`: an indexed sprite scaled to `w` x `h` at (`x`, `y`), palette index 0
    /// transparent, clipped to the raster.
    fn indexed_scaled(
        &mut self,
        sprite: &IndexedSprite,
        palette: &[i32],
        mut x: i32,
        mut y: i32,
        mut w: i32,
        mut h: i32,
    ) {
        let mut sx = 0i32;
        let mut sy = 0i32;
        let ow = sprite.original_width;
        let oh = sprite.original_height;
        let xs = (ow << 16) / w;
        let ys = (oh << 16) / h;
        if sprite.offset_x > 0 {
            let skip = ((sprite.offset_x << 16) + xs - 1) / xs;
            x += skip;
            sx += skip * xs - (sprite.offset_x << 16);
        }
        if sprite.offset_y > 0 {
            let skip = ((sprite.offset_y << 16) + ys - 1) / ys;
            y += skip;
            sy += skip * ys - (sprite.offset_y << 16);
        }
        if sprite.width < ow {
            w = ((sprite.width << 16) - sx + xs - 1) / xs;
        }
        if sprite.height < oh {
            h = ((sprite.height << 16) - sy + ys - 1) / ys;
        }
        let mut dest = x + y * self.width;
        let mut stride_gap = self.width - w;
        if y + h > self.height {
            h -= y + h - self.height;
        }
        if y < 0 {
            let cut = 0 - y;
            h -= cut;
            dest += cut * self.width;
            sy += ys * cut;
        }
        if x + w > self.width {
            let cut = x + w - self.width;
            w -= cut;
            stride_gap += cut;
        }
        if x < 0 {
            let cut = 0 - x;
            w -= cut;
            dest += cut;
            sx += xs * cut;
            stride_gap += cut;
        }
        // yz.af
        let start_sx = sx;
        for _ in 0..h.max(0) {
            let row = (sy >> 16) * sprite.width;
            for _ in 0..w.max(0) {
                // The original indexes unchecked; out-of-range only if the export were corrupt.
                let index = sprite
                    .indices
                    .get(((sx >> 16) + row) as usize)
                    .copied()
                    .unwrap_or(0);
                if index != 0
                    && let Some(p) = self.pixels.get_mut(dest as usize)
                {
                    *p = palette[index as usize] | 0xFF00_0000u32 as i32;
                }
                dest += 1;
                sx += xs;
            }
            sy += ys;
            sx = start_sx;
            dest += stride_gap;
        }
    }
}

/// `client.ed`: the per-tile minimap flags of every plane.
pub fn tile_flags(scene: &SceneData) -> Vec<i64> {
    let mut flags = vec![0i64; scene.tile_count];
    let exists = |index: usize| scene.flags.get(index).is_some_and(|f| f & 1 != 0);
    let stride = scene.plane_stride as usize;
    for plane in 0..4.min(scene.planes) {
        for x in 0..scene.width {
            for y in 0..scene.height {
                let index = scene.tile_index(plane, x, y);
                let mut f = flag::CANDIDATES;
                if let Some(paint) = scene.paints.get(&index) {
                    f |= i64::from(paint.rgb) & 0xFF_FFFF;
                }
                if let Some(model) = scene.tile_models.get(&index) {
                    f |= (i64::from(model.shape) & 15) << 50
                        | (i64::from(model.rotation) & 3) << 48
                        | (i64::from(model.overlay_rgb) & 0xFF_FFFF) << 24
                        | i64::from(model.underlay_rgb) & 0xFF_FFFF;
                }
                if plane == 0 && f & 0xFF_FFFF == 0 && scene.setting(1, x, y) & 2 != 0 {
                    f |= flag::BRIDGE;
                }
                if exists(index) && scene.setting(plane, x, y) & 24 == 0 {
                    f |= flag::TILE;
                }
                if plane < 3 && exists(index + stride) && scene.setting(plane + 1, x, y) & 8 != 0 {
                    f |= flag::TILE_ABOVE;
                }
                flags[index] = f;
            }
        }
    }
    flags
}

/// Per-call state of `client.bm` (`qy`, `wt`, `ue`, `mr`, `yi`).
struct View {
    scale: f64,
    start_x: i32,
    start_y: i32,
    off_x: i32,
    off_y: i32,
}

impl View {
    /// `client.nq`: quarter-tile x to raster x.
    fn px(&self, v: i32) -> i32 {
        let tile = v >> 2;
        let frac = v & 3;
        let d = f64::from(self.off_x) + f64::from(tile - self.start_x) * self.scale;
        let a = d as i32;
        let b = (d + self.scale) as i32;
        a + (((b - a) * frac) >> 2)
    }

    /// `client.sh`: quarter-tile y to raster y (measured from the bottom).
    fn py(&self, height: i32, v: i32) -> i32 {
        let tile = v >> 2;
        let frac = v & 3;
        let d = f64::from(height - self.off_y) - f64::from(tile - self.start_y) * self.scale;
        let a = d as i32;
        let b = (d - self.scale) as i32;
        a + (((b - a) * frac) >> 2)
    }
}

/// Counts of what the sweep drew, for telemetry and tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MinimapStats {
    pub terrain_tiles: u32,
    pub wall_marks: u32,
    pub diagonal_marks: u32,
    pub map_scenes: u32,
    /// Placements whose definition or wall config the inputs lacked (nothing invented).
    pub unresolved: u32,
    /// The first few unresolved placements, spelled out.
    pub notes: Vec<String>,
}

impl MinimapStats {
    const MAX_NOTES: usize = 8;

    fn note(&mut self, text: String) {
        if self.notes.len() < Self::MAX_NOTES {
            self.notes.push(text);
        }
    }
}

/// Draws the original minimap of `plane` into `raster`: `client.bm(world, sprite, scale, plane,
/// start_x, start_y, off_x, off_y)`. `overrides` replaces walls by tile index (door states).
#[allow(clippy::too_many_arguments)]
pub fn render(
    scene: &SceneData,
    plane: i32,
    assets: &MapScenes,
    colours: WallColours,
    overrides: &HashMap<usize, Wall>,
    scale: f64,
    (start_x, start_y): (i32, i32),
    (off_x, off_y): (i32, i32),
    raster: &mut Raster,
) -> Result<MinimapStats, RenderError> {
    if !(0..scene.planes).contains(&plane) {
        return Err(RenderError::Scene(format!(
            "minimap plane {plane} out of range"
        )));
    }
    raster.pixels.fill(1);
    let mut flags = tile_flags(scene);
    let view = View {
        scale,
        start_x,
        start_y,
        off_x,
        off_y,
    };
    let mut stats = MinimapStats::default();
    let pass = |terrain: bool,
                raster: &mut Raster,
                flags: &mut Vec<i64>,
                stats: &mut MinimapStats|
     -> Result<(), RenderError> {
        // client.pv
        let width = raster.width;
        let height = raster.height;
        let mut y = start_y;
        let mut bottom = f64::from(height - off_y);
        while y < scene.max_y {
            let top = bottom - scale;
            if (top as i32) < 0 {
                break;
            }
            let mut x = start_x;
            let mut left = f64::from(off_x);
            loop {
                if x < scene.max_x {
                    let right = left + scale;
                    if (right as i32) <= width {
                        let index = scene.tile_index(plane, x + scene.offset, y + scene.offset);
                        let f = flags[index];
                        let box_ = (left as i32, top as i32, right as i32, bottom as i32);
                        if f & flag::BRIDGE != 0 {
                            let target = index | (scene.plane_stride as usize * 3);
                            draw_tile(
                                scene, assets, colours, overrides, &view, terrain, target, x, y,
                                box_, raster, flags, stats,
                            )?;
                        }
                        if f & flag::TILE != 0 {
                            draw_tile(
                                scene, assets, colours, overrides, &view, terrain, index, x, y,
                                box_, raster, flags, stats,
                            )?;
                        }
                        if f & flag::TILE_ABOVE != 0 {
                            let target = index + scene.plane_stride as usize;
                            draw_tile(
                                scene, assets, colours, overrides, &view, terrain, target, x, y,
                                box_, raster, flags, stats,
                            )?;
                        }
                        left = right;
                        x += 1;
                        continue;
                    }
                }
                bottom = top;
                break;
            }
            y += 1;
        }
        Ok(())
    };
    pass(true, raster, &mut flags, &mut stats)?;
    pass(false, raster, &mut flags, &mut stats)?;
    Ok(stats)
}

#[allow(clippy::too_many_arguments)]
fn draw_tile(
    scene: &SceneData,
    assets: &MapScenes,
    colours: WallColours,
    overrides: &HashMap<usize, Wall>,
    view: &View,
    terrain: bool,
    index: usize,
    x: i32,
    y: i32,
    (px0, py0, px1, py1): (i32, i32, i32, i32),
    raster: &mut Raster,
    flags: &mut [i64],
    stats: &mut MinimapStats,
) -> Result<(), RenderError> {
    if index >= flags.len() {
        return Ok(());
    }
    if terrain {
        // client.nq -> client.wc
        let f = flags[index];
        terrain_fill(
            raster,
            &assets.shapes,
            ((f >> 50) & 15) as i32,
            ((f >> 48) & 3) as i32,
            ((f >> 24) & 0xFF_FFFF) as i32,
            (f & 0xFF_FFFF) as i32,
            px0,
            py0,
            px1 - px0,
            py1 - py0,
        );
        stats.terrain_tiles += 1;
        return Ok(());
    }
    // client.xx
    let mut f = flags[index];
    if f & flag::WALL != 0 {
        let wall = overrides.get(&index).or_else(|| scene.walls.get(&index));
        let mut drawn = false;
        if let Some(wall) = wall {
            if wall.config < 0 {
                // No placement config exported for this wall: nothing is invented.
                stats.unresolved += 1;
                stats.note(format!(
                    "wall at tile {x},{y} plane {} has no placement config (minimap sidecar missing)",
                    index >> scene.plane_shift
                ));
                flags[index] = f & !flag::WALL;
                return Ok(());
            }
            let kind = wall.config & 31;
            let rotation = wall.config >> 6 & 3;
            let colour = if tag_non_interactive(wall.hash) {
                colours.plain
            } else {
                colours.interactive
            };
            if f & flag::WALL_MAP_SCENE != 0 {
                if map_scene(scene, assets, view, wall.hash, x, y, raster, stats)? {
                    drawn = true;
                } else {
                    f &= !flag::WALL_MAP_SCENE;
                }
            }
            if f & flag::WALL_MAP_SCENE == 0 {
                if kind == 0 || kind == 2 {
                    drawn = true;
                    if rotation & 1 == 0 {
                        raster.vertical(
                            if rotation == 2 { px1 - 1 } else { px0 },
                            py0,
                            py1 - py0,
                            colour,
                        );
                    } else {
                        raster.horizontal(
                            px0,
                            if rotation == 3 { py1 - 1 } else { py0 },
                            px1 - px0,
                            colour,
                        );
                    }
                    stats.wall_marks += 1;
                }
                if kind == 3 {
                    drawn = true;
                    let left = rotation == 0 || rotation == 3;
                    let top = rotation < 2;
                    let w = ((px1 - px0) / 4).max(1);
                    let h = ((py1 - py0) / 4).max(1);
                    raster.fill(
                        if left { px0 } else { px1 - w },
                        if top { py0 } else { py1 - h },
                        w,
                        h,
                        colour,
                    );
                    stats.wall_marks += 1;
                }
                if kind == 2 {
                    if rotation & 1 == 1 {
                        raster.vertical(
                            if rotation == 1 { px1 - 1 } else { px0 },
                            py0,
                            py1 - py0,
                            colour,
                        );
                    } else {
                        raster.horizontal(
                            px0,
                            if rotation == 2 { py1 - 1 } else { py0 },
                            px1 - px0,
                            colour,
                        );
                    }
                    stats.wall_marks += 1;
                }
            }
        }
        if !drawn {
            f &= !flag::WALL;
        }
    }
    if f & flag::GAME_OBJECT != 0 {
        let mut drawn = false;
        let count = scene.object_count.get(index).copied().unwrap_or(0).max(0) as usize;
        let object = (0..count.min(5))
            .filter_map(|slot| scene.slots.get(&(index * 5 + slot)))
            .map(|&id| &scene.game_objects[id])
            .find(|o| (o.hash >> 16) & 7 == 2 && o.min_x == x && o.min_y == y);
        if let Some(object) = object {
            let kind = object.config & 31;
            let rotation = object.config >> 6 & 3;
            if map_scene(scene, assets, view, object.hash, x, y, raster, stats)? {
                drawn = true;
            } else if kind == 9 {
                drawn = true;
                let colour = if tag_non_interactive(object.hash) {
                    0xEEEEEE
                } else {
                    0xEE0000
                };
                let (ya, yb) = if rotation != 0 && rotation != 2 {
                    (py0, py1 - 1)
                } else {
                    (py1 - 1, py0)
                };
                raster.line(px0, ya, px1 - 1, yb, colour);
                stats.diagonal_marks += 1;
            }
        }
        if !drawn {
            f &= !flag::GAME_OBJECT;
        }
    }
    if f & flag::FLOOR_DECORATION != 0 {
        let mut drawn = false;
        if let Some(decor) = scene.floor_decorations.get(&index)
            && map_scene(scene, assets, view, decor.hash, x, y, raster, stats)?
        {
            drawn = true;
        }
        if !drawn {
            f &= !flag::FLOOR_DECORATION;
        }
    }
    flags[index] = f;
    Ok(())
}

/// `client.ga`: the object definition's map-scene sprite centred on its footprint. Returns
/// whether the definition has a map scene (drawn or not), as the original does.
#[allow(clippy::too_many_arguments)]
fn map_scene(
    scene: &SceneData,
    assets: &MapScenes,
    view: &View,
    hash: i64,
    x: i32,
    y: i32,
    raster: &mut Raster,
    stats: &mut MinimapStats,
) -> Result<bool, RenderError> {
    let id = tag_object_id(hash);
    let Some(def) = scene.object_defs.get(&id) else {
        stats.unresolved += 1;
        stats.note(format!(
            "object definition {id} at tile {x},{y} is not in the minimap sidecar"
        ));
        return Ok(false);
    };
    if def.map_scene == -1 {
        return Ok(false);
    }
    let Some(sprite) = assets.sprites.get(def.map_scene as usize) else {
        return Ok(true);
    };
    if sprite.width <= 0 || sprite.height <= 0 {
        return Ok(true);
    }
    let span_x = def.size_x * 4;
    let span_y = def.size_y * 4;
    let qx0 = x * 4 + (span_x - sprite.width) / 2;
    let qy0 = y * 4 + (span_y - sprite.height) / 2;
    let qx1 = qx0 + sprite.width;
    let qy1 = qy0 + sprite.height;
    let rx0 = view.px(qx0);
    let ry0 = view.py(raster.height, qy0);
    let rx1 = view.px(qx1);
    let ry1 = view.py(raster.height, qy1);
    let w = (rx1 - rx0) * sprite.original_width / sprite.width;
    let h = (ry0 - ry1) * sprite.original_height / sprite.height;
    if w > 0 && h > 0 {
        raster.indexed_scaled(sprite, &assets.palette, rx0, ry1, w, h);
        stats.map_scenes += 1;
    }
    Ok(true)
}

/// `client.wc`: one tile's terrain fill from its minimap flags.
#[allow(clippy::too_many_arguments)]
fn terrain_fill(
    raster: &mut Raster,
    shapes: &[i64; 16],
    shape: i32,
    rotation: i32,
    mut overlay: i32,
    mut underlay: i32,
    px: i32,
    py: i32,
    w: i32,
    h: i32,
) {
    let width = raster.width;
    let x0 = px.max(0);
    let x1 = (px + w).min(width);
    let y0 = py.max(0);
    let y1 = (py + h).min(raster.height);
    let under_empty = i32::from(underlay == 0);
    overlay = overlay.wrapping_sub(underlay);
    if shape <= 1 {
        if under_empty != 1 || shape != 0 {
            underlay = underlay.wrapping_add(overlay.wrapping_mul(shape));
            for yy in y0..y1 {
                let row = yy * width;
                for xx in x0..x1 {
                    raster.pixels[(row + xx) as usize] = underlay;
                }
            }
        }
        return;
    }
    if w <= 0 || h <= 0 {
        return;
    }
    let mask = shapes[shape as usize & 15];
    // Java long shifts mask the count by 63.
    let xs = ((0x1012_1519_202A_4080i64 >> (((w - 1) * 8) & 63)) as i32) & 0xFF;
    let ys = ((0x1012_1519_202A_4080i64 >> (((h - 1) * 8) & 63)) as i32) & 0xFF;
    let bx = px * 8 - 4;
    let by = py * 8 - 4;
    let code = (0x0871_7178_7801_0108i64 >> ((rotation << 4) & 63)) as i32;
    for yy in y0..y1 {
        let a = code >> 4 & 15;
        let b = code & 15;
        let row_off = ((((yy * 8 - by) * ys) >> 7) ^ a) * b;
        let row = yy * width;
        for xx in x0..x1 {
            let c = code >> 12 & 15;
            let d = code >> 8 & 15;
            let col_off = ((((xx * 8 - bx) * xs) >> 7) ^ c) * d;
            let bit = ((mask >> (row_off + col_off)) as i32) & 1;
            let mut colour = underlay.wrapping_add(bit.wrapping_mul(overlay));
            let keep = !bit & under_empty;
            let i = (row + xx) as usize;
            colour |= raster.pixels[i].wrapping_mul(keep);
            raster.pixels[i] = colour;
        }
    }
}

/// RGBA bytes (alpha 255 everywhere, as the native capture wrote them) and a coverage mask
/// (1 where the sweep drew, 0 where the fill value `1` survived).
pub fn to_rgba(raster: &Raster) -> (Vec<u8>, Vec<u8>) {
    let mut rgba = Vec::with_capacity(raster.pixels.len() * 4);
    let mut mask = Vec::with_capacity(raster.pixels.len());
    for &p in &raster.pixels {
        rgba.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8, 255]);
        mask.push(u8::from(p != 1));
    }
    (rgba, mask)
}

/// Where one minimap marker sprite lands on the minimap widget: the exact `client.zr` → `bo.as`
/// arithmetic. Everything after the fine-unit offset is in **minimap pixels** (at the stock
/// scale `1/32` a tile is 4 px), not source fine units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconPlacement {
    /// `bo.as` sprite-canvas top-left relative to the widget origin: `W/2 + rx − maxWidth/2`,
    /// `H/2 − ry − maxHeight/2` (the sprite's full canvas size, `ym.aw/ak`).
    pub x: i32,
    pub y: i32,
    /// Where the sprite's stored sub-image (`width × height` pixels) actually lands, relative
    /// to the widget origin. Plain blit (`aap.ro`): canvas position + the sprite's own
    /// `offsetX/offsetY`. Clipped blit (`ym.bc`): the canvas position itself — `bc` does not add
    /// the sprite offsets — with each row limited to the widget sprite's mask span
    /// (`kh.ab[row]..+kh.ae[row]`) and rows to `0..H`.
    pub draw_x: i32,
    pub draw_y: i32,
    /// `dx² + dy² > 2500` (beyond 50 px): drawn through the widget mask (`ym.bc`) instead of the
    /// plain blit (`aap.ro`). In both paths value-0 sprite pixels are skipped.
    pub clipped: bool,
    /// Scaled, unrotated offset from the player in minimap pixels (`(int)(fine * scale)`).
    pub dx: i32,
    pub dy: i32,
}

/// Stock minimap scale (`client.fa` → `zo(widget, x, y, 0.03125F)`): fine units → pixels, i.e.
/// 128 fine units (one tile) → 4 px. The RuneLite zoom path uses `zoom / 128` instead.
pub const MINIMAP_STOCK_SCALE: f32 = 0.03125;
/// `bo.as`: markers farther than this (squared pixels; 80 px = 20 tiles at the stock scale) are
/// not drawn.
pub const MINIMAP_ICON_MAX_DISTANCE_SQ: i32 = 6400;
/// `bo.as`: markers beyond this (squared pixels; 50 px) are drawn clipped to the widget mask.
pub const MINIMAP_ICON_CLIP_DISTANCE_SQ: i32 = 2500;

/// `client.zr` + `bo.as` for one map-element marker at scene tile `(tile_x, tile_y)` (scene-local
/// tiles, as `ba.au/ai` hold them) around the player's fine scene position `(player_fine_x,
/// player_fine_y)` (`client.np/nq`, 128 units per tile): `dx = ((tile << 7) + 64 - player) *
/// scale` (truncated), skipped when `dx² + dy² > 6400`, rotated by the minimap angle
/// (`up.aj`/`up.rj`: 16384-step 16.16 sine/cosine, `>> 16`), then placed at
/// `(W/2 + rx - maxWidth/2, H/2 - ry - maxHeight/2)` inside a `widget_w × widget_h` widget for
/// a sprite whose canvas is `sprite_max_w × sprite_max_h` with its sub-image at
/// `(sprite_offset_x, sprite_offset_y)` (`MapIconSprite` fields). `None` when the marker is out
/// of range.
#[allow(clippy::too_many_arguments)]
pub fn icon_placement(
    tile_x: i32,
    tile_y: i32,
    player_fine_x: i32,
    player_fine_y: i32,
    scale: f32,
    minimap_angle: i32,
    widget_w: i32,
    widget_h: i32,
    sprite_max_w: i32,
    sprite_max_h: i32,
    sprite_offset_x: i32,
    sprite_offset_y: i32,
) -> Option<IconPlacement> {
    let dx_fine = (tile_x << 7) + 64 - player_fine_x;
    let dy_fine = (tile_y << 7) + 64 - player_fine_y;
    // Java: `(int)(var6 * var3)` — float multiply, truncation toward zero.
    let dx = (dx_fine as f32 * scale) as i32;
    let dy = (dy_fine as f32 * scale) as i32;
    let dist_sq = dy * dy + dx * dx;
    if dist_sq > MINIMAP_ICON_MAX_DISTANCE_SQ {
        return None;
    }
    let t = crate::tables::tables();
    let angle = (minimap_angle & 16383) as usize;
    let sin = t.sin16384[angle];
    let cos = t.cos16384[angle];
    let rx = (dy * sin + dx * cos) >> 16;
    let ry = (cos * dy - sin * dx) >> 16;
    let clipped = dist_sq > MINIMAP_ICON_CLIP_DISTANCE_SQ;
    let x = widget_w / 2 + rx - sprite_max_w / 2;
    let y = widget_h / 2 - ry - sprite_max_h / 2;
    Some(IconPlacement {
        x,
        y,
        draw_x: if clipped { x } else { x + sprite_offset_x },
        draw_y: if clipped { y } else { y + sprite_offset_y },
        clipped,
        dx,
        dy,
    })
}

#[cfg(test)]
mod icon_placement_tests {
    use super::*;

    #[test]
    fn stock_scale_maps_one_tile_to_four_pixels_and_radius_80_to_twenty_tiles() {
        // Player at the centre of tile (50, 50): fine (50 * 128 + 64).
        let (px, py) = (50 * 128 + 64, 50 * 128 + 64);
        let place = |tx: i32, ty: i32, angle: i32| {
            icon_placement(
                tx,
                ty,
                px,
                py,
                MINIMAP_STOCK_SCALE,
                angle,
                152,
                152,
                15,
                15,
                0,
                0,
            )
        };
        // Same tile: centred (sprite 15x15 → top-left 76 − 7 = 69).
        let here = place(50, 50, 0).unwrap();
        assert_eq!(
            (here.dx, here.dy, here.x, here.y, here.clipped),
            (0, 0, 69, 69, false)
        );
        // One tile east = 4 px right; one tile north (+y) = 4 px up (screen y decreases).
        let east = place(51, 50, 0).unwrap();
        assert_eq!((east.dx, east.dy, east.x, east.y), (4, 0, 73, 69));
        let north = place(50, 51, 0).unwrap();
        assert_eq!((north.dx, north.dy, north.x, north.y), (0, 4, 69, 65));
        // 20 tiles = 80 px is the last drawn distance; 21 tiles (84 px) is not drawn.
        assert!(place(70, 50, 0).is_some_and(|p| p.clipped && p.dx == 80));
        assert!(place(71, 50, 0).is_none());
        assert!(place(50, 29, 0).is_none());
        // 12 tiles (48 px) plain; 13 tiles (52 px) clipped to the widget mask.
        assert!(!place(62, 50, 0).unwrap().clipped);
        assert!(place(63, 50, 0).unwrap().clipped);
        // Diagonal: 15 tiles east + 14 north = 60² + 56² = 6736 > 6400 → not drawn, although
        // each axis alone is within range.
        assert!(place(65, 64, 0).is_none());
        // Player off the tile centre: the fine offset is kept before scaling (truncation).
        let off = icon_placement(
            51,
            50,
            px + 40,
            py,
            MINIMAP_STOCK_SCALE,
            0,
            152,
            152,
            15,
            15,
            0,
            0,
        )
        .unwrap();
        assert_eq!(off.dx, ((128 - 40) as f32 * MINIMAP_STOCK_SCALE) as i32); // 2.75 → 2
        assert_eq!(off.dx, 2);
        // Sprite canvas vs sub-image: the plain blit adds the sprite's own offsets, the clipped
        // blit (`ym.bc`) does not.
        let plain = icon_placement(
            52,
            50,
            px,
            py,
            MINIMAP_STOCK_SCALE,
            0,
            152,
            152,
            15,
            15,
            3,
            2,
        )
        .unwrap();
        assert_eq!(
            (plain.x, plain.y, plain.draw_x, plain.draw_y, plain.clipped),
            (77, 69, 80, 71, false)
        );
        let masked = icon_placement(
            65,
            50,
            px,
            py,
            MINIMAP_STOCK_SCALE,
            0,
            152,
            152,
            15,
            15,
            3,
            2,
        )
        .unwrap();
        assert_eq!(
            (masked.x, masked.draw_x, masked.draw_y, masked.clipped),
            (129, 129, 69, true)
        );
    }

    #[test]
    fn rotation_uses_the_16384_step_fixed_point_tables() {
        let (px, py) = (50 * 128 + 64, 50 * 128 + 64);
        // A quarter turn (4096): one tile east lands one tile up (rx = dy·sin + dx·cos with
        // sin = 65536, cos = 0 → rx = 0, ry = −dx → y = H/2 + 4 ...). Check against the tables.
        let t = crate::tables::tables();
        for angle in [0, 1024, 4096, 6000, 8192, 12288, 16000] {
            let p = icon_placement(
                53,
                50,
                px,
                py,
                MINIMAP_STOCK_SCALE,
                angle,
                152,
                152,
                15,
                15,
                0,
                0,
            )
            .unwrap();
            let (dx, dy) = (12, 0);
            let rx = (dy * t.sin16384[angle as usize] + dx * t.cos16384[angle as usize]) >> 16;
            let ry = (t.cos16384[angle as usize] * dy - t.sin16384[angle as usize] * dx) >> 16;
            assert_eq!((p.x, p.y), (76 + rx - 7, 76 - ry - 7), "angle {angle}");
        }
        let quarter = icon_placement(
            53,
            50,
            px,
            py,
            MINIMAP_STOCK_SCALE,
            4096,
            152,
            152,
            15,
            15,
            0,
            0,
        )
        .unwrap();
        assert_eq!(
            (quarter.x, quarter.y),
            (69, 69 + 12),
            "east marker turns to screen-down at 4096"
        );
        // Angles wrap (`& 16383`).
        let wrapped = icon_placement(
            53,
            50,
            px,
            py,
            MINIMAP_STOCK_SCALE,
            4096 + 16384,
            152,
            152,
            15,
            15,
            0,
            0,
        )
        .unwrap();
        assert_eq!((wrapped.x, wrapped.y), (quarter.x, quarter.y));
    }

    #[test]
    fn zoomed_scale_changes_pixels_per_tile_not_the_pixel_thresholds() {
        let (px, py) = (50 * 128 + 64, 50 * 128 + 64);
        // RuneLite zoom 8 → scale 8/128 = 1/16: one tile is 8 px, so the 80 px cutoff is 10 tiles.
        let scale = 8.0f32 / 128.0;
        assert_eq!(
            icon_placement(51, 50, px, py, scale, 0, 152, 152, 15, 15, 0, 0)
                .unwrap()
                .dx,
            8
        );
        assert!(icon_placement(60, 50, px, py, scale, 0, 152, 152, 15, 15, 0, 0).is_some());
        assert!(icon_placement(61, 50, px, py, scale, 0, 152, 152, 15, 15, 0, 0).is_none());
    }
}

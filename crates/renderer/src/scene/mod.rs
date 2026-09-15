//! Original scene structures (`ez`) as exported by `tools/render-assets` and the exact port of
//! the original scene traversal that turns them into the ordered triangle stream.

pub mod draw;
pub mod tile;
pub mod visibility;

use std::collections::HashMap;

use crate::chunk::Chunks;
use crate::error::RenderError;

/// Tile flag bits (`xj`).
pub mod flag {
    pub const EXISTS: i32 = 1;
    pub const DRAW_PRIMARY: i32 = 2;
    pub const VISIBLE: i32 = 4;
    pub const DRAW_OBJECTS: i32 = 8;
    pub const WALL_DEFERRED: i32 = 16;
    pub const BRIDGE_BELOW: i32 = 32;
    pub const FORCE_PLANE_0: i32 = 64;
    pub const ZONE_DYNAMIC: i32 = 128;
    pub const PAINT: i32 = 256;
    pub const PAINT_VISIBLE: i32 = 512;
    pub const TILE_MODEL: i32 = 1024;
    pub const FLOOR_DECOR: i32 = 2048;
    pub const ITEM_LAYER: i32 = 4096;
    pub const ITEM_LAYER_DEFERRED: i32 = 8192;
    pub const WALL: i32 = 16384;
    pub const WALL_DECOR: i32 = 32768;
}

#[derive(Clone, Debug)]
pub struct TilePaint {
    pub sw: i32,
    pub se: i32,
    pub ne: i32,
    pub nw: i32,
    pub texture: i32,
    pub flat: bool,
    pub rgb: i32,
}

#[derive(Clone, Debug)]
pub struct TileModel {
    pub shape: i32,
    pub rotation: i32,
    pub flat: bool,
    pub underlay_rgb: i32,
    pub overlay_rgb: i32,
    pub xs: Vec<i32>,
    pub ys: Vec<i32>,
    pub zs: Vec<i32>,
    pub face_a: Vec<i32>,
    pub face_b: Vec<i32>,
    pub face_c: Vec<i32>,
    pub color_a: Vec<i32>,
    pub color_b: Vec<i32>,
    pub color_c: Vec<i32>,
    pub textures: Option<Vec<i32>>,
}

#[derive(Clone, Debug)]
pub struct Wall {
    pub model_a: i32,
    pub model_b: i32,
    pub orientation_a: i32,
    pub orientation_b: i32,
    pub x: i32,
    pub height: i32,
    pub z: i32,
    pub hash: i64,
}

#[derive(Clone, Debug)]
pub struct WallDecoration {
    pub model_a: i32,
    pub model_b: i32,
    pub orientation: i32,
    pub orientation2: i32,
    pub x: i32,
    pub height: i32,
    pub z: i32,
    pub offset_x: i32,
    pub offset_z: i32,
    pub offset_x2: i32,
    pub offset_z2: i32,
    pub hash: i64,
}

#[derive(Clone, Debug)]
pub struct FloorDecoration {
    pub model: i32,
    pub x: i32,
    pub height: i32,
    pub z: i32,
    pub hash: i64,
}

#[derive(Clone, Debug)]
pub struct GameObject {
    pub model: i32,
    pub orientation: i32,
    pub x: i32,
    pub height: i32,
    pub z: i32,
    /// Tile span in main-area coordinates (without the extension offset).
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
    pub config: i32,
    /// `fm`: span mask of this tile relative to the object.
    pub slot_flag: i32,
    pub dynamic: bool,
    pub hash: i64,
}

/// Static scene data straight from the exported original structures.
#[derive(Clone, Debug)]
pub struct SceneData {
    pub name: String,
    pub base_x: i32,
    pub base_y: i32,
    pub width: i32,
    pub height: i32,
    pub planes: i32,
    pub draw_distance: i32,
    /// `oy`: extension offset of the main 104x104 area inside the extended grid.
    pub offset: i32,
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
    pub main_scene: bool,
    pub roof_mode: i32,
    /// `fb`: bits for y in the tile index; `xu`: bits for x+y (plane shift).
    pub y_bits: i32,
    pub plane_shift: i32,
    pub plane_stride: i32,
    pub x_stride: i32,
    pub tile_count: usize,
    pub min_level: i32,
    pub flags: Vec<i32>,
    pub link: Vec<i8>,
    pub object_count: Vec<i8>,
    pub object_flags: Vec<i8>,
    heights: Vec<i32>,
    roofs: Vec<i32>,
    pub paints: HashMap<usize, TilePaint>,
    pub tile_models: HashMap<usize, TileModel>,
    pub walls: HashMap<usize, Wall>,
    pub wall_decorations: HashMap<usize, WallDecoration>,
    pub floor_decorations: HashMap<usize, FloorDecoration>,
    /// Distinct game objects (a multi-tile object appears once here).
    pub game_objects: Vec<GameObject>,
    /// Game object slot (`tile_index * 5 + slot`) to object id.
    pub slots: HashMap<usize, usize>,
    /// Dynamic zone object ids keyed by zone (x >> 3, y >> 3) in extended coordinates.
    pub zone_dynamic: HashMap<(i32, i32), Vec<usize>>,
    /// Model content hashes referenced by index from the records above.
    pub model_keys: Vec<String>,
}

impl SceneData {
    pub fn from_chunks(data: &[u8]) -> Result<Self, RenderError> {
        let chunks = Chunks::parse(data)?;
        let h = chunks.ints("SCHD")?;
        if h.len() < 19 {
            return Err(RenderError::Format("scene header".into()));
        }
        let width = h[2];
        let height = h[3];
        let planes = h[4];
        let tile_count = h[17] as usize;
        let flags = chunks.ints("FLAG")?;
        if flags.len() != tile_count {
            return Err(RenderError::InvalidAsset("scene flag array size".into()));
        }
        let heights = chunks.ints("HGHT")?;
        if heights.len() != (planes * (width + 1) * (height + 1)) as usize {
            return Err(RenderError::InvalidAsset("scene heights size".into()));
        }
        let roofs = chunks.ints("ROOF")?;
        let mut scene = SceneData {
            name: chunks.text("NAME")?,
            base_x: h[0],
            base_y: h[1],
            width,
            height,
            planes,
            draw_distance: h[5],
            offset: h[6],
            min_x: h[7],
            max_x: h[8],
            min_y: h[9],
            max_y: h[10],
            main_scene: h[11] != 0,
            roof_mode: h[12],
            y_bits: h[13],
            plane_shift: h[14],
            plane_stride: h[15],
            x_stride: h[16],
            tile_count,
            min_level: h[18],
            flags,
            link: chunks.bytes("LINK")?,
            object_count: chunks.bytes("OBJC")?,
            object_flags: chunks.bytes("OBJF")?,
            heights,
            roofs,
            paints: HashMap::new(),
            tile_models: HashMap::new(),
            walls: HashMap::new(),
            wall_decorations: HashMap::new(),
            floor_decorations: HashMap::new(),
            game_objects: Vec::new(),
            slots: HashMap::new(),
            zone_dynamic: HashMap::new(),
            model_keys: chunks
                .text("MODL")?
                .lines()
                .map(|s| s.to_string())
                .collect(),
        };
        if scene.link.len() != tile_count
            || scene.object_count.len() != tile_count
            || scene.object_flags.len() != tile_count * 5
        {
            return Err(RenderError::InvalidAsset(
                "scene per-tile arrays size".into(),
            ));
        }
        let paints = chunks.ints("PANT")?;
        for r in paints.as_chunks::<8>().0 {
            scene.paints.insert(
                r[0] as usize,
                TilePaint {
                    sw: r[1],
                    se: r[2],
                    ne: r[3],
                    nw: r[4],
                    texture: r[5],
                    flat: r[6] != 0,
                    rgb: r[7],
                },
            );
        }
        let tm = chunks.ints("TMOD")?;
        let mut cursor = 0usize;
        while cursor < tm.len() {
            if cursor + 9 > tm.len() {
                return Err(RenderError::Format("tile model header truncated".into()));
            }
            let head: Vec<i32> = tm[cursor..cursor + 9].to_vec();
            let vcount = head[6] as usize;
            let fcount = head[7] as usize;
            let has_tex = head[8] != 0;
            cursor += 9;
            let mut take = |n: usize| -> Result<Vec<i32>, RenderError> {
                let end = cursor + n;
                if end > tm.len() {
                    return Err(RenderError::Format("tile model truncated".into()));
                }
                let v = tm[cursor..end].to_vec();
                cursor = end;
                Ok(v)
            };
            let model = TileModel {
                shape: head[1],
                rotation: head[2],
                flat: head[3] != 0,
                underlay_rgb: head[4],
                overlay_rgb: head[5],
                xs: take(vcount)?,
                ys: take(vcount)?,
                zs: take(vcount)?,
                face_a: take(fcount)?,
                face_b: take(fcount)?,
                face_c: take(fcount)?,
                color_a: take(fcount)?,
                color_b: take(fcount)?,
                color_c: take(fcount)?,
                textures: if has_tex { Some(take(fcount)?) } else { None },
            };
            for arr in [&model.face_a, &model.face_b, &model.face_c] {
                if arr.iter().any(|&i| i < 0 || i as usize >= vcount) {
                    return Err(RenderError::InvalidAsset("tile model face index".into()));
                }
            }
            scene.tile_models.insert(head[0] as usize, model);
        }
        for r in chunks.ints("WALL")?.as_chunks::<10>().0 {
            scene.walls.insert(
                r[0] as usize,
                Wall {
                    model_a: r[1],
                    model_b: r[2],
                    orientation_a: r[3],
                    orientation_b: r[4],
                    x: r[5],
                    height: r[6],
                    z: r[7],
                    hash: join(r[8], r[9]),
                },
            );
        }
        for r in chunks.ints("WDEC")?.as_chunks::<14>().0 {
            scene.wall_decorations.insert(
                r[0] as usize,
                WallDecoration {
                    model_a: r[1],
                    model_b: r[2],
                    orientation: r[3],
                    orientation2: r[4],
                    x: r[5],
                    height: r[6],
                    z: r[7],
                    offset_x: r[8],
                    offset_z: r[9],
                    offset_x2: r[10],
                    offset_z2: r[11],
                    hash: join(r[12], r[13]),
                },
            );
        }
        for r in chunks.ints("FDEC")?.as_chunks::<7>().0 {
            scene.floor_decorations.insert(
                r[0] as usize,
                FloorDecoration {
                    model: r[1],
                    x: r[2],
                    height: r[3],
                    z: r[4],
                    hash: join(r[5], r[6]),
                },
            );
        }
        // The same original object instance is referenced from every tile it spans; group the
        // per-slot records back into one object so draw-state (frame marker, distance) is shared.
        let mut identity: HashMap<(i64, i32, i32, i32, i32, i32), usize> = HashMap::new();
        let mut intern = |scene: &mut SceneData, object: GameObject| -> usize {
            let key = (
                object.hash,
                object.x,
                object.z,
                object.height,
                object.orientation,
                object.model,
            );
            if let Some(&id) = identity.get(&key) {
                return id;
            }
            let id = scene.game_objects.len();
            scene.game_objects.push(object);
            identity.insert(key, id);
            id
        };
        for r in chunks.ints("GOBJ")?.as_chunks::<16>().0 {
            let object = GameObject {
                model: r[2],
                orientation: r[3],
                x: r[4],
                height: r[5],
                z: r[6],
                min_x: r[7],
                max_x: r[8],
                min_y: r[9],
                max_y: r[10],
                config: r[11],
                slot_flag: r[12],
                dynamic: r[13] != 0,
                hash: join(r[14], r[15]),
            };
            let id = intern(&mut scene, object);
            scene.slots.insert(r[0] as usize * 5 + r[1] as usize, id);
        }
        for r in chunks.ints("ZDYN")?.as_chunks::<14>().0 {
            let object = GameObject {
                model: r[2],
                orientation: r[3],
                x: r[4],
                height: r[5],
                z: r[6],
                min_x: r[7],
                max_x: r[8],
                min_y: r[9],
                max_y: r[10],
                config: r[11],
                slot_flag: 0,
                dynamic: true,
                hash: join(r[12], r[13]),
            };
            let id = intern(&mut scene, object);
            scene.zone_dynamic.entry((r[0], r[1])).or_default().push(id);
        }
        let model_count = scene.model_keys.len() as i32;
        let check = |m: i32| -> Result<(), RenderError> {
            if m < -1 || m >= model_count {
                return Err(RenderError::InvalidAsset(format!(
                    "scene model reference {m} out of range"
                )));
            }
            Ok(())
        };
        for w in scene.walls.values() {
            check(w.model_a)?;
            check(w.model_b)?;
        }
        for d in scene.wall_decorations.values() {
            check(d.model_a)?;
            check(d.model_b)?;
        }
        for f in scene.floor_decorations.values() {
            check(f.model)?;
        }
        for g in &scene.game_objects {
            check(g.model)?;
        }
        Ok(scene)
    }

    /// `ez.vy(plane, x, y)` in extended coordinates.
    #[inline]
    pub fn tile_index(&self, plane: i32, x: i32, y: i32) -> usize {
        ((plane << self.plane_shift) | (x << self.y_bits) | y) as usize
    }

    #[inline]
    pub fn height(&self, plane: i32, x: i32, y: i32) -> i32 {
        let w = (self.width + 1) as usize;
        let h = (self.height + 1) as usize;
        self.heights[plane as usize * w * h + x as usize * h + y as usize]
    }

    #[inline]
    pub fn roof(&self, plane: i32, x: i32, y: i32) -> i32 {
        let w = self.width as usize;
        let h = self.height as usize;
        self.roofs[plane as usize * w * h + x as usize * h + y as usize]
    }

    /// Decodes a tile index into (plane, x, y) in extended coordinates.
    #[inline]
    pub fn decode_index(&self, index: usize) -> (i32, i32, i32) {
        let i = index as i32;
        let plane = (i >> self.plane_shift) & 3;
        let x = (i >> self.y_bits) & ((1 << (self.plane_shift - self.y_bits)) - 1);
        let y = i & ((1 << self.y_bits) - 1);
        (plane, x, y)
    }
}

fn join(lo: i32, hi: i32) -> i64 {
    ((hi as i64) << 32) | (lo as u32 as i64)
}

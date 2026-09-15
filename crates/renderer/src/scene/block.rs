//! World blocks: 64x64 map squares exported by `tools/render-assets` (`blocks` profile) and
//! assembled here into a 104x104 scene around any chunk-aligned base, the way the original client
//! rebuilds its scene when the player approaches a region edge. Every block carries the exact
//! original per-tile arrays, placements (square-relative), baked animated scenery frame sets and
//! its own model pack; assembly re-indexes them into the extended 184x184 grid.

use std::collections::HashMap;

use crate::chunk::Chunks;
use crate::error::RenderError;
use crate::model::Model;
use crate::scene::{
    FloorDecoration, GameObject, MapObjectDef, SceneData, TileModel, TilePaint, Wall,
    WallDecoration,
};

pub const BLOCK_SIZE: i32 = 64;
const GRID: i32 = 184;
const OFFSET: i32 = 40;
const MAIN: i32 = 104;
const PLANES: i32 = 4;

/// Baked animated scenery: one lit, contoured model per source frame plus the sequence timing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimatedSet {
    /// `ou.bu`: frame step applied when the sequence passes its last frame (-1 = one-shot).
    pub frame_step: i32,
    /// `ou.bf`: maximum loops (99 by default).
    pub max_loops: i32,
    /// `ou.bo`: total sequence length in client cycles.
    pub total_cycles: i32,
    /// Model reference per frame (indices into the scene model list).
    pub models: Vec<i32>,
    /// Frame lengths in 20 ms client cycles.
    pub lengths: Vec<i32>,
}

impl AnimatedSet {
    /// Port of the sequence advance (`rd.az`) applied from a start state `(frame, cycle)` over
    /// `elapsed` client cycles: the frame shown, or `None` once a one-shot sequence
    /// (`frame_step == -1`) has finished — the original then resets the renderable to its plain
    /// model. A displayed state keeps `cycle` within `0..=lengths[frame]`; the loop only moves on
    /// once the cycle exceeds the frame length, exactly like the original `while` loop.
    pub fn frame_at(&self, start_frame: i32, start_cycle: i32, elapsed: i64) -> Option<usize> {
        self.advance(start_frame, start_cycle, elapsed)
            .map(|(frame, _)| frame)
    }

    /// The full controller state `(frame, cycle within the frame)` after the advance — what the
    /// original `qr` holds after `dy.rf` ran for `elapsed` cycles (`rd.az`: `cycle += elapsed`,
    /// then `while cycle > length[frame]` move on). `None` once a one-shot sequence finished.
    pub fn advance(
        &self,
        start_frame: i32,
        start_cycle: i32,
        elapsed: i64,
    ) -> Option<(usize, i64)> {
        let n = self.lengths.len() as i32;
        if n == 0 || self.models.len() < n as usize {
            return None;
        }
        let len = |f: i32| i64::from(self.lengths[f as usize].max(0));
        let mut frame = start_frame;
        let mut cycle = i64::from(start_cycle.max(0));
        if frame < 0 || frame >= n {
            frame = 0;
            cycle = 0;
        }
        cycle += elapsed.max(0);
        // Frames revisited after the first pass: `n - frame_step .. n` (all frames when the step
        // falls outside the sequence, which resets to frame 0).
        let loop_start = if self.frame_step >= 1 && self.frame_step <= n {
            n - self.frame_step
        } else {
            0
        };
        let period: i64 = (loop_start..n).map(len).sum();
        let mut guard = 0;
        while cycle > len(frame) {
            if period > 0 && frame >= loop_start && cycle > period + len(frame) {
                // Skip whole loops: a full pass over the looping frames returns to this frame
                // with the cycle reduced by the period.
                let rounds = (cycle - len(frame) - 1) / period;
                cycle -= rounds * period;
                continue;
            }
            if period <= 0 && frame >= loop_start {
                // Every looping frame has length 0: the original spins to the frame whose length
                // is exceeded; treat as the loop start.
                return Some((loop_start as usize, 0));
            }
            cycle -= len(frame);
            frame += 1;
            if frame >= n {
                if self.frame_step == -1 {
                    return None;
                }
                frame -= self.frame_step;
                if frame < 0 || frame >= n {
                    frame = 0;
                }
            }
            guard += 1;
            if guard > 4 * n as i64 + 8 {
                break;
            }
        }
        Some((frame as usize, cycle))
    }
}

/// Per-placement animation instance: which set and the original random start state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimatedInstance {
    pub set: usize,
    pub start_frame: i32,
    pub start_cycle: i32,
    /// Model shown once a one-shot sequence finished (the plain object model).
    pub plain: i32,
}

#[derive(Clone, Debug)]
struct Placed<T> {
    plane: i32,
    bx: i32,
    by: i32,
    record: T,
}

#[derive(Clone, Debug)]
struct BlockObject {
    plane: i32,
    bx: i32,
    by: i32,
    slot: i32,
    object: GameObject,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub square: i32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub roof_mode: i32,
    pub min_level: i32,
    flags: Vec<i32>,
    link: Vec<i8>,
    object_count: Vec<i8>,
    object_flags: Vec<i8>,
    heights: Vec<i32>,
    roofs: Vec<i32>,
    settings: Vec<i8>,
    paints: Vec<Placed<TilePaint>>,
    tile_models: Vec<Placed<TileModel>>,
    walls: Vec<Placed<Wall>>,
    wall_decorations: Vec<Placed<WallDecoration>>,
    floor_decorations: Vec<Placed<FloorDecoration>>,
    objects: Vec<BlockObject>,
    animated: Vec<AnimatedSet>,
    pub model_keys: Vec<String>,
    /// Object-definition minimap fields from the `minimap/blocks/<square>.bin` sidecar.
    pub object_defs: HashMap<i32, MapObjectDef>,
    /// The original minimap icon pass (`bu.aa`) recorded by the sidecar per plane:
    /// `(plane, bx, by, map element)` — the reference the renderer's own icon list is checked
    /// against, not a substitute for it.
    pub source_icons: Vec<(i32, i32, i32, i32)>,
    /// Whether the minimap sidecar (wall configs + definitions) has been attached.
    pub minimap_ready: bool,
    /// The square's raw terrain and scenery shadows (`BTER`/`BSHD`), for the assembly-time
    /// terrain pass; `None` for blocks exported before they were carried.
    pub raw_terrain: Option<super::terrain::RawTerrain>,
}

fn join(lo: i32, hi: i32) -> i64 {
    ((hi as i64) << 32) | (lo as u32 as i64)
}

impl Block {
    /// The exported (lit, full-neighbour) paint at a block tile, for diagnostics and tests.
    pub fn paint_at(&self, plane: i32, bx: i32, by: i32) -> Option<&TilePaint> {
        self.paints
            .iter()
            .find(|p| p.plane == plane && p.bx == bx && p.by == by)
            .map(|p| &p.record)
    }

    /// The exported shaped tile model at a block tile, for diagnostics and tests.
    pub fn tile_model_at(&self, plane: i32, bx: i32, by: i32) -> Option<&TileModel> {
        self.tile_models
            .iter()
            .find(|p| p.plane == plane && p.bx == bx && p.by == by)
            .map(|p| &p.record)
    }

    pub fn from_chunks(data: &[u8]) -> Result<Self, RenderError> {
        let chunks = Chunks::parse(data)?;
        let h = chunks.ints("BLHD")?;
        if h.len() < 13 {
            return Err(RenderError::Format("block header".into()));
        }
        let size = h[5];
        let planes = h[6];
        if size != BLOCK_SIZE || planes != PLANES {
            return Err(RenderError::InvalidAsset(format!(
                "block {} has size {size} planes {planes}",
                h[0]
            )));
        }
        let tiles = (planes * size * size) as usize;
        let flags = chunks.ints("BFLG")?;
        let link = chunks.bytes("BLNK")?;
        let object_count = chunks.bytes("BOBC")?;
        let object_flags = chunks.bytes("BOBF")?;
        let heights = chunks.ints("BHGT")?;
        let roofs = chunks.ints("BROF")?;
        let settings = chunks.bytes_opt("BSET")?.ok_or_else(|| {
            RenderError::InvalidAsset(
                "block export lacks BSET tile settings (re-export with the current blocks profile)"
                    .into(),
            )
        })?;
        if flags.len() != tiles
            || link.len() != tiles
            || object_count.len() != tiles
            || object_flags.len() != tiles * 5
            || heights.len() != (planes * (size + 1) * (size + 1)) as usize
            || roofs.len() != tiles
            || settings.len() != tiles
        {
            return Err(RenderError::InvalidAsset(
                "block per-tile array sizes".into(),
            ));
        }
        let mut block = Block {
            square: h[0],
            origin_x: h[3],
            origin_y: h[4],
            roof_mode: h[10],
            min_level: h[11],
            flags,
            link,
            object_count,
            object_flags,
            heights,
            roofs,
            settings,
            paints: Vec::new(),
            tile_models: Vec::new(),
            walls: Vec::new(),
            wall_decorations: Vec::new(),
            floor_decorations: Vec::new(),
            objects: Vec::new(),
            animated: Vec::new(),
            model_keys: chunks
                .text("MODL")?
                .lines()
                .map(|s| s.to_string())
                .collect(),
            object_defs: HashMap::new(),
            source_icons: Vec::new(),
            minimap_ready: false,
            raw_terrain: super::terrain::RawTerrain::from_block_chunks(&chunks)?,
        };
        for r in chunks.ints("BPNT")?.as_chunks::<10>().0 {
            block.paints.push(Placed {
                plane: r[0],
                bx: r[1],
                by: r[2],
                record: TilePaint {
                    sw: r[3],
                    se: r[4],
                    ne: r[5],
                    nw: r[6],
                    texture: r[7],
                    flat: r[8] != 0,
                    rgb: r[9],
                },
            });
        }
        let tm = chunks.ints("BTMD")?;
        let mut cursor = 0usize;
        while cursor < tm.len() {
            if cursor + 11 > tm.len() {
                return Err(RenderError::Format(
                    "block tile model header truncated".into(),
                ));
            }
            let head: Vec<i32> = tm[cursor..cursor + 11].to_vec();
            let vcount = head[8] as usize;
            let fcount = head[9] as usize;
            let has_tex = head[10] != 0;
            cursor += 11;
            let mut take = |n: usize| -> Result<Vec<i32>, RenderError> {
                let end = cursor + n;
                if end > tm.len() {
                    return Err(RenderError::Format("block tile model truncated".into()));
                }
                let v = tm[cursor..end].to_vec();
                cursor = end;
                Ok(v)
            };
            let model = TileModel {
                shape: head[3],
                rotation: head[4],
                flat: head[5] != 0,
                underlay_rgb: head[6],
                overlay_rgb: head[7],
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
                    return Err(RenderError::InvalidAsset(
                        "block tile model face index".into(),
                    ));
                }
            }
            block.tile_models.push(Placed {
                plane: head[0],
                bx: head[1],
                by: head[2],
                record: model,
            });
        }
        for r in chunks.ints("BWAL")?.as_chunks::<12>().0 {
            block.walls.push(Placed {
                plane: r[0],
                bx: r[1],
                by: r[2],
                record: Wall {
                    model_a: r[3],
                    model_b: r[4],
                    orientation_a: r[5],
                    orientation_b: r[6],
                    x: r[7],
                    height: r[8],
                    z: r[9],
                    hash: join(r[10], r[11]),
                    config: -1,
                },
            });
        }
        for r in chunks.ints("BWDC")?.as_chunks::<16>().0 {
            block.wall_decorations.push(Placed {
                plane: r[0],
                bx: r[1],
                by: r[2],
                record: WallDecoration {
                    model_a: r[3],
                    model_b: r[4],
                    orientation: r[5],
                    orientation2: r[6],
                    x: r[7],
                    height: r[8],
                    z: r[9],
                    offset_x: r[10],
                    offset_z: r[11],
                    offset_x2: r[12],
                    offset_z2: r[13],
                    hash: join(r[14], r[15]),
                },
            });
        }
        for r in chunks.ints("BFDC")?.as_chunks::<9>().0 {
            block.floor_decorations.push(Placed {
                plane: r[0],
                bx: r[1],
                by: r[2],
                record: FloorDecoration {
                    model: r[3],
                    x: r[4],
                    height: r[5],
                    z: r[6],
                    hash: join(r[7], r[8]),
                },
            });
        }
        for r in chunks.ints("BOBJ")?.as_chunks::<17>().0 {
            block.objects.push(BlockObject {
                plane: r[0],
                bx: r[1],
                by: r[2],
                slot: r[3],
                object: GameObject {
                    model: r[4],
                    orientation: r[5],
                    x: r[6],
                    height: r[7],
                    z: r[8],
                    min_x: r[9],
                    max_x: r[10],
                    min_y: r[11],
                    max_y: r[12],
                    config: r[13],
                    slot_flag: r[14],
                    dynamic: r[4] <= -2,
                    hash: join(r[15], r[16]),
                },
            });
        }
        let dyn_table = chunks.ints("BDYN")?;
        let mut cursor = 0usize;
        while cursor < dyn_table.len() {
            if cursor + 5 > dyn_table.len() {
                return Err(RenderError::Format(
                    "block animation table truncated".into(),
                ));
            }
            let frames = dyn_table[cursor] as usize;
            let frame_step = dyn_table[cursor + 1];
            let max_loops = dyn_table[cursor + 2];
            let total_cycles = dyn_table[cursor + 3];
            let plain = dyn_table[cursor + 4];
            cursor += 5;
            if cursor + frames * 2 > dyn_table.len() {
                return Err(RenderError::Format(
                    "block animation frames truncated".into(),
                ));
            }
            let mut models = dyn_table[cursor..cursor + frames].to_vec();
            let lengths = dyn_table[cursor + frames..cursor + frames * 2].to_vec();
            cursor += frames * 2;
            // The plain model rides along as an extra trailing entry (see `plain_of`).
            models.push(plain);
            block.animated.push(AnimatedSet {
                frame_step,
                max_loops,
                total_cycles,
                models,
                lengths,
            });
        }
        let model_count = block.model_keys.len() as i32;
        let animated_count = block.animated.len() as i32;
        let check = |m: i32| -> Result<(), RenderError> {
            if m >= model_count || (m <= -2 && -(m) - 2 >= animated_count) {
                return Err(RenderError::InvalidAsset(format!(
                    "block model reference {m} out of range"
                )));
            }
            Ok(())
        };
        for w in &block.walls {
            check(w.record.model_a)?;
            check(w.record.model_b)?;
        }
        for d in &block.wall_decorations {
            check(d.record.model_a)?;
            check(d.record.model_b)?;
        }
        for f in &block.floor_decorations {
            check(f.record.model)?;
        }
        for o in &block.objects {
            check(o.object.model)?;
        }
        for set in &block.animated {
            for &m in &set.models {
                if m >= model_count || m < -1 {
                    return Err(RenderError::InvalidAsset(
                        "block animation frame model out of range".into(),
                    ));
                }
            }
        }
        Ok(block)
    }

    #[inline]
    fn local(&self, plane: i32, bx: i32, by: i32) -> usize {
        ((plane * BLOCK_SIZE + bx) * BLOCK_SIZE + by) as usize
    }
}

/// Deterministic xorshift so assembled scenes get stable animation phases per placement.
fn phase_for(seed: u64, frames: usize, length_of_frame: impl Fn(usize) -> i32) -> (i32, i32) {
    let mut x = seed ^ 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    if frames == 0 {
        return (0, 0);
    }
    let frame = (next() % frames as u64) as i32;
    let len = length_of_frame(frame as usize).max(1);
    let cycle = (next() % len as u64) as i32;
    (frame, cycle)
}

/// Assembles a scene from blocks. `blocks` is a list of `(block, models)` where `models` are the
/// block's pack entries in key order; the returned scene's model list is the deduplicated union
/// (by content key) and every reference is remapped onto it.
impl Block {
    /// Attaches the `minimap` export sidecar: the original placement config of every wall
    /// (`fe.getConfig()`) and the minimap fields of every referenced object definition.
    pub fn attach_minimap(&mut self, data: &[u8]) -> Result<(), RenderError> {
        let chunks = Chunks::parse(data)?;
        let h = chunks.ints("MBHD")?;
        if h.len() < 9 {
            return Err(RenderError::Format("minimap block header".into()));
        }
        if h[0] != self.square {
            return Err(RenderError::InvalidAsset(format!(
                "minimap sidecar is square {} but block is {}",
                h[0], self.square
            )));
        }
        let mut configs: HashMap<(i32, i32, i32), i32> = HashMap::new();
        for r in chunks.ints("MWAL")?.as_chunks::<4>().0 {
            configs.insert((r[0], r[1], r[2]), r[3]);
        }
        for wall in &mut self.walls {
            match configs.get(&(wall.plane, wall.bx, wall.by)) {
                Some(&config) => wall.record.config = config,
                None => {
                    return Err(RenderError::InvalidAsset(format!(
                        "minimap sidecar of square {} lacks the wall at plane {} {},{}",
                        self.square, wall.plane, wall.bx, wall.by
                    )));
                }
            }
        }
        self.object_defs.clear();
        for r in chunks.ints("MDEF")?.as_chunks::<5>().0 {
            self.object_defs.insert(
                r[0],
                MapObjectDef {
                    map_scene: r[1],
                    size_x: r[2],
                    size_y: r[3],
                    map_icon: r[4],
                },
            );
        }
        self.source_icons = chunks
            .ints_opt("MICN")?
            .map(|icons| {
                icons
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|r| (r[0], r[1], r[2], r[3]))
                    .collect()
            })
            .unwrap_or_default();
        self.minimap_ready = true;
        Ok(())
    }
}

/// One instanced chunk placement (the original `rl4.fn` template entry, unpacked): the 8×8
/// source chunk (absolute chunk coordinates `world_tile >> 3`, plane) that appears at the
/// destination chunk of the instance scene, turned by `quarter_turns`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkMapping {
    pub plane: i32,
    pub chunk_x: i32,
    pub chunk_y: i32,
    pub source_plane: i32,
    pub source_chunk_x: i32,
    pub source_chunk_y: i32,
    pub quarter_turns: i32,
}

impl ChunkMapping {
    /// The original packed template word: `plane << 24 | chunk_x << 14 | chunk_y << 3 | turns << 1`
    /// (source coordinates; the destination is the array position).
    pub fn packed_template(&self) -> i32 {
        (self.source_plane << 24)
            | (self.source_chunk_x << 14)
            | (self.source_chunk_y << 3)
            | ((self.quarter_turns & 3) << 1)
    }
}

/// An instance scene: only the declared chunk mappings are loaded; every other chunk of the
/// 104×104 scene stays unloaded (no terrain, no scenery, black on the minimap), exactly as the
/// original template loader leaves undeclared chunks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InstanceLayout {
    pub template: String,
    pub chunks: Vec<ChunkMapping>,
}

pub fn assemble(
    base_x: i32,
    base_y: i32,
    blocks: &[(&Block, &[(String, Model)])],
    randomize_phases: bool,
) -> Result<(SceneData, Vec<Option<Model>>), RenderError> {
    assemble_mapped(base_x, base_y, blocks, randomize_phases, None)
}

/// [`assemble`] with an optional instance layout. With a layout, a block tile or placement is
/// copied only when its chunk is a declared source chunk, to that mapping's destination chunk
/// (a source chunk may appear several times); undeclared chunks are left unloaded. Block exports
/// carry the original scene's final placed geometry (terrain blended and lit, scenery models
/// lit at their placed orientation), so a mapping is exact for `quarter_turns == 0`; a turned
/// chunk would need the terrain re-lit and every model re-lit at the turned orientation from
/// raw inputs the blocks do not carry, and is rejected instead of approximated.
pub fn assemble_mapped(
    base_x: i32,
    base_y: i32,
    blocks: &[(&Block, &[(String, Model)])],
    randomize_phases: bool,
    layout: Option<&InstanceLayout>,
) -> Result<(SceneData, Vec<Option<Model>>), RenderError> {
    if let Some(layout) = layout {
        if let Some(turned) = layout.chunks.iter().find(|c| c.quarter_turns & 3 != 0) {
            return Err(RenderError::Scene(format!(
                "instance template {}: chunk ({},{},{}) from source ({},{},{}) turned {} quarter turns is not supported: block exports hold lit placed geometry that cannot be turned exactly (raw terrain/location inputs needed)",
                layout.template,
                turned.plane,
                turned.chunk_x,
                turned.chunk_y,
                turned.source_plane,
                turned.source_chunk_x,
                turned.source_chunk_y,
                turned.quarter_turns
            )));
        }
        if layout.chunks.is_empty() {
            return Err(RenderError::Scene(format!(
                "instance template {} declares no chunks",
                layout.template
            )));
        }
    }
    // Destination placements of a block tile: identity (offset 0) without a layout; with one,
    // every mapping whose source chunk is the tile's chunk, as a (plane, dx, dy) tile offset.
    let placements_for = |block_plane: i32, wx: i32, wy: i32| -> Vec<(i32, i32, i32)> {
        match layout {
            None => vec![(block_plane, 0, 0)],
            Some(layout) => layout
                .chunks
                .iter()
                .filter(|c| {
                    c.source_plane == block_plane
                        && c.source_chunk_x == wx >> 3
                        && c.source_chunk_y == wy >> 3
                })
                .map(|c| {
                    (
                        c.plane,
                        (c.chunk_x - c.source_chunk_x) * 8,
                        (c.chunk_y - c.source_chunk_y) * 8,
                    )
                })
                .collect(),
        }
    };
    let tile_count = (PLANES as usize) << 16;
    let mut scene = SceneData::empty_grid(
        match layout {
            Some(layout) => format!("blocks@{base_x},{base_y}#{}", layout.template),
            None => format!("blocks@{base_x},{base_y}"),
        },
        base_x,
        base_y,
        GRID,
        PLANES,
        OFFSET,
        MAIN,
        tile_count,
    );
    let mut merged_models: Vec<Option<Model>> = Vec::new();
    let mut key_index: HashMap<String, i32> = HashMap::new();
    let mut set_index: HashMap<(Vec<i32>, Vec<i32>, i32), usize> = HashMap::new();
    let mut identity: HashMap<(i64, i32, i32, i32, i32, i32), usize> = HashMap::new();
    for (block, models) in blocks {
        if models.len() != block.model_keys.len() {
            return Err(RenderError::InvalidAsset(format!(
                "block {} pack holds {} models for {} keys",
                block.square,
                models.len(),
                block.model_keys.len()
            )));
        }
        let mut map: Vec<i32> = Vec::with_capacity(models.len());
        for (i, (key, model)) in models.iter().enumerate() {
            if key != &block.model_keys[i] {
                return Err(RenderError::InvalidAsset(format!(
                    "block {} pack order mismatch",
                    block.square
                )));
            }
            let id = *key_index.entry(key.clone()).or_insert_with(|| {
                merged_models.push(Some(model.clone()));
                (merged_models.len() - 1) as i32
            });
            map.push(id);
        }
        let remap = |m: i32| -> i32 { if m >= 0 { map[m as usize] } else { m } };
        let mut set_map: Vec<usize> = Vec::with_capacity(block.animated.len());
        for set in &block.animated {
            let models: Vec<i32> = set.models.iter().map(|&m| remap(m)).collect();
            let key = (models.clone(), set.lengths.clone(), set.frame_step);
            let id = *set_index.entry(key).or_insert_with(|| {
                scene.animated.push(AnimatedSet {
                    frame_step: set.frame_step,
                    max_loops: set.max_loops,
                    total_cycles: set.total_cycles,
                    models,
                    lengths: set.lengths.clone(),
                });
                scene.animated.len() - 1
            });
            set_map.push(id);
        }
        let dx = block.origin_x - base_x;
        let dy = block.origin_y - base_y;
        // An animated reference becomes a per-placement instance with its own start phase.
        let instance = |scene: &mut SceneData, m: i32, hash: i64, salt: i32| -> i32 {
            if m > -2 {
                return remap(m);
            }
            let set = set_map[(-(m) - 2) as usize];
            let frames = scene.animated[set].models.len() - 1;
            let (start_frame, start_cycle) = if randomize_phases {
                let lengths = scene.animated[set].lengths.clone();
                phase_for(
                    (hash as u64)
                        ^ ((base_x as u64) << 40)
                        ^ ((base_y as u64) << 20)
                        ^ (salt as u64),
                    frames,
                    |f| lengths[f],
                )
            } else {
                (0, 0)
            };
            let plain = *scene.animated[set].models.last().unwrap_or(&-1);
            scene.animated_instances.push(AnimatedInstance {
                set,
                start_frame,
                start_cycle,
                plain,
            });
            -(scene.animated_instances.len() as i32 - 1) - 2
        };
        let rebase_hash = |hash: i64, mx: i32, my: i32| -> i64 {
            (hash & !0x3FFF) | i64::from(mx & 127) | (i64::from(my & 127) << 7)
        };
        let in_grid = |ex: i32, ey: i32| (0..GRID).contains(&ex) && (0..GRID).contains(&ey);
        // Every destination of a block tile: (destination plane, scene ex, scene ey, tile shift).
        let destinations = |plane: i32, bx: i32, by: i32| -> Vec<(i32, i32, i32, i32, i32)> {
            placements_for(plane, block.origin_x + bx, block.origin_y + by)
                .into_iter()
                .map(|(dest_plane, sx, sy)| {
                    (
                        dest_plane,
                        bx + dx + sx + OFFSET,
                        by + dy + sy + OFFSET,
                        dx + sx,
                        dy + sy,
                    )
                })
                .collect()
        };
        for plane in 0..PLANES {
            if layout.is_none() {
                for bx in 0..=BLOCK_SIZE {
                    for by in 0..=BLOCK_SIZE {
                        let ex = bx + dx + OFFSET;
                        let ey = by + dy + OFFSET;
                        if !(0..=GRID).contains(&ex) || !(0..=GRID).contains(&ey) {
                            continue;
                        }
                        let h = block.heights
                            [((plane * (BLOCK_SIZE + 1) + bx) * (BLOCK_SIZE + 1) + by) as usize];
                        scene.set_height(plane, ex, ey, h);
                    }
                }
            } else {
                // Mapped chunks: every loaded tile carries its four corner heights (the block
                // grid holds the far edge row/column), so a chunk's edge is exact and an
                // unloaded neighbour leaves nothing behind.
                for bx in 0..BLOCK_SIZE {
                    for by in 0..BLOCK_SIZE {
                        for (dest_plane, ex, ey, _, _) in destinations(plane, bx, by) {
                            for (cx, cy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                                let h = block.heights[((plane * (BLOCK_SIZE + 1) + bx + cx)
                                    * (BLOCK_SIZE + 1)
                                    + by
                                    + cy)
                                    as usize];
                                if (0..=GRID).contains(&(ex + cx))
                                    && (0..=GRID).contains(&(ey + cy))
                                {
                                    scene.set_height(dest_plane, ex + cx, ey + cy, h);
                                }
                            }
                        }
                    }
                }
            }
            for bx in 0..BLOCK_SIZE {
                for by in 0..BLOCK_SIZE {
                    let local = block.local(plane, bx, by);
                    for (dest_plane, ex, ey, _, _) in destinations(plane, bx, by) {
                        if !in_grid(ex, ey) {
                            continue;
                        }
                        let index = scene.tile_index(dest_plane, ex, ey);
                        scene.flags[index] = block.flags[local];
                        scene.link[index] = block.link[local];
                        scene.object_count[index] = block.object_count[local];
                        scene.object_flags[index * 5..index * 5 + 5]
                            .copy_from_slice(&block.object_flags[local * 5..local * 5 + 5]);
                        scene.set_roof(dest_plane, ex, ey, block.roofs[local]);
                        scene.set_setting(dest_plane, ex, ey, block.settings[local]);
                    }
                }
            }
        }
        let place = |plane: i32, bx: i32, by: i32| -> Vec<(i32, usize, i32, i32, i32, i32)> {
            destinations(plane, bx, by)
                .into_iter()
                .filter(|(_, ex, ey, _, _)| in_grid(*ex, *ey))
                .map(|(dest_plane, ex, ey, sx, sy)| {
                    (
                        dest_plane,
                        scene_index(dest_plane, ex, ey),
                        bx + sx,
                        by + sy,
                        sx,
                        sy,
                    )
                })
                .collect()
        };
        // Scenery placements follow the live loader (`rl4.ws` / `rl4.pn`): a location is placed
        // only when its origin tile lies strictly inside the scene (1..=102 on both axes); tiles
        // on the scene edge and in the margin carry none, exactly like the original scene.
        let placed = |plane: i32, bx: i32, by: i32| -> Vec<(i32, usize, i32, i32, i32, i32)> {
            place(plane, bx, by)
                .into_iter()
                .filter(|(_, _, tx, ty, _, _)| {
                    // (tx, ty) is the destination scene tile (block tile + total shift).
                    *tx > 0 && *ty > 0 && *tx < MAIN - 1 && *ty < MAIN - 1
                })
                .collect()
        };
        for p in &block.paints {
            for (_, index, _, _, _, _) in place(p.plane, p.bx, p.by) {
                scene.paints.insert(index, p.record.clone());
            }
        }
        for t in &block.tile_models {
            for (_, index, _, _, sx, sy) in place(t.plane, t.bx, t.by) {
                let mut model = t.record.clone();
                for v in &mut model.xs {
                    *v += sx * 128;
                }
                for v in &mut model.zs {
                    *v += sy * 128;
                }
                scene.tile_models.insert(index, model);
            }
        }
        for w in &block.walls {
            for (_, index, mx, my, sx, sy) in placed(w.plane, w.bx, w.by) {
                let mut wall = w.record.clone();
                wall.x += sx * 128;
                wall.z += sy * 128;
                wall.hash = rebase_hash(wall.hash, mx, my);
                wall.model_a = instance(&mut scene, wall.model_a, wall.hash, 1);
                wall.model_b = instance(&mut scene, wall.model_b, wall.hash, 2);
                scene.walls.insert(index, wall);
            }
        }
        for d in &block.wall_decorations {
            for (_, index, mx, my, sx, sy) in placed(d.plane, d.bx, d.by) {
                let mut decor = d.record.clone();
                decor.x += sx * 128;
                decor.z += sy * 128;
                decor.hash = rebase_hash(decor.hash, mx, my);
                decor.model_a = instance(&mut scene, decor.model_a, decor.hash, 3);
                decor.model_b = instance(&mut scene, decor.model_b, decor.hash, 4);
                scene.wall_decorations.insert(index, decor);
            }
        }
        for f in &block.floor_decorations {
            for (_, index, mx, my, sx, sy) in placed(f.plane, f.bx, f.by) {
                let mut floor = f.record.clone();
                floor.x += sx * 128;
                floor.z += sy * 128;
                floor.hash = rebase_hash(floor.hash, mx, my);
                floor.model = instance(&mut scene, floor.model, floor.hash, 5);
                scene.floor_decorations.insert(index, floor);
            }
        }
        for o in &block.objects {
            // Objects are copied per covered tile slot; a span reaching into an unloaded chunk
            // keeps its geometry (the original places a location whole by its origin) while
            // the unloaded tiles carry no slot.
            for (_, index, _, _, sx, sy) in place(o.plane, o.bx, o.by) {
                // The location's origin (its south-west tile) decides placement, not the
                // covered tile being copied.
                // (`sx`, `sy` is the total tile shift of this placement, block → scene.)
                let (ox, oy) = (o.object.min_x + sx, o.object.min_y + sy);
                if !(ox > 0 && oy > 0 && ox < MAIN - 1 && oy < MAIN - 1) {
                    continue;
                }
                let mut object = o.object.clone();
                object.x += sx * 128;
                object.z += sy * 128;
                object.min_x += sx;
                object.max_x += sx;
                object.min_y += sy;
                object.max_y += sy;
                object.hash = rebase_hash(object.hash, object.min_x, object.min_y);
                // The same original instance spans every tile it covers, possibly across blocks:
                // group by its world-anchored identity before drawing state is created.
                let key = (
                    object.hash,
                    object.x,
                    object.z,
                    object.height,
                    object.orientation,
                    remap_static(object.model, &map),
                );
                let id = match identity.get(&key) {
                    Some(&id) => id,
                    None => {
                        object.model = instance(&mut scene, object.model, object.hash, 6);
                        object.dynamic = object.model <= -2;
                        let id = scene.game_objects.len();
                        scene.game_objects.push(object);
                        identity.insert(key, id);
                        id
                    }
                };
                scene.slots.insert(index * 5 + o.slot as usize, id);
            }
        }
        scene.roof_mode = block.roof_mode;
        scene.min_level = block.min_level;
        scene
            .object_defs
            .extend(block.object_defs.iter().map(|(k, v)| (*k, *v)));
        // The original icon pass covers the 104x104 main area (`bu.aa`: 0..104) and reads the
        // floor decorations the loader placed — origins strictly inside the scene (1..=102),
        // like every location; the sidecar records the square's decorations, so the ones that
        // land on this scene's edge are not expected.
        for &(plane, bx, by, element) in &block.source_icons {
            for (dest_plane, ex, ey, _, _) in destinations(plane, bx, by) {
                if (OFFSET + 1..OFFSET + MAIN - 1).contains(&ex)
                    && (OFFSET + 1..OFFSET + MAIN - 1).contains(&ey)
                {
                    scene.source_icons.push((
                        dest_plane,
                        base_x + ex - OFFSET,
                        base_y + ey - OFFSET,
                        element,
                    ));
                }
            }
        }
    }
    scene.model_keys = {
        let mut keys = vec![String::new(); merged_models.len()];
        for (key, &id) in &key_index {
            keys[id as usize] = key.clone();
        }
        keys
    };
    Ok((scene, merged_models))
}

/// The live loader's raw terrain arrays for a scene at `base` from the present blocks (their
/// declared chunks only under a layout): every tile of the 104×104 scene that a loaded square
/// covers, its south-west corner height, and the shadows the loaded squares' scenery casts.
/// `None` when a present block was exported without raw terrain — the exported lit tiles then
/// stay in use and the caller reports it.
pub fn scene_terrain(
    base_x: i32,
    base_y: i32,
    blocks: &[(&Block, &[(String, Model)])],
    layout: Option<&InstanceLayout>,
) -> Option<super::terrain::SceneTerrain> {
    use super::terrain::SceneTerrain;
    let mut terrain = SceneTerrain::empty();
    for (block, _) in blocks {
        let raw = block.raw_terrain.as_ref()?;
        let dx = block.origin_x - base_x;
        let dy = block.origin_y - base_y;
        let placements = |plane: i32, wx: i32, wy: i32| -> Vec<(i32, i32, i32, i32)> {
            match layout {
                None => vec![(plane, 0, 0, 0)],
                Some(layout) => layout
                    .chunks
                    .iter()
                    .filter(|c| {
                        c.source_plane == plane
                            && c.source_chunk_x == wx >> 3
                            && c.source_chunk_y == wy >> 3
                    })
                    .map(|c| {
                        (
                            c.plane,
                            (c.chunk_x - c.source_chunk_x) * 8,
                            (c.chunk_y - c.source_chunk_y) * 8,
                            c.quarter_turns & 3,
                        )
                    })
                    .collect(),
            }
        };
        for plane in 0..PLANES {
            for bx in 0..BLOCK_SIZE {
                for by in 0..BLOCK_SIZE {
                    let wx = block.origin_x + bx;
                    let wy = block.origin_y + by;
                    for (dest_plane, sx, sy, turns) in placements(plane, wx, wy) {
                        let tx = bx + dx + sx;
                        let ty = by + dy + sy;
                        terrain.set_tile(raw, plane, bx, by, dest_plane, tx, ty, turns);
                        terrain.set_height(
                            dest_plane,
                            tx,
                            ty,
                            raw.heights[super::terrain::RawTerrain::index(plane, bx, by)],
                        );
                    }
                }
            }
        }
        for &(plane, ox, oy, x, y, value) in &raw.shadows {
            // A shadow write belongs to the location cast from origin (ox, oy): it is applied
            // only where the live loader would place that location — origin strictly inside the
            // scene (`rl4.ws`) and, under a layout, in a declared chunk (the instance loader
            // copies whole locations by their origin chunk); the write itself may spill past.
            for (dest_plane, sx, sy, _) in
                placements(plane, block.origin_x + ox, block.origin_y + oy)
            {
                let (origin_x, origin_y) = (ox + dx + sx, oy + dy + sy);
                if super::terrain::SceneTerrain::places_location(origin_x, origin_y) {
                    terrain.add_shadow(dest_plane, x + dx + sx, y + dy + sy, value);
                }
            }
        }
    }
    Some(terrain)
}

#[inline]
fn scene_index(plane: i32, ex: i32, ey: i32) -> usize {
    ((plane << 16) | (ex << 8) | ey) as usize
}

fn remap_static(m: i32, map: &[i32]) -> i32 {
    if m >= 0 { map[m as usize] } else { m }
}

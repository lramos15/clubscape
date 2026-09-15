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
    FloorDecoration, GameObject, SceneData, TileModel, TilePaint, Wall, WallDecoration,
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
                return Some(loop_start as usize);
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
        Some(frame as usize)
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
    paints: Vec<Placed<TilePaint>>,
    tile_models: Vec<Placed<TileModel>>,
    walls: Vec<Placed<Wall>>,
    wall_decorations: Vec<Placed<WallDecoration>>,
    floor_decorations: Vec<Placed<FloorDecoration>>,
    objects: Vec<BlockObject>,
    animated: Vec<AnimatedSet>,
    pub model_keys: Vec<String>,
}

fn join(lo: i32, hi: i32) -> i64 {
    ((hi as i64) << 32) | (lo as u32 as i64)
}

impl Block {
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
        if flags.len() != tiles
            || link.len() != tiles
            || object_count.len() != tiles
            || object_flags.len() != tiles * 5
            || heights.len() != (planes * (size + 1) * (size + 1)) as usize
            || roofs.len() != tiles
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
pub fn assemble(
    base_x: i32,
    base_y: i32,
    blocks: &[(&Block, &[(String, Model)])],
    randomize_phases: bool,
) -> Result<(SceneData, Vec<Option<Model>>), RenderError> {
    let tile_count = (PLANES as usize) << 16;
    let mut scene = SceneData::empty_grid(
        format!("blocks@{base_x},{base_y}"),
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
        let unit_dx = dx * 128;
        let unit_dy = dy * 128;
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
        for plane in 0..PLANES {
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
            for bx in 0..BLOCK_SIZE {
                for by in 0..BLOCK_SIZE {
                    let ex = bx + dx + OFFSET;
                    let ey = by + dy + OFFSET;
                    if !in_grid(ex, ey) {
                        continue;
                    }
                    let local = block.local(plane, bx, by);
                    let index = scene.tile_index(plane, ex, ey);
                    scene.flags[index] = block.flags[local];
                    scene.link[index] = block.link[local];
                    scene.object_count[index] = block.object_count[local];
                    scene.object_flags[index * 5..index * 5 + 5]
                        .copy_from_slice(&block.object_flags[local * 5..local * 5 + 5]);
                    scene.set_roof(plane, ex, ey, block.roofs[local]);
                }
            }
        }
        let place = |plane: i32, bx: i32, by: i32| -> Option<(usize, i32, i32)> {
            let ex = bx + dx + OFFSET;
            let ey = by + dy + OFFSET;
            in_grid(ex, ey).then(|| (scene_index(plane, ex, ey), bx + dx, by + dy))
        };
        for p in &block.paints {
            if let Some((index, _, _)) = place(p.plane, p.bx, p.by) {
                scene.paints.insert(index, p.record.clone());
            }
        }
        for t in &block.tile_models {
            if let Some((index, _, _)) = place(t.plane, t.bx, t.by) {
                let mut model = t.record.clone();
                for v in &mut model.xs {
                    *v += unit_dx;
                }
                for v in &mut model.zs {
                    *v += unit_dy;
                }
                scene.tile_models.insert(index, model);
            }
        }
        for w in &block.walls {
            if let Some((index, mx, my)) = place(w.plane, w.bx, w.by) {
                let mut wall = w.record.clone();
                wall.x += unit_dx;
                wall.z += unit_dy;
                wall.hash = rebase_hash(wall.hash, mx, my);
                wall.model_a = instance(&mut scene, wall.model_a, wall.hash, 1);
                wall.model_b = instance(&mut scene, wall.model_b, wall.hash, 2);
                scene.walls.insert(index, wall);
            }
        }
        for d in &block.wall_decorations {
            if let Some((index, mx, my)) = place(d.plane, d.bx, d.by) {
                let mut decor = d.record.clone();
                decor.x += unit_dx;
                decor.z += unit_dy;
                decor.hash = rebase_hash(decor.hash, mx, my);
                decor.model_a = instance(&mut scene, decor.model_a, decor.hash, 3);
                decor.model_b = instance(&mut scene, decor.model_b, decor.hash, 4);
                scene.wall_decorations.insert(index, decor);
            }
        }
        for f in &block.floor_decorations {
            if let Some((index, mx, my)) = place(f.plane, f.bx, f.by) {
                let mut floor = f.record.clone();
                floor.x += unit_dx;
                floor.z += unit_dy;
                floor.hash = rebase_hash(floor.hash, mx, my);
                floor.model = instance(&mut scene, floor.model, floor.hash, 5);
                scene.floor_decorations.insert(index, floor);
            }
        }
        for o in &block.objects {
            if let Some((index, _, _)) = place(o.plane, o.bx, o.by) {
                let mut object = o.object.clone();
                object.x += unit_dx;
                object.z += unit_dy;
                object.min_x += dx;
                object.max_x += dx;
                object.min_y += dy;
                object.max_y += dy;
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

#[inline]
fn scene_index(plane: i32, ex: i32, ey: i32) -> usize {
    ((plane << 16) | (ex << 8) | ey) as usize
}

fn remap_static(m: i32, map: &[i32]) -> i32 {
    if m >= 0 { map[m as usize] } else { m }
}

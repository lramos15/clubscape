//! Exact port of the original frame-based model animation: sequence definitions (`ou`), frame
//! transform lists (`et` + skeleton `em`) and the vertex-group transform routine (`fx.rx`, with
//! `fx.dn` driving it and `fx.jp` applying the NPC scale afterwards, as `pl.ag` does).
//!
//! Integer semantics mirror the Java code: vertex floats are truncated with `as i32` before the
//! 2048-unit sine/cosine rotation and written back as floats; origins accumulate through
//! `(int)((float)acc + v)`; translations add as floats.
//!
//! `LabelMap` implements the technical retargeting used for the penguin player body: every human
//! skeleton label drives the penguin labels whose normalised centroid is nearest, translations
//! are scaled by the height ratio, and rotations/scales pass through unchanged. It is a
//! documented proposal (comparison-policy `penguin_adaptation`), not an approved appearance.

use std::collections::HashMap;

use crate::chunk::Chunks;
use crate::error::RenderError;
use crate::model::Model;
use crate::tables::tables;

/// One skeleton (`em`): transform types and the label list each transform touches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skeleton {
    pub types: Vec<i32>,
    pub labels: Vec<Vec<i32>>,
}

/// One frame (`et`): ordered transforms referencing the skeleton by index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub reference: i32,
    pub skeleton: usize,
    /// `(transform index, dx, dy, dz)`
    pub transforms: Vec<(usize, i32, i32, i32)>,
    pub alpha: bool,
}

/// A sequence definition (`ou`) with its frames inlined.
#[derive(Clone, Debug)]
pub struct Sequence {
    pub id: i32,
    pub lengths: Vec<i32>,
    pub frame_refs: Vec<i32>,
    pub frame_step: i32,
    pub max_loops: i32,
    pub total_cycles: i32,
    pub left_hand_item: i32,
    pub right_hand_item: i32,
    pub priority: i32,
    pub reply_mode: i32,
    pub precedence: i32,
    pub stretches: bool,
    pub interleave: Option<Vec<i32>>,
    pub skeletons: Vec<Skeleton>,
    pub frames: Vec<Frame>,
}

impl Sequence {
    pub fn from_chunks(data: &[u8]) -> Result<Self, RenderError> {
        let chunks = Chunks::parse(data)?;
        let h = chunks.ints("SEQH")?;
        if h.len() < 11 {
            return Err(RenderError::Format("sequence header".into()));
        }
        let lengths = chunks.ints("SEQL")?;
        let frame_refs = chunks.ints("SEQF")?;
        if lengths.len() != frame_refs.len() || frame_refs.len() != h[1] as usize {
            return Err(RenderError::InvalidAsset(format!(
                "sequence {} frame tables disagree",
                h[0]
            )));
        }
        let skel = chunks.ints("SKEL")?;
        let mut skeletons = Vec::new();
        let mut cursor = 0usize;
        while cursor < skel.len() {
            let count = skel[cursor] as usize;
            cursor += 1;
            let mut types = Vec::with_capacity(count);
            let mut labels = Vec::with_capacity(count);
            for _ in 0..count {
                if cursor + 2 > skel.len() {
                    return Err(RenderError::Format("skeleton truncated".into()));
                }
                types.push(skel[cursor]);
                let n = skel[cursor + 1] as usize;
                cursor += 2;
                if cursor + n > skel.len() {
                    return Err(RenderError::Format("skeleton labels truncated".into()));
                }
                labels.push(skel[cursor..cursor + n].to_vec());
                cursor += n;
            }
            skeletons.push(Skeleton { types, labels });
        }
        let table = chunks.ints("FRMT")?;
        let mut frames = Vec::with_capacity(frame_refs.len());
        let mut cursor = 0usize;
        while cursor < table.len() {
            if cursor + 4 > table.len() {
                return Err(RenderError::Format("frame table truncated".into()));
            }
            let reference = table[cursor];
            let skeleton = table[cursor + 1] as usize;
            let count = table[cursor + 2] as usize;
            let alpha = table[cursor + 3] != 0;
            cursor += 4;
            if cursor + count * 4 > table.len() {
                return Err(RenderError::Format("frame transforms truncated".into()));
            }
            let skeleton_def = skeletons
                .get(skeleton)
                .ok_or_else(|| RenderError::InvalidAsset("frame skeleton index".into()))?;
            let mut transforms = Vec::with_capacity(count);
            for i in 0..count {
                let idx = table[cursor + i * 4] as usize;
                if idx >= skeleton_def.types.len() {
                    return Err(RenderError::InvalidAsset("frame transform index".into()));
                }
                transforms.push((
                    idx,
                    table[cursor + i * 4 + 1],
                    table[cursor + i * 4 + 2],
                    table[cursor + i * 4 + 3],
                ));
            }
            cursor += count * 4;
            frames.push(Frame {
                reference,
                skeleton,
                transforms,
                alpha,
            });
        }
        if frames.len() != frame_refs.len() {
            return Err(RenderError::InvalidAsset(format!(
                "sequence {} has {} frames but {} transform lists",
                h[0],
                frame_refs.len(),
                frames.len()
            )));
        }
        Ok(Sequence {
            id: h[0],
            lengths,
            frame_refs,
            frame_step: h[2],
            max_loops: h[3],
            total_cycles: h[4],
            left_hand_item: h[5],
            right_hand_item: h[6],
            priority: h[7],
            reply_mode: h[8],
            precedence: h[9],
            stretches: h[10] != 0,
            interleave: chunks.ints_opt("SEQI")?,
            skeletons,
            frames,
        })
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Frame shown after `elapsed` client cycles from frame 0 (looping like `rd.az`; a one-shot
    /// sequence returns `None` once it has ended).
    pub fn frame_at(&self, elapsed: i64) -> Option<usize> {
        let n = self.lengths.len() as i32;
        if n == 0 {
            return None;
        }
        let len = |f: i32| i64::from(self.lengths[f as usize].max(0));
        let mut frame = 0i32;
        let mut cycle = elapsed.max(0);
        let loop_start = if self.frame_step >= 1 && self.frame_step <= n {
            n - self.frame_step
        } else {
            0
        };
        let period: i64 = (loop_start..n).map(len).sum();
        let mut guard = 0;
        while cycle > len(frame) {
            if period > 0 && frame >= loop_start && cycle > period + len(frame) {
                let rounds = (cycle - len(frame) - 1) / period;
                cycle -= rounds * period;
                continue;
            }
            if period <= 0 && frame >= loop_start {
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

/// Retargeting table: human skeleton label → penguin labels it drives, plus translation scale.
#[derive(Clone, Debug, Default)]
pub struct LabelMap {
    pub driven: HashMap<i32, Vec<i32>>,
    /// Multiplier for type-1 translations (target height / source height).
    pub translation_scale: f64,
    /// Human label → penguin label chosen for each penguin label (for reporting).
    pub assignments: Vec<(i32, i32)>,
}

/// Label centroid in model units (from the exporter's `labels` summary or a loaded model).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LabelCentroid {
    pub label: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub fn label_centroids(model: &Model) -> Vec<LabelCentroid> {
    let mut out = Vec::new();
    if let Some(groups) = &model.vertex_groups {
        for (label, group) in groups.iter().enumerate() {
            if group.is_empty() {
                continue;
            }
            let n = group.len() as f64;
            let (mut sx, mut sy, mut sz) = (0.0, 0.0, 0.0);
            for &v in group {
                let v = v as usize;
                sx += f64::from(model.xs[v]);
                sy += f64::from(model.ys[v]);
                sz += f64::from(model.zs[v]);
            }
            out.push(LabelCentroid {
                label: label as i32,
                x: sx / n,
                y: sy / n,
                z: sz / n,
            });
        }
    }
    out
}

fn height_of(model_min_y: f64) -> f64 {
    (-model_min_y).max(1.0)
}

impl LabelMap {
    /// Builds a map from an explicit bone-level table (`human label -> penguin labels`), scaling
    /// translations by the body height ratio.
    pub fn from_table(table: &[(i32, &[i32])], source_min_y: f64, target_min_y: f64) -> Self {
        let mut driven: HashMap<i32, Vec<i32>> = HashMap::new();
        let mut assignments = Vec::new();
        for (human, penguins) in table {
            for p in *penguins {
                driven.entry(*human).or_default().push(*p);
                assignments.push((*human, *p));
            }
        }
        LabelMap {
            driven,
            translation_scale: height_of(target_min_y) / height_of(source_min_y),
            assignments,
        }
    }

    /// Nearest-centroid assignment after normalising both bodies to unit height (Y is
    /// negative-up in model space, feet at 0). Each target label is driven by exactly one
    /// source label, so no transform is applied twice to a vertex.
    pub fn nearest(
        source: &[LabelCentroid],
        source_min_y: f64,
        target: &[LabelCentroid],
        target_min_y: f64,
    ) -> Self {
        let sh = height_of(source_min_y);
        let th = height_of(target_min_y);
        let mut driven: HashMap<i32, Vec<i32>> = HashMap::new();
        let mut assignments = Vec::new();
        for t in target {
            let (tx, ty, tz) = (t.x / th, t.y / th, t.z / th);
            let mut best: Option<(f64, i32)> = None;
            for s in source {
                let (sx, sy, sz) = (s.x / sh, s.y / sh, s.z / sh);
                // Width matters less than height/depth for a stubby body: weight X by half.
                let d = ((tx - sx) * 0.5).powi(2) + (ty - sy).powi(2) + (tz - sz).powi(2);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, s.label));
                }
            }
            if let Some((_, label)) = best {
                driven.entry(label).or_default().push(t.label);
                assignments.push((label, t.label));
            }
        }
        LabelMap {
            driven,
            translation_scale: th / sh,
            assignments,
        }
    }

    /// Penguin labels driven by a transform's human label list. Two human labels may share a
    /// penguin label; each penguin label is listed once so no vertex is transformed twice.
    fn map_labels(&self, labels: &[i32]) -> Vec<i32> {
        let mut out: Vec<i32> = Vec::new();
        for label in labels {
            if let Some(targets) = self.driven.get(label) {
                for t in targets {
                    if !out.contains(t) {
                        out.push(*t);
                    }
                }
            }
        }
        out
    }
}

/// Origin accumulator (`rl21.rx/tg/pj`).
#[derive(Default)]
struct Origin {
    x: i32,
    y: i32,
    z: i32,
}

/// `fx.dn`: applies every transform of `frame` to `model` in order. `map` retargets the labels.
pub fn apply_frame(
    model: &mut Model,
    sequence: &Sequence,
    frame_index: usize,
    map: Option<&LabelMap>,
) -> Result<(), RenderError> {
    let frame = sequence.frames.get(frame_index).ok_or_else(|| {
        RenderError::Scene(format!(
            "sequence {} has no frame {frame_index}",
            sequence.id
        ))
    })?;
    let skeleton = &sequence.skeletons[frame.skeleton];
    if model.vertex_groups.is_none() {
        return Ok(());
    }
    let mut origin = Origin::default();
    for &(index, dx, dy, dz) in &frame.transforms {
        let kind = skeleton.types[index];
        let labels = &skeleton.labels[index];
        match map {
            Some(map) => {
                let mapped = map.map_labels(labels);
                let (dx, dy, dz) = if kind == 1 {
                    let s = map.translation_scale;
                    (
                        (f64::from(dx) * s).round() as i32,
                        (f64::from(dy) * s).round() as i32,
                        (f64::from(dz) * s).round() as i32,
                    )
                } else {
                    (dx, dy, dz)
                };
                transform(model, &mut origin, kind, &mapped, dx, dy, dz);
            }
            None => transform(model, &mut origin, kind, labels, dx, dy, dz),
        }
    }
    Ok(())
}

/// `fx.rx`: one transform over the vertex groups named by `labels`.
fn transform(
    model: &mut Model,
    origin: &mut Origin,
    kind: i32,
    labels: &[i32],
    dx: i32,
    dy: i32,
    dz: i32,
) {
    let Some(groups) = model.vertex_groups.as_ref() else {
        return;
    };
    let t = tables();
    match kind {
        0 => {
            let mut count = 0i32;
            origin.x = 0;
            origin.y = 0;
            origin.z = 0;
            for &label in labels {
                let Some(group) = groups.get(label as usize) else {
                    continue;
                };
                for &v in group {
                    let v = v as usize;
                    origin.x = (origin.x as f32 + model.xs[v]) as i32;
                    origin.y = (origin.y as f32 + model.ys[v]) as i32;
                    origin.z = (origin.z as f32 + model.zs[v]) as i32;
                    count += 1;
                }
            }
            if count > 0 {
                origin.x = dx.wrapping_add(origin.x / count);
                origin.y = dy.wrapping_add(origin.y / count);
                origin.z = dz.wrapping_add(origin.z / count);
            } else {
                origin.x = dx;
                origin.y = dy;
                origin.z = dz;
            }
        }
        1 => {
            let groups = groups.clone();
            for &label in labels {
                let Some(group) = groups.get(label as usize) else {
                    continue;
                };
                for &v in group {
                    let v = v as usize;
                    model.xs[v] += dx as f32;
                    model.ys[v] += dy as f32;
                    model.zs[v] += dz as f32;
                }
            }
        }
        2 => {
            let groups = groups.clone();
            let ox = origin.x as f32;
            let oy = origin.y as f32;
            let oz = origin.z as f32;
            let rx = (dx & 0xFF) * 8;
            let ry = (dy & 0xFF) * 8;
            let rz = (dz & 0xFF) * 8;
            for &label in labels {
                let Some(group) = groups.get(label as usize) else {
                    continue;
                };
                for &v in group {
                    let v = v as usize;
                    model.xs[v] -= ox;
                    model.ys[v] -= oy;
                    model.zs[v] -= oz;
                    if rz != 0 {
                        let s = t.sin2048[rz as usize];
                        let c = t.cos2048[rz as usize];
                        let x = model.xs[v] as i32;
                        let y = model.ys[v] as i32;
                        let nx = (s.wrapping_mul(y).wrapping_add(c.wrapping_mul(x))) >> 16;
                        model.ys[v] =
                            ((c.wrapping_mul(y)).wrapping_sub(s.wrapping_mul(x)) >> 16) as f32;
                        model.xs[v] = nx as f32;
                    }
                    if rx != 0 {
                        let s = t.sin2048[rx as usize];
                        let c = t.cos2048[rx as usize];
                        let y = model.ys[v] as i32;
                        let z = model.zs[v] as i32;
                        let ny = (c.wrapping_mul(y)).wrapping_sub(s.wrapping_mul(z)) >> 16;
                        model.zs[v] =
                            ((s.wrapping_mul(y)).wrapping_add(c.wrapping_mul(z)) >> 16) as f32;
                        model.ys[v] = ny as f32;
                    }
                    if ry != 0 {
                        let s = t.sin2048[ry as usize];
                        let c = t.cos2048[ry as usize];
                        let z = model.zs[v] as i32;
                        let x = model.xs[v] as i32;
                        let nx = (s.wrapping_mul(z)).wrapping_add(c.wrapping_mul(x)) >> 16;
                        model.zs[v] =
                            ((c.wrapping_mul(z)).wrapping_sub(s.wrapping_mul(x)) >> 16) as f32;
                        model.xs[v] = nx as f32;
                    }
                    model.xs[v] += ox;
                    model.ys[v] += oy;
                    model.zs[v] += oz;
                }
            }
        }
        3 => {
            let groups = groups.clone();
            let ox = origin.x as f32;
            let oy = origin.y as f32;
            let oz = origin.z as f32;
            for &label in labels {
                let Some(group) = groups.get(label as usize) else {
                    continue;
                };
                for &v in group {
                    let v = v as usize;
                    model.xs[v] -= ox;
                    model.ys[v] -= oy;
                    model.zs[v] -= oz;
                    model.xs[v] = (dx.wrapping_mul(model.xs[v] as i32) / 128) as f32;
                    model.ys[v] = (dy.wrapping_mul(model.ys[v] as i32) / 128) as f32;
                    model.zs[v] = (dz.wrapping_mul(model.zs[v] as i32) / 128) as f32;
                    model.xs[v] += ox;
                    model.ys[v] += oy;
                    model.zs[v] += oz;
                }
            }
        }
        5 => {
            let (Some(face_groups), Some(alphas)) =
                (model.face_groups_alt.clone(), model.alphas.as_mut())
            else {
                return;
            };
            for &label in labels {
                let Some(group) = face_groups.get(label as usize) else {
                    continue;
                };
                for &f in group {
                    let f = f as usize;
                    let mut a = (alphas[f] as i32 & 0xFF) + dx * 8;
                    a = a.clamp(0, 255);
                    alphas[f] = a as u8 as i8;
                }
            }
        }
        _ => {}
    }
}

/// `fx.jp`: float NPC scale applied after animation (`pl.ag`).
pub fn scale_float(model: &mut Model, width: i32, height: i32) {
    if width == 128 && height == 128 {
        return;
    }
    for v in 0..model.vertex_count {
        model.xs[v] = width as f32 * model.xs[v] / 128.0;
        model.ys[v] = height as f32 * model.ys[v] / 128.0;
        model.zs[v] = width as f32 * model.zs[v] / 128.0;
    }
}

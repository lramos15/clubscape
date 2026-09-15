//! Actor assembly for the renderer: NPC definitions animated through the exact skeletal port,
//! the penguin player body (approved NPC 2063 / model 21547 at 75/128) with technical
//! retargeting of the original human action sequences, and modular equipment attachment with
//! measured fit. State comes from the supplied `WorldView`; nothing here decides game rules.

use std::collections::HashMap;

use serde::Deserialize;

use crate::anim::{LabelCentroid, LabelMap, Sequence, apply_frame, label_centroids, scale_float};
use crate::error::RenderError;
use crate::model::Model;

/// Server appearance defaults for an unarmed player (`Player` appearance block: idle 808,
/// turn 823, walk 819, walk-back 820, shuffle 821/822, run 824). Item params in this cache carry
/// no stance overrides, so every M1 weapon uses them unless the WorldView names a sequence.
pub const PLAYER_IDLE: i32 = 808;
pub const PLAYER_WALK: i32 = 819;
pub const PLAYER_RUN: i32 = 824;
pub const PLAYER_DEATH: i32 = 836;

/// NPC definition record exported by `tools/render-assets` (`npc_definitions` in the manifest).
#[derive(Deserialize, Debug, Clone)]
pub struct NpcDefinitionRecord {
    pub npc_id: i32,
    #[serde(default)]
    pub name: String,
    pub size: i32,
    pub width_scale: i32,
    pub height_scale: i32,
    pub sequences: HashMap<String, i32>,
    #[serde(default)]
    pub combat_sequences: Vec<i32>,
}

#[derive(Debug, Clone)]
pub struct NpcDefinition {
    pub record: NpcDefinitionRecord,
    pub base: Model,
}

impl NpcDefinition {
    pub fn stand(&self) -> i32 {
        self.record.sequences.get("stand").copied().unwrap_or(-1)
    }
    pub fn walk(&self) -> i32 {
        self.record.sequences.get("walk").copied().unwrap_or(-1)
    }
}

/// Bone-level retargeting of the default human player skeleton onto the approved penguin body
/// (NPC 2063 / model 21547). Every human pivot/rotation label set maps onto the penguin label
/// sets the penguin's own skeleton rotates as a unit, so limbs move whole: head, neck, chest,
/// belly, pelvis (+ tail), shoulders → flipper roots, elbows → flipper middles, hands → flipper
/// tips, hips/knees/ankles/feet/toes. Derived from the label centroids of both models and the
/// bone lists of sequences 808 (human) and 5668 (penguin); see `crates/renderer/README.md`.
pub const HUMAN_TO_PENGUIN_LABELS: &[(i32, &[i32])] = &[
    // head (human 1 crown/face, 2 jaw, 3 chin) → penguin skull, brow, neck top
    (1, &[0]),
    (2, &[5]),
    (3, &[11]),
    // neck and collar → penguin neck
    (4, &[1]),
    (5, &[1]),
    (30, &[1]),
    // chest → penguin chest; lower torso → belly
    (8, &[27]),
    (29, &[28]),
    // pelvis → penguin pelvis and lower back; pelvis rear → tail
    (39, &[3, 7]),
    (41, &[9, 12, 13]),
    // right arm (character's right, model -X): shoulder top/shoulder → flipper root,
    // upper arm → flipper upper, forearm → flipper lower, hand → flipper tip
    (21, &[4]),
    (20, &[4]),
    (17, &[32, 33]),
    (19, &[34, 35]),
    (27, &[35, 36]),
    // left arm (model +X)
    (25, &[18]),
    (26, &[18]),
    (23, &[29, 30]),
    (22, &[31, 22]),
    (28, &[22, 23]),
    // right leg (model -X): hip, thigh, knee, shin, foot, toe
    (42, &[2]),
    (34, &[6]),
    (31, &[10]),
    (32, &[10]),
    (45, &[14, 15, 16, 38]),
    (48, &[8, 37]),
    // left leg (model +X)
    (40, &[17]),
    (35, &[19]),
    (37, &[21]),
    (38, &[21]),
    (46, &[24, 25, 26, 40]),
    (47, &[20, 39]),
];

/// Equipment slots of the shared contract (`PlayerView.equipment[].slot`).
pub const SLOTS: [&str; 11] = [
    "head", "cape", "amulet", "weapon", "body", "shield", "legs", "hands", "feet", "ring", "ammo",
];

/// An equippable item's original equipped model (`op.jm`, lit as `lc.bd`).
#[derive(Debug, Clone)]
pub struct EquipModel {
    pub item_id: i32,
    pub model: Model,
}

/// Fit measurement of one attached item relative to the penguin body part it is anchored to.
#[derive(Debug, Clone, PartialEq)]
pub struct FitReport {
    pub item_id: i32,
    pub slot: String,
    /// Human label the item's vertices are bound to, and the penguin label chosen.
    pub human_label: i32,
    pub penguin_label: i32,
    /// Distance (source units, unscaled body) between the item's anchor and the body anchor.
    pub anchor_gap: f64,
    /// Deepest body-part vertex found inside the item's bounding box (source units), 0 if none.
    pub penetration: f64,
    pub scale: f64,
}

/// Penguin player body with the human→penguin label retargeting table.
#[derive(Debug, Clone)]
pub struct PlayerBody {
    pub base: Model,
    pub width_scale: i32,
    pub height_scale: i32,
    pub native_sequences: Vec<i32>,
    pub label_map: LabelMap,
    pub human_centroids: Vec<LabelCentroid>,
    pub penguin_centroids: Vec<LabelCentroid>,
    pub human_min_y: f64,
    pub penguin_min_y: f64,
}

impl PlayerBody {
    pub fn new(
        base: Model,
        width_scale: i32,
        height_scale: i32,
        native_sequences: Vec<i32>,
        human_reference: &Model,
    ) -> Self {
        let human_centroids = label_centroids(human_reference);
        let penguin_centroids = label_centroids(&base);
        let human_min_y = human_reference.ys[..human_reference.vertex_count]
            .iter()
            .copied()
            .fold(0.0f32, f32::min) as f64;
        let penguin_min_y = base.ys[..base.vertex_count]
            .iter()
            .copied()
            .fold(0.0f32, f32::min) as f64;
        let label_map = LabelMap::from_table(HUMAN_TO_PENGUIN_LABELS, human_min_y, penguin_min_y);
        PlayerBody {
            base,
            width_scale,
            height_scale,
            native_sequences,
            label_map,
            human_centroids,
            penguin_centroids,
            human_min_y,
            penguin_min_y,
        }
    }

    /// Penguin label driven by `human_label`, or the driven label of the nearest human label.
    fn penguin_label_for(&self, human_label: i32) -> Option<i32> {
        if let Some(driven) = self.label_map.driven.get(&human_label) {
            let target = self
                .human_centroids
                .iter()
                .find(|c| c.label == human_label)?;
            let sh = -self.human_min_y;
            let th = -self.penguin_min_y;
            let mut best: Option<(f64, i32)> = None;
            for &p in driven {
                let pc = self.penguin_centroids.iter().find(|c| c.label == p)?;
                let d = (pc.x / th - target.x / sh).powi(2)
                    + (pc.y / th - target.y / sh).powi(2)
                    + (pc.z / th - target.z / sh).powi(2);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, p));
                }
            }
            return best.map(|(_, p)| p);
        }
        let target = self
            .human_centroids
            .iter()
            .find(|c| c.label == human_label)?;
        let mut best: Option<(f64, i32)> = None;
        for (human, penguin) in &self.label_map.assignments {
            let hc = self.human_centroids.iter().find(|c| c.label == *human)?;
            let d =
                (hc.x - target.x).powi(2) + (hc.y - target.y).powi(2) + (hc.z - target.z).powi(2);
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, *penguin));
            }
        }
        best.map(|(_, p)| p)
    }

    /// Attaches equipment to the body: every item is scaled by the height ratio, relabelled into
    /// penguin labels and translated so its bound human anchor meets the corresponding penguin
    /// label centroid. Returns the merged model and per-item fit measurements.
    pub fn assemble(&self, gear: &[(String, &EquipModel)]) -> (Model, Vec<FitReport>) {
        let mut merged = self.base.clone();
        let mut reports = Vec::new();
        let scale = self.label_map.translation_scale;
        for (slot, item) in gear {
            let item_centroids = label_centroids(&item.model);
            // The human label carrying most of the item's vertices anchors it.
            let mut counts: HashMap<i32, usize> = HashMap::new();
            if let Some(groups) = &item.model.vertex_groups {
                for (label, group) in groups.iter().enumerate() {
                    if !group.is_empty() {
                        *counts.entry(label as i32).or_default() += group.len();
                    }
                }
            }
            let Some((&item_label, _)) = counts.iter().max_by_key(|(label, n)| (**n, -**label))
            else {
                continue;
            };
            let Some(ia) = item_centroids
                .iter()
                .find(|c| c.label == item_label)
                .copied()
            else {
                continue;
            };
            // Weapon-only labels (e.g. 50) have no body vertices: bind to the body label nearest
            // the item's own anchor (the hand it is held in).
            let human_label = if self.human_centroids.iter().any(|c| c.label == item_label) {
                item_label
            } else {
                match self.human_centroids.iter().min_by(|a, b| {
                    let da = (a.x - ia.x).powi(2) + (a.y - ia.y).powi(2) + (a.z - ia.z).powi(2);
                    let db = (b.x - ia.x).powi(2) + (b.y - ia.y).powi(2) + (b.z - ia.z).powi(2);
                    da.total_cmp(&db)
                }) {
                    Some(c) => c.label,
                    None => continue,
                }
            };
            let Some(penguin_label) = self.penguin_label_for(human_label) else {
                continue;
            };
            let human_anchor = self.human_centroids.iter().find(|c| c.label == human_label);
            let penguin_anchor = self
                .penguin_centroids
                .iter()
                .find(|c| c.label == penguin_label);
            let (Some(ha), Some(pa)) = (human_anchor, penguin_anchor) else {
                continue;
            };
            let ia = &ia;
            // Offset of the item relative to the human part it is worn on, kept in penguin scale.
            let rel = (
                (ia.x - ha.x) * scale,
                (ia.y - ha.y) * scale,
                (ia.z - ha.z) * scale,
            );
            let translate = (pa.x + rel.0, pa.y + rel.1, pa.z + rel.2);
            let mut part = item.model.clone();
            for v in 0..part.vertex_count {
                part.xs[v] = ((f64::from(part.xs[v]) - ia.x) * scale + translate.0) as f32;
                part.ys[v] = ((f64::from(part.ys[v]) - ia.y) * scale + translate.1) as f32;
                part.zs[v] = ((f64::from(part.zs[v]) - ia.z) * scale + translate.2) as f32;
            }
            // Measured fit: body vertices of the anchor part inside the item's oriented box.
            // When the item cuts deeper than the allowed 1 unit, push it outward along the
            // box's thinnest axis, spending at most the 2-unit anchor allowance; the residual is
            // reported, never hidden.
            let mut penetration = penetration_depth(&self.base, penguin_label, &part);
            let mut anchor_gap = 0.0f64;
            if penetration > 1.0 {
                let (axis, sign) = outward_axis(&self.base, &part);
                let shift = (penetration - 1.0).min(2.0);
                for v in 0..part.vertex_count {
                    part.xs[v] += (axis[0] * sign * shift) as f32;
                    part.ys[v] += (axis[1] * sign * shift) as f32;
                    part.zs[v] += (axis[2] * sign * shift) as f32;
                }
                anchor_gap = shift;
                penetration = penetration_depth(&self.base, penguin_label, &part);
            }
            relabel(&mut part, |human| {
                if human == item_label {
                    penguin_label
                } else {
                    self.penguin_label_for(human).unwrap_or(penguin_label)
                }
            });
            merge_into(&mut merged, &part);
            reports.push(FitReport {
                item_id: item.item_id,
                slot: slot.clone(),
                human_label,
                penguin_label,
                anchor_gap,
                penetration,
                scale,
            });
        }
        (merged, reports)
    }

    /// Animated, scaled player model for a sequence frame. Native penguin sequences apply
    /// directly; human sequences are retargeted.
    pub fn frame(
        &self,
        assembled: &Model,
        sequence: &Sequence,
        frame: usize,
    ) -> Result<Model, RenderError> {
        let mut model = assembled.clone();
        let map = if self.native_sequences.contains(&sequence.id) {
            None
        } else {
            Some(&self.label_map)
        };
        apply_frame(&mut model, sequence, frame, map)?;
        scale_float(&mut model, self.width_scale, self.height_scale);
        model.compute_cylinder_bounds();
        Ok(model)
    }
}

/// Principal axes of a model's vertices (covariance eigenvectors by Jacobi iteration) and its
/// centroid: an oriented bounding box that hugs flat or elongated items.
#[allow(clippy::needless_range_loop)]
fn principal_axes(model: &Model) -> ([[f64; 3]; 3], [f64; 3]) {
    let n = model.vertex_count.max(1) as f64;
    let mut mean = [0.0f64; 3];
    for v in 0..model.vertex_count {
        mean[0] += f64::from(model.xs[v]);
        mean[1] += f64::from(model.ys[v]);
        mean[2] += f64::from(model.zs[v]);
    }
    for m in &mut mean {
        *m /= n;
    }
    let mut cov = [[0.0f64; 3]; 3];
    for v in 0..model.vertex_count {
        let d = [
            f64::from(model.xs[v]) - mean[0],
            f64::from(model.ys[v]) - mean[1],
            f64::from(model.zs[v]) - mean[2],
        ];
        for i in 0..3 {
            for j in 0..3 {
                cov[i][j] += d[i] * d[j] / n;
            }
        }
    }
    let mut a = cov;
    let mut vecs = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..32 {
        let (mut p, mut q, mut max) = (0, 1, 0.0f64);
        for i in 0..3 {
            for j in (i + 1)..3 {
                if a[i][j].abs() > max {
                    max = a[i][j].abs();
                    p = i;
                    q = j;
                }
            }
        }
        if max < 1e-9 {
            break;
        }
        let theta = 0.5 * (2.0 * a[p][q]).atan2(a[q][q] - a[p][p]);
        let (c, s) = (theta.cos(), theta.sin());
        let mut r = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        r[p][p] = c;
        r[q][q] = c;
        r[p][q] = s;
        r[q][p] = -s;
        let mut next = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    for l in 0..3 {
                        next[i][j] += r[k][i] * a[k][l] * r[l][j];
                    }
                }
            }
        }
        a = next;
        let mut nv = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                for k in 0..3 {
                    nv[i][j] += vecs[i][k] * r[k][j];
                }
            }
        }
        vecs = nv;
    }
    // Columns of `vecs` are eigenvectors; return them as rows sorted by variance ascending.
    let mut axes: Vec<([f64; 3], f64)> = (0..3)
        .map(|j| ([vecs[0][j], vecs[1][j], vecs[2][j]], a[j][j]))
        .collect();
    axes.sort_by(|l, r| l.1.total_cmp(&r.1));
    ([axes[0].0, axes[1].0, axes[2].0], mean)
}

fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Deepest body-part vertex inside the item's oriented bounding box (source units, unscaled).
fn penetration_depth(body: &Model, penguin_label: i32, item: &Model) -> f64 {
    let Some(groups) = &body.vertex_groups else {
        return 0.0;
    };
    let Some(group) = groups.get(penguin_label as usize) else {
        return 0.0;
    };
    if item.vertex_count == 0 || group.is_empty() {
        return 0.0;
    }
    let (axes, mean) = principal_axes(item);
    let mut min = [f64::MAX; 3];
    let mut max = [f64::MIN; 3];
    for v in 0..item.vertex_count {
        let d = [
            f64::from(item.xs[v]) - mean[0],
            f64::from(item.ys[v]) - mean[1],
            f64::from(item.zs[v]) - mean[2],
        ];
        for (i, axis) in axes.iter().enumerate() {
            let p = dot(&d, axis);
            min[i] = min[i].min(p);
            max[i] = max[i].max(p);
        }
    }
    let mut deepest = 0.0f64;
    for &v in group {
        let v = v as usize;
        let d = [
            f64::from(body.xs[v]) - mean[0],
            f64::from(body.ys[v]) - mean[1],
            f64::from(body.zs[v]) - mean[2],
        ];
        let mut inside = true;
        let mut depth = f64::MAX;
        for (i, axis) in axes.iter().enumerate() {
            let p = dot(&d, axis);
            if p < min[i] || p > max[i] {
                inside = false;
                break;
            }
            depth = depth.min((p - min[i]).min(max[i] - p));
        }
        if inside {
            deepest = deepest.max(depth);
        }
    }
    deepest
}

/// The item's thinnest principal axis, signed to point away from the body centroid.
fn outward_axis(body: &Model, item: &Model) -> ([f64; 3], f64) {
    let (axes, mean) = principal_axes(item);
    let n = body.vertex_count.max(1) as f64;
    let mut body_mean = [0.0f64; 3];
    for v in 0..body.vertex_count {
        body_mean[0] += f64::from(body.xs[v]) / n;
        body_mean[1] += f64::from(body.ys[v]) / n;
        body_mean[2] += f64::from(body.zs[v]) / n;
    }
    let away = [
        mean[0] - body_mean[0],
        mean[1] - body_mean[1],
        mean[2] - body_mean[2],
    ];
    let sign = if dot(&away, &axes[0]) >= 0.0 {
        1.0
    } else {
        -1.0
    };
    (axes[0], sign)
}

fn relabel(model: &mut Model, map: impl Fn(i32) -> i32) {
    if let Some(groups) = model.vertex_groups.take() {
        let mut out: Vec<Vec<i32>> = Vec::new();
        for (label, group) in groups.into_iter().enumerate() {
            if group.is_empty() {
                continue;
            }
            let target = map(label as i32).max(0) as usize;
            if out.len() <= target {
                out.resize(target + 1, Vec::new());
            }
            out[target].extend(group);
        }
        model.vertex_groups = Some(out);
    }
    if let Some(groups) = model.face_groups_alt.take() {
        let mut out: Vec<Vec<i32>> = Vec::new();
        for (label, group) in groups.into_iter().enumerate() {
            if group.is_empty() {
                continue;
            }
            let target = map(label as i32).max(0) as usize;
            if out.len() <= target {
                out.resize(target + 1, Vec::new());
            }
            out[target].extend(group);
        }
        model.face_groups_alt = Some(out);
    }
}

/// Concatenates `part` into `base` (vertices, faces, per-face attributes, groups) the way the
/// original model merge (`er(er[])`) does, without re-lighting (both are lit identically).
pub fn merge_into(base: &mut Model, part: &Model) {
    let vertex_offset = base.vertex_count as i32;
    let face_offset = base.face_count as i32;
    base.xs.truncate(base.vertex_count);
    base.ys.truncate(base.vertex_count);
    base.zs.truncate(base.vertex_count);
    base.xs.extend_from_slice(&part.xs[..part.vertex_count]);
    base.ys.extend_from_slice(&part.ys[..part.vertex_count]);
    base.zs.extend_from_slice(&part.zs[..part.vertex_count]);
    let fc = base.face_count;
    let pfc = part.face_count;
    for (dst, src) in [
        (&mut base.face_a, &part.face_a),
        (&mut base.face_b, &part.face_b),
        (&mut base.face_c, &part.face_c),
    ] {
        dst.truncate(fc);
        dst.extend(src[..pfc].iter().map(|&i| i + vertex_offset));
    }
    for (dst, src) in [
        (&mut base.color_a, &part.color_a),
        (&mut base.color_b, &part.color_b),
        (&mut base.color_c, &part.color_c),
    ] {
        dst.truncate(fc);
        dst.extend_from_slice(&src[..pfc]);
    }
    fn extend_opt<T: Copy + Default>(
        dst: &mut Option<Vec<T>>,
        src: &Option<Vec<T>>,
        fc: usize,
        pfc: usize,
        fill: T,
    ) {
        match (dst.as_mut(), src) {
            (None, None) => {}
            (Some(d), Some(s)) => {
                d.truncate(fc);
                d.extend_from_slice(&s[..pfc]);
            }
            (Some(d), None) => {
                d.truncate(fc);
                d.extend(std::iter::repeat_n(fill, pfc));
            }
            (None, Some(s)) => {
                let mut d = vec![fill; fc];
                d.extend_from_slice(&s[..pfc]);
                *dst = Some(d);
            }
        }
    }
    extend_opt(&mut base.textures, &part.textures, fc, pfc, -1i16);
    extend_opt(
        &mut base.texture_coords,
        &part.texture_coords,
        fc,
        pfc,
        -1i8,
    );
    extend_opt(&mut base.priorities, &part.priorities, fc, pfc, 0i8);
    extend_opt(&mut base.alphas, &part.alphas, fc, pfc, 0i8);
    extend_opt(&mut base.bias, &part.bias, fc, pfc, 0i8);
    if !part.tex_p.is_empty() {
        base.tex_p
            .extend(part.tex_p.iter().map(|&i| i + vertex_offset));
        base.tex_m
            .extend(part.tex_m.iter().map(|&i| i + vertex_offset));
        base.tex_n
            .extend(part.tex_n.iter().map(|&i| i + vertex_offset));
    }
    if let Some(groups) = &part.vertex_groups {
        let dst = base.vertex_groups.get_or_insert_with(Vec::new);
        for (label, group) in groups.iter().enumerate() {
            if dst.len() <= label {
                dst.resize(label + 1, Vec::new());
            }
            dst[label].extend(group.iter().map(|&v| v + vertex_offset));
        }
    }
    if let Some(groups) = &part.face_groups_alt {
        let dst = base.face_groups_alt.get_or_insert_with(Vec::new);
        for (label, group) in groups.iter().enumerate() {
            if dst.len() <= label {
                dst.resize(label + 1, Vec::new());
            }
            dst[label].extend(group.iter().map(|&f| f + face_offset));
        }
    }
    base.vertex_count += part.vertex_count;
    base.face_count += part.face_count;
    if part.priorities.is_some() && base.priorities.is_none() {
        base.priorities = Some(vec![0; base.face_count]);
    }
}

/// Activity-derived default sequence for the player when the WorldView names none. `context`
/// describes what stands next to the player (from the scene/WorldView) so gathering and
/// producing pick the original tool motion; the server-bound `animation` always wins.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ActivityContext {
    pub adjacent_tree: bool,
    pub adjacent_rock: bool,
    pub adjacent_fishing_spot: bool,
    pub adjacent_fire: bool,
    pub adjacent_range: bool,
    pub adjacent_furnace: bool,
    pub adjacent_anvil: bool,
    pub weapon_item: Option<i32>,
    pub moving: bool,
    pub running: bool,
    pub dead: bool,
}

pub fn player_sequence_for(activity: &str, context: &ActivityContext) -> i32 {
    if context.dead {
        return PLAYER_DEATH;
    }
    match activity {
        "walking" => {
            if context.running {
                PLAYER_RUN
            } else {
                PLAYER_WALK
            }
        }
        "gathering" => {
            if context.adjacent_fishing_spot {
                621
            } else if context.adjacent_rock {
                625
            } else {
                // Trees (and gathering with nothing recognised nearby): woodcutting motion.
                879
            }
        }
        "producing" => {
            if context.adjacent_anvil {
                898
            } else if context.adjacent_furnace {
                899
            } else if context.adjacent_range {
                896
            } else if context.adjacent_fire {
                897
            } else {
                733
            }
        }
        "fighting" => match context.weapon_item {
            Some(841) => 426,
            Some(1205) => 386,
            Some(1277) | Some(1237) | Some(1351) | Some(1265) => 390,
            _ => 422,
        },
        "casting" => 711,
        _ => {
            if context.moving {
                if context.running {
                    PLAYER_RUN
                } else {
                    PLAYER_WALK
                }
            } else {
                PLAYER_IDLE
            }
        }
    }
}

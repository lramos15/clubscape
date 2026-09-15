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

/// Fit measurement of one attached item on the penguin body (source units, unscaled body).
///
/// * `penetration`: deepest body vertex (any part) inside the item's oriented bounding box after
///   the contact solve — the "unintended penetration" figure, target ≤ 1.
/// * `gap`: clearance between the item and the body — the smallest distance from any item vertex
///   to a body triangle or from any body vertex to an item triangle (0 when they touch or
///   overlap) — the "gap" figure, target ≤ 2, i.e. the item rests on the body and does not float.
/// * `anchor_shift`: how far the contact solve translated the item from its retargeted design
///   position (informational; the original item geometry is never edited).
/// * `design_penetration`: the same box measure of the item on the human reference body it was
///   designed for (the source's own overlap, e.g. a hat wrapping the skull), for comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct FitReport {
    pub item_id: i32,
    pub slot: String,
    /// Human label the item's vertices are bound to, and the penguin label chosen.
    pub human_label: i32,
    pub penguin_label: i32,
    pub anchor_shift: f64,
    /// Direction of the contact-solve shift in body space (`x`, `y`, `z`), zero when unshifted.
    pub shift_direction: [f64; 3],
    pub penetration: f64,
    /// Body-vertex depth in the principal-axis box alone (looser frame; see `pca_box_penetration`).
    pub pca_box_penetration: f64,
    pub gap: f64,
    pub design_penetration: f64,
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
    /// The human reference body (design baseline for the fit measures).
    pub human: Model,
}

/// Maximum penetration (source units) the fit accepts; the contact solve stops here so the item
/// still touches the body instead of floating.
pub const FIT_MAX_PENETRATION: f64 = 1.0;
/// Maximum clearance (source units) between an item and the body.
pub const FIT_MAX_GAP: f64 = 2.0;

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
            human: human_reference.clone(),
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
            // Contact solve: the retargeted design position is kept unless a body vertex (any
            // part) sits deeper than FIT_MAX_PENETRATION inside the item's oriented box; then the
            // whole item is translated along the box axis that resolves it with the smallest
            // shift, stopping at contact so it rests on the body. Geometry is never edited.
            let design_penetration = fit_penetration(&self.human, &item.model);
            let (anchor_shift, shift_direction) =
                contact_solve(&self.base, slot_outward(slot), &mut part);
            let penetration = fit_penetration(&self.base, &part);
            let pca_box_penetration = pca_box_penetration(&self.base, &part);
            let gap = clearance(&self.base, &part);
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
                anchor_shift,
                shift_direction,
                penetration,
                pca_box_penetration,
                gap,
                design_penetration,
                scale,
            });
        }
        (merged, reports)
    }

    /// Animated player model for a sequence frame in source units (before the definition's
    /// 75/128 draw scale). Native penguin sequences apply directly; human sequences are
    /// retargeted.
    pub fn pose(
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
        Ok(model)
    }

    /// Animated, scaled player model for a sequence frame (what is drawn).
    pub fn frame(
        &self,
        assembled: &Model,
        sequence: &Sequence,
        frame: usize,
    ) -> Result<Model, RenderError> {
        let mut model = self.pose(assembled, sequence, frame)?;
        scale_float(&mut model, self.width_scale, self.height_scale);
        model.compute_cylinder_bounds();
        Ok(model)
    }
}

/// Splits an assembled+posed player model back into the body and one attached part (the part
/// occupies the vertices/faces appended after the body by `merge_into`).
pub fn split_part(assembled: &Model, body_vertices: usize, body_faces: usize) -> (Model, Model) {
    let mut body = assembled.clone();
    body.vertex_count = body_vertices;
    body.face_count = body_faces;
    body.xs.truncate(body_vertices);
    body.ys.truncate(body_vertices);
    body.zs.truncate(body_vertices);
    body.face_a.truncate(body_faces);
    body.face_b.truncate(body_faces);
    body.face_c.truncate(body_faces);
    let limit = body_vertices as i32;
    if let Some(groups) = body.vertex_groups.as_mut() {
        for group in groups.iter_mut() {
            group.retain(|v| *v < limit);
        }
    }
    let face_limit = body_faces as i32;
    for groups in [body.face_groups.as_mut(), body.face_groups_alt.as_mut()]
        .into_iter()
        .flatten()
    {
        for group in groups.iter_mut() {
            group.retain(|f| *f < face_limit);
        }
    }
    let mut part = assembled.clone();
    part.vertex_count = assembled.vertex_count - body_vertices;
    part.face_count = assembled.face_count - body_faces;
    part.xs = assembled.xs[body_vertices..assembled.vertex_count].to_vec();
    part.ys = assembled.ys[body_vertices..assembled.vertex_count].to_vec();
    part.zs = assembled.zs[body_vertices..assembled.vertex_count].to_vec();
    let offset = body_vertices as i32;
    part.face_a = assembled.face_a[body_faces..assembled.face_count]
        .iter()
        .map(|v| v - offset)
        .collect();
    part.face_b = assembled.face_b[body_faces..assembled.face_count]
        .iter()
        .map(|v| v - offset)
        .collect();
    part.face_c = assembled.face_c[body_faces..assembled.face_count]
        .iter()
        .map(|v| v - offset)
        .collect();
    part.vertex_groups = None;
    part.face_groups_alt = None;
    (body, part)
}

/// Oriented box: axes (rows), centre, and per-axis min/max extents of the item's vertices.
type ItemBox = ([[f64; 3]; 3], [f64; 3], [f64; 3], [f64; 3]);

/// Candidate oriented boxes for an item: its covariance (principal-axis) box and its
/// model-axis-aligned box. [`item_box`] keeps the tighter one: an oriented bounding box is the
/// minimum-volume box, and the principal-axis frame is only a heuristic for it that degenerates
/// (arbitrary tilt, loose box) when two variances are nearly equal, as for a chef's hat.
fn item_boxes(model: &Model) -> [ItemBox; 2] {
    let (pca_axes, mean) = principal_axes(model);
    let aligned = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let extents = |axes: [[f64; 3]; 3]| -> ItemBox {
        let mut min = [f64::MAX; 3];
        let mut max = [f64::MIN; 3];
        for v in 0..model.vertex_count {
            let d = [
                f64::from(model.xs[v]) - mean[0],
                f64::from(model.ys[v]) - mean[1],
                f64::from(model.zs[v]) - mean[2],
            ];
            for (i, axis) in axes.iter().enumerate() {
                let p = dot(&d, axis);
                min[i] = min[i].min(p);
                max[i] = max[i].max(p);
            }
        }
        (axes, mean, min, max)
    };
    [extents(pca_axes), extents(aligned)]
}

fn box_volume(b: &ItemBox) -> f64 {
    (0..3).map(|i| (b.3[i] - b.2[i]).max(1e-6)).product()
}

/// The item's minimum-volume box among the candidates (see [`item_boxes`]).
fn item_box(model: &Model) -> ItemBox {
    let [pca, aligned] = item_boxes(model);
    if box_volume(&aligned) < box_volume(&pca) {
        aligned
    } else {
        pca
    }
}

/// Deepest `group` vertex inside one oriented box.
fn depth_in_box(body: &Model, group: &[i32], (axes, mean, min, max): &ItemBox) -> f64 {
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

/// Deepest body vertex inside the item's oriented bounding box (source units, unscaled): the
/// smallest distance from the vertex to any box face. `part` restricts the body vertices to one
/// label; `None` measures the whole body.
pub fn penetration_depth(body: &Model, part: Option<i32>, item: &Model) -> f64 {
    let all: Vec<i32>;
    let group: &[i32] = match part {
        Some(label) => match body
            .vertex_groups
            .as_ref()
            .and_then(|g| g.get(label as usize))
        {
            Some(group) => group,
            None => return 0.0,
        },
        None => {
            all = (0..body.vertex_count as i32).collect();
            &all
        }
    };
    if item.vertex_count == 0 || group.is_empty() {
        return 0.0;
    }
    depth_in_box(body, group, &item_box(item))
}

/// The same measure in the principal-axis frame only (the earlier, looser reading of the box
/// metric); reported alongside so the box choice is visible, not silently applied.
pub fn pca_box_penetration(body: &Model, item: &Model) -> f64 {
    if item.vertex_count == 0 || body.vertex_count == 0 {
        return 0.0;
    }
    let all: Vec<i32> = (0..body.vertex_count as i32).collect();
    let [pca, _] = item_boxes(item);
    depth_in_box(body, &all, &pca)
}

/// Deepest item vertex inside the body mesh (source units): inside/outside by the generalized
/// winding number of the body triangles, depth as the distance to the body surface. Catches
/// items swallowed by a thicker body part, which the box measure above cannot see.
pub fn embedded_depth(body: &Model, item: &Model) -> f64 {
    let vertex = |m: &Model, v: usize| [f64::from(m.xs[v]), f64::from(m.ys[v]), f64::from(m.zs[v])];
    let mut deepest = 0.0f64;
    for p in 0..item.vertex_count {
        let point = vertex(item, p);
        let mut winding = 0.0f64;
        for f in 0..body.face_count {
            winding += solid_angle(
                &point,
                &vertex(body, body.face_a[f] as usize),
                &vertex(body, body.face_b[f] as usize),
                &vertex(body, body.face_c[f] as usize),
            );
        }
        if winding.abs() >= 2.0 * std::f64::consts::PI {
            let mut nearest = f64::MAX;
            for f in 0..body.face_count {
                nearest = nearest.min(point_triangle_distance(
                    &point,
                    &vertex(body, body.face_a[f] as usize),
                    &vertex(body, body.face_b[f] as usize),
                    &vertex(body, body.face_c[f] as usize),
                ));
            }
            deepest = deepest.max(nearest);
        }
    }
    deepest
}

/// Signed solid angle of triangle `abc` seen from `p` (Van Oosterom–Strackee).
fn solid_angle(p: &[f64; 3], a: &[f64; 3], b: &[f64; 3], c: &[f64; 3]) -> f64 {
    let ra = [a[0] - p[0], a[1] - p[1], a[2] - p[2]];
    let rb = [b[0] - p[0], b[1] - p[1], b[2] - p[2]];
    let rc = [c[0] - p[0], c[1] - p[1], c[2] - p[2]];
    let la = dot(&ra, &ra).sqrt();
    let lb = dot(&rb, &rb).sqrt();
    let lc = dot(&rc, &rc).sqrt();
    let cross = [
        rb[1] * rc[2] - rb[2] * rc[1],
        rb[2] * rc[0] - rb[0] * rc[2],
        rb[0] * rc[1] - rb[1] * rc[0],
    ];
    let numerator = dot(&ra, &cross);
    let denominator = la * lb * lc + dot(&ra, &rb) * lc + dot(&ra, &rc) * lb + dot(&rb, &rc) * la;
    2.0 * numerator.atan2(denominator)
}

/// Combined fit penetration: the deeper of body-into-item (box) and item-into-body (mesh).
pub fn fit_penetration(body: &Model, item: &Model) -> f64 {
    penetration_depth(body, None, item).max(embedded_depth(body, item))
}

/// Rigid motion (rotation rows, translation) that carries `from` onto `to`, recovered from a
/// non-degenerate vertex triple; exact for the per-label rigid transforms of sequences.
fn rigid_motion(from: &Model, to: &Model) -> Option<([[f64; 3]; 3], [f64; 3])> {
    let v = |m: &Model, i: usize| [f64::from(m.xs[i]), f64::from(m.ys[i]), f64::from(m.zs[i])];
    let sub = |a: &[f64; 3], b: &[f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let cross = |a: &[f64; 3], b: &[f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let unit = |a: &[f64; 3]| {
        let l = dot(a, a).sqrt();
        (l > 1e-9).then(|| [a[0] / l, a[1] / l, a[2] / l])
    };
    let frame = |m: &Model, a: usize, b: usize, c: usize| -> Option<[[f64; 3]; 3]> {
        let e0 = unit(&sub(&v(m, b), &v(m, a)))?;
        let ac = sub(&v(m, c), &v(m, a));
        let e2 = unit(&cross(&e0, &ac))?;
        let e1 = cross(&e2, &e0);
        Some([e0, e1, e2])
    };
    let n = from.vertex_count.min(to.vertex_count);
    if n < 3 {
        return None;
    }
    let mut best: Option<(f64, usize, usize, usize)> = None;
    for a in 0..n {
        for b in (a + 1)..n {
            for c in (b + 1)..n {
                let ab = sub(&v(from, b), &v(from, a));
                let ac = sub(&v(from, c), &v(from, a));
                let area = dot(&cross(&ab, &ac), &cross(&ab, &ac));
                if best.is_none_or(|(ba, _, _, _)| area > ba) {
                    best = Some((area, a, b, c));
                }
            }
            if best.is_some_and(|(area, _, _, _)| area > 1e6) {
                break;
            }
        }
    }
    let (_, a, b, c) = best?;
    let f0 = frame(from, a, b, c)?;
    let f1 = frame(to, a, b, c)?;
    // R = F1^T F0 maps bind-frame coordinates to posed coordinates: R x = F1^T (F0 x).
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = (0..3).map(|k| f1[k][i] * f0[k][j]).sum();
        }
    }
    let pa = v(from, a);
    let qa = v(to, a);
    let rpa = [dot(&r[0], &pa), dot(&r[1], &pa), dot(&r[2], &pa)];
    Some((r, [qa[0] - rpa[0], qa[1] - rpa[1], qa[2] - rpa[2]]))
}

/// Body-into-item box depth for a posed pair, with the box fixed at the bind pose (`bind_item`)
/// and carried rigidly with the item, so the figure does not change with the box's orientation
/// heuristics when a rigidly attached item merely turns with its part.
pub fn posed_box_penetration(bind_item: &Model, body: &Model, item: &Model) -> f64 {
    let (axes, mean, min, max) = item_box(bind_item);
    let Some((r, t)) = rigid_motion(bind_item, item) else {
        return penetration_depth(body, None, item);
    };
    let rot = |x: &[f64; 3]| [dot(&r[0], x), dot(&r[1], x), dot(&r[2], x)];
    let posed_axes = [rot(&axes[0]), rot(&axes[1]), rot(&axes[2])];
    let rm = rot(&mean);
    let posed_mean = [rm[0] + t[0], rm[1] + t[1], rm[2] + t[2]];
    let all: Vec<i32> = (0..body.vertex_count as i32).collect();
    depth_in_box(body, &all, &(posed_axes, posed_mean, min, max))
}

/// Combined fit penetration for a posed pair (see [`posed_box_penetration`]).
pub fn posed_fit_penetration(bind_item: &Model, body: &Model, item: &Model) -> f64 {
    posed_box_penetration(bind_item, body, item).max(embedded_depth(body, item))
}

/// The direction an item of `slot` moves to clear the body: away from the body along the
/// slot's natural outward axis (character's right hand is model −X, left hand +X, head up,
/// amulet forward, cape back). `None` for slots without a fixed side (radial fallback).
pub fn slot_outward(slot: &str) -> Option<[f64; 3]> {
    match slot {
        "weapon" | "hands" => Some([-1.0, 0.0, 0.0]),
        "shield" => Some([1.0, 0.0, 0.0]),
        "head" => Some([0.0, -1.0, 0.0]),
        "amulet" => Some([0.0, 0.0, -1.0]),
        "cape" => Some([0.0, 0.0, 1.0]),
        _ => None,
    }
}

/// Translates `item` along the slot's outward direction by the smallest distance that brings
/// the combined penetration to at most [`FIT_MAX_PENETRATION`] while the clearance stays within
/// [`FIT_MAX_GAP`] (the item rests on the body). When that direction cannot satisfy both, every
/// body axis, the radial direction and the item's box axes are tried and the smallest shift
/// meeting both wins (then the smallest meeting penetration alone). Geometry is never edited.
/// Returns the shift and its direction.
fn contact_solve(body: &Model, outward: Option<[f64; 3]>, item: &mut Model) -> (f64, [f64; 3]) {
    if fit_penetration(body, item) <= FIT_MAX_PENETRATION && clearance(body, item) <= FIT_MAX_GAP {
        return (0.0, [0.0; 3]);
    }
    let (axes, mean, _, _) = item_box(item);
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
    const COARSE: f64 = 0.5;
    const LIMIT: f64 = 64.0;
    let shifted = |dir: &[f64; 3], t: f64| {
        let mut probe = item.clone();
        for v in 0..probe.vertex_count {
            probe.xs[v] = (f64::from(item.xs[v]) + dir[0] * t) as f32;
            probe.ys[v] = (f64::from(item.ys[v]) + dir[1] * t) as f32;
            probe.zs[v] = (f64::from(item.zs[v]) + dir[2] * t) as f32;
        }
        probe
    };
    // Smallest shift along `dir` meeting the penetration target, and whether it also meets the
    // gap target there.
    let solve_along = |dir: &[f64; 3]| -> Option<(f64, bool)> {
        let mut previous = 0.0;
        let mut t = COARSE;
        while t <= LIMIT {
            if fit_penetration(body, &shifted(dir, t)) <= FIT_MAX_PENETRATION {
                let (mut lo, mut hi) = (previous, t);
                for _ in 0..6 {
                    let mid = 0.5 * (lo + hi);
                    if fit_penetration(body, &shifted(dir, mid)) <= FIT_MAX_PENETRATION {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let contact = clearance(body, &shifted(dir, hi)) <= FIT_MAX_GAP;
                return Some((hi, contact));
            }
            previous = t;
            t += COARSE;
        }
        None
    };
    let mut chosen: Option<(f64, [f64; 3])> = None;
    if let Some(dir) = outward
        && let Some((t, true)) = solve_along(&dir)
    {
        chosen = Some((t, dir));
    }
    if chosen.is_none() {
        let mut directions: Vec<[f64; 3]> = Vec::new();
        let radial = dot(&away, &away).sqrt();
        if radial > 1e-6 {
            directions.push([away[0] / radial, away[1] / radial, away[2] / radial]);
        }
        for axis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
            .iter()
            .chain(axes.iter())
        {
            directions.push(*axis);
            directions.push([-axis[0], -axis[1], -axis[2]]);
        }
        let mut best: Option<(f64, bool, [f64; 3])> = None;
        for dir in &directions {
            let Some((t, contact)) = solve_along(dir) else {
                continue;
            };
            let better = match best {
                None => true,
                Some((bt, bc, _)) => (contact && !bc) || (contact == bc && t < bt - 1e-9),
            };
            if better {
                best = Some((t, contact, *dir));
            }
        }
        chosen = best.map(|(t, _, dir)| (t, dir));
    }
    let Some((t, dir)) = chosen else {
        return (0.0, [0.0; 3]);
    };
    for v in 0..item.vertex_count {
        item.xs[v] = (f64::from(item.xs[v]) + dir[0] * t) as f32;
        item.ys[v] = (f64::from(item.ys[v]) + dir[1] * t) as f32;
        item.zs[v] = (f64::from(item.zs[v]) + dir[2] * t) as f32;
    }
    (t, dir)
}

/// Smallest distance between the item and the body surfaces: item vertices to body triangles
/// and body vertices to item triangles (0 when they touch or overlap).
pub fn clearance(body: &Model, item: &Model) -> f64 {
    fn sweep(points: &Model, tris: &Model, best: &mut f64) {
        let vertex =
            |m: &Model, v: usize| [f64::from(m.xs[v]), f64::from(m.ys[v]), f64::from(m.zs[v])];
        for p in 0..points.vertex_count {
            let point = vertex(points, p);
            for f in 0..tris.face_count {
                let d = point_triangle_distance(
                    &point,
                    &vertex(tris, tris.face_a[f] as usize),
                    &vertex(tris, tris.face_b[f] as usize),
                    &vertex(tris, tris.face_c[f] as usize),
                );
                if d < *best {
                    *best = d;
                    if *best == 0.0 {
                        return;
                    }
                }
            }
        }
    }
    let mut best = f64::MAX;
    sweep(item, body, &mut best);
    if best > 0.0 {
        sweep(body, item, &mut best);
    }
    if best == f64::MAX { 0.0 } else { best }
}

/// Euclidean distance from `p` to triangle `abc` (Ericson, Real-Time Collision Detection 5.1.5).
fn point_triangle_distance(p: &[f64; 3], a: &[f64; 3], b: &[f64; 3], c: &[f64; 3]) -> f64 {
    let sub = |u: &[f64; 3], v: &[f64; 3]| [u[0] - v[0], u[1] - v[1], u[2] - v[2]];
    let len = |u: &[f64; 3]| dot(u, u).sqrt();
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(&ab, &ap);
    let d2 = dot(&ac, &ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return len(&ap);
    }
    let bp = sub(p, b);
    let d3 = dot(&ab, &bp);
    let d4 = dot(&ac, &bp);
    if d3 >= 0.0 && d4 <= d3 {
        return len(&bp);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        let q = [a[0] + ab[0] * v, a[1] + ab[1] * v, a[2] + ab[2] * v];
        return len(&sub(p, &q));
    }
    let cp = sub(p, c);
    let d5 = dot(&ab, &cp);
    let d6 = dot(&ac, &cp);
    if d6 >= 0.0 && d5 <= d6 {
        return len(&cp);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        let q = [a[0] + ac[0] * w, a[1] + ac[1] * w, a[2] + ac[2] * w];
        return len(&sub(p, &q));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let q = [
            b[0] + (c[0] - b[0]) * w,
            b[1] + (c[1] - b[1]) * w,
            b[2] + (c[2] - b[2]) * w,
        ];
        return len(&sub(p, &q));
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    let q = [
        a[0] + ab[0] * v + ac[0] * w,
        a[1] + ab[1] * v + ac[1] * w,
        a[2] + ab[2] * v + ac[2] * w,
    ];
    len(&sub(p, &q))
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

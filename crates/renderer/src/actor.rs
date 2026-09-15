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
    /// Length of the translation the attachment fit applied (= the bind attachment gap).
    pub anchor_shift: f64,
    /// Direction of that translation in body space (`x`, `y`, `z`), zero when unshifted.
    pub shift_direction: [f64; 3],
    pub penetration: f64,
    /// Body-vertex depth in the principal-axis box alone (looser frame; see `pca_box_penetration`).
    pub pca_box_penetration: f64,
    pub gap: f64,
    pub design_penetration: f64,
    pub scale: f64,
    /// Attachment gap (source units): how far the item's grip/contact point sits from where the
    /// retargeted anchor carries it (target ≤ [`FIT_MAX_ATTACHMENT`]); see [`Attachment`].
    pub attachment_gap: f64,
    /// Rotation about the grip the attachment fit applied, degrees (0 = design orientation).
    pub rotation_deg: f64,
    /// Clearance between the item and the anchor body part alone (the part it is worn on /
    /// held by): 0 when touching it, larger when it rests on some other part.
    pub anchor_clearance: f64,
    /// The embedded half of `penetration` alone: the deepest item vertex inside the body mesh
    /// (source units). `penetration − embedded > 0` means the box half decides — body geometry
    /// inside the item's carried bind box without item geometry inside the body (an arm behind
    /// a shield plate). Provenance for the gate, not a second target.
    pub embedded: f64,
}

/// Maximum attachment gap (source units): the item's grip/contact point may sit at most this
/// far from where the retargeted anchor bone carries it (frozen comparison policy
/// `proposed_attachment_gap_source_units_max`).
pub const FIT_MAX_ATTACHMENT: f64 = 2.0;

/// How an item is attached to its anchor body part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Attachment {
    /// Gripped (weapon / shield slots): the grip point must follow the hand bone.
    Held,
    /// Resting (head / neck / body …): the contact point must follow the bone and the item
    /// must touch that part.
    Worn,
}

/// One item attached by [`PlayerBody::assemble_parts`]: where its vertices and faces sit in the
/// merged model (after the body and the parts attached before it).
#[derive(Debug, Clone, PartialEq)]
pub struct AttachedPart {
    pub item_id: i32,
    pub slot: String,
    pub vertices: std::ops::Range<usize>,
    pub faces: std::ops::Range<usize>,
    /// Index (within the part) of the grip / contact vertex: the item vertex nearest the human
    /// anchor label's centroid in the design.
    pub grip: usize,
    /// Outward direction of the slot in body space (away from the body).
    pub outward: [f64; 3],
    /// Penguin anchor label the grip follows.
    pub anchor_label: i32,
    pub attachment: Attachment,
    /// Translation the bind attachment fit applied (body space); the bind attachment gap is its
    /// length and every pose starts from it.
    pub bind_offset: [f64; 3],
}

/// Fit of one attached item in one posed frame after the per-pose contact solve (source units,
/// unscaled body): the rigid shift applied to the item that frame and the measures it left.
#[derive(Debug, Clone, PartialEq)]
pub struct PoseFit {
    pub item_id: i32,
    pub slot: String,
    /// Length of the translation applied this frame (body space, source units).
    pub shift: f64,
    pub direction: [f64; 3],
    /// The translation applied (body space, source units); `shift` is its length.
    pub offset: [f64; 3],
    /// Rotation about the posed grip point applied this frame (axis-angle, radians).
    pub rotation: [f64; 3],
    /// True when the transform came from a [`PoseFitTable`] entry instead of a live solve.
    pub precomputed: bool,
    /// Carried-bind-box / embedded penetration after the fit (target ≤ [`FIT_MAX_PENETRATION`]).
    pub penetration: f64,
    /// Item↔body surface clearance after the fit (target ≤ [`FIT_MAX_GAP`]).
    pub gap: f64,
    /// Attachment gap: distance between the fitted grip point and where the posed anchor bone
    /// carries the retargeted grip (target ≤ [`FIT_MAX_ATTACHMENT`]).
    pub attachment_gap: f64,
    /// Clearance between the item and its anchor body part alone.
    pub anchor_clearance: f64,
    /// Deepest item vertex inside the body mesh (the embedded half of `penetration`).
    pub embedded: f64,
}

impl PoseFit {
    /// Penetration, surface clearance and attachment targets all met.
    pub fn meets_targets(&self) -> bool {
        self.penetration <= FIT_MAX_PENETRATION
            && self.gap <= FIT_MAX_GAP
            && self.attachment_gap <= FIT_MAX_ATTACHMENT
    }
}

/// What a sequence's hand-item override (`lc.bd`, source `ou.by` / `ou.bq`) puts into the
/// shield (left) or weapon (right) equipment slot while it plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandSlot {
    /// The sequence does not touch the slot (value −1): the worn item shows.
    Worn,
    /// The slot draws nothing (a kit id the cache does not have, e.g. value 0 → kit 1280).
    Hidden,
    /// The slot draws this item's equipped model instead of the worn one.
    Item(i32),
    /// The slot draws a player kit (body part) — the source rule allows it, the penguin body has
    /// no kits; reported, drawn as nothing.
    Kit(i32),
}

/// Decodes a sequence hand-item value exactly as `lc.bd`/`lc.at` do: `value − 512 + 2048` is an
/// equipment id; `>= 2048` an item (`− 2048`), `256..2047` a kit (`− 256`), else nothing. `kits`
/// are the kit ids present in the source cache (manifest `sequence_hand_overrides`); a kit id
/// the cache lacks draws nothing.
pub fn hand_override(value: i32, kits: &std::collections::HashSet<i32>) -> HandSlot {
    if value < 0 {
        return HandSlot::Worn;
    }
    let equipment = value - 512 + 2048;
    if equipment >= 2048 {
        HandSlot::Item(equipment - 2048)
    } else if equipment >= 256 {
        let kit = equipment - 256;
        if kits.contains(&kit) {
            HandSlot::Kit(kit)
        } else {
            HandSlot::Hidden
        }
    } else {
        HandSlot::Hidden
    }
}

/// Equipment slot names the hand overrides address.
pub const WEAPON_SLOT: &str = "weapon";
pub const SHIELD_SLOT: &str = "shield";

/// The effective equipment of a frame: the worn `(slot, item)` list with the sequence's hand
/// overrides applied (`lc.bd`: `leftHandItem` replaces the shield slot, `rightHandItem` the
/// weapon slot; the same item in both slots is drawn twice, as the original merges both slot
/// models). Returns the list and the source-rule notes for slots that draw nothing or a kit.
pub fn effective_gear(
    worn: &[(String, i32)],
    sequence: &Sequence,
    kits: &std::collections::HashSet<i32>,
) -> (Vec<(String, i32)>, Vec<String>) {
    let left = hand_override(sequence.left_hand_item, kits);
    let right = hand_override(sequence.right_hand_item, kits);
    let mut out: Vec<(String, i32)> = worn
        .iter()
        .filter(|(slot, _)| {
            !(slot == SHIELD_SLOT && left != HandSlot::Worn)
                && !(slot == WEAPON_SLOT && right != HandSlot::Worn)
        })
        .cloned()
        .collect();
    let mut notes = Vec::new();
    for (slot, hand, value) in [
        (SHIELD_SLOT, left, sequence.left_hand_item),
        (WEAPON_SLOT, right, sequence.right_hand_item),
    ] {
        match hand {
            HandSlot::Worn => {}
            HandSlot::Item(item) => out.push((slot.to_string(), item)),
            HandSlot::Hidden => notes.push(format!(
                "sequence {} hand item {value} hides the {slot} slot (equipment id {} is no item and no cached kit)",
                sequence.id,
                value - 512 + 2048
            )),
            HandSlot::Kit(kit) => notes.push(format!(
                "sequence {} hand item {value} puts kit {kit} into the {slot} slot; player kits are not drawn on the penguin body",
                sequence.id
            )),
        }
    }
    (out, notes)
}

/// Whether an item worn in `slot` is drawn while `sequence` plays, from the source hand
/// overrides alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemVisibility {
    /// Drawn as worn (the sequence does not override its slot).
    Worn,
    /// Drawn because the sequence itself puts this item into a hand slot.
    Override,
    /// Not drawn: the sequence overrides its slot with something else or nothing.
    Hidden,
}

pub fn item_visibility(
    slot: &str,
    item_id: i32,
    sequence: &Sequence,
    kits: &std::collections::HashSet<i32>,
) -> ItemVisibility {
    let left = hand_override(sequence.left_hand_item, kits);
    let right = hand_override(sequence.right_hand_item, kits);
    if left == HandSlot::Item(item_id) || right == HandSlot::Item(item_id) {
        return ItemVisibility::Override;
    }
    let overridden = (slot == SHIELD_SLOT && left != HandSlot::Worn)
        || (slot == WEAPON_SLOT && right != HandSlot::Worn);
    if overridden {
        ItemVisibility::Hidden
    } else {
        ItemVisibility::Worn
    }
}

/// Combat sequences among the required set (stab, slash, punch, kick, bow): whether a given
/// worn weapon or shield can be present while one plays follows the backend's weapon-category →
/// animation binding (being assigned), not the sequence data; the fit gate keeps every such
/// combination (a superset) until that binding is published.
pub const COMBAT_SEQUENCES: [i32; 5] = [386, 390, 422, 423, 426];

/// Legality class of an item × sequence combination for the fit gate, from the source rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FitLegality {
    /// Drawn as equipped: the sequence leaves the item's slot alone.
    Worn,
    /// Drawn because the sequence itself puts the item into a hand slot (`lc.bd`).
    Override,
    /// A worn weapon/shield during a combat sequence: legal until the backend's
    /// weapon-category → animation binding says otherwise (kept as a superset).
    CombatBindingPending,
    /// The sequence's hand override replaces the item's slot: not drawn, no fit exists.
    Hidden,
    /// A sequence hand item (net, tinderbox, hammer) outside its own sequences: the player
    /// cannot wear it, so the combination never occurs.
    NotEquippable,
}

impl FitLegality {
    /// Whether the combination is drawn (and therefore fitted and gated).
    pub fn is_drawn(self) -> bool {
        matches!(
            self,
            FitLegality::Worn | FitLegality::Override | FitLegality::CombatBindingPending
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            FitLegality::Worn => "worn",
            FitLegality::Override => "override",
            FitLegality::CombatBindingPending => "combat_binding_pending",
            FitLegality::Hidden => "hidden",
            FitLegality::NotEquippable => "not_equippable",
        }
    }
}

/// [`FitLegality`] of `item_id` worn in `slot` while `sequence` plays; `equippable` is false
/// for sequence hand items the player cannot wear (manifest `role: sequence_hand_item`).
pub fn fit_legality(
    slot: &str,
    item_id: i32,
    sequence: &Sequence,
    kits: &std::collections::HashSet<i32>,
    equippable: bool,
) -> FitLegality {
    match item_visibility(slot, item_id, sequence, kits) {
        ItemVisibility::Hidden => FitLegality::Hidden,
        ItemVisibility::Override => FitLegality::Override,
        ItemVisibility::Worn if !equippable => FitLegality::NotEquippable,
        ItemVisibility::Worn => {
            if COMBAT_SEQUENCES.contains(&sequence.id)
                && (slot == WEAPON_SLOT || slot == SHIELD_SLOT)
            {
                FitLegality::CombatBindingPending
            } else {
                FitLegality::Worn
            }
        }
    }
}

/// Kit ids the source cache has, from the manifest's `sequence_hand_overrides.values`
/// (`kind: "kit"` with `kit_exists: true`).
pub fn cached_kits(manifest: &serde_json::Value) -> std::collections::HashSet<i32> {
    manifest["sequence_hand_overrides"]["values"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| v["kind"] == "kit" && v["kit_exists"] == true)
        .filter_map(|v| v["kit_id"].as_i64().map(|k| k as i32))
        .collect()
}

/// Required M1 player sequences (server appearance defaults, actions, combat, death, and the
/// penguin's native stand/walk): the pose set every worn item is fitted and gated against.
pub const REQUIRED_PLAYER_SEQUENCES: [i32; 27] = [
    808, 819, 824, 820, 821, 822, 823, 836, 829, 12526, 827, 625, 879, 621, 733, 897, 896, 899,
    898, 386, 390, 422, 423, 426, 711, 5668, 5666,
];

/// One precomputed per-pose fit: the rigid transform the attachment fit applies to the item in
/// that frame — a rotation (axis-angle) about the posed grip vertex followed by a translation —
/// and the measures it leaves (source units, unscaled body).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TableFrameFit {
    pub shift: [f64; 3],
    #[serde(default)]
    pub rotation: [f64; 3],
    pub penetration: f64,
    pub gap: f64,
    #[serde(default)]
    pub attachment_gap: f64,
    #[serde(default)]
    pub anchor_clearance: f64,
    #[serde(default)]
    pub embedded: f64,
}

/// Precomputed fits of one item: keyed by sequence id (decimal string), one entry per frame.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct TableItemFits {
    pub slot: String,
    /// Manifest SHA-256 of the item's equipped model the fits were computed against.
    pub model_sha256: String,
    /// Grip vertex (index within the item's part) the rotations pivot about.
    #[serde(default)]
    pub grip: usize,
    pub sequences: HashMap<String, Vec<TableFrameFit>>,
}

/// `gear/pose-fits.json`: the per-pose contact fits [`PlayerBody::pose_fitted`] would solve,
/// precomputed for the M1 items across [`REQUIRED_PLAYER_SEQUENCES`] so drawing a frame costs
/// no solve. Per item, never per gear combination (items are fitted against the body alone).
/// Entries are bound to the body and item geometry by manifest hashes; the runtime rejects a
/// table for another body and falls back to solving live for anything the table lacks.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PoseFitTable {
    pub schema_version: u32,
    pub body_npc: i32,
    pub body_model_sha256: String,
    pub human_reference_sha256: String,
    pub targets: TableTargets,
    pub items: HashMap<String, TableItemFits>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TableTargets {
    pub penetration: f64,
    pub gap: f64,
    #[serde(default)]
    pub attachment: f64,
}

impl PoseFitTable {
    pub fn frame(&self, item_id: i32, sequence_id: i32, frame: usize) -> Option<&TableFrameFit> {
        self.items
            .get(&item_id.to_string())?
            .sequences
            .get(&sequence_id.to_string())?
            .get(frame)
    }

    /// Item × sequence entries whose frame count differs from the sequence (stale table).
    pub fn frame_count_mismatches(
        &self,
        frame_count: impl Fn(i32) -> Option<usize>,
    ) -> Vec<String> {
        let mut out = Vec::new();
        for (item, fits) in &self.items {
            for (sequence, frames) in &fits.sequences {
                let Ok(id) = sequence.parse::<i32>() else {
                    out.push(format!(
                        "item {item}: sequence key {sequence:?} is not an id"
                    ));
                    continue;
                };
                match frame_count(id) {
                    Some(n) if n == frames.len() => {}
                    Some(n) => out.push(format!(
                        "item {item} sequence {id}: table has {} frames, sequence has {n}",
                        frames.len()
                    )),
                    None => {}
                }
            }
        }
        out
    }
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
        let (model, reports, _) = self.assemble_parts(gear);
        (model, reports)
    }

    /// [`PlayerBody::assemble`] that also returns where each attached item sits in the merged
    /// model, for the per-pose fit ([`PlayerBody::pose_fitted`]).
    pub fn assemble_parts(
        &self,
        gear: &[(String, &EquipModel)],
    ) -> (Model, Vec<FitReport>, Vec<AttachedPart>) {
        let mut merged = self.base.clone();
        let mut reports = Vec::new();
        let mut parts = Vec::new();
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
            let outward = outward_for(human_label, slot);
            let attachment = attachment_for(human_label, slot);
            // Grip / contact point: the item vertex nearest the human anchor label's centroid in
            // the design (the hilt in the hand, the strap on the arm, the hat's underside).
            let grip = (0..item.model.vertex_count)
                .min_by(|&a, &b| {
                    let d = |v: usize| {
                        (f64::from(item.model.xs[v]) - ha.x).powi(2)
                            + (f64::from(item.model.ys[v]) - ha.y).powi(2)
                            + (f64::from(item.model.zs[v]) - ha.z).powi(2)
                    };
                    d(a).total_cmp(&d(b))
                })
                .unwrap_or(0);
            let grip_design = [
                f64::from(item.model.xs[grip]),
                f64::from(item.model.ys[grip]),
                f64::from(item.model.zs[grip]),
            ];
            // Surface-anchored retarget: the design offset of the grip from the anchor part's
            // outward surface (its extreme along the slot's outward axis) is preserved, in
            // penguin scale, from the penguin anchor part's outward surface — so a thicker
            // flipper or chest carries the item out with it instead of swallowing it.
            let human_surface =
                label_surface_anchor(&self.human, human_label, ha, &outward, attachment);
            let penguin_surface =
                label_surface_anchor(&self.base, penguin_label, pa, &outward, attachment);
            let target = [
                penguin_surface[0] + (grip_design[0] - human_surface[0]) * scale,
                penguin_surface[1] + (grip_design[1] - human_surface[1]) * scale,
                penguin_surface[2] + (grip_design[2] - human_surface[2]) * scale,
            ];
            let mut part = item.model.clone();
            for v in 0..part.vertex_count {
                part.xs[v] = ((f64::from(part.xs[v]) - grip_design[0]) * scale + target[0]) as f32;
                part.ys[v] = ((f64::from(part.ys[v]) - grip_design[1]) * scale + target[1]) as f32;
                part.zs[v] = ((f64::from(part.zs[v]) - grip_design[2]) * scale + target[2]) as f32;
            }
            let design_penetration = fit_penetration(&self.human, &item.model);
            // Attachment fit at the bind pose: a rotation about the grip plus a translation of at
            // most FIT_MAX_ATTACHMENT — the grip stays where the anchor carries it (within that
            // bound) while the item turns away from the body. Geometry is never edited.
            let anchor_part = label_part(&self.base, penguin_label);
            let bind_reference = part.clone();
            let fit = attachment_fit(
                &self.base,
                &anchor_part,
                &outward,
                attachment,
                &bind_reference,
                &mut part,
                grip,
                &[0.0; 3],
                0.0,
            );
            let penetration = fit_penetration(&self.base, &part);
            let pca_box_penetration = pca_box_penetration(&self.base, &part);
            let gap = clearance(&self.base, &part);
            let anchor_clearance = clearance(&anchor_part, &part);
            let embedded = embedded_depth(&self.base, &part);
            relabel(&mut part, |human| {
                if human == item_label {
                    penguin_label
                } else {
                    self.penguin_label_for(human).unwrap_or(penguin_label)
                }
            });
            let vertex_start = merged.vertex_count;
            let face_start = merged.face_count;
            merge_into(&mut merged, &part);
            parts.push(AttachedPart {
                item_id: item.item_id,
                slot: slot.clone(),
                vertices: vertex_start..merged.vertex_count,
                faces: face_start..merged.face_count,
                grip,
                outward,
                anchor_label: penguin_label,
                attachment,
                bind_offset: fit.offset,
            });
            reports.push(FitReport {
                item_id: item.item_id,
                slot: slot.clone(),
                human_label,
                penguin_label,
                anchor_shift: fit.shift,
                shift_direction: fit.direction,
                penetration,
                pca_box_penetration,
                gap,
                design_penetration,
                scale,
                attachment_gap: fit.attachment_gap,
                rotation_deg: fit.rotation_deg(),
                anchor_clearance,
                embedded,
            });
        }
        (merged, reports, parts)
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

    /// [`PlayerBody::pose`] followed by the per-pose attachment fit: each attached item is
    /// measured against the posed body with the bind-pose box it was fitted with (carried
    /// rigidly with the item) and, where a body part swings into it deeper than
    /// [`FIT_MAX_PENETRATION`] or it drifts further than [`FIT_MAX_GAP`] from the body, the item
    /// alone is turned about its posed grip point and translated by at most
    /// [`FIT_MAX_ATTACHMENT`] — the grip keeps following the anchor bone, so the attachment gap
    /// is bounded by construction while penetration is minimised. Item geometry and the source
    /// frame transforms are never edited; items are fitted against the body only (no gear
    /// combination is special). Returns the model and one [`PoseFit`] per part with every
    /// measure (targets not met stay reported).
    pub fn pose_fitted(
        &self,
        assembled: &Model,
        parts: &[AttachedPart],
        sequence: &Sequence,
        frame: usize,
        table: Option<&PoseFitTable>,
    ) -> Result<(Model, Vec<PoseFit>), RenderError> {
        let mut model = self.pose(assembled, sequence, frame)?;
        let mut fits = Vec::with_capacity(parts.len());
        if parts.is_empty() {
            return Ok((model, fits));
        }
        let body = extract_range(&model, 0..self.base.vertex_count, 0..self.base.face_count);
        let anchor_parts: HashMap<i32, Model> = parts
            .iter()
            .map(|p| (p.anchor_label, label_part(&body, p.anchor_label)))
            .collect();
        for part in parts {
            if part.vertices.is_empty() {
                continue;
            }
            let bind_part = extract_range(assembled, part.vertices.clone(), part.faces.clone());
            let mut posed = extract_range(&model, part.vertices.clone(), part.faces.clone());
            let anchor_part = &anchor_parts[&part.anchor_label];
            if let Some(fit) = table.and_then(|t| t.frame(part.item_id, sequence.id, frame)) {
                let grip = grip_point(&posed, part.grip);
                apply_rigid(&mut posed, &grip, &fit.rotation, &fit.shift);
                for (i, v) in part.vertices.clone().enumerate() {
                    model.xs[v] = posed.xs[i];
                    model.ys[v] = posed.ys[i];
                    model.zs[v] = posed.zs[i];
                }
                let len = dot(&fit.shift, &fit.shift).sqrt();
                fits.push(PoseFit {
                    item_id: part.item_id,
                    slot: part.slot.clone(),
                    shift: len,
                    direction: if len > 1e-9 {
                        [fit.shift[0] / len, fit.shift[1] / len, fit.shift[2] / len]
                    } else {
                        [0.0; 3]
                    },
                    offset: fit.shift,
                    rotation: fit.rotation,
                    precomputed: true,
                    penetration: fit.penetration,
                    gap: fit.gap,
                    attachment_gap: fit.attachment_gap,
                    anchor_clearance: fit.anchor_clearance,
                    embedded: fit.embedded,
                });
                continue;
            }
            // The bind fit's translation carried into this pose by the anchor bone's rotation:
            // the grip already sits that far from the anchor-carried design grip.
            let carried = rigid_motion(&bind_part, &posed)
                .map(|(r, _)| {
                    [
                        dot(&r[0], &part.bind_offset),
                        dot(&r[1], &part.bind_offset),
                        dot(&r[2], &part.bind_offset),
                    ]
                })
                .unwrap_or(part.bind_offset);
            let fit = attachment_fit(
                &body,
                anchor_part,
                &part.outward,
                part.attachment,
                &bind_part,
                &mut posed,
                part.grip,
                &carried,
                dot(&part.bind_offset, &part.bind_offset).sqrt(),
            );
            let penetration = posed_fit_penetration(&bind_part, &body, &posed);
            let gap = clearance(&body, &posed);
            let anchor_clearance = clearance(anchor_part, &posed);
            let embedded = embedded_depth(&body, &posed);
            for (i, v) in part.vertices.clone().enumerate() {
                model.xs[v] = posed.xs[i];
                model.ys[v] = posed.ys[i];
                model.zs[v] = posed.zs[i];
            }
            fits.push(PoseFit {
                item_id: part.item_id,
                slot: part.slot.clone(),
                shift: fit.shift,
                direction: fit.direction,
                offset: fit.offset,
                rotation: fit.rotation,
                precomputed: false,
                penetration,
                gap,
                attachment_gap: fit.attachment_gap,
                anchor_clearance,
                embedded,
            });
        }
        Ok((model, fits))
    }

    /// The exact transform [`PlayerBody::pose_fitted`] applies to each part in a frame (for
    /// building a [`PoseFitTable`]): rotation about the posed grip, translation, and every
    /// measure per part.
    pub fn pose_fit_offsets(
        &self,
        assembled: &Model,
        parts: &[AttachedPart],
        sequence: &Sequence,
        frame: usize,
    ) -> Result<Vec<TableFrameFit>, RenderError> {
        let (_, fits) = self.pose_fitted(assembled, parts, sequence, frame, None)?;
        Ok(fits
            .into_iter()
            .map(|fit| TableFrameFit {
                shift: fit.offset,
                rotation: fit.rotation,
                penetration: fit.penetration,
                gap: fit.gap,
                attachment_gap: fit.attachment_gap,
                anchor_clearance: fit.anchor_clearance,
                embedded: fit.embedded,
            })
            .collect())
    }

    /// Animated, scaled player model for a sequence frame (what is drawn): the pose with the
    /// per-pose attachment fit applied. Returns the per-part fits alongside.
    pub fn frame_fitted(
        &self,
        assembled: &Model,
        parts: &[AttachedPart],
        sequence: &Sequence,
        frame: usize,
        table: Option<&PoseFitTable>,
    ) -> Result<(Model, Vec<PoseFit>), RenderError> {
        let (mut model, fits) = self.pose_fitted(assembled, parts, sequence, frame, table)?;
        scale_float(&mut model, self.width_scale, self.height_scale);
        model.compute_cylinder_bounds();
        Ok((model, fits))
    }

    /// Animated, scaled player model for a sequence frame without the per-pose fit (the raw
    /// retargeted pose; the bind-pose attachment only).
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

/// The vertices `vertices` and faces `faces` of `assembled` as a standalone model (face indices
/// rebased; vertex/face groups dropped).
pub fn extract_range(
    assembled: &Model,
    vertices: std::ops::Range<usize>,
    faces: std::ops::Range<usize>,
) -> Model {
    let mut part = assembled.clone();
    part.vertex_count = vertices.len();
    part.face_count = faces.len();
    part.xs = assembled.xs[vertices.clone()].to_vec();
    part.ys = assembled.ys[vertices.clone()].to_vec();
    part.zs = assembled.zs[vertices.clone()].to_vec();
    let offset = vertices.start as i32;
    part.face_a = assembled.face_a[faces.clone()]
        .iter()
        .map(|v| v - offset)
        .collect();
    part.face_b = assembled.face_b[faces.clone()]
        .iter()
        .map(|v| v - offset)
        .collect();
    part.face_c = assembled.face_c[faces.clone()]
        .iter()
        .map(|v| v - offset)
        .collect();
    part.vertex_groups = None;
    part.face_groups = None;
    part.face_groups_alt = None;
    part
}

/// The bind-pose oriented box of `bind_item` carried by the rigid motion onto `item`
/// (falls back to the item's own box when no rigid motion is recoverable).
fn carried_box(bind_item: &Model, item: &Model) -> ItemBox {
    let (axes, mean, min, max) = item_box(bind_item);
    let Some((r, t)) = rigid_motion(bind_item, item) else {
        return item_box(item);
    };
    let rot = |x: &[f64; 3]| [dot(&r[0], x), dot(&r[1], x), dot(&r[2], x)];
    let posed_axes = [rot(&axes[0]), rot(&axes[1]), rot(&axes[2])];
    let rm = rot(&mean);
    (
        posed_axes,
        [rm[0] + t[0], rm[1] + t[1], rm[2] + t[2]],
        min,
        max,
    )
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
    if body.vertex_count == 0 {
        return 0.0;
    }
    // Points outside the body's bounds cannot be inside it.
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for v in 0..body.vertex_count {
        let p = vertex(body, v);
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let mut deepest = 0.0f64;
    for p in 0..item.vertex_count {
        let point = vertex(item, p);
        if (0..3).any(|i| point[i] < lo[i] || point[i] > hi[i]) {
            continue;
        }
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
    let all: Vec<i32> = (0..body.vertex_count as i32).collect();
    depth_in_box(body, &all, &carried_box(bind_item, item))
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

/// Outward axis of an item from the human label its vertices are bound to (the side of the body
/// the design attaches it to), falling back to the equipment slot: right-hand labels 27/50 and
/// the right arm −X, left-hand 28 and the left arm +X, head labels up, neck/chest forward. A
/// sequence hand-item override can put a right-hand tool into the shield slot (`lc.bd`), so the
/// label decides before the slot does.
pub fn outward_for(human_label: i32, slot: &str) -> [f64; 3] {
    match human_label {
        27 | 50 | 17 | 19 | 20 | 21 => [-1.0, 0.0, 0.0],
        28 | 22 | 23 | 25 | 26 => [1.0, 0.0, 0.0],
        1..=3 => [0.0, -1.0, 0.0],
        4 | 5 | 8 | 29 | 30 => [0.0, 0.0, -1.0],
        _ => slot_outward(slot).unwrap_or([0.0, -1.0, 0.0]),
    }
}

/// How an item bound to `human_label` attaches: gripped by a hand label, worn otherwise.
pub fn attachment_for(human_label: i32, slot: &str) -> Attachment {
    match human_label {
        27 | 28 | 50 => Attachment::Held,
        _ if slot == "weapon" || slot == "shield" => Attachment::Held,
        _ => Attachment::Worn,
    }
}

/// The anchor part's outward surface point. A held item passes through its anchor (the hilt
/// through the palm): the surface is where the ray from the label centroid along `outward`
/// leaves the part's own triangles (the outer face of the hand or flipper at the centroid's
/// height). A worn item rests on its anchor's outer extreme (the hat on the top of the skull,
/// the necklace on the front of the chest): the label's extreme along `outward`. Falls back to
/// the extreme when the ray meets no triangle, and to the centroid for a label without vertices.
fn label_surface_anchor(
    model: &Model,
    label: i32,
    centroid: &LabelCentroid,
    outward: &[f64; 3],
    attachment: Attachment,
) -> [f64; 3] {
    let origin = [centroid.x, centroid.y, centroid.z];
    let part = if attachment == Attachment::Held {
        label_part(model, label)
    } else {
        let mut empty = model.clone();
        empty.face_count = 0;
        empty
    };
    let mut hit: Option<f64> = None;
    for f in 0..part.face_count {
        let tri = [part.face_a[f], part.face_b[f], part.face_c[f]].map(|i| {
            let i = i as usize;
            [
                f64::from(part.xs[i]),
                f64::from(part.ys[i]),
                f64::from(part.zs[i]),
            ]
        });
        if let Some(t) = ray_triangle(&origin, outward, &tri[0], &tri[1], &tri[2])
            && t >= 0.0
            && hit.is_none_or(|h| t > h)
        {
            // The farthest exit along the ray: the part's outer surface.
            hit = Some(t);
        }
    }
    let reach = hit.unwrap_or_else(|| {
        let mut reach = 0.0f64;
        if let Some(group) = model
            .vertex_groups
            .as_ref()
            .and_then(|g| g.get(label as usize))
        {
            for &v in group {
                let v = v as usize;
                let d = [
                    f64::from(model.xs[v]) - centroid.x,
                    f64::from(model.ys[v]) - centroid.y,
                    f64::from(model.zs[v]) - centroid.z,
                ];
                reach = reach.max(dot(&d, outward));
            }
        }
        reach
    });
    [
        centroid.x + outward[0] * reach,
        centroid.y + outward[1] * reach,
        centroid.z + outward[2] * reach,
    ]
}

/// Möller–Trumbore ray/triangle intersection: the ray parameter of the hit, if any (either
/// facing).
fn ray_triangle(
    origin: &[f64; 3],
    dir: &[f64; 3],
    a: &[f64; 3],
    b: &[f64; 3],
    c: &[f64; 3],
) -> Option<f64> {
    let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let cross = |u: &[f64; 3], v: &[f64; 3]| {
        [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ]
    };
    let h = cross(dir, &e2);
    let det = dot(&e1, &h);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = [origin[0] - a[0], origin[1] - a[1], origin[2] - a[2]];
    let u = inv * dot(&s, &h);
    if !(-1e-9..=1.0 + 1e-9).contains(&u) {
        return None;
    }
    let q = cross(&s, &e1);
    let v = inv * dot(dir, &q);
    if v < -1e-9 || u + v > 1.0 + 1e-9 {
        return None;
    }
    Some(inv * dot(&e2, &q))
}

/// The triangles of `model` whose three vertices all carry `label` (the anchor body part as a
/// standalone mesh, for the anchor-clearance measure); empty when the label has none.
pub fn label_part(model: &Model, label: i32) -> Model {
    let mut part = model.clone();
    let in_label: Vec<bool> = {
        let mut flags = vec![false; model.vertex_count];
        if let Some(group) = model
            .vertex_groups
            .as_ref()
            .and_then(|g| g.get(label as usize))
        {
            for &v in group {
                if (v as usize) < flags.len() {
                    flags[v as usize] = true;
                }
            }
        }
        flags
    };
    let mut faces = Vec::new();
    for f in 0..model.face_count {
        let (a, b, c) = (
            model.face_a[f] as usize,
            model.face_b[f] as usize,
            model.face_c[f] as usize,
        );
        if in_label[a] || in_label[b] || in_label[c] {
            faces.push(f);
        }
    }
    part.face_count = faces.len();
    part.face_a = faces.iter().map(|&f| model.face_a[f]).collect();
    part.face_b = faces.iter().map(|&f| model.face_b[f]).collect();
    part.face_c = faces.iter().map(|&f| model.face_c[f]).collect();
    part.vertex_groups = None;
    part.face_groups = None;
    part.face_groups_alt = None;
    part
}

/// Position of the part's grip vertex.
pub fn grip_point(part: &Model, grip: usize) -> [f64; 3] {
    let g = grip.min(part.vertex_count.saturating_sub(1));
    [
        f64::from(part.xs[g]),
        f64::from(part.ys[g]),
        f64::from(part.zs[g]),
    ]
}

/// Rotation matrix (rows) of an axis-angle vector (Rodrigues).
fn rotation_matrix(rotvec: &[f64; 3]) -> [[f64; 3]; 3] {
    let angle = dot(rotvec, rotvec).sqrt();
    if angle < 1e-12 {
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    }
    let k = [rotvec[0] / angle, rotvec[1] / angle, rotvec[2] / angle];
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    [
        [
            c + k[0] * k[0] * t,
            k[0] * k[1] * t - k[2] * s,
            k[0] * k[2] * t + k[1] * s,
        ],
        [
            k[1] * k[0] * t + k[2] * s,
            c + k[1] * k[1] * t,
            k[1] * k[2] * t - k[0] * s,
        ],
        [
            k[2] * k[0] * t - k[1] * s,
            k[2] * k[1] * t + k[0] * s,
            c + k[2] * k[2] * t,
        ],
    ]
}

/// Applies `v' = R (v − pivot) + pivot + shift` to every vertex of `part`.
pub fn apply_rigid(part: &mut Model, pivot: &[f64; 3], rotvec: &[f64; 3], shift: &[f64; 3]) {
    let r = rotation_matrix(rotvec);
    for v in 0..part.vertex_count {
        let p = [
            f64::from(part.xs[v]) - pivot[0],
            f64::from(part.ys[v]) - pivot[1],
            f64::from(part.zs[v]) - pivot[2],
        ];
        let q = [dot(&r[0], &p), dot(&r[1], &p), dot(&r[2], &p)];
        part.xs[v] = (q[0] + pivot[0] + shift[0]) as f32;
        part.ys[v] = (q[1] + pivot[1] + shift[1]) as f32;
        part.zs[v] = (q[2] + pivot[2] + shift[2]) as f32;
    }
}

/// Result of [`attachment_fit`]: the rigid transform applied about the grip and the attachment
/// gap it left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttachmentFit {
    pub rotation: [f64; 3],
    pub offset: [f64; 3],
    pub shift: f64,
    pub direction: [f64; 3],
    pub attachment_gap: f64,
}

impl AttachmentFit {
    pub fn rotation_deg(&self) -> f64 {
        dot(&self.rotation, &self.rotation).sqrt().to_degrees()
    }
}

/// Attachment-preserving fit of one item against the body: the item may turn about its grip
/// point (up to 90° about any of 13 lattice axes, refined) and translate by at most
/// [`FIT_MAX_ATTACHMENT`]; the grip therefore stays within the attachment bound of where the
/// anchor bone carries it. Among the candidates the one with the smallest combined penetration
/// (carried bind box + item-inside-body) wins, preferring ones that also keep the item touching
/// the body and its anchor part, then the smallest rotation; unmet targets remain measurable.
/// The item is left in its fitted position; geometry is never edited.
#[allow(clippy::too_many_arguments)]
fn attachment_fit(
    body: &Model,
    anchor_part: &Model,
    outward: &[f64; 3],
    attachment: Attachment,
    bind_reference: &Model,
    item: &mut Model,
    grip: usize,
    base_offset: &[f64; 3],
    base_len: f64,
) -> AttachmentFit {
    // `base_offset` is where the item's grip already sits relative to the anchor-carried design
    // grip (the bind fit's translation carried into this pose by the bone's rotation; its exact
    // length is `base_len`). The attachment gap of a candidate translation `s` is
    // |base_offset + s| and the whole budget is FIT_MAX_ATTACHMENT.
    let identity = AttachmentFit {
        rotation: [0.0; 3],
        offset: [0.0; 3],
        shift: 0.0,
        direction: [0.0; 3],
        attachment_gap: base_len,
    };
    let measure = |candidate: &Model| -> (f64, f64, f64) {
        let pen = posed_fit_penetration(bind_reference, body, candidate);
        let gap = clearance(body, candidate);
        let anchor_gap = if anchor_part.face_count > 0 {
            clearance(anchor_part, candidate)
        } else {
            gap
        };
        (pen, gap, anchor_gap)
    };
    let cheap =
        |candidate: &Model| -> f64 { posed_box_penetration(bind_reference, body, candidate) };
    let acceptable = |pen: f64, gap: f64, anchor_gap: f64| {
        pen <= FIT_MAX_PENETRATION
            && gap <= FIT_MAX_GAP
            && (attachment == Attachment::Held || anchor_gap <= FIT_MAX_GAP)
    };
    let (pen0, gap0, anchor0) = measure(item);
    if acceptable(pen0, gap0, anchor0) {
        return identity;
    }
    let pivot = grip_point(item, grip);
    let normalize = |v: [f64; 3]| -> Option<[f64; 3]> {
        let len = dot(&v, &v).sqrt();
        (len > 1e-9).then(|| [v[0] / len, v[1] / len, v[2] / len])
    };
    let mut axes: Vec<[f64; 3]> = Vec::new();
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                if let Some(a) = normalize([f64::from(x), f64::from(y), f64::from(z)])
                    && !axes.iter().any(|e| (dot(e, &a).abs() - 1.0).abs() < 1e-9)
                {
                    axes.push(a);
                }
            }
        }
    }
    // Candidate grip positions: the ball of radius FIT_MAX_ATTACHMENT around the anchor-carried
    // design grip (its centre, ±2 on each axis, +2 outward) plus staying put; expressed as
    // translations from the item's current position so every candidate keeps |gap| ≤ bound.
    let mut targets: Vec<[f64; 3]> = vec![[0.0; 3]];
    for axis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
        for sign in [1.0, -1.0] {
            targets.push([
                axis[0] * sign * FIT_MAX_ATTACHMENT,
                axis[1] * sign * FIT_MAX_ATTACHMENT,
                axis[2] * sign * FIT_MAX_ATTACHMENT,
            ]);
        }
    }
    targets.push([
        outward[0] * FIT_MAX_ATTACHMENT,
        outward[1] * FIT_MAX_ATTACHMENT,
        outward[2] * FIT_MAX_ATTACHMENT,
    ]);
    // (translation, exact attachment gap it leaves): the targets' components are exact so
    // their lengths are too (no boundary rounding against the bound).
    let mut shifts: Vec<([f64; 3], f64)> = vec![([0.0; 3], base_len)];
    for t in &targets {
        let s = [
            t[0] - base_offset[0],
            t[1] - base_offset[1],
            t[2] - base_offset[2],
        ];
        if dot(&s, &s) > 1e-18 {
            shifts.push((s, dot(t, t).sqrt()));
        }
    }
    let candidate_model = |rotvec: &[f64; 3], shift: &[f64; 3]| -> Model {
        let mut c = item.clone();
        apply_rigid(&mut c, &pivot, rotvec, shift);
        c
    };
    // Coarse: cheap box measure over rotations × bounded shifts.
    let angles_deg: [f64; 10] = [0.0, 5.0, 10.0, 15.0, 20.0, 30.0, 45.0, 60.0, 75.0, 90.0];
    /// (translation, exact attachment gap it leaves)
    type Shift = ([f64; 3], f64);
    /// (box penetration, |angle|°, rotation vector, shift)
    type Ranked = (f64, f64, [f64; 3], Shift);
    /// (penetration, gap, anchor clearance, |angle|°, rotation vector, shift)
    type Best = (f64, f64, f64, f64, [f64; 3], Shift);
    let mut ranked: Vec<Ranked> = Vec::new();
    for axis in &axes {
        for deg in angles_deg {
            for sign in [1.0f64, -1.0] {
                if deg == 0.0 && sign < 0.0 {
                    continue;
                }
                let angle: f64 = (deg * sign).to_radians();
                let rotvec = [axis[0] * angle, axis[1] * angle, axis[2] * angle];
                for shift in &shifts {
                    let c = candidate_model(&rotvec, &shift.0);
                    ranked.push((cheap(&c), deg, rotvec, *shift));
                }
                if deg == 0.0 {
                    break;
                }
            }
        }
    }
    ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    // Exact confirmation of the best few (box + embedded, clearance, anchor clearance): the first
    // acceptable candidate wins; otherwise the candidate with the smallest exact penetration.
    let mut best: Option<Best> = None;
    let mut budget = 24usize;
    for (_, deg, rotvec, shift) in ranked.iter() {
        if budget == 0 {
            break;
        }
        budget -= 1;
        let c = candidate_model(rotvec, &shift.0);
        let (pen, gap, anchor_gap) = measure(&c);
        let better = match &best {
            None => true,
            Some((bp, bg, ba, bd, _, _)) => {
                let ok_new = acceptable(pen, gap, anchor_gap);
                let ok_old = acceptable(*bp, *bg, *ba);
                (ok_new && !ok_old)
                    || (ok_new == ok_old
                        && (pen < bp - 1e-9 || ((pen - bp).abs() <= 1e-9 && deg < bd)))
            }
        };
        if better {
            best = Some((pen, gap, anchor_gap, *deg, *rotvec, *shift));
            if acceptable(pen, gap, anchor_gap) && *deg <= 15.0 {
                break;
            }
        }
    }
    let Some((pen, gap, anchor_gap, deg, mut rotvec, (shift, attachment_gap))) = best else {
        return identity;
    };
    // Refine the angle by bisection towards the smallest rotation that still meets the targets.
    if acceptable(pen, gap, anchor_gap) && deg > 0.0 {
        let axis = normalize(rotvec).unwrap_or([0.0, 1.0, 0.0]);
        let (mut lo, mut hi) = (0.0f64, deg);
        for _ in 0..5 {
            let mid = 0.5 * (lo + hi);
            let rv = [
                axis[0] * mid.to_radians(),
                axis[1] * mid.to_radians(),
                axis[2] * mid.to_radians(),
            ];
            let c = candidate_model(&rv, &shift);
            let (p, g, a) = measure(&c);
            if acceptable(p, g, a) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        rotvec = [
            axis[0] * hi.to_radians(),
            axis[1] * hi.to_radians(),
            axis[2] * hi.to_radians(),
        ];
    }
    apply_rigid(item, &pivot, &rotvec, &shift);
    let len = dot(&shift, &shift).sqrt();
    AttachmentFit {
        rotation: rotvec,
        offset: shift,
        shift: len,
        direction: normalize(shift).unwrap_or([0.0; 3]),
        attachment_gap,
    }
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

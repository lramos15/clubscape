//! Equipment fit on the approved penguin player body: every M1 worn model must attach with
//! penetration ≤ 1 and gap ≤ 2 source units (bind pose), and the same measures are swept over
//! every required player sequence frame for the penguin retarget and, for comparison, the human
//! body the items were designed for. The sweep is evidence (written to
//! `.local/render-assets/test-output/gear-fit.json`), not a relaxed target: frames above the bind
//! targets are listed as unmet.

mod common;

use clubscape_renderer::actor::{
    EquipModel, FIT_MAX_GAP, FIT_MAX_PENETRATION, PlayerBody, clearance, merge_into,
    posed_fit_penetration, split_part,
};
use clubscape_renderer::anim::{Sequence, apply_frame};
use clubscape_renderer::model::Model;
use common::read_asset;

/// Required player sequences (server appearance defaults, actions, combat, death).
const REQUIRED_SEQUENCES: [i32; 27] = [
    808, 819, 824, 820, 821, 822, 823, 836, 829, 12526, 827, 625, 879, 621, 733, 897, 896, 899,
    898, 386, 390, 422, 423, 426, 711, 5668, 5666,
];

fn slot_for(item_id: i32) -> &'static str {
    match item_id {
        1949 => "head",
        1171 | 1173 => "shield",
        1009 => "amulet",
        _ => "weapon",
    }
}

struct Inputs {
    body: PlayerBody,
    base: Model,
    human: Model,
    items: Vec<(i32, String, Model)>,
    sequences: Vec<Sequence>,
}

fn inputs() -> Inputs {
    let manifest: serde_json::Value = serde_json::from_slice(&read_asset("manifest.json")).unwrap();
    let penguin = manifest["npc_definitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["npc_id"] == 2063)
        .expect("penguin definition");
    let base = Model::from_chunks(&read_asset(penguin["base_model"].as_str().unwrap())).unwrap();
    let human = Model::from_chunks(&read_asset(
        manifest["player_reference"]["model"].as_str().unwrap(),
    ))
    .unwrap();
    let body = PlayerBody::new(
        base.clone(),
        penguin["width_scale"].as_i64().unwrap() as i32,
        penguin["height_scale"].as_i64().unwrap() as i32,
        vec![
            penguin["sequences"]["stand"].as_i64().unwrap() as i32,
            penguin["sequences"]["walk"].as_i64().unwrap() as i32,
        ],
        &human,
    );
    let mut items = Vec::new();
    for item in manifest["equipment_items"].as_array().unwrap() {
        if let Some(path) = item["equip_model"].as_str() {
            items.push((
                item["item_id"].as_i64().unwrap() as i32,
                item["name"].as_str().unwrap().to_string(),
                Model::from_chunks(&read_asset(path)).unwrap(),
            ));
        }
    }
    assert_eq!(items.len(), 10, "the 10 M1 items with a worn model");
    let sequences = REQUIRED_SEQUENCES
        .iter()
        .map(|id| Sequence::from_chunks(&read_asset(&format!("anim/seq-{id}.bin"))).unwrap())
        .collect();
    Inputs {
        body,
        base,
        human,
        items,
        sequences,
    }
}

#[test]
fn every_m1_worn_model_fits_the_penguin_at_the_bind_pose() {
    let inputs = inputs();
    let mut worst_penetration = 0.0f64;
    let mut worst_gap = 0.0f64;
    for (id, name, model) in &inputs.items {
        let equip = EquipModel {
            item_id: *id,
            model: model.clone(),
        };
        let (_, reports) = inputs.body.assemble(&[(slot_for(*id).to_string(), &equip)]);
        assert_eq!(reports.len(), 1, "{name} did not attach");
        let fit = &reports[0];
        eprintln!(
            "{id} {name}: penetration {:.3} (pca box {:.3}) gap {:.3} shift {:.3} along {:?} design {:.3}",
            fit.penetration,
            fit.pca_box_penetration,
            fit.gap,
            fit.anchor_shift,
            fit.shift_direction,
            fit.design_penetration
        );
        assert!(
            fit.penetration <= FIT_MAX_PENETRATION,
            "{name}: penetration {:.3} > {FIT_MAX_PENETRATION}",
            fit.penetration
        );
        assert!(
            fit.gap <= FIT_MAX_GAP,
            "{name}: gap {:.3} > {FIT_MAX_GAP}",
            fit.gap
        );
        worst_penetration = worst_penetration.max(fit.penetration);
        worst_gap = worst_gap.max(fit.gap);
    }
    eprintln!("bind fit: worst penetration {worst_penetration:.3}, worst gap {worst_gap:.3}");
    // All slots worn together attach independently (no item displaces another).
    let all: Vec<(String, EquipModel)> = inputs
        .items
        .iter()
        .filter(|(id, _, _)| matches!(id, 1277 | 1171 | 1949 | 1009))
        .map(|(id, _, m)| {
            (
                slot_for(*id).to_string(),
                EquipModel {
                    item_id: *id,
                    model: m.clone(),
                },
            )
        })
        .collect();
    let refs: Vec<(String, &EquipModel)> = all.iter().map(|(s, e)| (s.clone(), e)).collect();
    let (_, reports) = inputs.body.assemble(&refs);
    assert_eq!(reports.len(), 4);
    for fit in &reports {
        assert!(
            fit.penetration <= FIT_MAX_PENETRATION && fit.gap <= FIT_MAX_GAP,
            "{fit:?}"
        );
    }
}

/// Sweeps every frame of every required sequence with each item worn alone and measures the
/// item↔body penetration (combined box/mesh measure) and clearance on the posed models, for the
/// penguin retarget and for the human design. Writes the table and asserts only what the bind
/// fit guarantees: frames above the targets are reported per (item, sequence) as unmet.
#[test]
fn pose_sweep_records_fit_across_required_sequences() {
    let inputs = inputs();
    let mut table = serde_json::Map::new();
    let mut unmet_penguin = Vec::new();
    let mut unmet_human = Vec::new();
    for (id, name, model) in &inputs.items {
        let equip = EquipModel {
            item_id: *id,
            model: model.clone(),
        };
        let (assembled, _) = inputs.body.assemble(&[(slot_for(*id).to_string(), &equip)]);
        let (_, bind_part) =
            split_part(&assembled, inputs.base.vertex_count, inputs.base.face_count);
        let mut human_assembled = inputs.human.clone();
        merge_into(&mut human_assembled, model);
        let mut per_sequence = serde_json::Map::new();
        for sequence in &inputs.sequences {
            // (max penetration, its frame, max gap, frames with penetration over, frames with gap over)
            let mut penguin = (0.0f64, 0usize, 0.0f64, 0usize, 0usize);
            let mut human = (0.0f64, 0usize, 0.0f64, 0usize, 0usize);
            for frame in 0..sequence.frame_count() {
                let posed = inputs.body.pose(&assembled, sequence, frame).unwrap();
                let (body, part) =
                    split_part(&posed, inputs.base.vertex_count, inputs.base.face_count);
                let pen = posed_fit_penetration(&bind_part, &body, &part);
                let gap = clearance(&body, &part);
                if pen > penguin.0 {
                    penguin.0 = pen;
                    penguin.1 = frame;
                }
                if pen > FIT_MAX_PENETRATION {
                    penguin.3 += 1;
                }
                if gap > FIT_MAX_GAP {
                    penguin.4 += 1;
                }
                penguin.2 = penguin.2.max(gap);
                // The human body only plays its own (non-penguin) sequences.
                if !inputs.body.native_sequences.contains(&sequence.id) {
                    let mut hposed = human_assembled.clone();
                    apply_frame(&mut hposed, sequence, frame, None).unwrap();
                    let (hbody, hpart) =
                        split_part(&hposed, inputs.human.vertex_count, inputs.human.face_count);
                    let hpen = posed_fit_penetration(model, &hbody, &hpart);
                    let hgap = clearance(&hbody, &hpart);
                    if hpen > human.0 {
                        human.0 = hpen;
                        human.1 = frame;
                    }
                    if hpen > FIT_MAX_PENETRATION {
                        human.3 += 1;
                    }
                    if hgap > FIT_MAX_GAP {
                        human.4 += 1;
                    }
                    human.2 = human.2.max(hgap);
                }
            }
            if penguin.3 > 0 || penguin.4 > 0 {
                unmet_penguin.push(format!(
                    "{name} seq {}: {}/{} frames penetrate > {FIT_MAX_PENETRATION} (max {:.2} at frame {}), {} frames gap > {FIT_MAX_GAP} (max gap {:.2})",
                    sequence.id, penguin.3, sequence.frame_count(), penguin.0, penguin.1, penguin.4, penguin.2
                ));
            }
            if human.3 > 0 || human.4 > 0 {
                unmet_human.push(format!(
                    "{name} seq {}: {}/{} frames penetrate > {FIT_MAX_PENETRATION} on the human design (max {:.2} at frame {}), {} frames gap > {FIT_MAX_GAP} (max gap {:.2})",
                    sequence.id, human.3, sequence.frame_count(), human.0, human.1, human.4, human.2
                ));
            }
            per_sequence.insert(
                sequence.id.to_string(),
                serde_json::json!({
                    "frames": sequence.frame_count(),
                    "penguin": {"maxPenetration": penguin.0, "atFrame": penguin.1, "maxGap": penguin.2, "framesPenetrationOver": penguin.3, "framesGapOver": penguin.4},
                    "humanDesign": if inputs.body.native_sequences.contains(&sequence.id) { serde_json::Value::Null } else { serde_json::json!({"maxPenetration": human.0, "atFrame": human.1, "maxGap": human.2, "framesPenetrationOver": human.3, "framesGapOver": human.4}) },
                }),
            );
        }
        table.insert(
            format!("{id} {name}"),
            serde_json::Value::Object(per_sequence),
        );
    }
    let dir = common::repo_root().join(".local/render-assets/test-output");
    std::fs::create_dir_all(&dir).unwrap();
    let report = serde_json::json!({
        "targets": {"penetration": FIT_MAX_PENETRATION, "gap": FIT_MAX_GAP, "units": "source units, unscaled body"},
        "measure": "penetration = max(deepest body vertex inside the item's minimum-volume oriented box fixed at the bind pose and carried with the item, deepest item vertex inside the body mesh); gap = item↔body surface clearance",
        "sequences": REQUIRED_SEQUENCES,
        "items": table,
        "unmetPenguinRetarget": unmet_penguin,
        "unmetHumanDesign": unmet_human,
    });
    std::fs::write(
        dir.join("gear-fit.json"),
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();
    eprintln!(
        "penguin retarget frames over target ({}):",
        unmet_penguin.len()
    );
    for line in &unmet_penguin {
        eprintln!("  {line}");
    }
    eprintln!("human design frames over target ({}):", unmet_human.len());
    for line in &unmet_human {
        eprintln!("  {line}");
    }
    // Items stay attached through every required pose: the clearance never exceeds the gap
    // target in any frame (no floating or detached gear). Per-frame penetration is evidence
    // above: the human design itself exceeds 1 unit in most frames (source items overlap the
    // body by construction), so it is listed, compared, and not claimed as met.
    let mut frames_measured = 0u64;
    for (key, per_sequence) in &table {
        for (sequence, entry) in per_sequence.as_object().unwrap() {
            frames_measured += entry["frames"].as_u64().unwrap();
            assert_eq!(
                entry["penguin"]["framesGapOver"].as_u64().unwrap(),
                0,
                "{key} seq {sequence}: item detached from the body (gap > {FIT_MAX_GAP}) in {} frames",
                entry["penguin"]["framesGapOver"]
            );
        }
    }
    let expected: usize = inputs
        .sequences
        .iter()
        .map(|s| s.frame_count())
        .sum::<usize>()
        * inputs.items.len();
    assert_eq!(
        frames_measured as usize, expected,
        "every item × sequence × frame measured"
    );
    eprintln!(
        "pose sweep: {frames_measured} item-frames measured across {} sequences",
        inputs.sequences.len()
    );
}

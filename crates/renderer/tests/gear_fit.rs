//! Equipment fit on the approved penguin player body: every M1 worn model must attach with
//! penetration ≤ 1 and gap ≤ 2 source units (bind pose), and the same measures are swept over
//! every required player sequence frame for the penguin retarget and, for comparison, the human
//! body the items were designed for. The sweep is evidence (written to
//! `.local/render-assets/test-output/gear-fit.json`), not a relaxed target: frames above the bind
//! targets are listed as unmet.

mod common;

use clubscape_renderer::actor::{
    EquipModel, FIT_MAX_GAP, FIT_MAX_PENETRATION, PlayerBody, PoseFitTable, clearance, merge_into,
    posed_fit_penetration, split_part,
};
use clubscape_renderer::anim::{Sequence, apply_frame};
use clubscape_renderer::model::Model;
use common::read_asset;

/// Required player sequences (server appearance defaults, actions, combat, death).
const REQUIRED_SEQUENCES: [i32; 27] = clubscape_renderer::actor::REQUIRED_PLAYER_SEQUENCES;

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
        let (assembled, _, parts) = inputs
            .body
            .assemble_parts(&[(slot_for(*id).to_string(), &equip)]);
        let (_, bind_part) =
            split_part(&assembled, inputs.base.vertex_count, inputs.base.face_count);
        let mut human_assembled = inputs.human.clone();
        merge_into(&mut human_assembled, model);
        let mut per_sequence = serde_json::Map::new();
        for sequence in &inputs.sequences {
            // (max penetration, its frame, max gap, frames with penetration over, frames with gap over)
            let mut penguin = (0.0f64, 0usize, 0.0f64, 0usize, 0usize);
            let mut human = (0.0f64, 0usize, 0.0f64, 0usize, 0usize);
            let mut max_shift = 0.0f64;
            let mut solve_ms = 0.0f64;
            for frame in 0..sequence.frame_count() {
                // What is drawn: the retargeted pose with the per-pose contact fit.
                let started = std::time::Instant::now();
                let (posed, fits) = inputs
                    .body
                    .pose_fitted(&assembled, &parts, sequence, frame, None)
                    .unwrap();
                solve_ms += started.elapsed().as_secs_f64() * 1000.0;
                max_shift = max_shift.max(fits[0].shift);
                let (body, part) =
                    split_part(&posed, inputs.base.vertex_count, inputs.base.face_count);
                // Measured independently of the solve's own report.
                let pen = posed_fit_penetration(&bind_part, &body, &part);
                let gap = clearance(&body, &part);
                assert!(
                    (pen - fits[0].penetration).abs() < 5e-3 && (gap - fits[0].gap).abs() < 5e-3,
                    "{name} seq {} frame {frame}: PoseFit reports {:.3}/{:.3}, measured {pen:.3}/{gap:.3}",
                    sequence.id,
                    fits[0].penetration,
                    fits[0].gap
                );
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
                    "penguin": {"maxPenetration": penguin.0, "atFrame": penguin.1, "maxGap": penguin.2, "framesPenetrationOver": penguin.3, "framesGapOver": penguin.4,
                                "maxPoseShift": max_shift, "solveMsPerFrame": solve_ms / sequence.frame_count() as f64},
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
    // Pose gate: every required item × sequence × frame must meet both targets after the
    // per-pose contact fit. The human-design figures are diagnostic context only (the source
    // items overlap their own body by construction), never a waiver.
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
    assert!(
        unmet_penguin.is_empty(),
        "{} item×sequence entries exceed penetration {FIT_MAX_PENETRATION} or gap {FIT_MAX_GAP} on the penguin after the pose fit:\n  {}",
        unmet_penguin.len(),
        unmet_penguin.join("\n  ")
    );
}

/// The published `gear/pose-fits.json` is exactly what the live solve produces (vertex-identical
/// fitted frames), is bound to the approved body/items by manifest hashes, covers every required
/// item × sequence × frame, and its over-target count equals the sweep's remaining failures.
#[test]
fn published_pose_fit_table_matches_the_live_solve() {
    let inputs = inputs();
    let manifest: serde_json::Value = serde_json::from_slice(&read_asset("manifest.json")).unwrap();
    let entry = manifest
        .get("gear_pose_fits")
        .expect("manifest lists gear_pose_fits (export.py --profile pose-fits)");
    let table: PoseFitTable =
        serde_json::from_slice(&read_asset(entry["file"].as_str().unwrap())).unwrap();
    assert_eq!(table.schema_version, 1);
    assert_eq!(table.body_npc, 2063);
    let penguin = manifest["npc_definitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["npc_id"] == 2063)
        .unwrap();
    assert_eq!(
        table.body_model_sha256,
        manifest["files"][penguin["base_model"].as_str().unwrap()]["sha256"]
            .as_str()
            .unwrap(),
        "table is bound to the published penguin base model"
    );
    assert_eq!(table.targets.penetration, FIT_MAX_PENETRATION);
    assert_eq!(table.targets.gap, FIT_MAX_GAP);
    assert!(
        table
            .frame_count_mismatches(|id| inputs
                .sequences
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.frame_count()))
            .is_empty()
    );
    let mut over_target = 0usize;
    let mut compared = 0usize;
    for (id, name, model) in &inputs.items {
        let fits = table
            .items
            .get(&id.to_string())
            .unwrap_or_else(|| panic!("{name}: missing from the table"));
        assert_eq!(fits.slot, slot_for(*id));
        let equip = EquipModel {
            item_id: *id,
            model: model.clone(),
        };
        let (assembled, _, parts) = inputs
            .body
            .assemble_parts(&[(slot_for(*id).to_string(), &equip)]);
        for sequence in &inputs.sequences {
            let frames = fits
                .sequences
                .get(&sequence.id.to_string())
                .unwrap_or_else(|| panic!("{name}: sequence {} missing", sequence.id));
            assert_eq!(frames.len(), sequence.frame_count());
            over_target += frames
                .iter()
                .filter(|f| f.penetration > FIT_MAX_PENETRATION || f.gap > FIT_MAX_GAP)
                .count();
            // Every frame from the table must equal the live solve bit for bit; the frames
            // solved live here are a spread (first, middle, last) of each sequence.
            for frame in [0, sequence.frame_count() / 2, sequence.frame_count() - 1] {
                let (live, live_fits) = inputs
                    .body
                    .pose_fitted(&assembled, &parts, sequence, frame, None)
                    .unwrap();
                let (tabled, table_fits) = inputs
                    .body
                    .pose_fitted(&assembled, &parts, sequence, frame, Some(&table))
                    .unwrap();
                assert!(table_fits[0].precomputed && !live_fits[0].precomputed);
                // The generator binary and this test binary may differ in the last bit of the
                // solve (codegen), so the recorded shift is compared to 1e-9 and the fitted
                // vertices to a thousandth of a source unit (the f32 rounding of that bit).
                for (axis, (a, b)) in [
                    (&live.xs, &tabled.xs),
                    (&live.ys, &tabled.ys),
                    (&live.zs, &tabled.zs),
                ]
                .into_iter()
                .enumerate()
                {
                    assert_eq!(a.len(), b.len());
                    for (v, (p, q)) in a.iter().zip(b).enumerate() {
                        assert!(
                            (p - q).abs() <= 1e-3,
                            "{name} seq {} frame {frame}: vertex {v} axis {axis} {p} vs {q}",
                            sequence.id
                        );
                    }
                }
                for axis in 0..3 {
                    assert!(
                        (live_fits[0].offset[axis] - table_fits[0].offset[axis]).abs() < 1e-9,
                        "{name} seq {} frame {frame}: offset {:?} vs table {:?}",
                        sequence.id,
                        live_fits[0].offset,
                        table_fits[0].offset
                    );
                }
                assert!(
                    (live_fits[0].penetration - table_fits[0].penetration).abs() < 1e-9
                        && (live_fits[0].gap - table_fits[0].gap).abs() < 1e-9,
                    "{name} seq {} frame {frame}: measures {}/{} vs table {}/{}",
                    sequence.id,
                    live_fits[0].penetration,
                    live_fits[0].gap,
                    table_fits[0].penetration,
                    table_fits[0].gap
                );
                compared += 1;
            }
        }
    }
    assert_eq!(
        entry["item_frames_over_target"].as_u64().unwrap() as usize,
        over_target,
        "manifest over-target count equals the table's"
    );
    eprintln!(
        "pose-fit table: {compared} frames compared to the live solve; {over_target} of {} item-frames remain over target",
        entry["item_frames"]
    );
}

/// Developer experiment: for the deepest shield poses, the shift each fixed body-space direction
/// needs to meet the penetration target and the clearance it leaves.
#[test]
#[ignore = "developer experiment; prints direction candidates for the deepest shield poses"]
fn shield_direction_candidates() {
    let inputs = inputs();
    let cases = [
        (1173, 829, 5usize),
        (1173, 898, 6),
        (1173, 836, 5),
        (1171, 899, 9),
        (1173, 625, 11),
        (1949, 836, 9),
    ];
    for (item_id, seq_id, frame) in cases {
        let (id, name, model) = inputs
            .items
            .iter()
            .find(|(id, _, _)| *id == item_id)
            .unwrap();
        let equip = EquipModel {
            item_id: *id,
            model: model.clone(),
        };
        let (assembled, _, _) = inputs
            .body
            .assemble_parts(&[(slot_for(*id).to_string(), &equip)]);
        let (_, bind_part) =
            split_part(&assembled, inputs.base.vertex_count, inputs.base.face_count);
        let sequence = inputs.sequences.iter().find(|s| s.id == seq_id).unwrap();
        let posed = inputs.body.pose(&assembled, sequence, frame).unwrap();
        let (body, part) = split_part(&posed, inputs.base.vertex_count, inputs.base.face_count);
        eprintln!(
            "{name} seq {seq_id} frame {frame}: raw pen {:.2} gap {:.2}",
            posed_fit_penetration(&bind_part, &body, &part),
            clearance(&body, &part)
        );
        let dirs: [([f64; 3], &str); 6] = [
            ([1.0, 0.0, 0.0], "+X (left)"),
            ([-1.0, 0.0, 0.0], "-X (right)"),
            ([0.0, -1.0, 0.0], "-Y (up)"),
            ([0.0, 1.0, 0.0], "+Y (down)"),
            ([0.0, 0.0, 1.0], "+Z (front)"),
            ([0.0, 0.0, -1.0], "-Z (back)"),
        ];
        for (dir, label) in dirs {
            let shifted = |t: f64| {
                let mut probe = part.clone();
                for v in 0..probe.vertex_count {
                    probe.xs[v] = (f64::from(part.xs[v]) + dir[0] * t) as f32;
                    probe.ys[v] = (f64::from(part.ys[v]) + dir[1] * t) as f32;
                    probe.zs[v] = (f64::from(part.zs[v]) + dir[2] * t) as f32;
                }
                probe
            };
            let mut found = None;
            let mut t = 0.5;
            while t <= 80.0 {
                if posed_fit_penetration(&bind_part, &body, &shifted(t)) <= FIT_MAX_PENETRATION {
                    found = Some(t);
                    break;
                }
                t += 0.5;
            }
            match found {
                Some(t) => eprintln!(
                    "   {label}: shift {t:.1} → gap {:.2}",
                    clearance(&body, &shifted(t))
                ),
                None => eprintln!("   {label}: no shift ≤ 80 clears"),
            }
        }
    }
}

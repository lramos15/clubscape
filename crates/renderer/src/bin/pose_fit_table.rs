//! Builds `assets/compiled/render/gear/pose-fits.json`: the per-pose contact fits of every M1
//! worn model across the required player sequences, exactly as the runtime solves them
//! (`PlayerBody::pose_fitted`), so the browser applies a recorded shift instead of solving.
//!
//! Usage: `cargo run --release -p clubscape-renderer --features tools --bin pose-fit-table --
//! <assets/compiled/render>`; run through `tools/render-assets/export.py --profile pose-fits`,
//! which also registers the file in the manifest.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clubscape_renderer::actor::{
    EquipModel, FIT_MAX_ATTACHMENT, FIT_MAX_GAP, FIT_MAX_PENETRATION, PlayerBody, PoseFitTable,
    REQUIRED_PLAYER_SEQUENCES, TableItemFits, TableTargets, cached_kits, clearance, fit_legality,
    merge_into, posed_fit_penetration, split_part,
};
use clubscape_renderer::anim::{Sequence, apply_frame};
use clubscape_renderer::model::Model;
use sha2::Digest as _;

fn read(root: &Path, manifest: &serde_json::Value, key: &str) -> Vec<u8> {
    let path = root.join(key);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let pinned = manifest["files"][key]["sha256"]
        .as_str()
        .unwrap_or_else(|| panic!("{key} is not listed in the manifest"));
    let actual: String = sha2::Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert!(
        actual == pinned,
        "{key}: sha256 {actual} does not match the manifest pin {pinned}"
    );
    bytes
}

fn slot_for(item: &serde_json::Value) -> String {
    // The M1 items map by id to the renderer's equipment slots (the content pack's
    // `occupied_slots`); sequence hand items (net, tinderbox, hammer) are drawn through the
    // hand slot the sequence names and are right-hand tools by their labels.
    match item["item_id"].as_i64().unwrap_or(-1) {
        1949 => "head",
        1171 | 1173 => "shield",
        1009 => "amulet",
        _ => "weapon",
    }
    .to_string()
}

fn main() {
    let root: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets/compiled/render"));
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).expect("manifest"))
            .expect("manifest json");
    let penguin = manifest["npc_definitions"]
        .as_array()
        .expect("npc_definitions")
        .iter()
        .find(|d| d["npc_id"] == 2063)
        .expect("penguin definition 2063");
    let base_key = penguin["base_model"].as_str().expect("base_model");
    let human_key = manifest["player_reference"]["model"]
        .as_str()
        .expect("player_reference.model");
    let base = Model::from_chunks(&read(&root, &manifest, base_key)).expect("penguin base");
    let human = Model::from_chunks(&read(&root, &manifest, human_key)).expect("human reference");
    let body = PlayerBody::new(
        base,
        penguin["width_scale"].as_i64().expect("width_scale") as i32,
        penguin["height_scale"].as_i64().expect("height_scale") as i32,
        vec![
            penguin["sequences"]["stand"].as_i64().expect("stand") as i32,
            penguin["sequences"]["walk"].as_i64().expect("walk") as i32,
        ],
        &human,
    );
    let sequences: Vec<Sequence> = REQUIRED_PLAYER_SEQUENCES
        .iter()
        .map(|id| {
            Sequence::from_chunks(&read(&root, &manifest, &format!("anim/seq-{id}.bin")))
                .unwrap_or_else(|e| panic!("sequence {id}: {e}"))
        })
        .collect();
    // Kits the source cache has (for the hand-override decode); the manifest records every
    // value the required sequences use and whether its kit exists.
    assert!(
        manifest["sequence_hand_overrides"]["values"].is_array(),
        "manifest lacks sequence_hand_overrides (export.py --profile anim)"
    );
    let kits = cached_kits(&manifest);
    let mut items: HashMap<String, TableItemFits> = HashMap::new();
    let mut unmet = 0usize;
    let mut unmet_attachment = 0usize;
    // Failures whose penetration exceeds the source design's own overlap at the same pose by
    // more than the target (informational split of the failures, not a gate).
    let mut over_design = 0usize;
    let mut unmet_penetration = 0usize;
    let mut unmet_gap = 0usize;
    let mut frames = 0usize;
    let mut hidden_frames = 0usize;
    let mut classes: HashMap<&'static str, usize> = HashMap::new();
    let mut failures: Vec<serde_json::Value> = Vec::new();
    for item in manifest["equipment_items"]
        .as_array()
        .expect("equipment_items")
    {
        let Some(key) = item["equip_model"].as_str() else {
            continue;
        };
        let item_id = item["item_id"].as_i64().expect("item_id") as i32;
        let model = Model::from_chunks(&read(&root, &manifest, key)).expect("equip model");
        let slot = slot_for(item);
        let equippable = item["role"].as_str() != Some("sequence_hand_item");
        let equip = EquipModel { item_id, model };
        let (assembled, _, parts) = body.assemble_parts(&[(slot.clone(), &equip)]);
        // The same item on the human body it was designed for, posed by the same frame: the
        // source's own overlap at that pose (context beside every failure, never a waiver).
        let mut human_assembled = body.human.clone();
        merge_into(&mut human_assembled, &equip.model);
        let mut fits = TableItemFits {
            slot: slot.clone(),
            model_sha256: manifest["files"][key]["sha256"]
                .as_str()
                .expect("item sha")
                .to_string(),
            grip: parts[0].grip,
            sequences: HashMap::new(),
        };
        for sequence in &sequences {
            let legality = fit_legality(&slot, item_id, sequence, &kits, equippable);
            let class = legality.name();
            if !legality.is_drawn() {
                // Not drawn while this sequence plays (source override / not wearable): no fit.
                hidden_frames += sequence.frame_count();
                *classes.entry(class).or_default() += sequence.frame_count();
                continue;
            }
            let mut per_frame = Vec::with_capacity(sequence.frame_count());
            for frame in 0..sequence.frame_count() {
                let offsets = body
                    .pose_fit_offsets(&assembled, &parts, sequence, frame)
                    .expect("pose fit");
                let fit = offsets.into_iter().next().expect("one part");
                let over_pen = fit.penetration > FIT_MAX_PENETRATION;
                let over_gap = fit.gap > FIT_MAX_GAP;
                let over_att = fit.attachment_gap > FIT_MAX_ATTACHMENT;
                if over_pen || over_gap || over_att {
                    unmet += 1;
                    unmet_penetration += usize::from(over_pen);
                    unmet_gap += usize::from(over_gap);
                    unmet_attachment += usize::from(over_att);
                    let design = if body.native_sequences.contains(&sequence.id) {
                        None
                    } else {
                        let mut posed = human_assembled.clone();
                        apply_frame(&mut posed, sequence, frame, None).expect("human pose");
                        let (hbody, hpart) =
                            split_part(&posed, body.human.vertex_count, body.human.face_count);
                        Some((
                            posed_fit_penetration(&equip.model, &hbody, &hpart),
                            clearance(&hbody, &hpart),
                        ))
                    };
                    if design.is_some_and(|(pen, _)| fit.penetration > pen + FIT_MAX_PENETRATION) {
                        over_design += 1;
                    }
                    failures.push(serde_json::json!({
                        "item_id": item_id, "sequence": sequence.id, "frame": frame, "legality": class,
                        "penetration": fit.penetration, "gap": fit.gap, "attachment_gap": fit.attachment_gap,
                        "anchor_clearance": fit.anchor_clearance,
                        "human_design_penetration_same_pose": design.map(|(p, _)| p),
                        "human_design_gap_same_pose": design.map(|(_, g)| g),
                    }));
                }
                frames += 1;
                *classes.entry(class).or_default() += 1;
                per_frame.push(fit);
            }
            fits.sequences.insert(sequence.id.to_string(), per_frame);
        }
        eprintln!(
            "item {item_id} ({}): {} visible sequences",
            item["name"].as_str().unwrap_or(""),
            fits.sequences.len()
        );
        items.insert(item_id.to_string(), fits);
    }
    let table = PoseFitTable {
        schema_version: 2,
        body_npc: 2063,
        body_model_sha256: manifest["files"][base_key]["sha256"]
            .as_str()
            .expect("base sha")
            .to_string(),
        human_reference_sha256: manifest["files"][human_key]["sha256"]
            .as_str()
            .expect("human sha")
            .to_string(),
        targets: TableTargets {
            penetration: FIT_MAX_PENETRATION,
            gap: FIT_MAX_GAP,
            attachment: FIT_MAX_ATTACHMENT,
        },
        items,
    };
    let out = root.join("gear/pose-fits.json");
    std::fs::create_dir_all(out.parent().expect("parent")).expect("mkdir gear");
    // Deterministic key order for a reproducible file.
    let value = serde_json::to_value(&table).expect("table json");
    let text = serde_json::to_string(&sort_keys(value)).expect("json");
    std::fs::write(&out, text + "\n").expect("write table");
    let mut class_counts: Vec<(&str, usize)> = classes.into_iter().collect();
    class_counts.sort();
    let failure_path = root.join("gear/pose-fit-failures.json");
    std::fs::write(
        &failure_path,
        serde_json::to_string(&serde_json::json!({
            "schema_version": 1,
            "targets": {"penetration": FIT_MAX_PENETRATION, "gap": FIT_MAX_GAP, "attachment_gap": FIT_MAX_ATTACHMENT},
            "measures": {
                "penetration": "max(deepest body vertex inside the item's minimum-volume oriented box fixed at the bind pose and carried with the item, deepest item vertex inside the body mesh); counts the source design's own hand-in-hilt / arm-behind-plate overlap too",
                "gap": "item <-> body surface clearance (touching anywhere)",
                "attachment_gap": "distance of the item's grip/contact point from where the posed anchor bone carries the retargeted design grip",
                "anchor_clearance": "item <-> anchor body part surface clearance",
                "human_design_penetration_same_pose": "the same measure for the item on the human body it was designed for, posed by the same frame (null for the penguin's native sequences); context only"
            },
            "item_frames_measured": frames,
            "item_frames_not_drawn": hidden_frames,
            "legality_frame_counts": class_counts.iter().map(|(k, v)| serde_json::json!({"class": k, "item_frames": v})).collect::<Vec<_>>(),
            "failures_over_penetration_beyond_design_plus_target": over_design,
            "failures": failures,
        }))
        .expect("json")
            + "\n",
    )
    .expect("write failures");
    println!(
        "{}",
        serde_json::json!({
            "file": "gear/pose-fits.json",
            "failures_file": "gear/pose-fit-failures.json",
            "items": table.items.len(),
            "sequences": REQUIRED_PLAYER_SEQUENCES.len(),
            "item_frames": frames,
            "item_frames_not_drawn": hidden_frames,
            "legality_frame_counts": class_counts.iter().map(|(k, v)| serde_json::json!({"class": k, "item_frames": v})).collect::<Vec<_>>(),
            "item_frames_over_target": unmet,
            "over_penetration": unmet_penetration,
            "over_gap": unmet_gap,
            "over_attachment": unmet_attachment,
            "over_penetration_beyond_design_plus_target": over_design,
        })
    );
}

fn sort_keys(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(String, serde_json::Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| {
                // Numeric keys (item / sequence ids) sort numerically.
                match (a.0.parse::<i64>(), b.0.parse::<i64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    _ => a.0.cmp(&b.0),
                }
            });
            let mut out = serde_json::Map::new();
            for (k, v) in entries {
                out.insert(k, sort_keys(v));
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sort_keys).collect())
        }
        other => other,
    }
}

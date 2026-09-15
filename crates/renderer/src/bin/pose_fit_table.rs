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
    EquipModel, FIT_MAX_GAP, FIT_MAX_PENETRATION, PlayerBody, PoseFitTable,
    REQUIRED_PLAYER_SEQUENCES, TableItemFits, TableTargets,
};
use clubscape_renderer::anim::Sequence;
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
    // The manifest records the source wear position (`params` 2431 flag is not the slot); the
    // M1 items map by id, matching the renderer's equipment slots.
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
    let mut items: HashMap<String, TableItemFits> = HashMap::new();
    let mut unmet = 0usize;
    let mut frames = 0usize;
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
        let equip = EquipModel { item_id, model };
        let (assembled, _, parts) = body.assemble_parts(&[(slot.clone(), &equip)]);
        let mut fits = TableItemFits {
            slot,
            model_sha256: manifest["files"][key]["sha256"]
                .as_str()
                .expect("item sha")
                .to_string(),
            sequences: HashMap::new(),
        };
        for sequence in &sequences {
            let mut per_frame = Vec::with_capacity(sequence.frame_count());
            for frame in 0..sequence.frame_count() {
                let offsets = body
                    .pose_fit_offsets(&assembled, &parts, sequence, frame)
                    .expect("pose fit");
                let fit = offsets.into_iter().next().expect("one part");
                if fit.penetration > FIT_MAX_PENETRATION || fit.gap > FIT_MAX_GAP {
                    unmet += 1;
                }
                frames += 1;
                per_frame.push(fit);
            }
            fits.sequences.insert(sequence.id.to_string(), per_frame);
        }
        eprintln!(
            "item {item_id} ({}): {} sequences",
            item["name"].as_str().unwrap_or(""),
            fits.sequences.len()
        );
        items.insert(item_id.to_string(), fits);
    }
    let table = PoseFitTable {
        schema_version: 1,
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
        },
        items,
    };
    let out = root.join("gear/pose-fits.json");
    std::fs::create_dir_all(out.parent().expect("parent")).expect("mkdir gear");
    // Deterministic key order for a reproducible file.
    let value = serde_json::to_value(&table).expect("table json");
    let text = serde_json::to_string(&sort_keys(value)).expect("json");
    std::fs::write(&out, text + "\n").expect("write table");
    println!(
        "{}",
        serde_json::json!({
            "file": "gear/pose-fits.json",
            "items": table.items.len(),
            "sequences": REQUIRED_PLAYER_SEQUENCES.len(),
            "item_frames": frames,
            "item_frames_over_target": unmet,
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

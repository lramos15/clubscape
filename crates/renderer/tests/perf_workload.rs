//! Native CPU profile of the frozen representative workload (the region-mode `workload`
//! scenario of the browser capture): the Lumbridge scene assembled from blocks, the geared
//! fighting penguin, seven NPCs, a fire and ground items, at the 1920x1080 source camera.
//! Measures `build_frame` (traversal + projection) and `pack_frame` (GPU layout) per frame.
//! Ignored by default: needs the local block exports and takes a few seconds.

mod common;

use clubscape_renderer::core::{Camera, RendererCore};
use clubscape_renderer::gpu::pack::{PackScratch, PackedFrame};
use clubscape_renderer::palette::Palette;
use clubscape_renderer::texture::{Texture, TextureSet};

use common::repo_root;

fn asset(key: &str) -> Vec<u8> {
    common::read_asset(key)
}

fn manifest() -> serde_json::Value {
    serde_json::from_slice(&asset("manifest.json")).unwrap()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx]
}

#[test]
#[ignore = "profiling harness; needs the local world block exports"]
fn workload_cpu_profile() {
    let manifest = manifest();
    let mut core = RendererCore::new(
        Palette::from_chunks(&asset("palette.bin")).unwrap(),
        1920,
        1080,
    );
    let mut textures = TextureSet::default();
    for entry in std::fs::read_dir(repo_root().join("assets/compiled/render/textures")).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        core.add_texture(&bytes).unwrap();
        textures.insert(Texture::from_chunks(&bytes).unwrap());
    }
    for seq in manifest["sequences"].as_array().unwrap() {
        core.load_sequence(&asset(seq["file"].as_str().unwrap()))
            .unwrap();
    }
    for def in manifest["npc_definitions"].as_array().unwrap() {
        core.load_npc_definition(
            &def.to_string(),
            &asset(def["base_model"].as_str().unwrap()),
        )
        .unwrap();
    }
    let penguin = manifest["npc_definitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["npc_id"] == 2063)
        .unwrap();
    core.load_player_body(
        &asset(penguin["base_model"].as_str().unwrap()),
        penguin["width_scale"].as_i64().unwrap() as i32,
        penguin["height_scale"].as_i64().unwrap() as i32,
        vec![
            penguin["sequences"]["stand"].as_i64().unwrap() as i32,
            penguin["sequences"]["walk"].as_i64().unwrap() as i32,
        ],
        &asset(manifest["player_reference"]["model"].as_str().unwrap()),
    )
    .unwrap();
    for item in manifest["equipment_items"].as_array().unwrap() {
        if let Some(path) = item["equip_model"].as_str() {
            core.load_equip_model(item["item_id"].as_i64().unwrap() as i32, &asset(path))
                .unwrap();
        }
    }
    for object in manifest["dynamic_objects"].as_array().unwrap() {
        let id = object["object_id"].as_i64().unwrap() as i32;
        for variant in object["variants"].as_array().unwrap() {
            let frames: Vec<Vec<u8>> = variant["frames"]
                .as_array()
                .map(|f| f.iter().map(|p| asset(p.as_str().unwrap())).collect())
                .unwrap_or_default();
            let lengths: Vec<i32> = variant["frame_lengths_client_cycles"]
                .as_array()
                .map(|l| l.iter().map(|v| v.as_i64().unwrap() as i32).collect())
                .unwrap_or_default();
            core.load_dynamic_object(
                id,
                variant["type"].as_i64().unwrap() as i32,
                variant["orientation"].as_i64().unwrap() as i32,
                &asset(variant["model"].as_str().unwrap()),
                &frames,
                lengths,
            )
            .unwrap();
        }
    }
    for item in manifest["ground_items"].as_array().unwrap() {
        let id = item["item_id"].as_i64().unwrap() as i32;
        for variant in item["variants"].as_array().unwrap() {
            core.load_ground_item(
                id,
                variant["min_quantity"].as_i64().unwrap(),
                &asset(variant["model"].as_str().unwrap()),
            )
            .unwrap();
        }
    }
    let (px, py) = (3222, 3218);
    let (bx, by) = RendererCore::base_for_tile(px, py);
    for square in RendererCore::squares_for_base(bx, by) {
        let path = repo_root().join(format!("assets/compiled/render/blocks/{square}.bin"));
        if path.exists() {
            core.load_block(
                square,
                &std::fs::read(&path).unwrap(),
                &std::fs::read(
                    repo_root().join(format!("assets/compiled/render/blocks/{square}.models.bin")),
                )
                .unwrap(),
            )
            .unwrap();
        }
    }
    core.assemble_scene(bx, by, true, 0.0).unwrap();
    core.set_camera(Camera {
        x: px * 128 + 64,
        height: -1540,
        y: (py - 8) * 128,
        pitch: 2048,
        yaw: 0,
        zoom: 662,
        far: 32768,
    })
    .unwrap();
    let tile = |x: i32, y: i32| format!(r#"{{"x":{x},"y":{y},"plane":0}}"#);
    let npc = |id: &str, source: i32, x: i32, y: i32| {
        format!(
            r#"{{"id":"{id}","definitionId":"asset.source.osrs.cache2695.npc.{source}","sourceId":{source},"name":"{id}","kind":"npc","tile":{},"instance":null,"hitpoints":5,"maxHitpoints":5,"available":true,"animation":"","actions":[],"appearance":{{}},"equipment":[]}}"#,
            tile(x, y)
        )
    };
    let world = format!(
        r#"{{"revision":"dev","tick":"0","player":{{"id":"player-dev","displayName":"dev","appearance":{{}},"region":"region.osrs.12850","tile":{},"instance":null,"inventory":[],"equipment":[{{"slot":"weapon","item":{{"id":"item.bronze_sword","name":"s","quantity":1,"sourceId":1277}}}},{{"slot":"shield","item":{{"id":"item.wooden_shield","name":"w","quantity":1,"sourceId":1171}}}}],"skills":[],"hitpoints":10,"prayerPoints":1,"runEnergy":100,"questPoints":0,"tutorialStage":"","tutorialInstruction":"","quests":[],"unlockedInterfaces":[],"activePrayers":[],"activity":"fighting","animation":"asset.source.osrs.cache2695.sequence.390","settings":[]}},
        "entities":[{},{},{},{},{},{},{},{{"id":"fire","definitionId":"asset.source.osrs.cache2695.object.26185","sourceId":26185,"name":"Fire","kind":"temporary_object","tile":{},"instance":null,"hitpoints":0,"maxHitpoints":0,"available":true,"animation":"","actions":[],"appearance":{{}},"equipment":[]}}],
        "groundItems":[{{"id":"g1","tile":{},"item":{{"id":"item.logs","name":"l","quantity":1,"sourceId":1511}},"canTake":true}},{{"id":"g2","tile":{},"item":{{"id":"item.coins","name":"c","quantity":250,"sourceId":995}},"canTake":true}},{{"id":"g3","tile":{},"item":{{"id":"item.bronze_axe","name":"a","quantity":1,"sourceId":1351}},"canTake":true}}],
        "dialogue":null,"bank":null,"shop":null,"recovery":null,"messages":[]}}"#,
        tile(px, py),
        npc("goblin-a", 3028, px + 3, py + 2),
        npc("goblin-b", 3028, px - 4, py + 3),
        npc("goblin-c", 3028, px + 5, py - 3),
        npc("rat-a", 2813, px - 3, py - 2),
        npc("rat-b", 2814, px + 2, py - 4),
        npc("guide", 306, px + 1, py + 5),
        npc("survival-expert", 8503, px - 6, py),
        tile(px - 2, py + 1),
        tile(px + 1, py + 1),
        tile(px + 1, py + 1),
        tile(px - 1, py - 1)
    );
    core.update_world(&world, 0.0).unwrap();
    let mut packed = PackedFrame::default();
    let mut scratch = PackScratch::default();
    let frames = 240;
    let mut build = Vec::with_capacity(frames);
    let mut pack = Vec::with_capacity(frames);
    let mut prims = 0;
    for i in 0..frames {
        let now = i as f64 * 16.67;
        let t0 = std::time::Instant::now();
        let summary = core.build_frame(now).unwrap();
        prims = summary.triangles;
        let t1 = std::time::Instant::now();
        packed.pack(&mut scratch, &core.state, core.triangles(), &core.textures);
        let t2 = std::time::Instant::now();
        std::hint::black_box(&packed);
        build.push((t1 - t0).as_secs_f64() * 1000.0);
        pack.push((t2 - t1).as_secs_f64() * 1000.0);
    }
    build.sort_by(|a, b| a.partial_cmp(b).unwrap());
    pack.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "workload prims={prims} build p50={:.2} p95={:.2} max={:.2} ms | pack p50={:.2} p95={:.2} max={:.2} ms | skipped={:?}",
        percentile(&build, 0.5),
        percentile(&build, 0.95),
        build.last().unwrap(),
        percentile(&pack, 0.5),
        percentile(&pack, 0.95),
        pack.last().unwrap(),
        core.last_summary.entities_skipped
    );
}

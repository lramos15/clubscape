//! The static-placement projection cache must be invisible: a frame built after the camera has
//! held still (cache warm) must produce exactly the triangle stream a fresh renderer builds for
//! the same inputs, actors included; a camera change must drop the cache; picks must carry the
//! current frame's ids.

mod common;

use clubscape_renderer::core::{Camera, RendererCore};
use clubscape_renderer::palette::Palette;

fn core_with_scene(name: &str, camera: Camera) -> RendererCore {
    let mut core = RendererCore::new(
        Palette::from_chunks(&common::read_asset("palette.bin")).unwrap(),
        1920,
        1080,
    );
    for bytes in common::texture_bytes() {
        core.add_texture(&bytes).unwrap();
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&common::read_asset("manifest.json")).unwrap();
    for def in manifest["npc_definitions"].as_array().unwrap() {
        core.load_npc_definition(
            &def.to_string(),
            &common::read_asset(def["base_model"].as_str().unwrap()),
        )
        .unwrap();
    }
    for seq in manifest["sequences"].as_array().unwrap() {
        core.load_sequence(&common::read_asset(seq["file"].as_str().unwrap()))
            .unwrap();
    }
    core.load_scene(
        name,
        &common::read_asset(&format!("scenes/{name}.bin")),
        &common::read_asset(&format!("scenes/{name}.models.bin")),
    )
    .unwrap();
    core.set_camera(camera).unwrap();
    core
}

const PLAZA: Camera = Camera {
    x: 3168 * 128 + 6912,
    height: -1540,
    y: 3168 * 128 + 5120,
    pitch: 2048,
    yaw: 0,
    zoom: 662,
    far: 32768,
};

fn world(goblin_x: i32) -> String {
    format!(
        r#"{{"revision":"1","tick":"1","player":{{"id":"p","tile":{{"x":3222,"y":3218,"plane":0}},"animation":"808"}},
        "entities":[{{"id":"g","definitionId":"asset.source.osrs.cache2695.npc.3028","sourceId":3028,"name":"Goblin","kind":"npc","tile":{{"x":{goblin_x},"y":3221,"plane":0}},"instance":null,"hitpoints":5,"maxHitpoints":5,"available":true,"animation":"","actions":[],"appearance":{{}},"equipment":[]}}],
        "groundItems":[]}}"#
    )
}

#[test]
fn warm_cache_frames_equal_a_fresh_build() {
    let mut cached = core_with_scene("lumbridge-castle-plaza", PLAZA);
    cached.update_world(&world(3225), 0.0).unwrap();
    // Frame 1 cold (camera new), frame 2 settled (entries stored), frame 3 replayed.
    for _ in 0..3 {
        cached.build_frame(0.0).unwrap();
    }
    let (hits, misses) = cached.model_cache_stats();
    assert!(
        hits > 100,
        "static placements replayed: {hits} (misses {misses})"
    );
    let mut fresh = core_with_scene("lumbridge-castle-plaza", PLAZA);
    fresh.update_world(&world(3225), 0.0).unwrap();
    fresh.build_frame(0.0).unwrap();
    assert_eq!(
        fresh.model_cache_stats().0,
        0,
        "a fresh renderer projects everything"
    );
    assert_eq!(cached.triangles().len(), fresh.triangles().len());
    assert!(
        cached.triangles() == fresh.triangles(),
        "replayed triangle stream differs from the fresh build"
    );

    // The goblin moves: traversal order and pick ids change; the replay must follow.
    cached.update_world(&world(3219), 100.0).unwrap();
    cached.build_frame(100.0).unwrap();
    fresh.update_world(&world(3219), 100.0).unwrap();
    fresh.build_frame(100.0).unwrap();
    assert!(cached.model_cache_stats().0 > 100);
    assert!(
        cached.triangles() == fresh.triangles(),
        "replayed stream differs after an actor moved"
    );
    let picks_cached: Vec<u32> = cached.triangles().iter().map(|t| t.pick).collect();
    let picks_fresh: Vec<u32> = fresh.triangles().iter().map(|t| t.pick).collect();
    let pick_diff = common::diff_buffers(
        &picks_cached.iter().map(|&p| p as i32).collect::<Vec<_>>(),
        &picks_fresh.iter().map(|&p| p as i32).collect::<Vec<_>>(),
    );
    assert!(pick_diff.differing == 0, "pick ids differ: {pick_diff:?}");

    // A camera change drops every entry and the first frame at the new camera stores none.
    let moved = Camera {
        x: PLAZA.x + 128,
        ..PLAZA
    };
    cached.set_camera(moved).unwrap();
    cached.build_frame(200.0).unwrap();
    assert_eq!(
        cached.model_cache_stats().0,
        0,
        "no stale replay after a camera move"
    );
    // The comparison renderer receives the same world history (actor facing follows steps).
    let mut fresh_moved = core_with_scene("lumbridge-castle-plaza", moved);
    fresh_moved.update_world(&world(3225), 0.0).unwrap();
    fresh_moved.update_world(&world(3219), 100.0).unwrap();
    fresh_moved.build_frame(200.0).unwrap();
    assert!(cached.triangles() == fresh_moved.triangles());
    cached.build_frame(216.0).unwrap();
    cached.build_frame(233.0).unwrap();
    assert!(
        cached.model_cache_stats().0 > 100,
        "cache rebuilt once the camera settled"
    );
    fresh_moved.build_frame(233.0).unwrap();
    assert!(cached.triangles() == fresh_moved.triangles());
}

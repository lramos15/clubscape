//! Block assembly: scenes assembled from 64x64 world blocks must equal the original loader's
//! direct 104x104 export at the same base (both with animated scenery pinned to frame 0), and
//! recentering around the player must keep the whole visible world present.

mod common;

use clubscape_renderer::core::{Camera, RendererCore};
use clubscape_renderer::palette::Palette;
use clubscape_renderer::raster::software::Software;
use clubscape_renderer::scene::draw::PickTarget;
use clubscape_renderer::texture::{Texture, TextureSet};

use common::repo_root;

fn read(rel: &str) -> Vec<u8> {
    match rel.strip_prefix("assets/compiled/render/") {
        Some(key) => common::read_asset(key),
        None => std::fs::read(repo_root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}")),
    }
}

fn textures() -> TextureSet {
    let mut set = TextureSet::default();
    for bytes in common::texture_bytes() {
        set.insert(Texture::from_chunks(&bytes).unwrap());
    }
    set
}

fn core_with_textures() -> RendererCore {
    let mut core = RendererCore::new(
        Palette::from_chunks(&read("assets/compiled/render/palette.bin")).unwrap(),
        1920,
        1080,
    );
    for bytes in common::texture_bytes() {
        core.add_texture(&bytes).unwrap();
    }
    // Block scenes run the live terrain pass from raw block terrain (published floor defs).
    core.load_floor_defs(&common::read_asset("terrain/floors.bin"))
        .unwrap();
    core
}

const REPRODUCE_BLOCKS: &str = "python3 tools/render-assets/export.py --profile blocks (or --profile unpack-blocks <pack.tar>, see blocks.index.json)";
const REPRODUCE_PINNED: &str = "python3 tools/render-assets/export.py --profile scenes-pinned";

/// World blocks are reproducible local exports (61 squares, ~75 MB gzip, hashes in the
/// manifest) and deliberately not published; a missing square fails the (ignored-by-default)
/// test rather than skipping the case.
fn load_blocks(core: &mut RendererCore, squares: &[i32]) {
    for &square in squares {
        core.load_block(
            square,
            &common::read_local_export(&format!("blocks/{square}.bin"), REPRODUCE_BLOCKS),
            &common::read_local_export(&format!("blocks/{square}.models.bin"), REPRODUCE_BLOCKS),
        )
        .unwrap();
    }
}

fn rasterize(core: &RendererCore, textures: &TextureSet) -> Vec<i32> {
    let mut pixels = vec![0; (core.state.width * core.state.height) as usize];
    {
        let mut raster = Software::new(core.state, &mut pixels, &core.palette.rgb, textures);
        for tri in core.triangles() {
            let _ = raster.draw(tri);
        }
    }
    pixels.iter_mut().for_each(|p| *p &= 0xFF_FFFF);
    pixels
}

struct Fixture {
    name: &'static str,
    base: (i32, i32),
    camera: (i32, i32, i32),
    pitch: i32,
    yaw: i32,
}

const FIXTURES: [Fixture; 5] = [
    Fixture {
        name: "tutorial-starting-house",
        base: (3048, 3056),
        camera: (5888, -2360, 4992),
        pitch: 2048,
        yaw: 0,
    },
    Fixture {
        name: "tutorial-survival-coast",
        base: (3048, 3032),
        camera: (7296, -1996, 5632),
        pitch: 2048,
        yaw: 1024,
    },
    Fixture {
        name: "lumbridge-castle-plaza",
        base: (3168, 3168),
        camera: (6912, -1540, 5120),
        pitch: 2048,
        yaw: 0,
    },
    Fixture {
        name: "lumbridge-river-bridge",
        base: (3168, 3168),
        camera: (8576, -1540, 4736),
        pitch: 2048,
        yaw: 2048,
    },
    Fixture {
        name: "lumbridge-windmill-route",
        base: (3120, 3240),
        camera: (6912, -1890, 7040),
        pitch: 2048,
        yaw: 1536,
    },
];

fn camera(f: &Fixture) -> Camera {
    Camera {
        x: f.base.0 * 128 + f.camera.0,
        height: f.camera.1,
        y: f.base.1 * 128 + f.camera.2,
        pitch: f.pitch,
        yaw: f.yaw,
        zoom: 662,
        far: 32768,
    }
}

/// Ignored by default: needs the reproducible local block and pinned-scene exports. Run with
/// `cargo test -p clubscape-renderer --release --test blocks -- --include-ignored`; every one of
/// the five fixture bases must then be present and identical.
#[test]
#[ignore = "needs local exports: export.py --profile blocks and --profile scenes-pinned"]
fn assembled_blocks_match_direct_pinned_exports() {
    let textures = textures();
    let mut checked = 0;
    for fixture in &FIXTURES {
        let squares = RendererCore::squares_for_base(fixture.base.0, fixture.base.1);
        let mut direct = core_with_textures();
        direct
            .load_scene(
                fixture.name,
                &common::read_local_export(
                    &format!("scenes/{}.pinned.bin", fixture.name),
                    REPRODUCE_PINNED,
                ),
                &common::read_local_export(
                    &format!("scenes/{}.pinned.models.bin", fixture.name),
                    REPRODUCE_PINNED,
                ),
            )
            .unwrap();
        direct.set_camera(camera(fixture)).unwrap();
        let direct_summary = direct.build_frame(0.0).unwrap().clone();
        let direct_pixels = rasterize(&direct, &textures);

        let mut assembled = core_with_textures();
        load_blocks(&mut assembled, &squares);
        let missing = assembled
            .assemble_scene(fixture.base.0, fixture.base.1, false, 0.0)
            .unwrap();
        assert!(
            missing.is_empty(),
            "{}: blocks missing {missing:?}",
            fixture.name
        );
        assembled.set_camera(camera(fixture)).unwrap();
        let assembled_summary = assembled.build_frame(0.0).unwrap().clone();
        let assembled_pixels = rasterize(&assembled, &textures);

        assert_eq!(
            assembled_summary.triangles, direct_summary.triangles,
            "{}: triangle count differs (assembled vs direct)",
            fixture.name
        );
        let differing = assembled_pixels
            .iter()
            .zip(&direct_pixels)
            .filter(|(a, b)| a != b)
            .count();
        if differing > 0 {
            // Cluster the differing pixels into coarse boxes to point at the offending placement.
            type Box = (i32, i32, i32, i32, usize);
            let mut boxes: std::collections::BTreeMap<(i32, i32), Box> = Default::default();
            for (i, (a, b)) in assembled_pixels.iter().zip(&direct_pixels).enumerate() {
                if a != b {
                    let (x, y) = ((i % 1920) as i32, (i / 1920) as i32);
                    let e = boxes.entry((x / 64, y / 64)).or_insert((x, y, x, y, 0));
                    e.0 = e.0.min(x);
                    e.1 = e.1.min(y);
                    e.2 = e.2.max(x);
                    e.3 = e.3.max(y);
                    e.4 += 1;
                }
            }
            let hits: Vec<_> = boxes.values().take(12).collect();
            let picks: Vec<_> = boxes
                .values()
                .take(6)
                .map(|b| {
                    (
                        assembled.pick((b.0 + b.2) / 2, (b.1 + b.3) / 2),
                        direct.pick((b.0 + b.2) / 2, (b.1 + b.3) / 2),
                    )
                })
                .collect();
            panic!(
                "{}: {differing} pixels differ between block assembly and direct export; boxes {hits:?}; picks {picks:?}",
                fixture.name
            );
        }
        // Picking decodes to the same world tiles in both scenes.
        for (x, y) in [(960, 540), (300, 700), (1500, 900)] {
            let a = assembled.pick(x, y).map(|p| match p {
                PickTarget::Tile { plane, x, y } | PickTarget::Object { plane, x, y, .. } => {
                    (plane, x, y)
                }
            });
            let d = direct.pick(x, y).map(|p| match p {
                PickTarget::Tile { plane, x, y } | PickTarget::Object { plane, x, y, .. } => {
                    (plane, x, y)
                }
            });
            assert_eq!(a, d, "{}: pick at {x},{y} differs", fixture.name);
        }
        checked += 1;
        eprintln!(
            "{}: block assembly identical ({} triangles)",
            fixture.name, assembled_summary.triangles
        );
    }
    eprintln!("checked {checked} fixture bases");
}

#[test]
#[ignore = "needs local exports: export.py --profile blocks"]
fn recentering_follows_the_player_and_keeps_scenery() {
    // Player on the Tutorial Island starting-house tile: the original centres the scene on the
    // player's chunk, base = ((tile >> 3) - 6) * 8.
    let (bx, by) = RendererCore::base_for_tile(3094, 3103);
    assert_eq!((bx, by), (3040, 3048));
    let squares = RendererCore::squares_for_base(bx, by);
    let textures = textures();
    let mut core = core_with_textures();
    load_blocks(&mut core, &squares);
    let missing = core.assemble_scene(bx, by, true, 0.0).unwrap();
    assert!(missing.is_empty(), "{missing:?}");
    assert!(!core.needs_recenter(3094, 3103, 16));
    assert!(core.needs_recenter(3094, by + 10, 16));
    assert!(core.needs_recenter(bx + 100, 3103, 16));
    core.set_camera(Camera {
        x: 3094 * 128,
        height: -2360,
        y: 3103 * 128 - 1024,
        pitch: 2048,
        yaw: 0,
        zoom: 662,
        far: 32768,
    })
    .unwrap();
    core.build_frame(0.0).unwrap();
    let pixels = rasterize(&core, &textures);
    let drawn = pixels.iter().filter(|&&p| p != 0).count();
    assert!(
        drawn > pixels.len() / 2,
        "assembled scene drew only {drawn} pixels"
    );
    // Animated scenery advances with the scene clock when present.
    let animated = core.scene().unwrap().animated_instances.len();
    if animated > 0 {
        let before = core.triangles().len();
        core.build_frame(20.0 * 7.0).unwrap();
        let later = rasterize(&core, &textures);
        assert!(!core.triangles().is_empty() && before > 0);
        eprintln!(
            "animated placements {animated} changed pixels after 7 cycles: {}",
            later.iter().zip(&pixels).filter(|(a, b)| a != b).count()
        );
    }
}

/// Instance scenes: only the declared chunks are assembled (the M1 Death Office template — four
/// 8×8 identity mappings of region 12633 at plane 0, turn 0 — and a translated turn-0 mapping),
/// everything else stays unloaded in the scene and on the minimap, block fetches shrink to the
/// declared source squares, and a turned chunk is rejected explicitly rather than approximated.
#[test]
#[ignore = "needs local exports: export.py --profile blocks"]
fn instance_layout_assembles_only_declared_chunks() {
    let textures = textures();
    // Death Office: template region 12633 (square x 49, y 89 → chunk origin 392,712); origins
    // (3168,5720) (3168,5728) (3176,5720) (3176,5728) → chunks (396,715) (396,716) (397,715)
    // (397,716); the player stands inside.
    let death_office: Vec<serde_json::Value> = [(396, 715), (396, 716), (397, 715), (397, 716)]
        .iter()
        .map(|(cx, cy)| {
            serde_json::json!({"plane": 0, "chunkX": cx, "chunkY": cy, "sourcePlane": 0, "sourceChunkX": cx, "sourceChunkY": cy, "quarterTurns": 0})
        })
        .collect();
    let player = (3172, 5724);
    let (base_x, base_y) = RendererCore::base_for_tile(player.0, player.1);
    assert_eq!((base_x, base_y), (3120, 5672));
    let world = |layout: serde_json::Value| -> String {
        serde_json::json!({
            "revision": "1", "tick": "1",
            "player": {"id": "player-1", "tile": {"x": player.0, "y": player.1, "plane": 0}, "animation": "808",
                       "activity": "idle", "hitpoints": 10, "instance": "instance.death-office.1", "equipment": []},
            "entities": [], "instanceLayout": layout
        })
        .to_string()
    };
    // Ordinary world at the same base (all squares the original loader would request).
    let mut plain = core_with_textures();
    let squares = RendererCore::squares_for_base(base_x, base_y);
    let available: Vec<i32> = squares
        .iter()
        .copied()
        .filter(|s| {
            repo_root()
                .join(format!("assets/compiled/render/blocks/{s}.bin"))
                .is_file()
        })
        .collect();
    assert!(available.contains(&12633), "square 12633 export present");
    load_blocks(&mut plain, &available);
    plain
        .load_map_scenes(&common::read_asset("minimap/mapscenes.bin"))
        .unwrap();
    for &s in &available {
        plain
            .load_minimap_block(s, &common::read_asset(&format!("minimap/blocks/{s}.bin")))
            .unwrap();
    }
    plain
        .update_world(&world(serde_json::Value::Null), 0.0)
        .unwrap();
    assert!(!plain.instance_layout_changed());
    plain.assemble_scene(base_x, base_y, false, 0.0).unwrap();
    let plain_scene = plain.scene().unwrap().clone();

    // The instance: same blocks loaded, layout applied.
    let mut inst = core_with_textures();
    load_blocks(&mut inst, &available);
    inst.load_map_scenes(&common::read_asset("minimap/mapscenes.bin"))
        .unwrap();
    for &s in &available {
        inst.load_minimap_block(s, &common::read_asset(&format!("minimap/blocks/{s}.bin")))
            .unwrap();
    }
    inst.update_world(
        &world(serde_json::json!({"template": "instance.template.death-office", "chunks": death_office})),
        0.0,
    )
    .unwrap();
    assert!(
        inst.instance_layout_changed(),
        "a new layout asks for reassembly"
    );
    assert_eq!(
        inst.squares_needed(base_x, base_y),
        vec![12633],
        "only the declared source square"
    );
    inst.assemble_scene(base_x, base_y, false, 0.0).unwrap();
    assert!(!inst.instance_layout_changed());
    let scene = inst.scene().unwrap().clone();
    assert_eq!(
        scene.name,
        "blocks@3120,5672#instance.template.death-office"
    );
    // The template declares plane 0 of the four chunks only; the upper planes of those chunks
    // are as unloaded as every other chunk (the original template array has no entry for them).
    let declared = |plane: i32, x: i32, y: i32| {
        plane == 0 && (396..=397).contains(&(x >> 3)) && (715..=716).contains(&(y >> 3))
    };
    let mut inside_paints = 0usize;
    let mut compared = 0usize;
    for plane in 0..4 {
        for lx in 0..104 {
            for ly in 0..104 {
                let (wx, wy) = (base_x + lx, base_y + ly);
                let index = scene.tile_index(plane, lx + scene.offset, ly + scene.offset);
                let plain_index =
                    plain_scene.tile_index(plane, lx + plain_scene.offset, ly + plain_scene.offset);
                if declared(plane, wx, wy) {
                    // Declared chunks equal the ordinary world tile for tile (walls, floor
                    // decorations, object slots, heights of declared corners). Terrain colours
                    // equal the ordinary world where the live pass sees the same inputs — the
                    // 5-tile blend window and the light normals inside the declared area; at
                    // the area's edge the instance blends against nothing, exactly like the
                    // original instance scene, so those tiles legitimately differ.
                    let blend_interior =
                        (-5..=5).all(|ox| (-5..=5).all(|oy| declared(plane, wx + ox, wy + oy)));
                    if blend_interior {
                        assert_eq!(
                            scene.paints.get(&index),
                            plain_scene.paints.get(&plain_index),
                            "paint {plane} {wx},{wy}"
                        );
                    } else {
                        assert_eq!(
                            scene.paints.contains_key(&index)
                                || scene.tile_models.contains_key(&index),
                            plain_scene.paints.contains_key(&plain_index)
                                || plain_scene.tile_models.contains_key(&plain_index),
                            "tile presence {plane} {wx},{wy}"
                        );
                    }
                    assert_eq!(
                        scene.walls.get(&index),
                        plain_scene.walls.get(&plain_index),
                        "wall {plane} {wx},{wy}"
                    );
                    assert_eq!(
                        scene.floor_decorations.get(&index),
                        plain_scene.floor_decorations.get(&plain_index),
                        "floor {plane} {wx},{wy}"
                    );
                    for corner in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        // The live loader stores a corner height with the tile it is the
                        // south-west corner of: a corner on an undeclared tile holds none (0).
                        let expected = if declared(plane, wx + corner.0, wy + corner.1) {
                            plain_scene.height(
                                plane,
                                lx + plain_scene.offset + corner.0,
                                ly + plain_scene.offset + corner.1,
                            )
                        } else {
                            0
                        };
                        assert_eq!(
                            scene.height(
                                plane,
                                lx + scene.offset + corner.0,
                                ly + scene.offset + corner.1
                            ),
                            expected,
                            "height {plane} {wx},{wy} corner {corner:?}"
                        );
                    }
                    assert_eq!(
                        scene.object_count[index], plain_scene.object_count[plain_index],
                        "objects {plane} {wx},{wy}"
                    );
                    assert_eq!(
                        scene.tile_models.get(&index),
                        plain_scene.tile_models.get(&plain_index),
                        "tile model {plane} {wx},{wy}"
                    );
                    inside_paints += usize::from(
                        scene.paints.contains_key(&index) || scene.tile_models.contains_key(&index),
                    );
                    compared += 1;
                } else {
                    // Undeclared chunks are unloaded: no paint, wall, decoration or object slot.
                    assert!(
                        !scene.paints.contains_key(&index),
                        "paint leaked to {plane} {wx},{wy}"
                    );
                    assert!(
                        !scene.walls.contains_key(&index),
                        "wall leaked to {plane} {wx},{wy}"
                    );
                    assert!(!scene.floor_decorations.contains_key(&index));
                    assert!(!scene.wall_decorations.contains_key(&index));
                    assert!(!scene.tile_models.contains_key(&index));
                    for slot in 0..5 {
                        assert!(
                            !scene.slots.contains_key(&(index * 5 + slot)),
                            "object slot leaked to {plane} {wx},{wy}"
                        );
                    }
                }
            }
        }
    }
    assert_eq!(compared, 256, "16x16 declared tiles on plane 0 compared");
    assert!(
        inside_paints >= 84,
        "the Death Office chunks carry terrain ({inside_paints} painted/shaped tiles)"
    );
    // A frame renders; the minimap shows map data only inside the declared chunks.
    inst.set_camera(Camera {
        x: 3172 * 128,
        height: -1200,
        y: 5724 * 128,
        pitch: 2048,
        yaw: 0,
        zoom: 410,
        far: 32768,
    })
    .unwrap();
    inst.build_frame(0.0).unwrap();
    let pixels = rasterize(&inst, &textures);
    common::write_png("instance-death-office", 1920, 1080, &pixels);
    let surface = inst.minimap_surface().unwrap().clone();
    let minimap_rgb: Vec<i32> = surface
        .rgba
        .chunks(4)
        .map(|p| (i32::from(p[0]) << 16) | (i32::from(p[1]) << 8) | i32::from(p[2]))
        .collect();
    common::write_png(
        "instance-death-office-minimap",
        surface.width as u32,
        surface.height as u32,
        &minimap_rgb,
    );
    let (sx, sy) = surface.margin;
    let mut drawn_outside = 0usize;
    let mut drawn_inside = 0usize;
    for ty in 0..104 {
        for tx in 0..104 {
            let (wx, wy) = (base_x + tx, base_y + ty);
            // Centre pixel of the tile in the 4 px/tile raster (y grows downwards from the top).
            let px = sx + tx * surface.scale + 2;
            let py = surface.height - 1 - (sy + ty * surface.scale + 2);
            let drawn = surface.mask[(py * surface.width + px) as usize] != 0;
            if declared(0, wx, wy) {
                drawn_inside += usize::from(drawn);
            } else {
                drawn_outside += usize::from(drawn);
            }
        }
    }
    assert_eq!(
        drawn_outside, 0,
        "minimap drew {drawn_outside} tiles outside the declared chunks"
    );
    assert!(
        drawn_inside >= 60,
        "minimap shows the declared chunks ({drawn_inside} tiles)"
    );

    // A translated turn-0 mapping: source chunk (396,715) shown at (400,718) equals the source
    // chunk's tiles shifted by (32, 24) tiles.
    let mut moved = core_with_textures();
    load_blocks(&mut moved, &available);
    moved
        .update_world(
            &world(serde_json::json!({"template": "test.translated", "chunks": [
                {"plane": 0, "chunkX": 400, "chunkY": 718, "sourcePlane": 0, "sourceChunkX": 396, "sourceChunkY": 715, "quarterTurns": 0}
            ]})),
            0.0,
        )
        .unwrap();
    moved.assemble_scene(base_x, base_y, false, 0.0).unwrap();
    let moved_scene = moved.scene().unwrap().clone();
    let mut moved_paints = 0usize;
    for lx in 0..8 {
        for ly in 0..8 {
            let src = plain_scene.tile_index(
                0,
                396 * 8 + lx - base_x + plain_scene.offset,
                715 * 8 + ly - base_y + plain_scene.offset,
            );
            let dst = moved_scene.tile_index(
                0,
                400 * 8 + lx - base_x + moved_scene.offset,
                718 * 8 + ly - base_y + moved_scene.offset,
            );
            // A lone 8x8 chunk blends and lights against nothing beyond itself (live instance
            // semantics), so its colours are not the ordinary world's: the tile set is.
            assert_eq!(
                moved_scene.paints.contains_key(&dst) || moved_scene.tile_models.contains_key(&dst),
                plain_scene.paints.contains_key(&src) || plain_scene.tile_models.contains_key(&src),
                "translated tile presence {lx},{ly}"
            );
            moved_paints += usize::from(moved_scene.paints.contains_key(&dst));
            if let (Some(w), Some(p)) = (moved_scene.walls.get(&dst), plain_scene.walls.get(&src)) {
                assert_eq!(w.x - p.x, 32 * 128, "wall x shifted by 32 tiles");
                assert_eq!(w.z - p.z, 24 * 128, "wall z shifted by 24 tiles");
            }
            for corner in [(0, 0), (1, 1)] {
                // A corner is stored with the tile it is the south-west corner of; the lone
                // chunk's far corners belong to undeclared tiles and hold no height (0), as in
                // the original instance loader.
                let inside = lx + corner.0 < 8 && ly + corner.1 < 8;
                assert_eq!(
                    moved_scene.height(
                        0,
                        400 * 8 + lx - base_x + moved_scene.offset + corner.0,
                        718 * 8 + ly - base_y + moved_scene.offset + corner.1
                    ),
                    if inside {
                        plain_scene.height(
                            0,
                            396 * 8 + lx - base_x + plain_scene.offset + corner.0,
                            715 * 8 + ly - base_y + plain_scene.offset + corner.1,
                        )
                    } else {
                        0
                    },
                    "translated corner {lx},{ly} {corner:?}"
                );
            }
        }
    }
    assert!(moved_paints > 0);
    let moved_src = moved_scene.tile_index(
        0,
        396 * 8 - base_x + moved_scene.offset,
        715 * 8 - base_y + moved_scene.offset,
    );
    assert!(
        !moved_scene.paints.contains_key(&moved_src),
        "the source position itself is unloaded"
    );

    // A turned chunk is rejected, naming the mapping.
    let mut turned = core_with_textures();
    load_blocks(&mut turned, &[12633]);
    turned
        .update_world(
            &world(serde_json::json!({"template": "test.turned", "chunks": [
                {"plane": 0, "chunkX": 396, "chunkY": 715, "sourcePlane": 0, "sourceChunkX": 396, "sourceChunkY": 715, "quarterTurns": 2}
            ]})),
            0.0,
        )
        .unwrap();
    let error = turned
        .assemble_scene(base_x, base_y, false, 0.0)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("turned 2 quarter turns is not supported"),
        "{error}"
    );
    // Back to the ordinary world: `null` clears the layout and asks for reassembly.
    inst.update_world(&world(serde_json::Value::Null), 0.0)
        .unwrap();
    assert!(inst.instance_layout_changed());
    assert_eq!(inst.squares_needed(base_x, base_y), squares);
}

/// The assembly-time terrain pass (`scene::terrain`, the `rl4.ad` port over the squares' raw
/// terrain) reproduces the directly exported per-base scene tile for tile over the whole
/// 104×104 main area — every paint colour, every shaped tile model vertex/colour/texture and
/// every corner height on all four planes, outer five tiles included — for each of the five
/// fixture bases. The direct export is the original loader's own output at that base, so this
/// is the exactness oracle of the port; before it, the blocks' lit tiles differed in the band.
#[test]
#[ignore = "needs local exports: export.py --profile blocks (with BTER raw terrain) and --profile scenes-pinned"]
fn terrain_pass_reproduces_the_direct_scene_tiles() {
    let floors = common::read_asset("terrain/floors.bin");
    let mut compared_paints = 0usize;
    let mut compared_models = 0usize;
    for fixture in &FIXTURES {
        let squares = RendererCore::squares_for_base(fixture.base.0, fixture.base.1);
        let mut direct = core_with_textures();
        direct
            .load_scene(
                fixture.name,
                &common::read_local_export(
                    &format!("scenes/{}.pinned.bin", fixture.name),
                    REPRODUCE_PINNED,
                ),
                &common::read_local_export(
                    &format!("scenes/{}.pinned.models.bin", fixture.name),
                    REPRODUCE_PINNED,
                ),
            )
            .unwrap();
        let mut assembled = core_with_textures();
        assembled.load_floor_defs(&floors).unwrap();
        load_blocks(&mut assembled, &squares);
        let missing = assembled
            .assemble_scene(fixture.base.0, fixture.base.1, false, 0.0)
            .unwrap();
        assert!(
            missing.is_empty(),
            "{}: blocks missing {missing:?}",
            fixture.name
        );
        let stats = assembled
            .terrain_rebuilt()
            .unwrap_or_else(|| {
                panic!(
                    "{}: terrain was not rebuilt from raw block terrain: {:?}",
                    fixture.name,
                    assembled.unknown_motions()
                )
            })
            .clone();
        assert_eq!(stats.missing_overlays, 0, "{}: {stats:?}", fixture.name);
        assert_eq!(stats.missing_underlays, 0, "{}: {stats:?}", fixture.name);
        let expected = direct.scene().unwrap();
        let actual = assembled.scene().unwrap();
        assert_eq!(expected.offset, actual.offset);
        let mut differences: Vec<String> = Vec::new();
        for plane in 0..4 {
            for sx in 0..104 {
                for sy in 0..104 {
                    let ex = sx + expected.offset;
                    let ey = sy + expected.offset;
                    let index = expected.tile_index(plane, ex, ey);
                    if expected.paints.get(&index) != actual.paints.get(&index) {
                        differences.push(format!(
                            "plane {plane} tile {sx},{sy}: paint direct {:?} vs rebuilt {:?}",
                            expected.paints.get(&index),
                            actual.paints.get(&index)
                        ));
                    } else if expected.paints.contains_key(&index) {
                        compared_paints += 1;
                    }
                    if expected.tile_models.get(&index) != actual.tile_models.get(&index) {
                        let (e, a) = (
                            expected.tile_models.get(&index),
                            actual.tile_models.get(&index),
                        );
                        differences.push(format!(
                            "plane {plane} tile {sx},{sy}: tile model direct {} vs rebuilt {}",
                            e.map(|m| format!("shape {} rot {} colours {:?}/{:?}/{:?} xs {:?} ys {:?} zs {:?} tex {:?} rgb {}/{}", m.shape, m.rotation, m.color_a, m.color_b, m.color_c, m.xs, m.ys, m.zs, m.textures, m.underlay_rgb, m.overlay_rgb)).unwrap_or("none".into()),
                            a.map(|m| format!("shape {} rot {} colours {:?}/{:?}/{:?} xs {:?} ys {:?} zs {:?} tex {:?} rgb {}/{}", m.shape, m.rotation, m.color_a, m.color_b, m.color_c, m.xs, m.ys, m.zs, m.textures, m.underlay_rgb, m.overlay_rgb)).unwrap_or("none".into())
                        ));
                    } else if expected.tile_models.contains_key(&index) {
                        compared_models += 1;
                    }
                    let (fe, fa) = (
                        expected.flags[index] & (256 | 512 | 1024),
                        actual.flags[index] & (256 | 512 | 1024),
                    );
                    if fe != fa {
                        differences.push(format!(
                            "plane {plane} tile {sx},{sy}: tile flags direct {fe} vs rebuilt {fa}"
                        ));
                    }
                }
            }
            for sx in 0..=104 {
                for sy in 0..=104 {
                    let (he, ha) = (
                        expected.height(plane, sx + expected.offset, sy + expected.offset),
                        actual.height(plane, sx + actual.offset, sy + actual.offset),
                    );
                    if he != ha {
                        differences.push(format!(
                            "plane {plane} corner {sx},{sy}: height direct {he} vs rebuilt {ha}"
                        ));
                    }
                }
            }
        }
        assert!(
            differences.is_empty(),
            "{}: {} tile differences between the direct export and the rebuilt block scene; first 12:\n  {}",
            fixture.name,
            differences.len(),
            differences
                .iter()
                .take(12)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n  ")
        );
        eprintln!("{}: terrain pass exact ({stats:?})", fixture.name);
    }
    eprintln!(
        "terrain pass: {compared_paints} paints and {compared_models} tile models identical to the direct exports over the five bases"
    );
}

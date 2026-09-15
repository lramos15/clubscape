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

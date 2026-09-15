//! Pixel-exact fixture tests against the original-runtime captures in `assets/reference/osrs240`.
//!
//! These compare the CPU reference rasterizer + original draw path port against the source
//! software renderer output for the approved model fixtures. They are the foundation for the
//! GPU differential tests; they do not by themselves constitute presentation acceptance.

use std::path::{Path, PathBuf};

use clubscape_renderer::model::Model;
use clubscape_renderer::model_draw::{ModelDrawer, ModelScratch};
use clubscape_renderer::palette::Palette;
use clubscape_renderer::raster::software::Software;
use clubscape_renderer::raster::{RasterState, Tri};
use clubscape_renderer::texture::{Texture, TextureSet};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_png_rgb(path: &Path) -> (u32, u32, Vec<i32>) {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info().expect("png info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("png frame");
    let bytes = &buf[..info.buffer_size()];
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        other => panic!("unsupported color type {other:?}"),
    };
    let pixels = bytes
        .chunks_exact(channels)
        .map(|p| ((p[0] as i32) << 16) | ((p[1] as i32) << 8) | p[2] as i32)
        .collect();
    (info.width, info.height, pixels)
}

fn load_palette() -> Palette {
    let data =
        std::fs::read(repo_root().join("assets/compiled/render/palette.bin")).expect("palette.bin");
    Palette::from_chunks(&data).expect("palette")
}

fn load_textures() -> TextureSet {
    let mut set = TextureSet::default();
    let dir = repo_root().join("assets/compiled/render/textures");
    for entry in std::fs::read_dir(&dir).expect("textures dir") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "bin") {
            set.insert(Texture::from_chunks(&std::fs::read(&path).unwrap()).expect("texture"));
        }
    }
    assert!(!set.is_empty());
    set
}

fn load_model(name: &str) -> Model {
    let data = std::fs::read(repo_root().join(format!("assets/compiled/render/models/{name}.bin")))
        .expect("model");
    Model::from_chunks(&data).expect("model parse")
}

struct Diff {
    differing: usize,
    max_channel: i32,
    first: Option<(usize, usize, i32, i32)>,
}

fn compare(width: usize, actual: &[i32], expected: &[i32]) -> Diff {
    let mut diff = Diff {
        differing: 0,
        max_channel: 0,
        first: None,
    };
    for (i, (&a, &e)) in actual.iter().zip(expected).enumerate() {
        let a = a & 0xffffff;
        let e = e & 0xffffff;
        if a != e {
            diff.differing += 1;
            for shift in [16, 8, 0] {
                let d = ((a >> shift & 0xff) - (e >> shift & 0xff)).abs();
                diff.max_channel = diff.max_channel.max(d);
            }
            if diff.first.is_none() {
                diff.first = Some((i % width, i / width, a, e));
            }
        }
    }
    diff
}

fn render_legacy(
    model: &Model,
    yaw: i32,
    y_camera: i32,
    z_camera: i32,
    palette: &Palette,
    textures: &TextureSet,
) -> Vec<i32> {
    let state = RasterState::new(1920, 1080, 1024);
    let mut pixels = vec![0x303030; 1920 * 1080];
    let mut scratch = ModelScratch::default();
    let mut tris: Vec<Tri> = Vec::new();
    let mut drawer = ModelDrawer {
        state,
        palette: &palette.rgb,
        scratch: &mut scratch,
        alpha_pass: 2,
    };
    drawer
        .draw_legacy(model, 0, yaw, 0, 128, 0, y_camera, z_camera, 0, &mut tris)
        .expect("draw");
    assert!(!tris.is_empty(), "no triangles emitted");
    let mut raster = Software::new(state, &mut pixels, &palette.rgb, textures);
    for tri in &tris {
        raster.draw(tri).expect("raster");
    }
    pixels
}

fn assert_exact(name: &str, actual: &[i32]) {
    let (w, h, expected) =
        read_png_rgb(&repo_root().join(format!("assets/reference/osrs240/models/{name}.png")));
    assert_eq!((w, h), (1920, 1080));
    let diff = compare(w as usize, actual, &expected);
    assert_eq!(
        diff.differing, 0,
        "{name}: {} pixels differ (max channel error {}), first at {:?}",
        diff.differing, diff.max_channel, diff.first
    );
}

#[test]
fn tree_1277_all_yaws_match_source_pixels() {
    let palette = load_palette();
    let textures = load_textures();
    let model = load_model("object-1277-model-1570-lit");
    assert_eq!((model.vertex_count, model.face_count), (90, 110));
    for yaw in [0, 256, 512, 1024] {
        let pixels = render_legacy(&model, yaw, 250, 750, &palette, &textures);
        assert_exact(&format!("tree-1277-yaw-{yaw}"), &pixels);
    }
}

#[test]
fn baked_goblin_frames_match_source_pixels() {
    let palette = load_palette();
    let textures = load_textures();
    for (seq, frames) in [(6181, 16), (6180, 16)] {
        for frame in 0..frames {
            let name = format!("npc-3028-seq-{seq}-frame-{frame}");
            let path = repo_root().join(format!("assets/compiled/render/models/baked/{name}.bin"));
            if !path.exists() {
                eprintln!("skipping {name}: baked frame not exported locally");
                continue;
            }
            let model = Model::from_chunks(&std::fs::read(path).unwrap()).unwrap();
            let pixels = render_legacy(&model, 256, 240, 650, &palette, &textures);
            assert_exact(&format!("npc-3028-sequence-{seq}-frame-{frame}"), &pixels);
        }
    }
}

#[test]
fn baked_penguin_frames_match_source_pixels() {
    let palette = load_palette();
    let textures = load_textures();
    for (seq, frames) in [(5668, 14), (5666, 8)] {
        for frame in 0..frames {
            let name = format!("npc-2063-seq-{seq}-frame-{frame}");
            let path = repo_root().join(format!("assets/compiled/render/models/baked/{name}.bin"));
            if !path.exists() {
                eprintln!("skipping {name}: baked frame not exported locally");
                continue;
            }
            let model = Model::from_chunks(&std::fs::read(path).unwrap()).unwrap();
            let pixels = render_legacy(&model, 256, 160, 400, &palette, &textures);
            assert_exact(&format!("npc-2063-sequence-{seq}-frame-{frame}"), &pixels);
        }
    }
}

#[test]
fn palette_build_matches_exported_palette() {
    let exported = load_palette();
    let built = Palette::build(0.8);
    let mismatches = exported
        .rgb
        .iter()
        .zip(&built.rgb)
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(
        mismatches, 0,
        "palette computation differs from the original export in {mismatches} entries"
    );
}

#[test]
fn trig_tables_match_exported_hash() {
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("assets/compiled/render/manifest.json")).unwrap(),
    )
    .unwrap();
    let expected = manifest["files"]["tables.bin"]["sha256"]
        .as_str()
        .expect("tables hash recorded");
    let bytes = clubscape_renderer::tables::tables_csrc_bytes();
    let actual = sha256_hex(&bytes);
    assert_eq!(
        actual, expected,
        "computed trig tables differ from the original runtime's Perspective tables"
    );
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum");
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string()
}

// ---------------------------------------------------------------- scene fixtures

struct SceneFixture {
    scene: &'static str,
    capture: &'static str,
    camera: [i32; 3],
    pitch: i32,
    yaw: i32,
    focal_tile: [i32; 2],
    base: [i32; 2],
}

const SCENE_FIXTURES: &[SceneFixture] = &[
    SceneFixture {
        scene: "tutorial-starting-house",
        capture: "tutorial-starting-house",
        camera: [5888, -2360, 4992],
        pitch: 2048,
        yaw: 0,
        focal_tile: [3094, 3103],
        base: [3048, 3056],
    },
    SceneFixture {
        scene: "tutorial-survival-coast",
        capture: "tutorial-survival-coast",
        camera: [7296, 0, 5632],
        pitch: 2048,
        yaw: 1024,
        focal_tile: [3101, 3085],
        base: [3048, 3032],
    },
    SceneFixture {
        scene: "lumbridge-castle-plaza",
        capture: "lumbridge-castle-plaza",
        camera: [6912, 0, 5120],
        pitch: 2048,
        yaw: 0,
        focal_tile: [3222, 3218],
        base: [3168, 3168],
    },
    SceneFixture {
        scene: "lumbridge-river-bridge",
        capture: "lumbridge-river-bridge",
        camera: [8576, 0, 4736],
        pitch: 2048,
        yaw: 2048,
        focal_tile: [3223, 3217],
        base: [3168, 3168],
    },
    SceneFixture {
        scene: "lumbridge-windmill-route",
        capture: "lumbridge-windmill-route",
        camera: [6912, 0, 7040],
        pitch: 2048,
        yaw: 1536,
        focal_tile: [3166, 3306],
        base: [3120, 3240],
    },
];

fn load_scene(name: &str) -> (clubscape_renderer::scene::SceneData, Vec<Option<Model>>) {
    let data = std::fs::read(repo_root().join(format!("assets/compiled/render/scenes/{name}.bin")))
        .expect("scene file");
    let scene = clubscape_renderer::scene::SceneData::from_chunks(&data).expect("scene parse");
    let pack =
        std::fs::read(repo_root().join(format!("assets/compiled/render/scenes/{name}.models.bin")))
            .expect("scene model pack");
    let entries = clubscape_renderer::model::parse_model_pack(&pack).expect("model pack parse");
    assert_eq!(entries.len(), scene.model_keys.len());
    let models = entries
        .into_iter()
        .zip(&scene.model_keys)
        .map(|((key, model), expected)| {
            assert_eq!(
                &key, expected,
                "model pack order differs from scene model keys"
            );
            Some(model)
        })
        .collect();
    (scene, models)
}

fn fixture_camera(fixture: &SceneFixture, manifest: &serde_json::Value) -> [i32; 3] {
    // Camera height comes from the recorded native readback (source focal ground + offset).
    let inputs = manifest["original_inputs"].as_array().expect("inputs");
    let record = inputs
        .iter()
        .find(|i| i["id"].as_str() == Some(&format!("original.scenes.{}", fixture.capture)))
        .expect("fixture record");
    let cam = record["settings"]["camera_local_units"]
        .as_array()
        .expect("camera");
    [
        cam[0].as_i64().unwrap() as i32,
        cam[1].as_i64().unwrap() as i32,
        cam[2].as_i64().unwrap() as i32,
    ]
}

fn render_scene_fixture(fixture: &SceneFixture) -> (Vec<i32>, usize) {
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("research/reference-pack/v1/manifest.json"))
            .unwrap(),
    )
    .unwrap();
    let camera = fixture_camera(fixture, &manifest);
    let palette = load_palette();
    let textures = load_textures();
    let (scene, models) = load_scene(fixture.scene);
    let state = RasterState::new(1920, 1080, 662);
    let mut drawer =
        clubscape_renderer::scene::draw::SceneDrawer::new(&scene, state, &palette.rgb, 32768);
    let view = clubscape_renderer::scene::draw::SceneView {
        camera_x: camera[0],
        camera_height: camera[1],
        camera_z: camera[2],
        pitch: fixture.pitch,
        yaw: fixture.yaw,
        plane: 0,
        focal_x: (fixture.focal_tile[0] - fixture.base[0]) * 128,
        focal_z: (fixture.focal_tile[1] - fixture.base[1]) * 128,
        center_on_camera: true,
        far_clip: 32768,
    };
    let mut tris: Vec<Tri> = Vec::new();
    drawer.begin_frame(&scene);
    drawer.draw(&scene, &models, &[], &view, &mut tris);
    assert!(
        drawer.missing_models.is_empty(),
        "missing models: {:?}",
        drawer.missing_models
    );
    assert!(
        tris.len() > 10000,
        "scene emitted only {} triangles",
        tris.len()
    );
    let mut pixels = vec![0i32; 1920 * 1080];
    let mut raster = Software::new(state, &mut pixels, &palette.rgb, &textures);
    // Aborted fills mirror the original swallowed ArrayIndexOutOfBounds; the pixel comparison
    // below is the check that matters.
    for tri in &tris {
        let _ = raster.draw(tri);
    }
    let _ = fixture.camera;
    (pixels, tris.len())
}

fn write_debug_png(name: &str, width: u32, height: u32, pixels: &[i32]) {
    let dir = repo_root().join(".local/render-assets/test-output");
    std::fs::create_dir_all(&dir).unwrap();
    let file = std::fs::File::create(dir.join(format!("{name}.png"))).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let bytes: Vec<u8> = pixels
        .iter()
        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
        .collect();
    writer.write_image_data(&bytes).unwrap();
}

fn compare_scene(fixture: &SceneFixture) {
    let (pixels, tri_count) = render_scene_fixture(fixture);
    let (w, h, expected) = read_png_rgb(&repo_root().join(format!(
        "assets/reference/osrs240/scenes/{}.png",
        fixture.capture
    )));
    assert_eq!((w, h), (1920, 1080));
    let diff = compare(w as usize, &pixels, &expected);
    write_debug_png(&format!("{}-candidate", fixture.capture), w, h, &pixels);
    let mut diffmap = vec![0i32; pixels.len()];
    for (i, slot) in diffmap.iter_mut().enumerate() {
        *slot = if (pixels[i] & 0xffffff) != (expected[i] & 0xffffff) {
            0xff0000
        } else {
            (expected[i] & 0xffffff) / 4
        };
    }
    write_debug_png(&format!("{}-diff", fixture.capture), w, h, &diffmap);
    eprintln!(
        "{}: {} triangles, {} differing pixels, max channel {}",
        fixture.capture, tri_count, diff.differing, diff.max_channel
    );
    assert_eq!(
        diff.differing, 0,
        "{}: {} pixels differ (max channel error {}), first at {:?}",
        fixture.capture, diff.differing, diff.max_channel, diff.first
    );
}

#[test]
fn scene_tutorial_starting_house_matches_source_pixels() {
    compare_scene(&SCENE_FIXTURES[0]);
}

#[test]
fn scene_tutorial_survival_coast_matches_source_pixels() {
    compare_scene(&SCENE_FIXTURES[1]);
}

#[test]
fn scene_lumbridge_castle_plaza_matches_source_pixels() {
    compare_scene(&SCENE_FIXTURES[2]);
}

#[test]
fn scene_lumbridge_river_bridge_matches_source_pixels() {
    compare_scene(&SCENE_FIXTURES[3]);
}

#[test]
fn scene_lumbridge_windmill_route_matches_source_pixels() {
    compare_scene(&SCENE_FIXTURES[4]);
}

#[test]
fn scene_models_report_alpha_254_flat_faces() {
    // The original flat fill treats alpha 254 as a one-pixel shift copy; the GPU path needs to
    // know whether any exported scene face uses it.
    let mut flat_254 = 0usize;
    let mut total_alpha = 0usize;
    for fixture in SCENE_FIXTURES {
        let (_, models) = load_scene(fixture.scene);
        for model in models.iter().flatten() {
            if let Some(alphas) = &model.alphas {
                for f in 0..model.face_count {
                    if alphas[f] != 0 {
                        total_alpha += 1;
                    }
                    if alphas[f] == -2
                        && model.color_c[f] == -1
                        && model.textures.as_ref().map(|t| t[f] == -1).unwrap_or(true)
                    {
                        flat_254 += 1;
                    }
                }
            }
        }
    }
    eprintln!("alpha faces: {total_alpha}, flat alpha-254 faces: {flat_254}");
}

// ---------------------------------------------------------------- core with entities

#[test]
fn core_places_world_view_entities_in_scene() {
    use clubscape_renderer::core::{Camera, RendererCore};
    let root = repo_root();
    let scene_path = root.join("assets/compiled/render/scenes/tutorial-starting-house.bin");
    if !scene_path.exists() {
        eprintln!("skipping: scene export not present");
        return;
    }
    let palette = load_palette();
    let mut core = RendererCore::new(palette, 1920, 1080);
    for entry in std::fs::read_dir(root.join("assets/compiled/render/textures")).unwrap() {
        core.add_texture(&std::fs::read(entry.unwrap().path()).unwrap())
            .unwrap();
    }
    core.load_npc_pack_as(
        3028,
        &std::fs::read(root.join("assets/compiled/render/models/npc-3028.pack.bin")).unwrap(),
    )
    .unwrap();
    core.load_npc_pack_as(
        2063,
        &std::fs::read(root.join("assets/compiled/render/models/npc-2063.pack.bin")).unwrap(),
    )
    .unwrap();
    core.load_scene(
        "tutorial-starting-house",
        &std::fs::read(&scene_path).unwrap(),
        &std::fs::read(
            root.join("assets/compiled/render/scenes/tutorial-starting-house.models.bin"),
        )
        .unwrap(),
    )
    .unwrap();
    // Fixture camera expressed in world units (tile 3094,3095 base 3048,3056).
    core.set_camera(Camera {
        x: 3048 * 128 + 5888,
        height: -2360,
        y: 3056 * 128 + 4992,
        pitch: 2048,
        yaw: 0,
        zoom: 662,
        far: 32768,
    })
    .unwrap();
    let baseline = core.build_frame(0.0).unwrap().triangles;
    let baseline_pixels = {
        let textures = load_textures();
        let mut pixels = vec![0i32; 1920 * 1080];
        let mut raster = Software::new(core.state, &mut pixels, &core.palette.rgb, &textures);
        for tri in core.triangles() {
            let _ = raster.draw(tri);
        }
        pixels
    };
    let world = serde_json::json!({
        "revision": "1", "tick": "1",
        "player": {"id": "player-1", "tile": {"x": 3098, "y": 3098, "plane": 0}, "animation": "5668"},
        "entities": [
            {"id": "npc-goblin", "kind": "npc", "sourceId": 3028, "tile": {"x": 3097, "y": 3097, "plane": 0}, "animation": "6180"},
            {"id": "npc-unknown", "kind": "npc", "sourceId": 3308, "tile": {"x": 3092, "y": 3103, "plane": 0}, "animation": "808"}
        ]
    });
    core.update_world(&world.to_string(), 1000.0).unwrap();
    let summary = core.build_frame(1000.0).unwrap().clone();
    eprintln!(
        "summary: drawn {} skipped {:?} missing {}",
        summary.entities_drawn, summary.entities_skipped, summary.missing_models
    );
    assert_eq!(summary.entities_drawn, 2, "{:?}", summary.entities_skipped);
    assert_eq!(
        summary.entities_skipped.len(),
        1,
        "unknown npc must be reported, not hidden"
    );
    assert!(summary.triangles > baseline, "entities added no triangles");
    // Animation advances: walk frame index changes over time within the source frame lengths.
    let textures = load_textures();
    let mut frames = Vec::new();
    for t in [1000.0, 1000.0 + 20.0 * 3.0, 1000.0 + 20.0 * 8.0] {
        core.build_frame(t).unwrap();
        let mut pixels = vec![0i32; 1920 * 1080];
        let state = core.state;
        let mut raster = Software::new(state, &mut pixels, &core.palette.rgb, &textures);
        for tri in core.triangles() {
            let _ = raster.draw(tri);
        }
        frames.push(pixels);
    }
    assert_ne!(frames[0], frames[2], "walk animation did not advance");
    {
        let mut bbox = (i32::MAX, i32::MAX, i32::MIN, i32::MIN, 0usize);
        for (i, (&a, &b)) in frames[0].iter().zip(&baseline_pixels).enumerate() {
            if a != b {
                let (x, y) = ((i % 1920) as i32, (i / 1920) as i32);
                bbox = (
                    bbox.0.min(x),
                    bbox.1.min(y),
                    bbox.2.max(x),
                    bbox.3.max(y),
                    bbox.4 + 1,
                );
            }
        }
        eprintln!("visible entity pixels vs baseline: {bbox:?}");
    }
    write_debug_png("tutorial-starting-house-entities", 1920, 1080, &frames[0]);
    // Picking returns the entity under a pixel it covers and tiles elsewhere.
    let mut boxes: std::collections::HashMap<i64, (i32, i32, i32, i32)> =
        std::collections::HashMap::new();
    for y in 0..1080 {
        for x in 0..1920 {
            if let Some(clubscape_renderer::scene::draw::PickTarget::Object { hash, .. }) =
                core.pick(x, y)
                && (!(0..0x1000_0000_0000).contains(&hash))
            {
                let b = boxes.entry(hash).or_insert((x, y, x, y));
                b.0 = b.0.min(x);
                b.1 = b.1.min(y);
                b.2 = b.2.max(x);
                b.3 = b.3.max(y);
            }
        }
    }
    eprintln!("entity pick boxes: {boxes:?}");
    {
        let picks = core.pick_targets().to_vec();
        let mut per: std::collections::HashMap<i64, (usize, i32, i32, i32, i32)> =
            std::collections::HashMap::new();
        for tri in core.triangles() {
            if tri.pick == 0 {
                continue;
            }
            if let clubscape_renderer::scene::draw::PickTarget::Object { hash, .. } =
                picks[tri.pick as usize - 1]
                && (!(0..0x1000_0000_0000).contains(&hash))
            {
                let e = per
                    .entry(hash)
                    .or_insert((0, i32::MAX, i32::MAX, i32::MIN, i32::MIN));
                e.0 += 1;
                for i in 0..3 {
                    e.1 = e.1.min(tri.x[i]);
                    e.2 = e.2.min(tri.y[i]);
                    e.3 = e.3.max(tri.x[i]);
                    e.4 = e.4.max(tri.y[i]);
                }
            }
        }
        eprintln!("entity triangles: {per:?}");
    }
    let pick_center = core.pick(960, 540);
    assert!(
        pick_center.is_some(),
        "center pixel should resolve to scene geometry"
    );
    let mut found_entity = false;
    for y in (300..900).step_by(4) {
        for x in (600..1300).step_by(4) {
            if let Some(clubscape_renderer::scene::draw::PickTarget::Object { hash, .. }) =
                core.pick(x, y)
                && (!(0..0x1000_0000_0000).contains(&hash))
            {
                found_entity = true;
            }
        }
    }
    assert!(found_entity, "no pixel resolved to an entity");
    assert_eq!(
        boxes.len(),
        2,
        "both the goblin and the penguin player must be pickable on open ground: {boxes:?}"
    );
}

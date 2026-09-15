//! Candidate comparisons against the independent original dynamic-layer references
//! (`assets/reference/osrs240/m1-dynamic`, controlled offline original-client renderings):
//! door orientations, ground-item piles, fire frames, roof planes and the plane-1 view, each
//! rebuilt from the same inputs (fixture scene, source object/item ids, quantities, frame,
//! plane, camera and the native full-HUD zoom 410) and measured with the approved
//! `native_scene_model` profile (interior max channel error <= 2, interior mean <= 0.35, 1 px
//! band around SOURCE edges, band-changed fraction <= 0.5 % of the full image).
//!
//! The source frames are full-HUD compositions: the original HUD panels (minimap, chat, sidebar
//! containers) are painted over the scene. The renderer draws the scene only, so the pixels
//! inside the three native HUD container rectangles are compared separately and reported —
//! they are the UI layer's, not masked scene geometry — and the scene metric covers every other
//! pixel of the frame. Nothing from a source frame is copied into a candidate.

mod common;

use std::path::Path;

use clubscape_renderer::core::{Camera, RendererCore};
use clubscape_renderer::palette::Palette;
use clubscape_renderer::raster::software::Software;
use clubscape_renderer::texture::{Texture, TextureSet};

use common::repo_root;

const REFERENCE_DIR: &str = "assets/reference/osrs240/m1-dynamic";
/// Native full-HUD viewport zoom the cases were rendered with (`client.fk`); the frozen
/// viewport-only fixtures use 662 at the same height — two original projections, not one.
const FULL_HUD_ZOOM: i32 = 410;

fn read_png_rgb(path: &Path) -> (u32, u32, Vec<i32>) {
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    ));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let channels = info.color_type.samples();
    let mut out = Vec::with_capacity((info.width * info.height) as usize);
    for px in buf[..info.buffer_size()].chunks(channels) {
        out.push((i32::from(px[0]) << 16) | (i32::from(px[1]) << 8) | i32::from(px[2]));
    }
    (info.width, info.height, out)
}

fn write_png_rgb(path: &Path, width: u32, height: u32, rgb: &[i32]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    let bytes: Vec<u8> = rgb
        .iter()
        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])
        .collect();
    writer.write_image_data(&bytes).unwrap();
}

struct Inputs {
    manifest: serde_json::Value,
    textures: TextureSet,
}

fn inputs() -> Inputs {
    let manifest: serde_json::Value =
        serde_json::from_slice(&common::read_asset("manifest.json")).unwrap();
    let mut textures = TextureSet::default();
    for bytes in common::texture_bytes() {
        textures.insert(Texture::from_chunks(&bytes).unwrap());
    }
    Inputs { manifest, textures }
}

/// A renderer with the fixture scene, every dynamic object variant and every ground item (with
/// its `lj.es` value inputs) from the published manifest.
fn core_for(inputs: &Inputs, scene: &str) -> RendererCore {
    let manifest = &inputs.manifest;
    let mut core = RendererCore::new(
        Palette::from_chunks(&common::read_asset("palette.bin")).unwrap(),
        1920,
        1080,
    );
    for bytes in common::texture_bytes() {
        core.add_texture(&bytes).unwrap();
    }
    for object in manifest["dynamic_objects"].as_array().unwrap() {
        let id = object["object_id"].as_i64().unwrap() as i32;
        for variant in object["variants"].as_array().unwrap() {
            let frames: Vec<Vec<u8>> = variant["frames"]
                .as_array()
                .map(|f| {
                    f.iter()
                        .map(|p| common::read_asset(p.as_str().unwrap()))
                        .collect()
                })
                .unwrap_or_default();
            let lengths: Vec<i32> = variant["frame_lengths_client_cycles"]
                .as_array()
                .map(|l| l.iter().map(|v| v.as_i64().unwrap() as i32).collect())
                .unwrap_or_default();
            core.load_dynamic_object(
                id,
                variant["type"].as_i64().unwrap() as i32,
                variant["orientation"].as_i64().unwrap() as i32,
                &common::read_asset(variant["model"].as_str().unwrap()),
                &frames,
                lengths,
            )
            .unwrap();
        }
    }
    for item in manifest["ground_items"].as_array().unwrap() {
        let id = item["item_id"].as_i64().unwrap() as i32;
        core.register_ground_item_definition(
            id,
            item["price"]
                .as_i64()
                .expect("manifest ground_items[].price"),
            item["stackable"]
                .as_bool()
                .expect("manifest ground_items[].stackable"),
        );
        for variant in item["variants"].as_array().unwrap() {
            core.load_ground_item(
                id,
                variant["min_quantity"].as_i64().unwrap(),
                &common::read_asset(variant["model"].as_str().unwrap()),
            )
            .unwrap();
        }
    }
    core.load_scene(
        scene,
        &common::read_asset(&format!("scenes/{scene}.bin")),
        &common::read_asset(&format!("scenes/{scene}.models.bin")),
    )
    .unwrap();
    core
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

/// One case record from the reference index.
struct CaseRecord {
    json: serde_json::Value,
}

impl CaseRecord {
    fn load(id: &str) -> Self {
        let path = repo_root().join(REFERENCE_DIR).join(format!("{id}.json"));
        let json: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
        )
        .unwrap();
        CaseRecord { json }
    }
    fn settings(&self) -> &serde_json::Value {
        &self.json["capture"]["settings"]
    }
    fn camera_local(&self) -> [i32; 3] {
        let c = self.settings()["camera_local_units"].as_array().unwrap();
        [
            c[0].as_i64().unwrap() as i32,
            c[1].as_i64().unwrap() as i32,
            c[2].as_i64().unwrap() as i32,
        ]
    }
    fn zoom(&self) -> i32 {
        self.settings()["zoom"].as_i64().unwrap() as i32
    }
    fn draw_plane(&self) -> i32 {
        self.settings()["source_scene_draw_plane"].as_i64().unwrap() as i32
    }
    fn source_plane(&self) -> i32 {
        self.settings()["source_plane"].as_i64().unwrap() as i32
    }
    fn frame_path(&self) -> std::path::PathBuf {
        repo_root()
            .join(REFERENCE_DIR)
            .join(self.json["capture"]["path"].as_str().unwrap())
    }
    /// Native HUD container rectangles painted over the scene in the source frame.
    fn hud_rects(&self) -> Vec<(String, [i32; 4])> {
        self.settings()["native_ui_regions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["name"].as_str().unwrap().starts_with("root161:"))
            .map(|r| {
                let b = r["bounds"].as_array().unwrap();
                (
                    r["name"].as_str().unwrap().to_string(),
                    [
                        b[0].as_i64().unwrap() as i32,
                        b[1].as_i64().unwrap() as i32,
                        b[2].as_i64().unwrap() as i32,
                        b[3].as_i64().unwrap() as i32,
                    ],
                )
            })
            .collect()
    }
    fn hide_roofs(&self) -> bool {
        self.json["input"]["hide_roofs"].as_bool().unwrap()
    }
    fn stock_plane_selector(&self) -> Option<i64> {
        self.json["capture"]["source"]["native_operations"]
            .as_array()?
            .iter()
            .find(|op| op["entrypoint"] == "cz.ch")?["normal_camera_stock_plane_selector"]
            .as_i64()
    }
}

fn scene_base(scene: &str) -> (i32, i32) {
    match scene {
        "tutorial-starting-house" => (3048, 3056),
        "lumbridge-castle-plaza" => (3168, 3168),
        other => panic!("unknown fixture scene {other}"),
    }
}

fn camera(case: &CaseRecord, scene: &str) -> Camera {
    let (bx, by) = scene_base(scene);
    let [x, height, z] = case.camera_local();
    assert_eq!(
        case.zoom(),
        FULL_HUD_ZOOM,
        "case zoom is the native full-HUD projection"
    );
    assert_eq!(case.settings()["pitch"], 2048);
    assert_eq!(case.settings()["yaw"], 0);
    Camera {
        x: bx * 128 + x,
        height,
        y: by * 128 + z,
        pitch: 2048,
        yaw: 0,
        zoom: FULL_HUD_ZOOM,
        far: 32768,
    }
}

/// `native_scene_model` metric over the scene pixels (outside the HUD container rectangles),
/// with the HUD rectangles accounted separately.
#[derive(Debug, Default, Clone)]
struct Metric {
    pixels: usize,
    scene_pixels: usize,
    hud_pixels: usize,
    hud_changed: usize,
    scene_identical: usize,
    band_pixels: usize,
    interior_pixels: usize,
    interior_changed: usize,
    interior_max: i32,
    interior_mean: f64,
    band_changed: usize,
    band_changed_fraction: f64,
    max_anywhere_scene: i32,
    first_interior: Vec<(i32, i32, i32, i32)>,
}

impl Metric {
    fn passed(&self) -> bool {
        self.interior_max <= 2 && self.interior_mean <= 0.35 && self.band_changed_fraction <= 0.005
    }
}

fn metric(
    candidate: &[i32],
    source: &[i32],
    w: usize,
    h: usize,
    hud: &[(String, [i32; 4])],
    diff_out: Option<&Path>,
) -> Metric {
    const EDGE_STEP: i32 = 24;
    let ch = |p: i32, s: u32| (p >> s) & 255;
    let in_hud = |x: usize, y: usize| {
        hud.iter().any(|(_, [rx, ry, rw, rh])| {
            (x as i32) >= *rx && (x as i32) < rx + rw && (y as i32) >= *ry && (y as i32) < ry + rh
        })
    };
    // Source edges: neighbouring source pixels differing by more than EDGE_STEP in any channel.
    let mut edge = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if x + 1 < w {
                let j = i + 1;
                if [16, 8, 0]
                    .iter()
                    .any(|&s| (ch(source[i], s) - ch(source[j], s)).abs() > EDGE_STEP)
                {
                    edge[i] = true;
                    edge[j] = true;
                }
            }
            if y + 1 < h {
                let j = i + w;
                if [16, 8, 0]
                    .iter()
                    .any(|&s| (ch(source[i], s) - ch(source[j], s)).abs() > EDGE_STEP)
                {
                    edge[i] = true;
                    edge[j] = true;
                }
            }
        }
    }
    // Band = edges dilated by one pixel (4-neighbourhood, as compare.py grows it).
    let mut band = edge.clone();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if edge[i] {
                if x > 0 {
                    band[i - 1] = true;
                }
                if x + 1 < w {
                    band[i + 1] = true;
                }
                if y > 0 {
                    band[i - w] = true;
                }
                if y + 1 < h {
                    band[i + w] = true;
                }
            }
        }
    }
    let mut m = Metric {
        pixels: w * h,
        ..Metric::default()
    };
    let mut interior_sum: f64 = 0.0;
    let mut vis = if diff_out.is_some() {
        vec![0i32; w * h]
    } else {
        Vec::new()
    };
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let err = [16, 8, 0]
                .iter()
                .map(|&s| (ch(candidate[i], s) - ch(source[i], s)).abs())
                .max()
                .unwrap();
            let errs: i32 = [16, 8, 0]
                .iter()
                .map(|&s| (ch(candidate[i], s) - ch(source[i], s)).abs())
                .sum();
            if !vis.is_empty() {
                let gray = ((ch(source[i], 16) + ch(source[i], 8) + ch(source[i], 0)) / 9) & 255;
                vis[i] = (gray << 16) | (gray << 8) | gray;
            }
            if in_hud(x, y) {
                m.hud_pixels += 1;
                if err > 0 {
                    m.hud_changed += 1;
                    if !vis.is_empty() {
                        vis[i] = 0x0040A0;
                    }
                }
                continue;
            }
            m.scene_pixels += 1;
            if err == 0 {
                m.scene_identical += 1;
            }
            m.max_anywhere_scene = m.max_anywhere_scene.max(err);
            if band[i] {
                m.band_pixels += 1;
                if err > 0 {
                    m.band_changed += 1;
                    if !vis.is_empty() {
                        vis[i] = 0xFFC800;
                    }
                }
            } else {
                m.interior_pixels += 1;
                interior_sum += f64::from(errs) / 3.0;
                if err > 0 {
                    m.interior_changed += 1;
                    m.interior_max = m.interior_max.max(err);
                    if m.first_interior.len() < 5 {
                        m.first_interior
                            .push((x as i32, y as i32, candidate[i], source[i]));
                    }
                    if !vis.is_empty() {
                        vis[i] = 0xFF0000;
                    }
                }
            }
        }
    }
    m.interior_mean = if m.interior_pixels > 0 {
        interior_sum / m.interior_pixels as f64
    } else {
        0.0
    };
    m.band_changed_fraction = m.band_changed as f64 / (w * h) as f64;
    if let Some(path) = diff_out {
        write_png_rgb(path, w as u32, h as u32, &vis);
    }
    m
}

fn tile(x: i32, y: i32, plane: i32) -> String {
    format!(r#"{{"x":{x},"y":{y},"plane":{plane}}}"#)
}

fn world(
    player: (i32, i32, i32),
    entities: &str,
    ground_items: &str,
    dynamic_objects: &str,
) -> String {
    format!(
        r#"{{"revision":"case","tick":"0","player":{{"id":"player-case","tile":{},"animation":"808","instance":null}},"entities":[{entities}],"groundItems":[{ground_items}],"dynamicObjects":[{dynamic_objects}]}}"#,
        tile(player.0, player.1, player.2)
    )
}

struct Outcome {
    id: String,
    metric: Metric,
    triangles: usize,
    skipped: Vec<String>,
    stock_selector: Option<(i64, i32)>,
}

fn report_line(o: &Outcome) -> String {
    let m = &o.metric;
    format!(
        "{}: {} | frame px {} | scene px {} identical {} ({:.3}%) | interior max {} mean {:.4} changed {} | band changed {} ({:.5} of frame) | hud px {} (changed {}) | tris {} | skipped {:?} | first interior diffs {:?} | stock cz.ch expected/candidate {:?}",
        o.id,
        if m.passed() { "PASS" } else { "FAIL" },
        m.pixels,
        m.scene_pixels,
        m.scene_identical,
        100.0 * m.scene_identical as f64 / m.scene_pixels.max(1) as f64,
        m.interior_max,
        m.interior_mean,
        m.interior_changed,
        m.band_changed,
        m.band_changed_fraction,
        m.hud_pixels,
        m.hud_changed,
        o.triangles,
        o.skipped,
        m.first_interior
            .iter()
            .map(|(x, y, c, s)| format!("({x},{y}) {c:06x}/{s:06x}"))
            .collect::<Vec<_>>(),
        o.stock_selector
    )
}

fn door_view(orientation: i32) -> String {
    world(
        (3094, 3103, 0),
        "",
        "",
        &format!(
            r#"{{"id":"door-9398","sourceId":9398,"tile":{},"instance":null,"doorOpen":{},"quarterTurns":{orientation}}}"#,
            tile(3098, 3107, 0),
            orientation != 0
        ),
    )
}

fn ground_view(items: &[(i32, i64)]) -> String {
    let list: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(i, (id, qty))| {
            format!(
                r#"{{"id":"g{i}","tile":{},"item":{{"id":"item.{id}","name":"{id}","quantity":{qty},"sourceId":{id}}},"canTake":true}}"#,
                tile(3221, 3217, 0)
            )
        })
        .collect();
    world((3222, 3218, 0), "", &list.join(","), "")
}

fn fire_view() -> String {
    world(
        (3222, 3218, 0),
        &format!(
            r#"{{"id":"fire","definitionId":"asset.source.osrs.cache2695.object.26185","sourceId":26185,"name":"Fire","kind":"temporary_object","tile":{},"instance":null,"hitpoints":0,"maxHitpoints":0,"available":true,"animation":"","actions":[],"appearance":{{}},"equipment":[]}}"#,
            tile(3221, 3217, 0)
        ),
        "",
        "",
    )
}

/// The Tutorial plane-0 frames show animated flames — object 24969 (a 5-frame flame at
/// 3095,3102) and object 196 wall torches (5-frame sets at 3096,3105 and 3096,3110) — whose
/// start phases the original picks with `Math.random()` per placement at scene load. The
/// independent source phase sidecar (`assets/reference/osrs240/m1-dynamic/phases`, read-only
/// observations of the ACTIVE controller `dy.ac` in byte-identical replays of the three cases)
/// records them: seq 477 frame 4, seq 481 frame 3, seq 481 frame 2, all at cycle 0 within the
/// frame, drawn at client cycle 0 with `lastUpdate` −1 (one cycle of advance, after which the
/// controller holds frame/cycle 1). The static fixture scene export bakes one frame per flame, so
/// those cases are matched on the block-assembled scene (every frame baked) at exactly that
/// recorded state — never at a frame guessed from the reference pixels.
const ANIMATED_PLACEMENTS: [(i32, i32, i32); 3] =
    [(24969, 3095, 3102), (196, 3096, 3105), (196, 3096, 3110)];

const PHASE_INDEX_SHA256: &str = "b54a72df1eaa3777a87ae0f4360aca4f6be544a765ab00e4f306c7f9ed2d0d16";

/// One original observation of an animated placement's active controller.
#[derive(Debug, Clone, PartialEq)]
struct SourcePhase {
    object: i32,
    tile: (i32, i32, i32),
    sequence: i32,
    frame: i32,
    frame_cycle: i32,
    frame_lengths: Vec<i32>,
    last_update_cycle: i64,
    source_cycle: i64,
}

/// The recorded `before-original-draw` and `after-original-draw` controller states of a case,
/// with the sidecar verified against the phase index and the phase index against the case
/// index it was recorded for.
fn source_phases(case_id: &str) -> (Vec<SourcePhase>, Vec<SourcePhase>) {
    let root = repo_root().join("assets/reference/osrs240/m1-dynamic");
    let index_bytes = std::fs::read(root.join("phases/phase-index.json")).expect("phase index");
    assert_eq!(
        common::sha256_hex(&index_bytes),
        PHASE_INDEX_SHA256,
        "phase-index.json is not the recorded sidecar index"
    );
    let index: serde_json::Value = serde_json::from_slice(&index_bytes).unwrap();
    let case_index_bytes = std::fs::read(root.join("case-index.json")).unwrap();
    assert_eq!(
        common::sha256_hex(&case_index_bytes),
        index["original_case_index"]["sha256"].as_str().unwrap(),
        "phase sidecar was recorded against another case index"
    );
    let entry = index["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["case_id"] == case_id)
        .unwrap_or_else(|| panic!("{case_id}: no phase sidecar"));
    let sidecar_path = repo_root().join(entry["sidecar"]["path"].as_str().unwrap());
    let sidecar_bytes = std::fs::read(&sidecar_path).unwrap();
    assert_eq!(
        common::sha256_hex(&sidecar_bytes),
        entry["sidecar"]["sha256"].as_str().unwrap(),
        "{case_id}: sidecar hash"
    );
    let sidecar: serde_json::Value = serde_json::from_slice(&sidecar_bytes).unwrap();
    assert_eq!(sidecar["candidate_images_or_frame_guesses_read"], false);
    assert_eq!(sidecar["animation_state_modified_by_probe"], false);
    // The replay reproduced the published image byte for byte, so its observations describe
    // exactly the state that drew it.
    let case = CaseRecord::load(case_id);
    assert_eq!(
        sidecar["source_image_replay"]["png_sha256"], case.json["capture"]["sha256"],
        "{case_id}: replay image differs from the published case image"
    );
    let parse = |o: &serde_json::Value| SourcePhase {
        object: o["source_object_id"].as_i64().unwrap() as i32,
        tile: (
            o["world_tile"][0].as_i64().unwrap() as i32,
            o["world_tile"][1].as_i64().unwrap() as i32,
            o["world_tile"][2].as_i64().unwrap() as i32,
        ),
        sequence: o["sequence_id"].as_i64().unwrap() as i32,
        frame: o["active_controller"]["frame"].as_i64().unwrap() as i32,
        frame_cycle: o["active_controller"]["frame_cycle"].as_i64().unwrap() as i32,
        frame_lengths: o["active_controller"]["frame_lengths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect(),
        last_update_cycle: o["last_update_cycle"].as_i64().unwrap(),
        source_cycle: o["source_cycle"].as_i64().unwrap(),
    };
    let observations = sidecar["observations"].as_array().unwrap();
    let phase = |name: &str| -> Vec<SourcePhase> {
        observations
            .iter()
            .filter(|o| o["observation_phase"] == name && o["rendering_controller"] == "dy.ac")
            .map(parse)
            .collect()
    };
    let before = phase("before-original-draw");
    let after = phase("after-original-draw");
    assert_eq!(
        before.len(),
        ANIMATED_PLACEMENTS.len(),
        "{case_id}: before-draw observations"
    );
    assert_eq!(
        after.len(),
        ANIMATED_PLACEMENTS.len(),
        "{case_id}: after-draw observations"
    );
    for (object, x, y) in ANIMATED_PLACEMENTS {
        assert!(
            before
                .iter()
                .any(|p| p.object == object && p.tile == (x, y, 0)),
            "{case_id}: no observation for object {object} at {x},{y}"
        );
    }
    (before, after)
}

/// Loads the world blocks of the Tutorial starting-house base into a core (block exports:
/// every baked flame frame), assembled without random phases.
fn core_from_tutorial_blocks(inputs: &Inputs) -> RendererCore {
    let (bx, by) = scene_base("tutorial-starting-house");
    let mut core = core_for(inputs, "lumbridge-castle-plaza");
    let mut loaded = 0;
    for square in RendererCore::squares_for_base(bx, by) {
        let path = repo_root().join(format!("assets/compiled/render/blocks/{square}.bin"));
        if path.exists() {
            core.load_block(
                square,
                &common::read_local_export(
                    &format!("blocks/{square}.bin"),
                    "python3 tools/render-assets/export.py --profile blocks",
                ),
                &common::read_local_export(
                    &format!("blocks/{square}.models.bin"),
                    "python3 tools/render-assets/export.py --profile blocks",
                ),
            )
            .unwrap();
            loaded += 1;
        }
    }
    assert!(
        loaded > 0,
        "no Tutorial block exports (export.py --profile blocks)"
    );
    core.assemble_scene(bx, by, false, 0.0).unwrap();
    core
}

/// Replays the recorded controller states on the block scene and freezes its clock at the
/// original's advance (`source_cycle − last_update_cycle` cycles), checking the bake's sequence
/// timing and the post-draw state against the sidecar. Returns the elapsed cycles.
fn apply_source_phases(core: &mut RendererCore, case_id: &str) -> i64 {
    let (before, after) = source_phases(case_id);
    let mut elapsed: Option<i64> = None;
    for phase in &before {
        let this = phase.source_cycle - phase.last_update_cycle;
        assert!(elapsed.is_none_or(|e| e == this), "{case_id}: mixed clocks");
        elapsed = Some(this);
        let (x, y, plane) = phase.tile;
        assert!(
            core.set_scenery_phase(plane, x, y, phase.object, phase.frame, phase.frame_cycle) > 0,
            "{case_id}: no animated instance of object {} at {x},{y},{plane}",
            phase.object
        );
        // The bake carries the same sequence timing the original controller ran.
        let scene = core.scene().unwrap();
        let states = core.scenery_phase_state(plane, x, y, phase.object, 0);
        assert!(!states.is_empty());
        let ex = x - scene.base_x + scene.offset;
        let ey = y - scene.base_y + scene.offset;
        let index = scene.tile_index(plane, ex, ey);
        let set_lengths: Vec<Vec<i32>> = scene
            .walls
            .get(&index)
            .map(|w| vec![w.model_a, w.model_b])
            .into_iter()
            .chain(
                scene
                    .wall_decorations
                    .get(&index)
                    .map(|d| vec![d.model_a, d.model_b]),
            )
            .chain(scene.floor_decorations.get(&index).map(|f| vec![f.model]))
            .chain(
                (0..5)
                    .filter_map(|slot| scene.slots.get(&(index * 5 + slot)))
                    .map(|&id| vec![scene.game_objects[id].model]),
            )
            .flatten()
            .filter(|r| *r <= -2)
            .filter_map(|r| scene.animated_instances.get((-(r) - 2) as usize))
            .map(|i| scene.animated[i.set].lengths.clone())
            .collect();
        assert!(
            set_lengths.contains(&phase.frame_lengths),
            "{case_id}: object {} at {x},{y}: baked frame lengths {set_lengths:?} differ from the original controller's {:?} (seq {})",
            phase.object,
            phase.frame_lengths,
            phase.sequence
        );
    }
    let elapsed = elapsed.expect("observations");
    core.set_scenery_clock_override(Some(elapsed));
    // After the original draw the controller advanced by `elapsed` cycles: the port must hold
    // the same frame and cycle (frame 4 of a 1-cycle terminal frame stays put at cycle 1).
    for phase in &after {
        let (x, y, plane) = phase.tile;
        let states = core.scenery_phase_state(plane, x, y, phase.object, elapsed);
        assert!(
            states
                .iter()
                .any(|&(f, c)| f as i32 == phase.frame && c == i64::from(phase.frame_cycle)),
            "{case_id}: object {} at {x},{y}: port advanced to {states:?}, original controller holds frame {} cycle {}",
            phase.object,
            phase.frame,
            phase.frame_cycle
        );
    }
    elapsed
}

/// Screen rectangle (1 px dilated) covered by the triangles of one placed object in the last
/// frame, from the exact triangle stream and its pick targets.
fn placement_screen_box(core: &RendererCore, object: i32) -> Option<(i32, i32, i32, i32)> {
    let targets = core.pick_targets();
    let mut bbox: Option<(i32, i32, i32, i32)> = None;
    for tri in core.triangles() {
        if tri.pick == 0 {
            continue;
        }
        let Some(clubscape_renderer::scene::draw::PickTarget::Object { hash, .. }) =
            targets.get(tri.pick as usize - 1)
        else {
            continue;
        };
        if ((hash >> 20) & 0xFFFF_FFFF) as i32 != object {
            continue;
        }
        let (x0, x1) = (*tri.x.iter().min().unwrap(), *tri.x.iter().max().unwrap());
        let (y0, y1) = (*tri.y.iter().min().unwrap(), *tri.y.iter().max().unwrap());
        bbox = Some(match bbox {
            None => (x0, y0, x1, y1),
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
        });
    }
    bbox.map(|(a, b, c, d)| (a - 1, b - 1, c + 1, d + 1))
}

/// Differing scene pixels that lie outside every rectangle in `rects` (the placements whose
/// source phase is unrecorded).
fn pixels_outside_rects(
    candidate: &[i32],
    source: &[i32],
    w: usize,
    hud: &[(String, [i32; 4])],
    rects: &[(i32, i32, i32, i32)],
) -> usize {
    let in_hud = |x: usize, y: usize| {
        hud.iter().any(|(_, [rx, ry, rw, rh])| {
            (x as i32) >= *rx && (x as i32) < rx + rw && (y as i32) >= *ry && (y as i32) < ry + rh
        })
    };
    let mut outside = 0;
    for (i, (c, s)) in candidate.iter().zip(source).enumerate() {
        if c == s {
            continue;
        }
        let (x, y) = ((i % w) as i32, (i / w) as i32);
        if in_hud(x as usize, y as usize) {
            continue;
        }
        if !rects
            .iter()
            .any(|r| x >= r.0 && x <= r.2 && y >= r.1 && y <= r.3)
        {
            outside += 1;
        }
    }
    outside
}

struct CaseSpec {
    id: &'static str,
    scene: &'static str,
    player: (i32, i32, i32),
    view: String,
    build_ms: f64,
    /// Whether the frame shows the flame whose source phase is unrecorded.
    flame: bool,
}

fn case_specs() -> Vec<CaseSpec> {
    vec![
        CaseSpec {
            id: "tutorial-door-closed",
            scene: "tutorial-starting-house",
            player: (3094, 3103, 0),
            view: door_view(0),
            build_ms: 0.0,
            flame: true,
        },
        CaseSpec {
            id: "tutorial-door-open",
            scene: "tutorial-starting-house",
            player: (3094, 3103, 0),
            view: door_view(1),
            build_ms: 0.0,
            flame: true,
        },
        CaseSpec {
            id: "lumbridge-ground-single",
            scene: "lumbridge-castle-plaza",
            player: (3222, 3218, 0),
            view: ground_view(&[(995, 1)]),
            build_ms: 0.0,
            flame: false,
        },
        CaseSpec {
            id: "lumbridge-ground-stack",
            scene: "lumbridge-castle-plaza",
            player: (3222, 3218, 0),
            view: ground_view(&[(995, 10000)]),
            build_ms: 0.0,
            flame: false,
        },
        CaseSpec {
            id: "lumbridge-ground-top-three",
            scene: "lumbridge-castle-plaza",
            player: (3222, 3218, 0),
            view: ground_view(&[(1925, 1), (1277, 1), (995, 10000), (1511, 1)]),
            build_ms: 0.0,
            flame: false,
        },
        // Fire: frame 0 at the first cycle; frame 3 after 19 native advancement cycles (20 ms each).
        CaseSpec {
            id: "lumbridge-fire-frame-zero",
            scene: "lumbridge-castle-plaza",
            player: (3222, 3218, 0),
            view: fire_view(),
            build_ms: 0.0,
            flame: false,
        },
        CaseSpec {
            id: "lumbridge-fire-frame-three",
            scene: "lumbridge-castle-plaza",
            player: (3222, 3218, 0),
            view: fire_view(),
            build_ms: 19.0 * 20.0,
            flame: false,
        },
        CaseSpec {
            id: "tutorial-roofs-outside",
            scene: "tutorial-starting-house",
            player: (3094, 3099, 0),
            view: world((3094, 3099, 0), "", "", ""),
            build_ms: 0.0,
            flame: false,
        },
        CaseSpec {
            id: "tutorial-roofs-inside",
            scene: "tutorial-starting-house",
            player: (3094, 3103, 0),
            view: world((3094, 3103, 0), "", "", ""),
            build_ms: 0.0,
            flame: false,
        },
        CaseSpec {
            id: "tutorial-roofs-hidden",
            scene: "tutorial-starting-house",
            player: (3094, 3099, 0),
            view: world((3094, 3099, 0), "", "", ""),
            build_ms: 0.0,
            flame: true,
        },
        CaseSpec {
            id: "lumbridge-minimap-plane-one",
            scene: "lumbridge-castle-plaza",
            player: (3222, 3218, 1),
            view: world((3222, 3218, 1), "", "", ""),
            build_ms: 0.0,
            flame: false,
        },
    ]
}

/// The source frames carry no drawn player (the native local player is not in view); the
/// renderer has no player body loaded here and reports exactly that.
const NO_PLAYER_BODY: &str = "player-case: no animation pack for npc 2063";

/// Every phase-independent case must meet the approved profile over the scene pixels; the
/// flame cases are attributed and reported (unpassed at the fixture phase). The report lists
/// each case's numbers and the HUD-rectangle accounting.
#[test]
fn dynamic_layer_cases_match_the_original_references() {
    let inputs = inputs();
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for spec in case_specs() {
        let case = CaseRecord::load(spec.id);
        assert_eq!(
            case.json["input"]["player_tile"],
            serde_json::json!([spec.player.0, spec.player.1]),
            "{}: player tile",
            spec.id
        );
        assert_eq!(
            case.json["input"]["plane"], spec.player.2,
            "{}: plane",
            spec.id
        );
        let mut core = core_for(&inputs, spec.scene);
        core.set_camera(camera(&case, spec.scene)).unwrap();
        core.set_plane(case.source_plane());
        // The locked-camera capture drew the recorded plane; the stock normal-camera selector
        // (`cz.ch`) is evaluated separately below, never assumed to have painted the image.
        core.set_top_plane_override(Some(case.draw_plane()));
        core.update_world(&spec.view, 0.0).unwrap();
        core.build_frame(spec.build_ms).unwrap();
        let summary = core.last_summary.clone();
        assert_eq!(
            summary.entities_skipped,
            vec![NO_PLAYER_BODY.to_string()],
            "{}",
            spec.id
        );
        let candidate = rasterize(&core, &inputs.textures);
        let (w, h, source) = read_png_rgb(&case.frame_path());
        assert_eq!((w, h), (1920, 1080), "{}: source frame size", spec.id);
        let out_dir = repo_root().join(".local/render-assets/test-output/dynamic");
        write_png_rgb(
            &out_dir.join(format!("{}-candidate.png", spec.id)),
            w,
            h,
            &candidate,
        );
        let hud = case.hud_rects();
        let m = metric(
            &candidate,
            &source,
            w as usize,
            h as usize,
            &hud,
            Some(&out_dir.join(format!("{}-diff.png", spec.id))),
        );
        // The case's original hide-roofs preference feeds the stock selector (`cz.ch` reads it
        // first); the locked-camera image itself was drawn at the recorded plane.
        core.set_hide_roofs(case.hide_roofs());
        let stock = case.stock_plane_selector().map(|expected| {
            core.set_top_plane_override(None);
            let (bx, by) = scene_base(spec.scene);
            (expected, core.stock_top_plane_public(bx, by))
        });
        core.set_top_plane_override(Some(case.draw_plane()));
        let outside_flame = if spec.flame {
            let rects: Vec<(i32, i32, i32, i32)> = ANIMATED_PLACEMENTS
                .iter()
                .filter_map(|&(object, _, _)| placement_screen_box(&core, object))
                .collect();
            assert!(!rects.is_empty(), "{}: no animated flame drawn", spec.id);
            Some((
                rects.clone(),
                pixels_outside_rects(&candidate, &source, w as usize, &hud, &rects),
            ))
        } else {
            None
        };
        let outcome = Outcome {
            id: spec.id.to_string(),
            metric: m.clone(),
            triangles: summary.triangles,
            skipped: summary.entities_skipped.clone(),
            stock_selector: stock,
        };
        let mut line = report_line(&outcome);
        if let Some((rects, outside)) = outside_flame {
            // The static fixture scene holds one baked frame per flame (the fixture session's
            // own random phase), so this case is decided on the block scene at the recorded
            // source controller state (`tutorial_flame_cases_match_at_the_recorded_source_phases`);
            // here every differing pixel must still lie on those placements.
            line.push_str(&format!(
                " | animated flames {ANIMATED_PLACEMENTS:?} (screen boxes {rects:?}) at the static fixture's phase; differing scene pixels outside them: {outside}; decided at the recorded source phases on the block scene"
            ));
            if outside > 0 {
                failures.push(format!(
                    "{}: {outside} differing pixels are not on the animated flames",
                    spec.id
                ));
            }
        } else if !m.passed() {
            failures.push(line.clone());
        }
        if let Some((expected, got)) = stock
            && i64::from(got) != expected
        {
            failures.push(format!(
                "{}: stock cz.ch plane selector expected {expected}, candidate {got}",
                spec.id
            ));
        }
        eprintln!("{line}");
        report.push(line);
    }
    // The controlled chunk mapping (source chunk 402,402 rotated 180 degrees at local chunk 6,6)
    // needs the original instanced-chunk decode/placement (`rl4.fn` template path), which the
    // renderer's block/scene assembly does not implement: no candidate exists for that case.
    let mapped = CaseRecord::load("lumbridge-minimap-mapped-chunk");
    assert_eq!(mapped.zoom(), FULL_HUD_ZOOM);
    let unsupported = "lumbridge-minimap-mapped-chunk: UNSUPPORTED | instanced chunk mapping (rl4.fn template placement) is not implemented; no candidate rendered, case not passed".to_string();
    eprintln!("{unsupported}");
    report.push(unsupported);
    let out_dir = repo_root().join(".local/render-assets/test-output/dynamic");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(out_dir.join("report.txt"), report.join("\n") + "\n").unwrap();
    assert!(
        failures.is_empty(),
        "dynamic layer cases outside the approved profile:\n{}",
        failures.join("\n")
    );
}

/// The three Tutorial flame cases on the block-assembled scene at the recorded source controller
/// states (phase sidecar): full strict `native_scene_model` metric, no flame-box accounting,
/// pixel-identical over every scene pixel. Needs the block exports.
#[test]
#[ignore = "needs the local world block exports (export.py --profile blocks)"]
fn tutorial_flame_cases_match_at_the_recorded_source_phases() {
    let inputs = inputs();
    let mut report = Vec::new();
    for spec in case_specs().into_iter().filter(|s| s.flame) {
        let case = CaseRecord::load(spec.id);
        let mut core = core_from_tutorial_blocks(&inputs);
        core.set_camera(camera(&case, "tutorial-starting-house"))
            .unwrap();
        core.set_plane(case.source_plane());
        core.set_top_plane_override(Some(case.draw_plane()));
        core.set_hide_roofs(case.hide_roofs());
        core.update_world(&spec.view, 0.0).unwrap();
        let elapsed = apply_source_phases(&mut core, spec.id);
        core.build_frame(0.0).unwrap();
        let summary = core.last_summary.clone();
        assert_eq!(
            summary.entities_skipped,
            vec![NO_PLAYER_BODY.to_string()],
            "{}",
            spec.id
        );
        let candidate = rasterize(&core, &inputs.textures);
        let (w, h, source) = read_png_rgb(&case.frame_path());
        let hud = case.hud_rects();
        let out_dir = repo_root().join(".local/render-assets/test-output/dynamic");
        std::fs::create_dir_all(&out_dir).unwrap();
        write_png_rgb(
            &out_dir.join(format!("{}-phased-candidate.png", spec.id)),
            w,
            h,
            &candidate,
        );
        let m = metric(
            &candidate,
            &source,
            w as usize,
            h as usize,
            &hud,
            Some(&out_dir.join(format!("{}-phased-diff.png", spec.id))),
        );
        let line = format!(
            "{}: {} at the recorded source phases (dy.ac frames {:?}, advance {elapsed} cycle) | scene identical {} of {} | interior max {} mean {:.4} changed {} | band changed {} ({:.5}) | first {:?}",
            spec.id,
            if m.passed() { "PASS" } else { "FAIL" },
            source_phases(spec.id)
                .0
                .iter()
                .map(|p| p.frame)
                .collect::<Vec<_>>(),
            m.scene_identical,
            m.scene_pixels,
            m.interior_max,
            m.interior_mean,
            m.interior_changed,
            m.band_changed,
            m.band_changed_fraction,
            m.first_interior
                .iter()
                .map(|(x, y, c, s)| format!("({x},{y}) {c:06x}/{s:06x}"))
                .collect::<Vec<_>>()
        );
        eprintln!("{line}");
        report.push(line.clone());
        assert!(m.passed(), "{line}");
        assert_eq!(
            m.scene_identical, m.scene_pixels,
            "{}: not identical to the source over the scene",
            spec.id
        );
    }
    let out_dir = repo_root().join(".local/render-assets/test-output/dynamic");
    std::fs::write(
        out_dir.join("flame-phase-report.txt"),
        report.join("\n") + "\n",
    )
    .unwrap();
}

/// The same three cases through the wgpu compute rasterizer: GPU = CPU = source at the recorded
/// phases. Compiled with `--features gpu`; needs the block exports and a hardware adapter.
#[cfg(feature = "gpu")]
#[test]
#[ignore = "needs the local world block exports (export.py --profile blocks) and a hardware adapter"]
fn tutorial_flame_cases_match_on_the_gpu_at_the_recorded_source_phases() {
    use clubscape_renderer::gpu::pack::pack_frame;
    use clubscape_renderer::gpu::{GpuRasterizer, GpuTextures};
    let inputs = inputs();
    let (adapter, device, queue) = match pollster::block_on(
        clubscape_renderer::gpu::device::request_native_device(),
    ) {
        Ok(v) => v,
        Err(e) => panic!(
            "GPU fidelity tests need a hardware wgpu adapter (Vulkan/Metal): {e}. A missing GPU is not a pass."
        ),
    };
    let palette = Palette::from_chunks(&common::read_asset("palette.bin")).unwrap();
    let gpu_textures = GpuTextures::from_set(&inputs.textures);
    let mut raster =
        GpuRasterizer::new(device, queue, &palette.rgb, &gpu_textures, 1920, 1080).unwrap();
    eprintln!("adapter: {}", adapter.get_info().name);
    for spec in case_specs().into_iter().filter(|s| s.flame) {
        let case = CaseRecord::load(spec.id);
        let mut core = core_from_tutorial_blocks(&inputs);
        core.set_camera(camera(&case, "tutorial-starting-house"))
            .unwrap();
        core.set_plane(case.source_plane());
        core.set_top_plane_override(Some(case.draw_plane()));
        core.set_hide_roofs(case.hide_roofs());
        core.update_world(&spec.view, 0.0).unwrap();
        apply_source_phases(&mut core, spec.id);
        core.build_frame(0.0).unwrap();
        let cpu = rasterize(&core, &inputs.textures);
        let packed = pack_frame(&core.state, core.triangles(), &inputs.textures);
        let frame = raster.render(&core.state, &packed, 0).unwrap();
        let mut gpu = raster.read_back().unwrap();
        assert!(
            frame.is_complete(),
            "{}: queue completion did not fire",
            spec.id
        );
        gpu.iter_mut().for_each(|p| *p &= 0xFF_FFFF);
        let (w, h, source) = read_png_rgb(&case.frame_path());
        let hud = case.hud_rects();
        let vs_cpu = common::diff_buffers(&gpu, &cpu);
        let m = metric(&gpu, &source, w as usize, h as usize, &hud, None);
        eprintln!(
            "{}: gpu vs cpu {} px differ | gpu vs source at recorded phases: {} | scene identical {} of {}",
            spec.id,
            vs_cpu.differing,
            if m.passed() { "PASS" } else { "FAIL" },
            m.scene_identical,
            m.scene_pixels
        );
        assert_eq!(
            vs_cpu.differing, 0,
            "{}: GPU differs from CPU: {vs_cpu:?}",
            spec.id
        );
        assert!(
            m.passed(),
            "{}: GPU frame outside the approved profile",
            spec.id
        );
        assert_eq!(
            m.scene_identical, m.scene_pixels,
            "{}: GPU frame not identical",
            spec.id
        );
    }
}

/// The same eight phase-independent cases through the wgpu compute rasterizer on a hardware
/// adapter: the GPU readback must equal the CPU frame and the original reference exactly.
/// Compiled with `--features gpu`; a missing adapter fails, it is not a pass.
#[cfg(feature = "gpu")]
#[test]
fn dynamic_layer_cases_match_on_the_gpu() {
    use clubscape_renderer::gpu::pack::pack_frame;
    use clubscape_renderer::gpu::{GpuRasterizer, GpuTextures};
    let inputs = inputs();
    let (adapter, device, queue) =
        match pollster::block_on(clubscape_renderer::gpu::device::request_native_device()) {
            Ok(v) => v,
            Err(e) => panic!(
                "GPU fidelity tests need a hardware wgpu adapter (Vulkan/Metal): {e}. \
                 Run without `--features gpu` on machines without one; a missing GPU is not a pass."
            ),
        };
    let palette = Palette::from_chunks(&common::read_asset("palette.bin")).unwrap();
    let gpu_textures = GpuTextures::from_set(&inputs.textures);
    let mut raster =
        GpuRasterizer::new(device, queue, &palette.rgb, &gpu_textures, 1920, 1080).unwrap();
    eprintln!("adapter: {}", adapter.get_info().name);
    let mut report = Vec::new();
    for spec in case_specs().into_iter().filter(|s| !s.flame) {
        let case = CaseRecord::load(spec.id);
        let mut core = core_for(&inputs, spec.scene);
        core.set_camera(camera(&case, spec.scene)).unwrap();
        core.set_plane(case.source_plane());
        core.set_top_plane_override(Some(case.draw_plane()));
        core.update_world(&spec.view, 0.0).unwrap();
        core.build_frame(spec.build_ms).unwrap();
        let cpu = rasterize(&core, &inputs.textures);
        let packed = pack_frame(&core.state, core.triangles(), &inputs.textures);
        let frame = raster.render(&core.state, &packed, 0).unwrap();
        let mut gpu = raster.read_back().unwrap();
        assert!(
            frame.is_complete(),
            "{}: queue completion did not fire",
            spec.id
        );
        gpu.iter_mut().for_each(|p| *p &= 0xFF_FFFF);
        let (w, h, source) = read_png_rgb(&case.frame_path());
        let hud = case.hud_rects();
        let vs_cpu = common::diff_buffers(&gpu, &cpu);
        let m = metric(&gpu, &source, w as usize, h as usize, &hud, None);
        let line = format!(
            "{}: gpu vs cpu {} px differ | gpu vs source: {} | scene identical {} of {} | interior max {} | band changed {}",
            spec.id,
            vs_cpu.differing,
            if m.passed() { "PASS" } else { "FAIL" },
            m.scene_identical,
            m.scene_pixels,
            m.interior_max,
            m.band_changed
        );
        eprintln!("{line}");
        report.push(line);
        assert_eq!(
            vs_cpu.differing, 0,
            "{}: GPU differs from CPU: {vs_cpu:?}",
            spec.id
        );
        assert!(
            m.passed(),
            "{}: GPU frame outside the approved profile",
            spec.id
        );
        assert_eq!(
            m.scene_identical, m.scene_pixels,
            "{}: GPU frame not identical to the source over the scene",
            spec.id
        );
    }
    let out_dir = repo_root().join(".local/render-assets/test-output/dynamic");
    std::fs::create_dir_all(&out_dir).unwrap();
    std::fs::write(out_dir.join("gpu-report.txt"), report.join("\n") + "\n").unwrap();
}

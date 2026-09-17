//! Robustness and behaviour tests: corrupt/missing assets fail explicitly, animation cadence
//! follows the source frame lengths, NPC packs reproduce the approved frame captures, resizing
//! keeps the projection consistent and picking stays inside bounds.

mod common;

use std::path::Path;

use clubscape_renderer::RenderError;
use clubscape_renderer::core::{Camera, ModelFixture, NpcPack, RendererCore};
use clubscape_renderer::model::Model;
use clubscape_renderer::palette::Palette;
use clubscape_renderer::raster::software::Software;
use clubscape_renderer::scene::SceneData;
use clubscape_renderer::scene::draw::PickTarget;
use clubscape_renderer::texture::{Texture, TextureSet};

use common::repo_root;

/// Reads a repository file; render assets under `assets/compiled/render/` go through the shared
/// loader so published gzip twins serve a clean checkout.
fn read(rel: &str) -> Vec<u8> {
    match rel.strip_prefix("assets/compiled/render/") {
        Some(key) => common::read_asset(key),
        None => std::fs::read(repo_root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}")),
    }
}

fn read_png_rgb(path: &Path) -> (u32, u32, Vec<i32>) {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info().expect("png info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("png frame");
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        other => panic!("unsupported color type {other:?}"),
    };
    let pixels = buf[..info.buffer_size()]
        .chunks_exact(channels)
        .map(|p| (i32::from(p[0]) << 16) | (i32::from(p[1]) << 8) | i32::from(p[2]))
        .collect();
    (info.width, info.height, pixels)
}

fn palette() -> Palette {
    Palette::from_chunks(&read("assets/compiled/render/palette.bin")).unwrap()
}

fn textures() -> TextureSet {
    let mut set = TextureSet::default();
    for bytes in common::texture_bytes() {
        set.insert(Texture::from_chunks(&bytes).unwrap());
    }
    set
}

fn core_with_assets(width: i32, height: i32) -> RendererCore {
    let mut core = RendererCore::new(palette(), width, height);
    for bytes in common::texture_bytes() {
        core.add_texture(&bytes).unwrap();
    }
    core.load_npc_pack_as(
        3028,
        &read("assets/compiled/render/models/npc-3028.pack.bin"),
    )
    .unwrap();
    core.load_npc_pack_as(
        2063,
        &read("assets/compiled/render/models/npc-2063.pack.bin"),
    )
    .unwrap();
    core
}

fn load_house(core: &mut RendererCore) {
    core.load_scene(
        "tutorial-starting-house",
        &read("assets/compiled/render/scenes/tutorial-starting-house.bin"),
        &read("assets/compiled/render/scenes/tutorial-starting-house.models.bin"),
    )
    .unwrap();
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
}

/// Rasterizes the current triangle stream; the original textured fills carry 0xFF in the
/// unused high byte, which the display ignores, so pixels are masked to 24-bit RGB.
fn rasterize(core: &RendererCore, textures: &TextureSet) -> Vec<i32> {
    let mut pixels = vec![0x30_3030; (core.state.width * core.state.height) as usize];
    {
        let mut raster = Software::new(core.state, &mut pixels, &core.palette.rgb, textures);
        for tri in core.triangles() {
            let _ = raster.draw(tri);
        }
    }
    pixels.iter_mut().for_each(|p| *p &= 0xFF_FFFF);
    pixels
}

// ------------------------------------------------------------------ corrupt / missing assets

#[test]
fn truncated_and_corrupt_buffers_are_rejected() {
    let model = read("assets/compiled/render/models/object-1277-model-1570-lit.bin");
    assert!(matches!(
        Model::from_chunks(&model[..model.len() / 2]),
        Err(RenderError::Format(_))
    ));
    let mut garbage = model.clone();
    for byte in garbage.iter_mut().skip(8).take(64) {
        *byte = 0xFF;
    }
    assert!(
        Model::from_chunks(&garbage).is_err(),
        "corrupted chunk table must not load"
    );
    assert!(Model::from_chunks(b"not a chunk file").is_err());
    let texture = read("assets/compiled/render/textures/1.bin");
    assert!(Texture::from_chunks(&texture[..texture.len() - 100]).is_err());
    assert!(Palette::from_chunks(&[0u8; 16]).is_err());
    let pack = read("assets/compiled/render/models/npc-3028.pack.bin");
    assert!(NpcPack::from_chunks(&pack[..pack.len() - 4096]).is_err());
}

/// The test-input pinning relies on the shared SHA-256 helper: FIPS 180-4 vectors, and every
/// published scene the tests read must hash to its manifest entry (a raw file that does not
/// is ignored in favour of the published twin, see `common::read_asset`).
#[test]
fn test_inputs_are_pinned_to_the_manifest() {
    assert_eq!(
        common::sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        common::sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let long = vec![b'a'; 1_000_000];
    assert_eq!(
        common::sha256_hex(&long),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&common::read_asset("manifest.json")).unwrap();
    for name in [
        "tutorial-starting-house",
        "tutorial-survival-coast",
        "lumbridge-castle-plaza",
        "lumbridge-river-bridge",
        "lumbridge-windmill-route",
    ] {
        let key = format!("scenes/{name}.bin");
        let bytes = common::read_asset(&key);
        let pin = manifest["files"][&key]["sha256"].as_str().unwrap();
        assert_eq!(common::sha256_hex(&bytes), pin, "{key}");
        // The bytes the tests use carry the tile settings roof removal depends on.
        assert!(
            clubscape_renderer::chunk::Chunks::parse(&bytes)
                .unwrap()
                .has("TSET"),
            "{key} lacks TSET"
        );
    }
}

/// Removes one tagged chunk from a chunk file (header + every other chunk kept verbatim).
fn without_chunk(file: &[u8], tag: &[u8; 4]) -> Vec<u8> {
    let mut out = file[..8].to_vec();
    let mut cursor = 8;
    while cursor + 8 <= file.len() {
        let len = u32::from_le_bytes(file[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        let end = cursor + 8 + len;
        if &file[cursor..cursor + 4] != tag {
            out.extend_from_slice(&file[cursor..end]);
        }
        cursor = end;
    }
    out
}

/// A scene or block export without its tile settings would silently draw every roof (the
/// roof-removal and stock top-plane rules read `ez.vs`); such a buffer must be refused, never
/// defaulted to "no roofs".
#[test]
fn scene_exports_without_tile_settings_are_rejected() {
    let scene = read("assets/compiled/render/scenes/tutorial-starting-house.bin");
    assert!(
        SceneData::from_chunks(&scene).is_ok(),
        "the published scene carries TSET"
    );
    let stripped = without_chunk(&scene, b"TSET");
    assert!(stripped.len() < scene.len());
    match SceneData::from_chunks(&stripped) {
        Err(e) => assert!(e.to_string().contains("TSET"), "{e}"),
        Ok(_) => panic!("scene without TSET loaded with defaulted tile settings"),
    }
    // Every published fixture scene carries roof-flagged tiles somewhere (the settings are real).
    let loaded = SceneData::from_chunks(&scene).unwrap();
    let roofed = (0..loaded.width)
        .flat_map(|x| (0..loaded.height).map(move |y| (x, y)))
        .filter(|&(x, y)| loaded.is_roof_tile(0, x, y))
        .count();
    assert!(
        roofed > 0,
        "starting-house scene has no roof-flagged tiles on plane 0"
    );
}

#[test]
fn face_indices_out_of_range_are_rejected() {
    let bytes = read("assets/compiled/render/models/object-1277-model-1570-lit.bin");
    let model = Model::from_chunks(&bytes).unwrap();
    // Locate the face index chunk and corrupt one index past the vertex count.
    let needle = model.face_a[0].to_le_bytes();
    let mut corrupted = bytes.clone();
    let position = corrupted
        .windows(needle.len())
        .position(|w| w == needle)
        .expect("face chunk present");
    let bad = (model.vertex_count as i32 + 7).to_le_bytes();
    corrupted[position..position + 4].copy_from_slice(&bad);
    match Model::from_chunks(&corrupted) {
        Err(RenderError::InvalidAsset(_)) | Err(RenderError::Format(_)) => {}
        other => panic!("out-of-range face index accepted: {other:?}"),
    }
}

#[test]
fn scene_load_reports_missing_textures_and_pack_mismatch() {
    let scene = read("assets/compiled/render/scenes/tutorial-starting-house.bin");
    let pack = read("assets/compiled/render/scenes/tutorial-starting-house.models.bin");
    // No textures loaded: the scene must refuse to load rather than draw fallbacks silently.
    let mut bare = RendererCore::new(palette(), 1920, 1080);
    bare.load_npc_pack_as(
        3028,
        &read("assets/compiled/render/models/npc-3028.pack.bin"),
    )
    .unwrap();
    match bare.load_scene("tutorial-starting-house", &scene, &pack) {
        Err(RenderError::MissingAsset(message)) => {
            assert!(message.contains("textures"), "{message}")
        }
        other => panic!("missing textures were not reported: {other:?}"),
    }
    assert!(bare.scene_id().is_none());
    // Wrong pack for the scene.
    let mut core = core_with_assets(1920, 1080);
    let other_pack = read("assets/compiled/render/scenes/tutorial-survival-coast.models.bin");
    assert!(matches!(
        core.load_scene("tutorial-starting-house", &scene, &other_pack),
        Err(RenderError::InvalidAsset(_))
    ));
    // Truncated pack.
    assert!(
        core.load_scene("tutorial-starting-house", &scene, &pack[..pack.len() / 3])
            .is_err()
    );
    assert!(
        core.build_frame(0.0).is_err(),
        "frame without a scene must fail"
    );
}

#[test]
fn unknown_npcs_and_sequences_are_reported_not_faked() {
    let mut core = core_with_assets(1920, 1080);
    load_house(&mut core);
    let world = serde_json::json!({
        "revision": "1", "tick": "1",
        "player": {"id": "p", "tile": {"x": 3098, "y": 3098, "plane": 0}, "animation": "808"},
        "entities": [
            {"id": "g", "kind": "npc", "sourceId": 3028, "tile": {"x": 3097, "y": 3097, "plane": 0}, "animation": "424"},
            {"id": "u", "kind": "npc", "sourceId": 1, "tile": {"x": 3096, "y": 3096, "plane": 0}, "animation": "6181"}
        ]
    });
    core.update_world(&world.to_string(), 0.0).unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    // Player (808 is not baked for the penguin) and goblin (424 not baked) fall back to the
    // first baked sequence and are reported; the unknown npc is skipped and reported.
    assert_eq!(summary.entities_drawn, 2);
    assert_eq!(
        summary.entities_skipped.len(),
        3,
        "{:?}",
        summary.entities_skipped
    );
    assert!(
        summary
            .entities_skipped
            .iter()
            .any(|s| s.contains("no animation pack for npc 1"))
    );
    assert!(core.update_world("{not json", 0.0).is_err());
}

#[test]
fn model_pack_deltas_reconstruct_full_models_and_reject_bad_bases() {
    use clubscape_renderer::model::parse_model_pack;
    let base = read("assets/compiled/render/models/object-1277-model-1570-lit.bin");
    let model = Model::from_chunks(&base).unwrap();
    // Build a delta entry: BASE 0 + header + bounds + a shifted VRTY.
    let mut delta = Vec::new();
    delta.extend_from_slice(b"CSRC");
    delta.extend_from_slice(&1u32.to_le_bytes());
    let mut chunk = |tag: &[u8], payload: &[u8]| {
        delta.extend_from_slice(tag);
        delta.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        delta.extend_from_slice(payload);
    };
    chunk(b"BASE", &0i32.to_le_bytes());
    let header: Vec<u8> = [
        model.vertex_count as i32,
        model.face_count as i32,
        model.tex_p.len() as i32,
        model.override_faces,
        model.transparency,
        0,
        model.render_mode,
        1,
        0,
    ]
    .iter()
    .flat_map(|v| v.to_le_bytes())
    .collect();
    chunk(b"MDHD", &header);
    let bounds: Vec<u8> = [7, 8, 9, 10, 11i32]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    chunk(b"BNDC", &bounds);
    let ys: Vec<u8> = model
        .ys
        .iter()
        .flat_map(|y| (y + 16.0).to_le_bytes())
        .collect();
    chunk(b"VRTY", &ys);
    let mut pack = Vec::new();
    pack.extend_from_slice(b"CSMP");
    pack.extend_from_slice(&2u32.to_le_bytes());
    for (key, body) in [("full", &base), ("delta", &delta)] {
        pack.extend_from_slice(&(key.len() as u32).to_le_bytes());
        pack.extend_from_slice(&(body.len() as u32).to_le_bytes());
        pack.extend_from_slice(key.as_bytes());
        pack.extend_from_slice(body);
    }
    let parsed = parse_model_pack(&pack).unwrap();
    assert_eq!(parsed.len(), 2);
    let (_, rebuilt) = &parsed[1];
    assert_eq!(rebuilt.xs, model.xs);
    assert_eq!(rebuilt.zs, model.zs);
    assert!(
        rebuilt
            .ys
            .iter()
            .zip(&model.ys)
            .all(|(a, b)| (a - b - 16.0).abs() < 1e-6)
    );
    assert_eq!(rebuilt.face_a, model.face_a);
    assert_eq!(rebuilt.color_a, model.color_a);
    assert_eq!(rebuilt.bounds.radius, 8);
    // A delta whose base comes later (or does not exist) is rejected.
    let mut bad = pack.clone();
    let base_pos = bad.windows(4).position(|w| w == b"BASE").unwrap() + 8;
    bad[base_pos..base_pos + 4].copy_from_slice(&5i32.to_le_bytes());
    assert!(matches!(
        parse_model_pack(&bad),
        Err(RenderError::InvalidAsset(_))
    ));
}

// ------------------------------------------------------------------ animation cadence

#[test]
fn animation_cadence_follows_source_frame_lengths() {
    let pack =
        NpcPack::from_chunks(&read("assets/compiled/render/models/npc-3028.pack.bin")).unwrap();
    let idle = pack.sequences.get(&6181).expect("goblin idle");
    assert_eq!(idle.lengths.len(), 16);
    let total: i32 = idle.lengths.iter().sum();
    // Frame 0 lasts lengths[0] client cycles of 20 ms.
    let first = idle.lengths[0];
    assert_eq!(pack.frame_index(6181, 0.0), Some(0));
    assert_eq!(
        pack.frame_index(6181, f64::from(first) * 20.0 - 0.01),
        Some(0)
    );
    assert_eq!(pack.frame_index(6181, f64::from(first) * 20.0), Some(1));
    // Wraps after the full cycle.
    assert_eq!(pack.frame_index(6181, f64::from(total) * 20.0), Some(0));
    assert_eq!(
        pack.frame_index(6181, f64::from(total) * 20.0 + 20.0 * f64::from(first)),
        Some(1)
    );
    // Every frame is reached exactly for its declared span.
    let mut visited = vec![0i32; 16];
    for cycle in 0..total {
        let frame = pack.frame_index(6181, f64::from(cycle) * 20.0).unwrap();
        visited[frame] += 1;
    }
    assert_eq!(
        visited, idle.lengths,
        "frame spans must equal the source frame lengths"
    );
    assert_eq!(pack.frame_index(9999, 0.0), None);
    let penguin =
        NpcPack::from_chunks(&read("assets/compiled/render/models/npc-2063.pack.bin")).unwrap();
    assert_eq!((penguin.width_scale, penguin.height_scale), (75, 75));
    assert_eq!(penguin.sequences[&5668].lengths.len(), 14);
    assert_eq!(penguin.sequences[&5666].lengths.len(), 8);
}

// ------------------------------------------------------------------ packs vs approved captures

#[test]
fn npc_pack_frames_match_source_captures_via_core() {
    let textures = textures();
    let mut core = core_with_assets(1920, 1080);
    let cases: [(i32, i32, usize, i32, i32); 4] = [
        (3028, 6181, 16, 240, 650),
        (3028, 6180, 16, 240, 650),
        (2063, 5668, 14, 160, 400),
        (2063, 5666, 8, 160, 400),
    ];
    for (npc, sequence, frames, camera_y, camera_z) in cases {
        for frame in 0..frames {
            core.build_model_fixture_frame(&ModelFixture {
                model: String::new(),
                npc: Some((npc, sequence, frame)),
                yaw: 256,
                camera_y,
                camera_z,
            })
            .unwrap();
            let pixels = rasterize(&core, &textures);
            let (w, h, expected) = read_png_rgb(&repo_root().join(format!(
                "assets/reference/osrs240/models/npc-{npc}-sequence-{sequence}-frame-{frame}.png"
            )));
            assert_eq!((w, h), (1920, 1080));
            let differing = pixels.iter().zip(&expected).filter(|(a, b)| a != b).count();
            assert_eq!(
                differing, 0,
                "npc {npc} seq {sequence} frame {frame}: {differing} pixels differ"
            );
        }
    }
    core.load_model(
        "tree",
        &read("assets/compiled/render/models/object-1277-model-1570-lit.bin"),
    )
    .unwrap();
    for yaw in [0, 256, 512, 1024] {
        core.build_model_fixture_frame(&ModelFixture {
            model: "tree".into(),
            npc: None,
            yaw,
            camera_y: 250,
            camera_z: 750,
        })
        .unwrap();
        let pixels = rasterize(&core, &textures);
        let (_, _, expected) = read_png_rgb(&repo_root().join(format!(
            "assets/reference/osrs240/models/tree-1277-yaw-{yaw}.png"
        )));
        let diffs: Vec<(usize, i32, i32)> = pixels
            .iter()
            .zip(&expected)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| (i, *a, *b))
            .collect();
        assert!(
            diffs.is_empty(),
            "tree yaw {yaw}: {} pixels differ, first {:?} (tris {})",
            diffs.len(),
            &diffs[..diffs.len().min(5)],
            core.triangles().len()
        );
    }
    assert!(
        core.build_model_fixture_frame(&ModelFixture {
            model: "nope".into(),
            npc: None,
            yaw: 0,
            camera_y: 0,
            camera_z: 0
        })
        .is_err()
    );
}

// ------------------------------------------------------------------ resize and picking

#[test]
fn resize_reprojects_and_picking_stays_in_bounds() {
    let textures = textures();
    let mut core = core_with_assets(1920, 1080);
    load_house(&mut core);
    core.build_frame(0.0).unwrap();
    let full = rasterize(&core, &textures);
    let full_background =
        full.iter().filter(|&&p| p == 0x30_3030).count() as f64 / full.len() as f64;
    eprintln!("1920x1080 background fraction {full_background:.3}");
    let center_tile = match core.pick(960, 540) {
        Some(PickTarget::Tile { x, y, .. } | PickTarget::Object { x, y, .. }) => (x, y),
        None => panic!("center pixel must hit the scene"),
    };
    assert!(core.pick(-1, 0).is_none());
    assert!(core.pick(1920, 0).is_none());
    assert!(core.pick(0, 1080).is_none());
    assert!(matches!(
        core.pick(960, 540),
        Some(PickTarget::Tile { .. } | PickTarget::Object { .. })
    ));
    // A pick target always names a tile inside the loaded scene grid.
    for (x, y) in [(10, 700), (960, 540), (1900, 1070), (400, 900)] {
        if let Some(
            PickTarget::Tile {
                x: tx,
                y: ty,
                plane,
            }
            | PickTarget::Object {
                x: tx,
                y: ty,
                plane,
                ..
            },
        ) = core.pick(x, y)
        {
            assert!(
                (0..104).contains(&tx) && (0..104).contains(&ty) && (0..4).contains(&plane),
                "{tx},{ty},{plane}"
            );
        }
    }
    assert_eq!(Camera::source_zoom_for_height(1080), 662);
    for (w, h) in [(1024, 768), (1280, 720), (2560, 1440)] {
        core.resize(w, h);
        assert_eq!((core.state.width, core.state.height), (w, h));
        assert_eq!((core.state.center_x, core.state.center_y), (w / 2, h / 2));
        // The core never rescales on its own: the shell supplies the original viewport-derived
        // zoom (`Camera::source_zoom_for_height`) with the camera, as the client does.
        let mut camera = core.camera;
        camera.zoom = Camera::source_zoom_for_height(h);
        core.set_camera(camera).unwrap();
        core.build_frame(0.0).unwrap();
        let pixels = rasterize(&core, &textures);
        // The projection center still lands on the same source tile, the near ground (bottom
        // rows) is fully covered, and beyond-draw-distance background stays bounded.
        let picked = match core.pick(w / 2, h / 2) {
            Some(PickTarget::Tile { x, y, .. } | PickTarget::Object { x, y, .. }) => (x, y),
            None => panic!("{w}x{h}: center pixel must hit the scene"),
        };
        assert_eq!(
            picked, center_tile,
            "{w}x{h}: projection center moved off the focal tile"
        );
        let bottom = &pixels[((h - 40) * w) as usize..];
        assert!(
            bottom.iter().all(|&p| p != 0x30_3030),
            "{w}x{h}: near ground rows show background"
        );
        // With the source zoom curve the field of view is constant above 434 px, so the
        // beyond-draw-distance background fraction must track the 1080p frame.
        let background =
            pixels.iter().filter(|&&p| p == 0x30_3030).count() as f64 / pixels.len() as f64;
        eprintln!("{w}x{h} background fraction {background:.3}");
        // (A narrower 4:3 viewport trims the horizon sides and shows a little less of it.)
        assert!(
            background < full_background + 0.06 && background > full_background - 0.15,
            "{w}x{h}: background fraction {background:.3} vs 1080p {full_background:.3}"
        );
        assert!(core.pick(w, h).is_none());
    }
    core.resize(1920, 1080);
    let mut camera = core.camera;
    camera.zoom = Camera::source_zoom_for_height(1080);
    core.set_camera(camera).unwrap();
    core.build_frame(0.0).unwrap();
    common::assert_pixels_equal(
        &rasterize(&core, &textures),
        &full,
        1920,
        "resizing back must reproduce the original frame",
    );
}

// ------------------------------------------------------------------ skeletal animation port

#[test]
fn animation_port_reproduces_baked_original_frames_exactly() {
    use clubscape_renderer::anim::{Sequence, apply_frame, scale_float};
    let textures = textures();
    let mut checked = 0;
    for (npc, sequences, camera_y, camera_z) in [
        (3028, [6181, 6180], 240, 650),
        (2063, [5668, 5666], 160, 400),
    ] {
        let pack = NpcPack::from_chunks(&read(&format!(
            "assets/compiled/render/models/npc-{npc}.pack.bin"
        )))
        .unwrap();
        let base = Model::from_chunks(&read(&format!(
            "assets/compiled/render/models/npc-{npc}-base.bin"
        )))
        .unwrap();
        for sequence_id in sequences {
            // Published input (`anim/seq-<id>.bin`): missing means a broken checkout, not a skip.
            let sequence = Sequence::from_chunks(&read(&format!(
                "assets/compiled/render/anim/seq-{sequence_id}.bin"
            )))
            .unwrap();
            let baked = pack.sequences.get(&sequence_id).expect("baked sequence");
            assert_eq!(
                sequence.lengths, baked.lengths,
                "sequence {sequence_id} frame lengths"
            );
            for frame in 0..sequence.frame_count() {
                let mut model = base.clone();
                apply_frame(&mut model, &sequence, frame, None).unwrap();
                scale_float(&mut model, pack.width_scale, pack.height_scale);
                let expected = pack.frame_model(sequence_id, frame).unwrap();
                // The baked pack stores int-truncated positions exactly as the draw path reads them.
                for v in 0..model.vertex_count {
                    let got = (model.xs[v] as i32, model.ys[v] as i32, model.zs[v] as i32);
                    let want = (
                        expected.xs[v] as i32,
                        expected.ys[v] as i32,
                        expected.zs[v] as i32,
                    );
                    assert_eq!(
                        got, want,
                        "npc {npc} seq {sequence_id} frame {frame} vertex {v}"
                    );
                }
                if let Some(alphas) = &expected.alphas {
                    assert_eq!(
                        model.alphas.as_ref().unwrap(),
                        alphas,
                        "npc {npc} seq {sequence_id} frame {frame} alphas"
                    );
                }
                // And the drawn frame equals the approved capture.
                model.compute_cylinder_bounds();
                let mut core = RendererCore::new(palette(), 1920, 1080);
                core.load_model_value("frame", model);
                core.build_model_fixture_frame(&ModelFixture {
                    model: "frame".into(),
                    npc: None,
                    yaw: 256,
                    camera_y,
                    camera_z,
                })
                .unwrap();
                let pixels = rasterize(&core, &textures);
                let (_, _, reference) = read_png_rgb(&repo_root().join(format!(
                    "assets/reference/osrs240/models/npc-{npc}-sequence-{sequence_id}-frame-{frame}.png"
                )));
                let differing = pixels
                    .iter()
                    .zip(&reference)
                    .filter(|(a, b)| a != b)
                    .count();
                assert_eq!(
                    differing, 0,
                    "npc {npc} seq {sequence_id} frame {frame}: {differing} pixels differ from the capture"
                );
                checked += 1;
            }
        }
    }
    eprintln!("animation port checked {checked} frames");
}

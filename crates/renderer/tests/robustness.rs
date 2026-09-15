//! Robustness and behaviour tests: corrupt/missing assets fail explicitly, animation cadence
//! follows the source frame lengths, NPC packs reproduce the approved frame captures, resizing
//! keeps the projection consistent and picking stays inside bounds.

use std::path::{Path, PathBuf};

use clubscape_renderer::RenderError;
use clubscape_renderer::core::{Camera, ModelFixture, NpcPack, RendererCore};
use clubscape_renderer::model::Model;
use clubscape_renderer::palette::Palette;
use clubscape_renderer::raster::software::Software;
use clubscape_renderer::scene::draw::PickTarget;
use clubscape_renderer::texture::{Texture, TextureSet};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn read(rel: &str) -> Vec<u8> {
    std::fs::read(repo_root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
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
    for entry in std::fs::read_dir(repo_root().join("assets/compiled/render/textures")).unwrap() {
        set.insert(Texture::from_chunks(&std::fs::read(entry.unwrap().path()).unwrap()).unwrap());
    }
    set
}

fn core_with_assets(width: i32, height: i32) -> RendererCore {
    let mut core = RendererCore::new(palette(), width, height);
    for entry in std::fs::read_dir(repo_root().join("assets/compiled/render/textures")).unwrap() {
        core.add_texture(&std::fs::read(entry.unwrap().path()).unwrap())
            .unwrap();
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

fn scene_available(name: &str) -> bool {
    repo_root()
        .join(format!("assets/compiled/render/scenes/{name}.bin"))
        .exists()
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
    if !scene_available("tutorial-starting-house") {
        eprintln!(
            "skipping: scene export not present (run tools/render-assets/export.py --profile unpack)"
        );
        return;
    }
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
    if !scene_available("tutorial-starting-house") {
        return;
    }
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
    if !scene_available("tutorial-starting-house") {
        return;
    }
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
    assert_eq!(
        rasterize(&core, &textures),
        full,
        "resizing back must reproduce the original frame"
    );
}

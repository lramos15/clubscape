mod common;

use std::io::Read;

use clubscape_camera::{Terrain, bilinear_height};
use clubscape_renderer::camera::{CameraObjectDefinition, CameraScene, CameraTerrain};
use clubscape_renderer::core::RendererCore;
use clubscape_renderer::palette::Palette;
use clubscape_renderer::scene::terrain::{self, FloorDefs, SceneTerrain};
use clubscape_renderer::scene::{SceneData, TilePaint, flag};

fn source_scene() -> SceneData {
    SceneData::from_chunks(&common::read_asset("scenes/lumbridge-castle-plaza.bin")).unwrap()
}

fn source_definitions() -> Vec<CameraObjectDefinition> {
    let bytes = std::fs::read(
        common::repo_root().join("assets/source/osrs/cache2695/collections/object.json.gz"),
    )
    .unwrap();
    assert_eq!(
        common::sha256_hex(&bytes),
        "d8b0212b1fa5fe34dff300f731de356f51562b58f08d7512c74fe560c4fff021"
    );
    let mut json = Vec::new();
    flate2::read::GzDecoder::new(bytes.as_slice())
        .read_to_end(&mut json)
        .unwrap();
    let definitions: serde_json::Map<String, serde_json::Value> =
        serde_json::from_slice(&json).unwrap();
    definitions
        .into_values()
        .map(|v| serde_json::from_value(v).unwrap())
        .collect()
}

#[test]
fn actual_source_triangles_and_object_941_raise_are_not_bilinear() {
    let scene = source_scene();
    let snapshot = CameraScene::from_scene(&scene, &scene.name, 1).unwrap();
    assert!(snapshot.required_objects().contains(&941));
    let terrain = CameraTerrain::new(snapshot, source_definitions()).unwrap();
    let point = |x, y| [(x - scene.base_x) * 128 + 64, (y - scene.base_y) * 128 + 64];
    let [x, y] = point(3215, 3218);
    assert_eq!(
        bilinear_height(&terrain, x as f32, y as f32, 0).unwrap(),
        -464.0
    );
    assert_eq!(terrain.surface_height(0, x, y).unwrap(), -471);
    let [x, y] = point(3223, 3218);
    assert_eq!(
        bilinear_height(&terrain, x as f32, y as f32, 0).unwrap(),
        -238.0
    );
    assert_eq!(terrain.surface_height(0, x, y).unwrap(), -240);
}

#[test]
#[ignore = "requires manifest-pinned blocks/12850.bin.gz and 12850.models.bin.gz; absent in this admission, no export/copy authorized"]
fn streamed_original_block_getters_preserve_real_cases_and_unloaded_boundaries() {
    let mut core = RendererCore::new(
        Palette::from_chunks(&common::read_asset("palette.bin")).unwrap(),
        1920,
        1080,
    );
    for texture in common::texture_bytes() {
        core.add_texture(&texture).unwrap();
    }
    core.load_floor_defs(&common::read_asset("terrain/floors.bin"))
        .unwrap();
    core.load_block(
        12850,
        &common::read_asset("blocks/12850.bin"),
        &common::read_asset("blocks/12850.models.bin"),
    )
    .unwrap();
    assert!(
        !core
            .assemble_scene(3168, 3168, false, 0.0)
            .unwrap()
            .is_empty()
    );
    let terrain = CameraTerrain::new(core.camera_scene().unwrap(), source_definitions()).unwrap();
    let point = |x, y| [(x - 3168) * 128 + 64, (y - 3168) * 128 + 64];
    let [x, y] = point(3215, 3218);
    assert_eq!(terrain.surface_height(0, x, y).unwrap(), -471);
    let [x, y] = point(3223, 3218);
    assert_eq!(terrain.surface_height(0, x, y).unwrap(), -240);
    assert_eq!(
        bilinear_height(&terrain, x as f32, y as f32, 0).unwrap(),
        -238.0
    );
    assert!(
        terrain.height_corner(0, 0, 0).is_err(),
        "missing source block is not zero-height ground"
    );
    assert!(terrain.tile_settings(1, 0, 0).is_err());
    assert!(terrain.require_surface(0, [64, 64]).is_err());
}

#[test]
fn terrain_rebuild_preserves_observed_zero_and_clears_unloaded_height_presence() {
    let mut scene = SceneData::empty_grid(
        "controlled-height-presence".into(),
        0,
        0,
        terrain::GRID as i32,
        4,
        terrain::MARGIN,
        terrain::MAIN,
        4 << 16,
    );
    let margin = terrain::MARGIN;
    scene.set_height(0, margin, margin, 999);
    scene.set_height(0, margin + 2, margin + 3, 777);
    scene.set_height(0, margin + terrain::MAIN, margin, 4321);
    let mut source = SceneTerrain::empty();
    source.set_height(0, 0, 0, 0);
    source.set_height(2, 13, 27, -700);
    source.set_height(0, terrain::MAIN, 0, 123);
    let palette = Palette::from_chunks(&common::read_asset("palette.bin")).unwrap();
    terrain::apply(
        &mut scene,
        &source,
        &FloorDefs::default(),
        &palette,
        &|_| panic!("controlled empty terrain has no textured floors"),
    )
    .unwrap();
    for plane in 0..4 {
        for x in 0..=terrain::MAIN {
            for y in 0..=terrain::MAIN {
                let observed = match (plane, x, y) {
                    (0, 0, 0) => Some(0),
                    (2, 13, 27) => Some(-700),
                    _ => None,
                };
                assert_eq!(scene.camera_height(plane, x + margin, y + margin), observed);
                assert_eq!(
                    scene.height(plane, x + margin, y + margin),
                    source.height(plane, x, y),
                );
            }
        }
    }
    let camera = CameraTerrain::new(
        CameraScene::from_scene(&scene, &scene.name, 1).unwrap(),
        vec![],
    )
    .unwrap();
    assert_eq!(camera.height_corner(0, 0, 0).unwrap(), 0);
    assert_eq!(camera.height_corner(2, 13, 27).unwrap(), -700);
    assert!(camera.height_corner(0, 2, 3).is_err());
    assert!(camera.height_corner(0, terrain::MAIN as u32, 0).is_err());
}

#[test]
fn original_shaped_model_faces_are_transported_in_source_order() {
    let scene = source_scene();
    let snapshot = CameraScene::from_scene(&scene, &scene.name, 1).unwrap();
    let mut checked = 0;
    for (&index, model) in &scene.tile_models {
        let (plane, ex, ey) = scene.decode_index(index);
        let x = ex - scene.offset;
        let y = ey - scene.offset;
        if !(0..104).contains(&x) || !(0..104).contains(&y) {
            continue;
        }
        let Some(tile) = &snapshot.surfaces[((plane * 104 + x) * 104 + y) as usize] else {
            continue;
        };
        if scene.flags[index] & flag::PAINT != 0 {
            continue;
        }
        assert_eq!(tile.triangles.len(), model.face_a.len());
        for (i, triangle) in tile.triangles.iter().enumerate() {
            let vertices = [model.face_a[i], model.face_b[i], model.face_c[i]].map(|v| v as usize);
            assert_eq!(
                triangle.horizontal,
                vertices.map(|v| [model.xs[v], model.zs[v]])
            );
            assert_eq!(triangle.heights, vertices.map(|v| model.ys[v]));
        }
        checked += 1;
    }
    assert!(
        checked > 100,
        "actual source models, not a paint-only fixture adapter"
    );
}

#[test]
fn missing_original_raise_and_unloaded_height_are_explicit() {
    let scene = source_scene();
    let terrain = CameraTerrain::new(
        CameraScene::from_scene(&scene, &scene.name, 1).unwrap(),
        vec![],
    )
    .unwrap();
    let p = [
        (3215 - scene.base_x) * 128 + 64,
        (3218 - scene.base_y) * 128 + 64,
    ];
    assert!(
        terrain
            .require_surface(0, p)
            .unwrap_err()
            .contains("object 941")
    );
    assert!(terrain.surface_height(0, p[0], p[1]).is_err());
    assert!(terrain.surface_height(0, -1, -1).is_err());
    assert!(terrain.height_corner(4, 0, 0).is_err());
    assert!(terrain.tile_settings(0, u32::MAX, u32::MAX).is_err());
    let mut empty = SceneData::empty_grid(
        "controlled-empty-input".into(),
        0,
        0,
        104,
        4,
        0,
        104,
        4 * 65536,
    );
    assert_eq!(empty.camera_height(0, 0, 0), None);
    assert_eq!(empty.camera_setting(0, 0, 0), None);
    empty.set_height(0, 0, 0, 0);
    empty.set_setting(0, 0, 0, 0);
    assert_eq!(empty.camera_height(0, 0, 0), Some(0));
    assert_eq!(empty.camera_setting(0, 0, 0), Some(0));
    let snapshot = CameraScene::from_scene(&empty, &empty.name, 2).unwrap();
    let empty = CameraTerrain::new(snapshot, vec![]).unwrap();
    assert_eq!(empty.height_corner(0, 0, 0).unwrap(), 0);
    assert!(empty.height_corner(0, 1, 0).is_err());
    assert!(empty.require_surface(0, [0, 0]).is_err());
}

#[test]
fn controlled_bridge_getter_uses_original_settings_and_shifted_geometry() {
    let mut scene = SceneData::empty_grid(
        "controlled-bridge-input".into(),
        0,
        0,
        104,
        4,
        0,
        104,
        4 * 65536,
    );
    scene.set_setting(0, 10, 10, 0);
    scene.set_setting(1, 10, 10, 2);
    for (x, y) in [(10, 10), (11, 10), (11, 11), (10, 11)] {
        scene.set_height(0, x, y, -64);
        scene.set_height(1, x, y, -256);
    }
    let index = scene.tile_index(0, 10, 10);
    scene.flags[index] = flag::EXISTS | flag::PAINT | flag::BRIDGE_BELOW;
    scene.paints.insert(
        index,
        TilePaint {
            sw: 1,
            se: 1,
            ne: 1,
            nw: 1,
            texture: -1,
            flat: true,
            rgb: 1,
        },
    );
    let terrain = CameraTerrain::new(
        CameraScene::from_scene(&scene, &scene.name, 3).unwrap(),
        vec![],
    )
    .unwrap();
    assert_eq!(
        terrain
            .surface_height(0, 10 * 128 + 64, 10 * 128 + 64)
            .unwrap(),
        -256
    );
    assert_eq!(
        bilinear_height(&terrain, 1344.0, 1344.0, 0).unwrap(),
        -256.0
    );
    scene.flags[index] &= !flag::BRIDGE_BELOW;
    assert!(
        CameraScene::from_scene(&scene, &scene.name, 4)
            .unwrap_err()
            .to_string()
            .contains("bridge")
    );
}

#[test]
fn real_renderer_metadata_does_not_relabel_tile_placement_as_native_focus() {
    let mut core = RendererCore::new(
        Palette::from_chunks(&common::read_asset("palette.bin")).unwrap(),
        1920,
        1080,
    );
    for texture in common::texture_bytes() {
        core.add_texture(&texture).unwrap();
    }
    core.load_scene(
        "source-scene",
        &common::read_asset("scenes/lumbridge-castle-plaza.bin"),
        &common::read_asset("scenes/lumbridge-castle-plaza.models.bin"),
    )
    .unwrap();
    assert!(core.camera_source().is_err());
    core.update_world(r#"{"revision":"7","tick":"9","player":{"id":"actor.controlled-input","region":"region.osrs.12850","tile":{"x":3215,"y":3218,"plane":0},"running":false}}"#, 0.0).unwrap();
    let sample = core.camera_source().unwrap();
    assert_eq!(sample.context.actor_id, "actor.controlled-input");
    assert_eq!(sample.context.revision, "7");
    assert_eq!(sample.context.scene_generation, "1");
    let placement = sample.rendered_actor.unwrap();
    assert_eq!(
        placement.local,
        [(3215 - 3168) * 128 + 64, (3218 - 3168) * 128 + 64]
    );
    assert_eq!(placement.size_tiles, 1);
    assert!(sample.focus.is_none());
    assert!(sample.effects.is_none());
    assert_eq!(sample.missing.len(), 3);
    assert_eq!(core.camera_scene().unwrap().base.x, 3168);
}

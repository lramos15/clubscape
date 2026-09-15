//! Actors through the skeletal animation port: NPC definitions with their original stand/walk
//! sequences, the penguin player with retargeted human action sequences, modular equipment
//! attachment with measured fit, instance filtering and picking by WorldView id.

use std::path::PathBuf;

use clubscape_renderer::actor::{ActivityContext, player_sequence_for};
use clubscape_renderer::core::{Camera, RendererCore};
use clubscape_renderer::palette::Palette;
use clubscape_renderer::raster::software::Software;
use clubscape_renderer::scene::draw::PickTarget;
use clubscape_renderer::texture::{Texture, TextureSet};

fn repo_root() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn read(rel: &str) -> Vec<u8> {
    std::fs::read(repo_root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// Manifest file keys are relative to assets/compiled/render.
fn asset(key: &str) -> Vec<u8> {
    read(&format!("assets/compiled/render/{key}"))
}

fn textures() -> TextureSet {
    let mut set = TextureSet::default();
    for entry in std::fs::read_dir(repo_root().join("assets/compiled/render/textures")).unwrap() {
        set.insert(Texture::from_chunks(&std::fs::read(entry.unwrap().path()).unwrap()).unwrap());
    }
    set
}

fn manifest() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("assets/compiled/render/manifest.json")).unwrap(),
    )
    .unwrap()
}

/// Loads textures, every exported sequence, every NPC definition, the player body and gear.
fn core_with_actors() -> Option<RendererCore> {
    let manifest = manifest();
    if manifest.get("sequences").is_none() || manifest.get("player_reference").is_none() {
        eprintln!("skipping: run tools/render-assets/export.py --profile anim");
        return None;
    }
    let mut core = RendererCore::new(
        Palette::from_chunks(&read("assets/compiled/render/palette.bin")).unwrap(),
        1920,
        1080,
    );
    for entry in std::fs::read_dir(repo_root().join("assets/compiled/render/textures")).unwrap() {
        core.add_texture(&std::fs::read(entry.unwrap().path()).unwrap())
            .unwrap();
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
    Some(core)
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

fn world(
    player_tile: (i32, i32),
    activity: &str,
    animation: &str,
    equipment: &[(&str, i32)],
    entities: serde_json::Value,
) -> String {
    let equipment: Vec<serde_json::Value> = equipment
        .iter()
        .map(|(slot, id)| serde_json::json!({"slot": slot, "item": {"id": format!("item.{id}"), "name": "", "quantity": 1, "sourceId": id}}))
        .collect();
    serde_json::json!({
        "revision": "1", "tick": "1",
        "player": {"id": "player-1", "tile": {"x": player_tile.0, "y": player_tile.1, "plane": 0}, "animation": animation,
                   "activity": activity, "hitpoints": 10, "instance": null, "equipment": equipment},
        "entities": entities
    })
    .to_string()
}

#[test]
fn activity_defaults_follow_the_bound_source_sequences() {
    let ctx = ActivityContext::default();
    assert_eq!(player_sequence_for("idle", &ctx), 808);
    assert_eq!(player_sequence_for("walking", &ctx), 819);
    assert_eq!(
        player_sequence_for(
            "walking",
            &ActivityContext {
                running: true,
                ..ctx
            }
        ),
        824
    );
    assert_eq!(
        player_sequence_for(
            "gathering",
            &ActivityContext {
                adjacent_fishing_spot: true,
                ..ctx
            }
        ),
        621
    );
    assert_eq!(
        player_sequence_for(
            "gathering",
            &ActivityContext {
                adjacent_rock: true,
                ..ctx
            }
        ),
        625
    );
    assert_eq!(
        player_sequence_for(
            "gathering",
            &ActivityContext {
                adjacent_tree: true,
                ..ctx
            }
        ),
        879
    );
    assert_eq!(
        player_sequence_for(
            "producing",
            &ActivityContext {
                adjacent_fire: true,
                ..ctx
            }
        ),
        897
    );
    assert_eq!(
        player_sequence_for(
            "producing",
            &ActivityContext {
                adjacent_range: true,
                ..ctx
            }
        ),
        896
    );
    assert_eq!(
        player_sequence_for(
            "producing",
            &ActivityContext {
                adjacent_furnace: true,
                ..ctx
            }
        ),
        899
    );
    assert_eq!(
        player_sequence_for(
            "producing",
            &ActivityContext {
                adjacent_anvil: true,
                ..ctx
            }
        ),
        898
    );
    assert_eq!(player_sequence_for("producing", &ctx), 733);
    assert_eq!(
        player_sequence_for(
            "fighting",
            &ActivityContext {
                weapon_item: Some(1205),
                ..ctx
            }
        ),
        386
    );
    assert_eq!(
        player_sequence_for(
            "fighting",
            &ActivityContext {
                weapon_item: Some(1277),
                ..ctx
            }
        ),
        390
    );
    assert_eq!(
        player_sequence_for(
            "fighting",
            &ActivityContext {
                weapon_item: Some(841),
                ..ctx
            }
        ),
        426
    );
    assert_eq!(player_sequence_for("fighting", &ctx), 422);
    assert_eq!(player_sequence_for("casting", &ctx), 711);
    assert_eq!(
        player_sequence_for("idle", &ActivityContext { dead: true, ..ctx }),
        836
    );
}

#[test]
fn npc_definitions_animate_through_the_skeletal_port() {
    let Some(mut core) = core_with_actors() else {
        return;
    };
    let textures = textures();
    load_house(&mut core);
    // Goblin (own stand sequence), giant rat (2x2 footprint), chicken, Survival Expert (human 808).
    let entities = serde_json::json!([
        {"id": "goblin", "kind": "npc", "sourceId": 3028, "tile": {"x": 3097, "y": 3097, "plane": 0}, "animation": ""},
        {"id": "rat", "kind": "npc", "sourceId": 3313, "tile": {"x": 3099, "y": 3099, "plane": 0}, "animation": ""},
        {"id": "chicken", "kind": "npc", "sourceId": 3316, "tile": {"x": 3096, "y": 3096, "plane": 0}, "animation": ""},
        {"id": "expert", "kind": "npc", "sourceId": 8503, "tile": {"x": 3100, "y": 3097, "plane": 0}, "animation": ""},
        {"id": "other-instance", "kind": "npc", "sourceId": 3028, "tile": {"x": 3098, "y": 3096, "plane": 0}, "animation": "", "instance": "inst-2"}
    ]);
    core.update_world(&world((3098, 3098), "idle", "", &[], entities), 0.0)
        .unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    assert_eq!(summary.entities_drawn, 5, "{:?}", summary.entities_skipped);
    assert!(
        summary.entities_skipped.is_empty(),
        "{:?}",
        summary.entities_skipped
    );
    let first = rasterize(&core, &textures);
    core.build_frame(20.0 * 9.0).unwrap();
    let later = rasterize(&core, &textures);
    assert_ne!(first, later, "definition stand sequences did not advance");
    // Every drawn actor is pickable by its WorldView id somewhere in the frame.
    let mut found = std::collections::HashSet::new();
    for y in (300..1000).step_by(3) {
        for x in (500..1500).step_by(3) {
            if let Some(clubscape_renderer::core::WorldPick::Actor { id, .. }) =
                core.pick_world(x, y)
            {
                found.insert(id);
            }
        }
    }
    for id in ["player-1", "goblin", "rat", "chicken", "expert"] {
        assert!(
            found.contains(id),
            "actor {id} not pickable; found {found:?}"
        );
    }
    assert!(
        !found.contains("other-instance"),
        "instanced NPC outside the player's instance was drawn"
    );
}

#[test]
fn player_uses_original_action_motion_and_wears_modular_gear() {
    let Some(mut core) = core_with_actors() else {
        return;
    };
    let textures = textures();
    load_house(&mut core);
    let (map, _) = core.player_fit_report().unwrap();
    eprintln!(
        "retarget: {} human labels drive {} penguin labels, translation scale {:.3}",
        map.driven.len(),
        map.assignments.len(),
        map.translation_scale
    );
    assert!(map.translation_scale > 0.5 && map.translation_scale < 1.5);
    // Idle penguin (native 5668 through the same port) vs woodcutting (human 879 retargeted).
    core.update_world(
        &world((3098, 3098), "idle", "", &[], serde_json::json!([])),
        0.0,
    )
    .unwrap();
    core.build_frame(0.0).unwrap();
    let idle = rasterize(&core, &textures);
    core.update_world(
        &world(
            (3098, 3098),
            "gathering",
            "879",
            &[("weapon", 1351)],
            serde_json::json!([]),
        ),
        0.0,
    )
    .unwrap();
    core.build_frame(0.0).unwrap();
    let chop0 = rasterize(&core, &textures);
    core.build_frame(20.0 * 8.0).unwrap();
    let chop1 = rasterize(&core, &textures);
    assert_ne!(idle, chop0, "woodcutting pose equals idle");
    assert_ne!(chop0, chop1, "woodcutting animation did not advance");
    let (_, fits) = core.player_fit_report().unwrap();
    assert_eq!(fits.len(), 1, "axe should be attached: {fits:?}");
    eprintln!("fit: {:?}", fits[0]);
    // Every M1 equippable item attaches, with measured fit reported per slot.
    let gear = [
        ("weapon", 1277),
        ("shield", 1171),
        ("head", 1949),
        ("amulet", 1009),
        ("ammo", 882),
    ];
    core.update_world(
        &world((3098, 3098), "fighting", "", &gear, serde_json::json!([])),
        0.0,
    )
    .unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    assert_eq!(summary.entities_drawn, 1, "{:?}", summary.entities_skipped);
    let (_, fits) = core.player_fit_report().unwrap();
    // Arrows have no worn model in the original (op.jm male model -1), so four items attach.
    assert_eq!(fits.len(), 4, "{fits:?}");
    for fit in fits {
        eprintln!("fit {:?}", fit);
        assert!(fit.anchor_gap <= 2.0, "{fit:?}");
    }
    let armed = rasterize(&core, &textures);
    assert_ne!(armed, chop0);
    // Sword slash is chosen for a bronze sword; the frame differs from the unarmed punch.
    core.update_world(
        &world((3098, 3098), "fighting", "", &[], serde_json::json!([])),
        0.0,
    )
    .unwrap();
    core.build_frame(20.0 * 3.0).unwrap();
    let punch = rasterize(&core, &textures);
    core.update_world(
        &world((3098, 3098), "fighting", "", &gear, serde_json::json!([])),
        0.0,
    )
    .unwrap();
    core.build_frame(20.0 * 3.0).unwrap();
    let slash = rasterize(&core, &textures);
    assert_ne!(punch, slash);
    // Explicit server-bound animation wins over the activity default.
    core.update_world(
        &world((3098, 3098), "idle", "836", &[], serde_json::json!([])),
        0.0,
    )
    .unwrap();
    core.build_frame(20.0 * 40.0).unwrap();
    let dead = rasterize(&core, &textures);
    assert_ne!(dead, idle);
    if let Some(PickTarget::Object { .. }) = core.pick(960, 700) {}
}

fn write_png(name: &str, width: u32, height: u32, pixels: &[i32]) {
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

/// Developer preview sheet of the penguin player in retargeted action poses (visual review aid).
#[test]
fn player_pose_sheet_renders() {
    let Some(mut core) = core_with_actors() else {
        return;
    };
    let textures = textures();
    type Pose<'a> = (&'a str, i32, usize, &'a [(&'a str, i32)]);
    let poses: [Pose; 8] = [
        ("idle-808", 808, 0, &[]),
        ("walk-819", 819, 3, &[]),
        ("woodcut-879", 879, 2, &[("weapon", 1351)]),
        ("mine-625", 625, 5, &[("weapon", 1265)]),
        ("fish-621", 621, 10, &[]),
        (
            "slash-390",
            390,
            2,
            &[
                ("weapon", 1277),
                ("shield", 1171),
                ("head", 1949),
                ("amulet", 1009),
            ],
        ),
        ("bow-426", 426, 4, &[("weapon", 841)]),
        ("death-836", 836, 9, &[]),
    ];
    let mut sheet = vec![0x30_3030; 1920 * 1080];
    for (i, (name, sequence, frame, gear)) in poses.iter().enumerate() {
        let model = core
            .player_model_for_preview(sequence.to_owned(), *frame, gear)
            .unwrap()
            .expect(name);
        core.load_model_value("pose", model);
        core.build_model_fixture_frame(&clubscape_renderer::core::ModelFixture {
            model: "pose".into(),
            npc: None,
            yaw: 256,
            camera_y: 160,
            camera_z: 400,
        })
        .unwrap();
        let pixels = rasterize(&core, &textures);
        // Crop the centre 480x540 region of each render into a 4x2 sheet.
        let (cx, cy) = (960 - 240, 540 - 320);
        let (ox, oy) = ((i % 4) * 480, (i / 4) * 540);
        for y in 0..540 {
            for x in 0..480 {
                sheet[(oy + y) * 1920 + ox + x] = pixels[(cy + y) * 1920 + cx + x];
            }
        }
        eprintln!("pose {name}: {} triangles", core.triangles().len());
    }
    write_png("player-pose-sheet", 1920, 1080, &sheet);
}

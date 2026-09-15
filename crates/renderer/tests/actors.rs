//! Actors through the skeletal animation port: NPC definitions with their original stand/walk
//! sequences, the penguin player with retargeted human action sequences, modular equipment
//! attachment with measured fit, instance filtering and picking by WorldView id.

mod common;

use clubscape_renderer::actor::{ActivityContext, player_sequence_for};
use clubscape_renderer::core::{Camera, PlayerPreview, RendererCore};
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

/// Manifest file keys are relative to assets/compiled/render.
fn asset(key: &str) -> Vec<u8> {
    common::read_asset(key)
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
/// These are published manifest sections; a manifest without them is a broken checkout.
fn core_with_actors() -> Option<RendererCore> {
    let manifest = manifest();
    for section in [
        "sequences",
        "npc_definitions",
        "equipment_items",
        "player_reference",
    ] {
        assert!(
            manifest.get(section).is_some(),
            "manifest section {section} missing: assets/compiled/render is incomplete \
             (reproduce with tools/render-assets/export.py --profile anim)"
        );
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

/// The developer fallback table (activity + surroundings → original motion) is opt-in and
/// not final M1 logic; it still has to name the bound source sequences it stands in for.
#[test]
fn developer_activity_fallback_names_the_bound_source_sequences() {
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

/// Motion identity comes only from explicit data: `animation` (bare or catalog sequence id),
/// animation events, the `run` setting / two-tiles-per-tick rule. Missing motion is reported,
/// never guessed, unless the developer fallback is switched on.
#[test]
fn motion_identity_is_explicit_or_reported_unknown() {
    use clubscape_renderer::core::parse_sequence_id;
    assert_eq!(parse_sequence_id("879"), Some(879));
    assert_eq!(parse_sequence_id(" 879 "), Some(879));
    assert_eq!(parse_sequence_id("sequence.879"), Some(879));
    assert_eq!(
        parse_sequence_id("asset.source.osrs.cache2695.sequence.879"),
        Some(879)
    );
    assert_eq!(
        parse_sequence_id("asset.source.osrs.cache2695.object.879"),
        None,
        "not a sequence"
    );
    assert_eq!(parse_sequence_id(""), None);
    assert_eq!(parse_sequence_id("-1"), None);
    assert_eq!(parse_sequence_id("chop"), None);

    let Some(mut core) = core_with_actors() else {
        return;
    };
    let textures = textures();
    load_house(&mut core);
    let mut view: serde_json::Value = serde_json::from_str(&world(
        (3098, 3098),
        "idle",
        "",
        &[("weapon", 1351)],
        serde_json::json!([]),
    ))
    .unwrap();
    // Idle: no diagnostic, stand sequence.
    core.update_world(&view.to_string(), 0.0).unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    assert!(
        summary.entities_skipped.is_empty(),
        "{:?}",
        summary.entities_skipped
    );
    let idle = rasterize(&core, &textures);
    // Gathering without a source animation: explicit unknown-motion diagnostic, stand stance,
    // no invented woodcutting.
    view["player"]["activity"] = serde_json::json!("gathering");
    view["entities"] = serde_json::json!([{"id": "tree", "kind": "object", "sourceId": 1276, "tile": {"x": 3099, "y": 3098, "plane": 0}, "animation": ""}]);
    core.update_world(&view.to_string(), 0.0).unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    assert!(
        summary
            .entities_skipped
            .iter()
            .any(|s| s.starts_with("motion unknown: player-1: activity \"gathering\"")),
        "{:?}",
        summary.entities_skipped
    );
    assert_eq!(core.unknown_motions().len(), 1);
    assert_eq!(
        rasterize(&core, &textures),
        idle,
        "unknown motion must not invent a pose"
    );
    // Catalog sequence id in player.animation: the source motion plays, no diagnostic.
    view["player"]["animation"] = serde_json::json!("asset.source.osrs.cache2695.sequence.879");
    core.update_world(&view.to_string(), 0.0).unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    assert!(
        summary.entities_skipped.is_empty(),
        "{:?}",
        summary.entities_skipped
    );
    let chop = rasterize(&core, &textures);
    assert_ne!(chop, idle);
    // Animation event (shell extension) for the player: same result through the event channel.
    view["player"]["animation"] = serde_json::json!("");
    view["events"] = serde_json::json!([{"kind": "animation", "eventId": "evt-1", "actorId": "player-1", "animationAsset": "asset.source.osrs.cache2695.sequence.879"}]);
    core.update_world(&view.to_string(), 0.0).unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    assert!(
        summary.entities_skipped.is_empty(),
        "{:?}",
        summary.entities_skipped
    );
    assert_eq!(
        rasterize(&core, &textures),
        chop,
        "event-driven motion equals the named motion"
    );
    // An event naming a non-sequence asset is reported, not applied.
    view["events"] = serde_json::json!([{"kind": "animation", "eventId": "evt-2", "actorId": "player-1", "animationAsset": "asset.source.osrs.cache2695.object.1276"}]);
    core.update_world(&view.to_string(), 0.0).unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    assert!(
        summary
            .entities_skipped
            .iter()
            .any(|s| s.contains("names no source sequence")),
        "{:?}",
        summary.entities_skipped
    );
    // Dead player without a source animation: reported, no invented death pose.
    view["events"] = serde_json::json!([]);
    view["player"]["activity"] = serde_json::json!("idle");
    view["player"]["hitpoints"] = serde_json::json!(0);
    core.update_world(&view.to_string(), 0.0).unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    assert!(
        summary
            .entities_skipped
            .iter()
            .any(|s| s.contains("hitpoints 0")),
        "{:?}",
        summary.entities_skipped
    );
    assert_eq!(rasterize(&core, &textures), idle);
    // Developer fallback on: the activity table applies (and is labelled as such by the flag).
    core.set_motion_fallback(true);
    view["player"]["hitpoints"] = serde_json::json!(10);
    view["player"]["activity"] = serde_json::json!("gathering");
    core.update_world(&view.to_string(), 0.0).unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    assert!(
        summary.entities_skipped.is_empty(),
        "{:?}",
        summary.entities_skipped
    );
    assert_eq!(rasterize(&core, &textures), chop);
    core.set_motion_fallback(false);

    // Running: two tiles in one server tick plays 824, one tile plays 819; the run setting
    // decides only when several ticks elapsed and the step count is between.
    let step = |core: &mut RendererCore, tick: i64, x: i32, run: Option<bool>| {
        let mut v: serde_json::Value =
            serde_json::from_str(&world((x, 3098), "walking", "", &[], serde_json::json!([])))
                .unwrap();
        v["tick"] = serde_json::json!(tick.to_string());
        if let Some(run) = run {
            v["player"]["settings"] = serde_json::json!([{"setting": "run", "enabled": run}]);
        }
        core.update_world(&v.to_string(), 0.0).unwrap();
        core.build_frame(0.0).unwrap();
        core.player_running()
    };
    step(&mut core, 10, 3090, None);
    assert!(!step(&mut core, 11, 3091, None), "one tile per tick walks");
    assert!(step(&mut core, 12, 3093, None), "two tiles per tick runs");
    assert!(
        step(&mut core, 14, 3097, None),
        "four tiles over two ticks runs"
    );
    assert!(
        !step(&mut core, 16, 3099, None),
        "two tiles over two ticks walks"
    );
    assert!(
        step(&mut core, 18, 3102, Some(true)),
        "three tiles over two ticks: run setting decides"
    );
    assert!(!step(&mut core, 20, 3105, Some(false)));
    assert!(
        !step(&mut core, 22, 3108, None),
        "ambiguous without a setting keeps the last state"
    );
    // No tick information at all: only the setting can say.
    let mut v: serde_json::Value = serde_json::from_str(&world(
        (3110, 3098),
        "walking",
        "",
        &[],
        serde_json::json!([]),
    ))
    .unwrap();
    v["tick"] = serde_json::json!("");
    v["player"]["settings"] = serde_json::json!([{"setting": "run", "enabled": true}]);
    core.update_world(&v.to_string(), 0.0).unwrap();
    assert!(core.player_running());
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
        &world(
            (3098, 3098),
            "fighting",
            "390",
            &gear,
            serde_json::json!([]),
        ),
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
        assert!(
            fit.penetration <= clubscape_renderer::actor::FIT_MAX_PENETRATION,
            "{fit:?}"
        );
        assert!(fit.gap <= clubscape_renderer::actor::FIT_MAX_GAP, "{fit:?}");
    }
    let armed = rasterize(&core, &textures);
    assert_ne!(armed, chop0);
    // The server names the combat motion: punch (422) and sword slash (390) differ.
    core.update_world(
        &world((3098, 3098), "fighting", "422", &[], serde_json::json!([])),
        0.0,
    )
    .unwrap();
    core.build_frame(20.0 * 3.0).unwrap();
    let punch = rasterize(&core, &textures);
    core.update_world(
        &world(
            (3098, 3098),
            "fighting",
            "390",
            &gear,
            serde_json::json!([]),
        ),
        0.0,
    )
    .unwrap();
    core.build_frame(20.0 * 3.0).unwrap();
    let slash = rasterize(&core, &textures);
    assert_ne!(punch, slash);
    // Server-bound death animation.
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

/// Live layers: ground items, the animated fire temporary object, door states and roof removal.
#[test]
fn dynamic_layers_draw_from_the_world_view() {
    let manifest = manifest();
    assert!(
        manifest.get("dynamic_objects").is_some() && manifest.get("ground_items").is_some(),
        "manifest sections dynamic_objects/ground_items missing (export.py --profile dynamic)"
    );
    let Some(mut core) = core_with_actors() else {
        return;
    };
    let textures = textures();
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
    load_house(&mut core);
    core.update_world(
        &world((3098, 3098), "idle", "", &[], serde_json::json!([])),
        0.0,
    )
    .unwrap();
    core.build_frame(0.0).unwrap();
    let baseline = rasterize(&core, &textures);
    let baseline_tris = core.triangles().len();

    // Ground items: logs, coins (quantity variant) and shrimps on one tile → top three drawn.
    let mut view: serde_json::Value =
        serde_json::from_str(&world((3098, 3098), "idle", "", &[], serde_json::json!([]))).unwrap();
    view["groundItems"] = serde_json::json!([
        {"id": "g1", "tile": {"x": 3096, "y": 3099, "plane": 0}, "item": {"id": "item.logs", "name": "Logs", "quantity": 1, "sourceId": 1511}, "canTake": true},
        {"id": "g2", "tile": {"x": 3096, "y": 3099, "plane": 0}, "item": {"id": "item.coins", "name": "Coins", "quantity": 250, "sourceId": 995}, "canTake": true},
        {"id": "g3", "tile": {"x": 3097, "y": 3100, "plane": 0}, "item": {"id": "item.shrimps", "name": "Raw shrimps", "quantity": 1, "sourceId": 317}, "canTake": true}
    ]);
    core.update_world(&view.to_string(), 0.0).unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    assert!(
        summary.entities_skipped.is_empty(),
        "{:?}",
        summary.entities_skipped
    );
    let with_items = rasterize(&core, &textures);
    assert_ne!(with_items, baseline, "ground items drew nothing");
    assert!(core.triangles().len() > baseline_tris);
    let mut item_pick = None;
    for y in (400..1000).step_by(2) {
        for x in (600..1400).step_by(2) {
            if let Some(clubscape_renderer::core::WorldPick::Scenery {
                object_id,
                kind,
                x: tx,
                y: ty,
                ..
            }) = core.pick_world(x, y)
                && kind == 3
            {
                item_pick = Some((object_id, tx, ty));
            }
        }
    }
    assert!(
        matches!(
            item_pick,
            Some((1511, 3096, 3099)) | Some((995, 3096, 3099)) | Some((317, 3097, 3100))
        ),
        "ground item pick {item_pick:?}"
    );

    // Fire temporary object: animated through its exported frames.
    view["groundItems"] = serde_json::json!([]);
    view["entities"] = serde_json::json!([
        {"id": "dynamic_object.fire_1", "kind": "temporary_object", "definitionId": "object.fire.normal", "sourceId": 26185, "name": "Fire",
         "tile": {"x": 3097, "y": 3096, "plane": 0}, "animation": "", "available": true, "actions": [], "appearance": {}, "equipment": []}
    ]);
    core.update_world(&view.to_string(), 0.0).unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    assert!(
        summary.entities_skipped.is_empty(),
        "{:?}",
        summary.entities_skipped
    );
    let fire0 = rasterize(&core, &textures);
    core.build_frame(20.0 * 7.0).unwrap();
    let fire1 = rasterize(&core, &textures);
    assert_ne!(fire0, baseline, "fire drew nothing");
    assert_ne!(fire0, fire1, "fire did not animate");

    // Door state: the starting-house door (object 9398 at 3098,3107) opens by one quarter turn.
    view["entities"] = serde_json::json!([]);
    view["dynamicObjects"] = serde_json::json!([
        {"id": "transform.scenery.start_door", "objectId": "asset.source.osrs.cache2695.object.9398", "tile": {"x": 3098, "y": 3107, "plane": 0},
         "instance": null, "state": "object_state.open", "doorOpen": true, "quarterTurns": 1}
    ]);
    core.update_world(&view.to_string(), 0.0).unwrap();
    let summary = core.build_frame(0.0).unwrap().clone();
    assert!(
        summary.entities_skipped.is_empty(),
        "{:?}",
        summary.entities_skipped
    );
    let door_open = rasterize(&core, &textures);
    assert_ne!(door_open, baseline, "open door did not change the wall");
    write_png("dynamic-layers-door-open", 1920, 1080, &door_open);

    // Roof removal mode 1 hides the roof over the player's tile inside the house.
    view["dynamicObjects"] = serde_json::json!([]);
    view["player"]["tile"] = serde_json::json!({"x": 3094, "y": 3106, "plane": 0});
    core.update_world(&view.to_string(), 0.0).unwrap();
    core.set_roof_mode(0);
    core.build_frame(0.0).unwrap();
    let roofs_on = rasterize(&core, &textures);
    let on_tris = core.triangles().len();
    core.set_roof_mode(1);
    core.build_frame(0.0).unwrap();
    let roofs_off = rasterize(&core, &textures);
    eprintln!(
        "roof triangles: visible {on_tris}, removed {}",
        core.triangles().len()
    );
    assert_ne!(
        roofs_on, roofs_off,
        "roof removal mode 1 changed nothing over the player"
    );
    write_png("dynamic-layers-roof-removed", 1920, 1080, &roofs_off);
    core.set_roof_mode(0);
}

/// The character-creator preview: interface 679 component 73's model draw (zoom 512, model zoom
/// 450, offsetY2 175, rotations 0) centred on the component inside its 480x315 parent layer.
#[test]
fn player_preview_reproduces_the_interface_model_draw() {
    let Some(mut core) = core_with_actors() else {
        return;
    };
    let textures = textures();
    let manifest = manifest();
    let widget = manifest["model_widgets"]
        .as_array()
        .and_then(|w| w.iter().find(|w| w["id"] == 44499017))
        .expect("exported interface 679 component 73 (export.py --profile widgets)");
    // The exported source fields are the defaults the preview uses.
    let defaults = PlayerPreview::default();
    assert_eq!(
        widget["model_zoom"].as_i64().unwrap() as i32,
        defaults.model_zoom
    );
    assert_eq!(
        widget["offset_y2"].as_i64().unwrap() as i32,
        defaults.offset_y
    );
    assert_eq!(
        widget["offset_x2"].as_i64().unwrap() as i32,
        defaults.offset_x
    );
    assert_eq!(
        widget["rotation_x"].as_i64().unwrap() as i32,
        defaults.rotation_x
    );
    assert_eq!(
        widget["rotation_z"].as_i64().unwrap() as i32,
        defaults.rotation_z
    );
    assert_eq!(
        widget["content_type"].as_i64().unwrap() as i32,
        defaults.content_type
    );
    // Content type 328: pitch 150 and the client-cycle sway (cycle 0 → 0, cycle 63 → 255).
    assert_eq!(defaults.draw_rotation(0.0), (150, 0));
    assert_eq!(
        defaults.draw_rotation(63.0 * 20.0),
        (150, ((63.0f64 / 40.0).sin() * 256.0) as i32)
    );
    assert_eq!(
        defaults.draw_rotation(200.0 * 20.0),
        (150, (((200.0f64 / 40.0).sin() * 256.0) as i32) & 2047)
    );
    assert!(
        defaults.draw_rotation(200.0 * 20.0).1 > 1024,
        "negative sway must wrap into 0..2047"
    );
    assert_eq!(
        widget["rasterizer_zoom"].as_i64().unwrap() as i32,
        defaults.rasterizer_zoom
    );
    assert!(!widget["ortho"].as_bool().unwrap());
    assert_eq!(
        (
            widget["width"].as_i64().unwrap(),
            widget["height"].as_i64().unwrap()
        ),
        (136, 192)
    );

    // No world yet (character creation): the naked approved body at the idle motion.
    let frame = core
        .build_player_preview_frame(&defaults, 0.0)
        .unwrap()
        .expect("player body loaded");
    assert_eq!(frame.sequence, clubscape_renderer::actor::PLAYER_IDLE);
    assert!(
        frame.summary.triangles > 100,
        "preview drew {} triangles",
        frame.summary.triangles
    );
    let state = frame.state;
    assert_eq!(
        (
            state.width,
            state.height,
            state.center_x,
            state.center_y,
            state.zoom
        ),
        (480, 315, 240, 187, 512)
    );
    let render = |core: &RendererCore, clear: i32| {
        let mut pixels = vec![clear; (480 * 315) as usize];
        {
            let mut raster = Software::new(state, &mut pixels, &core.palette.rgb, &textures);
            for tri in core.preview_triangles() {
                let _ = raster.draw(tri);
            }
        }
        pixels
    };
    let dark = render(&core, 0x010203);
    let light = render(&core, 0xFEFDFC);
    let covered: Vec<bool> = dark
        .iter()
        .zip(&light)
        .map(|(a, b)| !((a & 0xFFFFFF) == 0x010203 && (b & 0xFFFFFF) == 0xFEFDFC))
        .collect();
    let count = covered.iter().filter(|c| **c).count();
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
    for (i, c) in covered.iter().enumerate() {
        if *c {
            let (x, y) = ((i % 480) as i32, (i / 480) as i32);
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
    }
    eprintln!("preview coverage {count} px, bounds x {min_x}..{max_x} y {min_y}..{max_y}");
    assert!(count > 800, "preview covered only {count} pixels");
    // At sway phase 0 the body faces the viewer, horizontally centred on the component; with
    // pitch 150 and offsetY2 175 its feet project to about 245 px, inside the 91..283 component.
    assert!(
        (min_x + max_x) / 2 >= 240 - 8 && (min_x + max_x) / 2 <= 240 + 8,
        "preview off-centre: {min_x}..{max_x}"
    );
    assert!(
        max_y <= 283 && min_y >= 91,
        "preview leaves the 136x192 component: {min_y}..{max_y}"
    );
    assert!(
        (235..=255).contains(&max_y),
        "feet not where the original projection puts them: {max_y}"
    );
    let mut png = vec![0; dark.len()];
    for (i, p) in png.iter_mut().enumerate() {
        *p = if covered[i] {
            dark[i] & 0xFFFFFF
        } else {
            0x303030
        };
    }
    write_png("player-preview-679-73", 480, 315, &png);

    // A frame override and a walk sequence produce different, still in-bounds images.
    let walking = PlayerPreview {
        sequence: Some(clubscape_renderer::actor::PLAYER_WALK),
        frame: Some(2),
        ..PlayerPreview::default()
    };
    core.build_player_preview_frame(&walking, 0.0)
        .unwrap()
        .unwrap();
    let walk = render(&core, 0x010203);
    assert_ne!(walk, dark, "walk frame identical to the idle preview");
    // The scene raster state and picks are untouched by preview rendering.
    assert_eq!((core.state.width, core.state.height), (1920, 1080));

    // Gear from the world view appears in the preview (bronze axe in the weapon slot).
    load_house(&mut core);
    core.update_world(
        &world(
            (3098, 3098),
            "idle",
            "",
            &[("weapon", 1351)],
            serde_json::json!([]),
        ),
        0.0,
    )
    .unwrap();
    let with_axe = core
        .build_player_preview_frame(
            &PlayerPreview {
                frame: Some(0),
                ..PlayerPreview::default()
            },
            0.0,
        )
        .unwrap()
        .unwrap();
    let axe = render(&core, 0x010203);
    let idle0 = {
        core.update_world(
            &world((3098, 3098), "idle", "", &[], serde_json::json!([])),
            0.0,
        )
        .unwrap();
        core.build_player_preview_frame(
            &PlayerPreview {
                frame: Some(0),
                ..PlayerPreview::default()
            },
            0.0,
        )
        .unwrap()
        .unwrap();
        render(&core, 0x010203)
    };
    assert!(
        with_axe.summary.triangles > frame.summary.triangles,
        "gear added no triangles"
    );
    assert_ne!(axe, idle0, "gear did not change the preview");
}

/// The stock top-plane rule (`cz.ch`): every plane is drawn until a roof-flagged tile of the
/// player's plane lies on the camera tile or the camera→player line (pitch < 2480), then only the
/// player's plane; the approved fixture captures pinned the `dh` plane argument to 0 instead.
#[test]
fn stock_top_plane_rule_hides_upper_planes_under_roofs() {
    let Some(mut core) = core_with_actors() else {
        return;
    };
    let textures = textures();
    load_house(&mut core);
    // Fixture reproduction: draw plane 0, the approved capture inputs.
    core.set_top_plane_override(Some(0));
    core.update_world(
        &world((3098, 3098), "idle", "", &[], serde_json::json!([])),
        0.0,
    )
    .unwrap();
    core.build_frame(0.0).unwrap();
    let pinned = core.triangles().len();
    let pinned_pixels = rasterize(&core, &textures);
    // Stock rule with the player outside (camera at 3094,3095 looking north over open ground):
    // upper planes (the house roof and walls) are drawn.
    core.set_top_plane_override(None);
    core.build_frame(0.0).unwrap();
    let outside = core.triangles().len();
    let outside_pixels = rasterize(&core, &textures);
    assert!(
        outside > pinned,
        "stock rule drew {outside} triangles vs pinned plane 0 {pinned}"
    );
    assert_ne!(outside_pixels, pinned_pixels);
    write_png("top-plane-stock-outside", 1920, 1080, &outside_pixels);
    // Player inside the starting house (roof-flagged tile): the top plane drops to plane 0 again.
    core.update_world(
        &world((3094, 3106), "idle", "", &[], serde_json::json!([])),
        0.0,
    )
    .unwrap();
    core.build_frame(0.0).unwrap();
    let inside = core.triangles().len();
    let inside_pixels = rasterize(&core, &textures);
    assert!(
        inside < outside,
        "player under the roof still drew {inside} triangles (outside {outside})"
    );
    write_png("top-plane-stock-inside", 1920, 1080, &inside_pixels);
    // A player outside but with the camera line crossing the house also hides the upper planes.
    core.set_camera(Camera {
        x: 3048 * 128 + (3094 - 3048) * 128 + 64,
        height: -2360,
        y: 3056 * 128 + (3112 - 3056) * 128,
        pitch: 2048,
        yaw: 8192,
        zoom: 662,
        far: 32768,
    })
    .unwrap();
    core.update_world(
        &world((3094, 3100), "idle", "", &[], serde_json::json!([])),
        0.0,
    )
    .unwrap();
    core.build_frame(0.0).unwrap();
    let crossing = core.triangles().len();
    let crossing_pixels = rasterize(&core, &textures);
    write_png("top-plane-stock-camera-line", 1920, 1080, &crossing_pixels);
    core.set_camera(Camera {
        x: 3048 * 128 + (3094 - 3048) * 128 + 64,
        height: -2360,
        y: 3056 * 128 + (3112 - 3056) * 128,
        pitch: 2048 + 1024,
        zoom: 662,
        yaw: 8192,
        far: 32768,
    })
    .unwrap();
    core.build_frame(0.0).unwrap();
    let steep = core.triangles().len();
    eprintln!(
        "top plane triangles: pinned0 {pinned}, stock outside {outside}, inside {inside}, camera line crossing {crossing}, steep pitch {steep}"
    );
    assert!(
        steep > crossing,
        "pitch >= 2480 must not apply the roof line rule ({steep} vs {crossing})"
    );
    // Instanced maps always draw up to the player's plane (same steep camera: fewer triangles).
    core.set_instanced_map(true);
    core.build_frame(0.0).unwrap();
    let instanced = core.triangles().len();
    assert!(
        instanced < steep,
        "instanced map still drew upper planes ({instanced} vs {steep})"
    );
    core.set_instanced_map(false);
}

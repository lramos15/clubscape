mod support;

use std::{
    collections::BTreeMap,
    io::Read,
    sync::{Arc, OnceLock},
};

use clubscape_content::{ValidationMode, load_compiled, sha256};
use clubscape_game_types::*;
use clubscape_simulation::navigation::CollisionMap;
use clubscape_world_engine::{LifecycleTransition, WorldEngine};
use support::{v2 as v, *};

fn fixture() -> GameContent {
    let mut content = v::content();
    add_item_spawn(&mut content);
    content.spawns.get_mut(&spawn("egg")).unwrap().tile = tile(11, 10, 0);
    let target = cell(&mut content, tile(11, 10, 0));
    target.walkable = false;
    target.blocked_movement = u8::MAX;
    target.blocked_sight = 0;
    content
}

fn ground(world: &WorldState) -> String {
    world
        .ground_items
        .iter()
        .find(|item| item.stack == stack("egg", 1))
        .unwrap()
        .id
        .clone()
}

fn take(id: &str) -> GameIntent {
    GameIntent::TakeGroundItem {
        ground_item_id: id.into(),
    }
}

#[test]
fn declared_blocked_spawn_is_takeable_at_its_clear_cardinal_face_without_moving() {
    let (engine, mut world) = setup(fixture());
    let id = ground(&world);
    let before = world.clone();
    let view = engine.ground_item_views(&world, &actor()).unwrap();
    assert!(
        world == before,
        "Ground permissions must not mutate the world"
    );
    assert!(
        view.iter()
            .find(|item| item.id == id)
            .unwrap()
            .can_take
            .allowed
    );
    let amount = count(&engine, &world, "egg");
    apply(&engine, &mut world, take(&id));
    assert_eq!(count(&engine, &world, "egg"), amount + 1);
    assert_eq!(state(&world).tile, state(&before).tile);
    assert_eq!(state(&world).skills, state(&before).skills);
    assert_eq!(state(&world).bank, state(&before).bank);
    assert!(!world.ground_items.iter().any(|item| item.id == id));
    assert_eq!(
        world.entities[&spawn("egg")].available_at_tick,
        world.tick + 3
    );
    let map = CollisionMap::from_regions(engine.content().regions.values()).unwrap();
    assert!(!map.can_step(tile(10, 10, 0), tile(11, 10, 0)));
}

#[test]
fn blocked_spawn_contact_keeps_walls_sight_planes_and_cardinal_range() {
    for case in [
        "near_movement",
        "near_sight",
        "target_sight",
        "opaque",
        "partial_target",
        "far",
        "diagonal",
        "plane",
        "blocked_near",
    ] {
        let mut content = fixture();
        match case {
            "near_movement" => {
                cell(&mut content, tile(10, 10, 0)).blocked_movement = Direction::East.mask()
            }
            "near_sight" => {
                cell(&mut content, tile(10, 10, 0)).blocked_sight = Direction::East.mask()
            }
            "target_sight" => {
                cell(&mut content, tile(11, 10, 0)).blocked_sight = Direction::West.mask()
            }
            "opaque" => cell(&mut content, tile(11, 10, 0)).blocked_sight = u8::MAX,
            "partial_target" => {
                cell(&mut content, tile(11, 10, 0)).blocked_movement = Direction::West.mask()
            }
            "far" => content.initial_state.tile = tile(13, 10, 0),
            "diagonal" => content.initial_state.tile = tile(10, 11, 0),
            "plane" => {
                content.initial_state.region = RegionId::new("region.synthetic.floor_1").unwrap();
                content.initial_state.tile = tile(10, 10, 1);
            }
            "blocked_near" => {
                let near = cell(&mut content, tile(11, 11, 0));
                near.walkable = false;
                near.blocked_movement = u8::MAX;
            }
            _ => unreachable!(),
        }
        let (engine, mut world) = setup(content);
        if case == "blocked_near" {
            state_mut(&mut world).tile = tile(11, 11, 0);
        }
        let id = ground(&world);
        let before = world.clone();
        assert!(
            !engine
                .ground_item_views(&world, &actor())
                .unwrap()
                .iter()
                .any(|item| item.id == id && item.can_take.allowed),
            "{case}"
        );
        assert!(world == before);
        error_unchanged(&engine, &mut world, take(&id), GameErrorCode::OutOfReach);
    }
}

#[test]
fn ordinary_ground_and_walkable_spawns_do_not_gain_adjacent_pickup() {
    for ordinary in [false, true] {
        let mut content = fixture();
        if !ordinary {
            let target = cell(&mut content, tile(11, 10, 0));
            target.walkable = true;
            target.blocked_movement = 0;
        }
        let (engine, mut world) = setup(content);
        if ordinary {
            let id = ground(&world);
            world
                .ground_items
                .iter_mut()
                .find(|item| item.id == id)
                .unwrap()
                .id = "ordinary-ground-fixture".into();
        }
        let id = ground(&world);
        error_unchanged(&engine, &mut world, take(&id), GameErrorCode::OutOfReach);
        if !ordinary {
            state_mut(&mut world).tile = tile(11, 10, 0);
            apply(&engine, &mut world, take(&id));
        }
    }
}

#[test]
fn a_source_prefix_cannot_rebind_a_different_stack_or_location() {
    for case in ["stack", "item_location", "source_entity_and_item_location"] {
        let (engine, mut world) = setup(fixture());
        let id = ground(&world);
        let entry = world
            .ground_items
            .iter_mut()
            .find(|item| item.id == id)
            .unwrap();
        if case == "stack" {
            entry.stack = stack("pot", 1);
        } else {
            entry.tile = tile(10, 11, 0);
        }
        if case == "source_entity_and_item_location" {
            world.entities.get_mut(&spawn("egg")).unwrap().tile = tile(10, 11, 0);
        }
        let before = world.clone();
        assert!(engine.ground_item_views(&world, &actor()).is_err());
        assert!(world == before);
        assert!(
            engine
                .apply_intent(&mut world, &actor(), &take(&id), &mut NeverDraw)
                .is_err()
        );
        assert!(world == before);
    }
}

#[test]
fn source_contact_does_not_bypass_private_expired_or_full_inventory_checks() {
    for case in ["private", "expired", "full", "instance"] {
        let (engine, mut world) = setup(fixture());
        let id = ground(&world);
        let entry = world
            .ground_items
            .iter_mut()
            .find(|item| item.id == id)
            .unwrap();
        let error = match case {
            "private" => {
                entry.owner = Some(actor_two());
                entry.public_at_tick = world.tick + 10;
                GameErrorCode::NotOwned
            }
            "expired" => {
                entry.expires_at_tick = world.tick;
                GameErrorCode::NotOwned
            }
            "instance" => {
                entry.instance = Some(InstanceId::new("instance.other").unwrap());
                GameErrorCode::InvalidInput
            }
            "full" => {
                state_mut(&mut world)
                    .inventory
                    .slots
                    .fill(Some(stack("egg", 1)));
                GameErrorCode::InventoryFull
            }
            _ => unreachable!(),
        };
        let before = world.clone();
        if case == "instance" {
            assert!(engine.ground_item_views(&world, &actor()).is_err());
            assert!(world == before);
            error_unchanged(&engine, &mut world, take(&id), error);
            continue;
        }
        assert!(
            !engine
                .ground_item_views(&world, &actor())
                .unwrap()
                .iter()
                .any(|item| item.id == id && item.can_take.allowed),
            "{case}"
        );
        assert!(world == before);
        error_unchanged(&engine, &mut world, take(&id), error);
    }
}

fn source_content() -> Arc<GameContent> {
    static CONTENT: OnceLock<Arc<GameContent>> = OnceLock::new();
    CONTENT
        .get_or_init(|| {
            let mut bytes = Vec::new();
            flate2::read::GzDecoder::new(
                &include_bytes!("../../../content/m1/game-content.csc.gz")[..],
            )
            .read_to_end(&mut bytes)
            .unwrap();
            let manifest: serde_json::Value =
                serde_json::from_slice(include_bytes!("../../../content/m1/manifest.json"))
                    .unwrap();
            assert_eq!(
                sha256(&bytes),
                manifest["compiled_artifact"]["uncompressed_sha256"]
                    .as_str()
                    .unwrap()
            );
            Arc::new(
                load_compiled(&bytes, ValidationMode::Runtime)
                    .unwrap()
                    .definition()
                    .clone(),
            )
        })
        .clone()
}

#[test]
fn both_required_source_counter_spawns_keep_exact_geometry_and_respawn_on_pickup() {
    let content = source_content();
    for (id, near) in [
        ("spawn.pot.3209.3214.p0", tile(3209, 3213, 0)),
        ("spawn.bucket.3216.9625.p0", tile(3216, 9624, 0)),
    ] {
        let spawn = SpawnId::new(id).unwrap();
        let definition = &content.spawns[&spawn];
        let SpawnKind::Item {
            stack,
            respawn_ticks,
        } = &definition.kind
        else {
            panic!("Source item required");
        };
        let engine = WorldEngine::new(content.clone()).unwrap();
        let mut world = engine.initial_world().unwrap();
        let mut character = engine
            .character_from_initial(
                actor(),
                "Source contact fixture",
                BTreeMap::from([("body_type".into(), 0)]),
            )
            .unwrap();
        // Controlled source preconditions, not a player journey or an archive restore.
        character.tutorial_stage = StageId::new("stage.tutorial.mainland").unwrap();
        character.runtime.settings.experience =
            Some(ExperienceId::new("experience.brand_new").unwrap());
        character.region = definition.region.clone();
        character.tile = near;
        character.runtime.ui.as_mut().unwrap().active_interface = None;
        world.characters.insert(actor(), character);
        engine
            .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
            .unwrap();
        let ground = world
            .ground_items
            .iter()
            .find(|item| item.tile == definition.tile && &item.stack == stack)
            .unwrap()
            .id
            .clone();
        let before = world.clone();
        let view = engine.ground_item_views(&world, &actor()).unwrap();
        assert!(
            world == before,
            "{id}: source permission query changed state"
        );
        assert!(
            view.iter()
                .find(|item| item.id == ground)
                .unwrap()
                .can_take
                .allowed,
            "{id}"
        );
        let map = CollisionMap::from_regions(content.regions.values()).unwrap();
        assert!(!map.cell(definition.tile).unwrap().walkable);
        assert!(!map.can_step(near, definition.tile));
        apply(&engine, &mut world, take(&ground));
        assert_eq!(world.characters[&actor()].tile, near);
        assert_eq!(
            world.characters[&actor()].skills,
            before.characters[&actor()].skills
        );
        assert_eq!(
            world.entities[&spawn].available_at_tick,
            world.tick + u64::from(*respawn_ticks)
        );
        assert_eq!(world.entities[&spawn].tile, definition.tile);
        assert!(!world.ground_items.iter().any(|item| item.id == ground));
        assert_eq!(
            clubscape_simulation::inventory::count(
                &world.characters[&actor()].inventory,
                &content.items,
                &stack.item
            )
            .unwrap(),
            stack.quantity.get()
        );
    }
}

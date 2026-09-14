mod support;

use std::{collections::BTreeMap, sync::Arc};

use clubscape_game_types::*;
use clubscape_world_engine::WorldEngine;
use support::*;

#[test]
fn initial_state_is_copied_without_invented_grants_or_progress() {
    let mut content = content();
    content
        .initial_state
        .bank
        .slots
        .push(Some(stack("coins", 25)));
    content.initial_state.flags.insert("source_flag".into(), 17);
    let expected = content.initial_state.clone();
    let engine = WorldEngine::new(Arc::new(content)).unwrap();
    let appearance = BTreeMap::from([("synthetic_coat".into(), 4)]);
    let character = engine
        .character_from_initial(actor(), "Test name", appearance.clone())
        .unwrap();
    assert_eq!(character.inventory, expected.inventory);
    assert_eq!(character.equipment, expected.equipment);
    assert_eq!(character.bank, expected.bank);
    assert_eq!(character.skills, expected.skills);
    assert_eq!(character.flags, expected.flags);
    assert_eq!(character.tutorial_stage, expected.tutorial_stage);
    assert_eq!(character.appearance, appearance);
    assert_eq!(character.quest_points, 0);
    assert!(matches!(character.activity, Activity::Idle));
    assert!(engine.initial_world().unwrap().characters.is_empty());
}

#[test]
fn actor_authentication_and_world_revision_remain_caller_owned() {
    let (engine, mut world) = setup(content());
    world.revision = 91;
    state_mut(&mut world).last_command_sequence = 23;
    let before = world.clone();
    assert_eq!(
        engine
            .apply_intent(&mut world, &actor_two(), &interact("rock"), &mut NeverDraw)
            .unwrap_err()
            .code,
        GameErrorCode::NotOwned
    );
    assert_eq!(world, before);
    let events = apply(
        &engine,
        &mut world,
        GameIntent::OpenInterface {
            interface: interface(),
        },
    );
    assert!(events.iter().all(|event| event.actor_id == actor()));
    next(&engine, &mut world);
    assert_eq!(world.revision, 91);
    assert_eq!(state(&world).last_command_sequence, 23);
}

#[test]
fn wrong_revision_and_actor_key_roll_back() {
    let (engine, mut world) = setup(content());
    world.content_revision = "other".into();
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::CancelActivity,
        GameErrorCode::InvalidInput,
    );
    world.content_revision = engine.content().revision.clone();
    state_mut(&mut world).actor_id = actor_two();
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::CancelActivity,
        GameErrorCode::InvalidInput,
    );
}

#[test]
fn walking_is_real_one_tile_per_tick_and_never_a_client_teleport() {
    let (engine, mut world) = setup(content());
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(14, 10, 0),
            running: false,
        },
    );
    assert_eq!(state(&world).tile, tile(10, 10, 0));
    for x in 11..=14 {
        let events = next(&engine, &mut world);
        assert_eq!(state(&world).tile, tile(x, 10, 0));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.event, GameEvent::Moved { .. }))
                .count(),
            1
        );
    }
    assert!(matches!(state(&world).activity, Activity::Idle));
}

#[test]
fn walking_routes_around_walls_without_corner_clipping() {
    let mut content = content();
    cell(&mut content, tile(11, 10, 0)).walkable = false;
    let (engine, mut world) = setup(content);
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(12, 10, 0),
            running: false,
        },
    );
    let Activity::Walking { path, .. } = &state(&world).activity else {
        panic!("Missing actual path")
    };
    assert!(path.len() > 2);
    assert_eq!(path[0], tile(10, 11, 0));
    assert!(!path.contains(&tile(11, 10, 0)));
    ticks(&engine, &mut world, 6, &mut NeverDraw);
    assert_eq!(state(&world).tile, tile(12, 10, 0));
}

#[test]
fn planes_and_unlisted_tiles_are_not_walkable() {
    let (engine, mut world) = setup(content());
    for destination in [tile(12, 10, 1), tile(25, 10, 0)] {
        error_unchanged(
            &engine,
            &mut world,
            GameIntent::Walk {
                destination,
                running: false,
            },
            GameErrorCode::OutOfReach,
        );
    }
}

#[test]
fn blocked_walk_prefix_cancels_without_moving_or_granting() {
    let (engine, mut world) = setup(content());
    state_mut(&mut world).activity = Activity::Walking {
        path: vec![tile(14, 10, 0)],
        running: false,
    };
    let events = next(&engine, &mut world);
    assert_eq!(state(&world).tile, tile(10, 10, 0));
    assert!(matches!(state(&world).activity, Activity::Idle));
    assert!(matches!(&events[0].event, GameEvent::Message { text } if text.contains("stopped")));
}

#[test]
fn interaction_requires_correct_identity_range_and_plane() {
    let mut content = content();
    set_position(&mut content, "rock", tile(20, 10, 0));
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        interact("rock"),
        GameErrorCode::OutOfReach,
    );
    error_unchanged(
        &engine,
        &mut world,
        interact("absent"),
        GameErrorCode::UnknownContent,
    );
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::Interact {
            target: spawn("bank"),
            action: "grant".into(),
        },
        GameErrorCode::UnknownContent,
    );
    world.entities.get_mut(&spawn("bank")).unwrap().tile = tile(11, 10, 1);
    error_unchanged(
        &engine,
        &mut world,
        interact("bank"),
        GameErrorCode::OutOfReach,
    );
}

#[test]
fn interaction_obeys_both_sight_and_touch_edges() {
    for sight in [false, true] {
        let mut content = content();
        if sight {
            cell(&mut content, tile(10, 10, 0)).blocked_sight = Direction::East.mask();
        } else {
            cell(&mut content, tile(10, 10, 0)).blocked_movement = Direction::East.mask();
        }
        let (engine, mut world) = setup(content);
        error_unchanged(
            &engine,
            &mut world,
            interact("rock"),
            GameErrorCode::OutOfReach,
        );
    }
}

#[test]
fn object_footprint_can_be_touched_without_walking_inside_it() {
    let mut content = content();
    set_position(&mut content, "bank", tile(11, 10, 0));
    cell(&mut content, tile(11, 10, 0)).walkable = false;
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("bank"));
    assert_eq!(state(&world).tile, tile(10, 10, 0));
}

#[test]
fn source_travel_changes_plane_only_after_guarded_reach() {
    let mut content = content();
    let destination = tile(10, 10, 1);
    add_object(
        &mut content,
        "stairs",
        InteractionAction::Travel {
            destination,
            region: RegionId::new("region.synthetic.floor_1").unwrap(),
        },
    );
    interaction(&mut content, "stairs").guard = Guard::Flag {
        name: "stairs_unlocked".into(),
        equals: 1,
    };
    content
        .initial_state
        .flags
        .insert("stairs_unlocked".into(), 0);
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        interact("stairs"),
        GameErrorCode::RequirementNotMet,
    );
    state_mut(&mut world)
        .flags
        .insert("stairs_unlocked".into(), 1);
    let events = apply(&engine, &mut world, interact("stairs"));
    assert_eq!(state(&world).tile, destination);
    assert!(
        events
            .iter()
            .any(|event| matches!(event.event, GameEvent::Moved { .. }))
    );
}

#[test]
fn source_item_spawn_pickup_has_ownership_capacity_and_respawn() {
    let mut content = content();
    add_item_spawn(&mut content);
    let (engine, mut world) = setup(content);
    let id = world.ground_items[0].id.clone();
    apply(
        &engine,
        &mut world,
        GameIntent::TakeGroundItem {
            ground_item_id: id.clone(),
        },
    );
    assert_eq!(count(&engine, &world, "egg"), 1);
    assert!(world.ground_items.is_empty());
    next(&engine, &mut world);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::TakeGroundItem { ground_item_id: id },
        GameErrorCode::NotOwned,
    );
    next(&engine, &mut world);
    assert!(world.ground_items.is_empty());
    next(&engine, &mut world);
    assert_eq!(world.ground_items.len(), 1);
    assert!(world.ground_items[0].id.ends_with(":3"));
}

#[test]
fn ground_pickup_failure_keeps_item_and_inventory_unchanged() {
    let mut content = content();
    add_item_spawn(&mut content);
    content.initial_state.inventory = Inventory {
        slots: std::array::from_fn(|_| Some(stack("pebble", 1))),
    };
    let (engine, mut world) = setup(content);
    let id = world.ground_items[0].id.clone();
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::TakeGroundItem {
            ground_item_id: id.clone(),
        },
        GameErrorCode::InventoryFull,
    );
    world.ground_items[0].owner = Some(actor_two());
    world.ground_items[0].public_at_tick = 10;
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::TakeGroundItem { ground_item_id: id },
        GameErrorCode::NotOwned,
    );
}

#[test]
fn equip_uses_base_level_even_when_production_uses_current_level() {
    let mut content = content();
    content
        .items
        .get_mut(&item("pick"))
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
        .requirements
        .push(SkillRequirement {
            skill: skill(),
            level: 2,
            basis: SkillLevelBasis::Base,
        });
    content
        .initial_state
        .skills
        .get_mut(&skill())
        .unwrap()
        .current_level = 2;
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::Equip { inventory_slot: 0 },
        GameErrorCode::RequirementNotMet,
    );
}

#[test]
fn food_consumes_one_heals_to_base_and_does_not_award_xp() {
    let mut content = content();
    give_initial(&mut content, &[stack("cooked", 2)]);
    content.initial_state.hitpoints = 8;
    let (engine, mut world) = setup(content);
    let slot = locate(&world, "cooked");
    let xp = state(&world).skills.clone();
    let events = apply(
        &engine,
        &mut world,
        GameIntent::Eat {
            inventory_slot: slot,
        },
    );
    assert_eq!(state(&world).hitpoints, 10);
    assert_eq!(count(&engine, &world, "cooked"), 1);
    assert_eq!(state(&world).skills, xp);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.event, GameEvent::XpGained { .. }))
    );
}

#[test]
fn food_three_tick_delay_survives_cancel_and_serialization() {
    let mut content = content();
    give_initial(&mut content, &[stack("cooked", 2)]);
    let (engine, mut world) = setup(content);
    let slot = locate(&world, "cooked");
    apply(
        &engine,
        &mut world,
        GameIntent::Eat {
            inventory_slot: slot,
        },
    );
    next(&engine, &mut world);
    apply(&engine, &mut world, GameIntent::CancelActivity);
    next(&engine, &mut world);
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    let slot = locate(&world, "cooked");
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::Eat {
            inventory_slot: slot,
        },
        GameErrorCode::Busy,
    );
    next(&engine, &mut world);
    apply(
        &engine,
        &mut world,
        GameIntent::Eat {
            inventory_slot: slot,
        },
    );
    assert_eq!(count(&engine, &world, "cooked"), 0);
    assert_eq!(state(&world).flags["__world_engine.attack_ready"], 6);
}

#[test]
fn burned_food_is_not_edible() {
    let mut content = content();
    give_initial(&mut content, &[stack("burnt", 1)]);
    let (engine, mut world) = setup(content);
    let slot = locate(&world, "burnt");
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::Eat {
            inventory_slot: slot,
        },
        GameErrorCode::InvalidInput,
    );
}

#[test]
fn missing_mechanic_contracts_fail_without_mutation_or_fake_success_events() {
    let mut content = content();
    add_object(&mut content, "enemy", InteractionAction::Attack);
    let (engine, mut world) = setup(content);
    for intent in [
        interact("enemy"),
        GameIntent::SetCombatStyle {
            style: "accurate".into(),
        },
        GameIntent::Cast {
            spell: "spell.wind_strike".into(),
            target: Some(spawn("enemy")),
        },
        GameIntent::SetPrayer {
            prayer: "prayer.thick_skin".into(),
            enabled: true,
        },
        GameIntent::Walk {
            destination: tile(12, 10, 0),
            running: true,
        },
        GameIntent::Drop {
            inventory_slot: 0,
            quantity: quantity(1),
        },
    ] {
        error_unchanged(&engine, &mut world, intent, GameErrorCode::Unavailable);
    }
}

#[test]
fn unsupported_persisted_combat_and_death_fail_the_entire_tick() {
    for dead in [false, true] {
        let (engine, mut world) = setup(content());
        if dead {
            state_mut(&mut world).hitpoints = 0;
        } else {
            state_mut(&mut world).activity = Activity::Fighting {
                target: spawn("rock"),
                style: "accurate".into(),
                next_tick: 0,
            };
        }
        let before = world.clone();
        assert_eq!(
            engine.tick(&mut world, &mut NeverDraw).unwrap_err().code,
            GameErrorCode::Unavailable
        );
        assert_eq!(world, before);
    }
}

#[test]
fn runtime_metadata_is_not_a_content_effect_backdoor() {
    let mut content = content();
    add_object(
        &mut content,
        "forged",
        InteractionAction::Effects {
            effects: vec![Effect::SetFlag {
                name: "__world_engine.access.bank:spawn.synthetic.bank".into(),
                value: 1,
            }],
        },
    );
    assert_eq!(
        WorldEngine::new(Arc::new(content)).unwrap_err().code,
        GameErrorCode::InvalidContent
    );
}

#[test]
fn integer_tick_overflow_is_atomic() {
    let (engine, mut world) = setup(content());
    world.tick = i64::MAX as u64;
    let before = world.clone();
    assert_eq!(
        engine.tick(&mut world, &mut NeverDraw).unwrap_err().code,
        GameErrorCode::InvalidInput
    );
    assert_eq!(world, before);
}

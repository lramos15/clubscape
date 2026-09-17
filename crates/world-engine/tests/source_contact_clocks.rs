mod support;

use clubscape_game_types::*;
use clubscape_world_engine::{ActorPresence, LifecycleTransition, TickContext};
use std::collections::BTreeMap;
use support::{cell as cell_mut, v2 as v, *};

fn solid_fixture() -> GameContent {
    let mut content = v::content();
    add_object(
        &mut content,
        "solid",
        InteractionAction::Effects {
            effects: vec![Effect::Message {
                text: "Reached the near face.".into(),
            }],
        },
    );
    content.objects.get_mut(&object("solid")).unwrap().clip = Some(ObjectClipDefinition {
        blocks_movement: true,
        blocks_projectiles: true,
        access_blocked_sides: 0,
    });
    content.spawns.get_mut(&spawn("solid")).unwrap().placement = Some(SourceObjectPlacement {
        shape: 10,
        quarter_turns: 0,
        layer: ObjectLayer::GameObject,
    });
    let target = cell_mut(&mut content, tile(11, 10, 0));
    target.walkable = false;
    target.blocked_movement = u8::MAX;
    target.blocked_sight = u8::MAX;
    content
}

#[test]
fn solid_source_object_is_reachable_at_its_face_without_opening_collision() {
    let content = solid_fixture();
    let collision = content.regions.clone();
    let (engine, mut world) = setup(content);
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    apply(&engine, &mut world, interact("solid"));
    assert_eq!(engine.content().regions, collision);
    next(&engine, &mut world);
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(11, 10, 0),
            running: false,
        },
    );
    for _ in 0..8 {
        next(&engine, &mut world);
        assert_ne!(state(&world).tile, tile(11, 10, 0));
    }
    assert_eq!(engine.content().regions, collision);
}

#[test]
fn solid_contact_retains_source_access_masks_walls_sight_and_corner_restrictions() {
    for obstruction in [
        "side",
        "wall",
        "sight",
        "corner",
        "wall_layer",
        "far",
        "plane",
    ] {
        let mut content = solid_fixture();
        match obstruction {
            "side" => {
                content
                    .objects
                    .get_mut(&object("solid"))
                    .unwrap()
                    .clip
                    .as_mut()
                    .unwrap()
                    .access_blocked_sides = Direction::West.mask();
            }
            "wall" => {
                cell_mut(&mut content, tile(10, 10, 0)).blocked_movement = Direction::East.mask()
            }
            "sight" => {
                cell_mut(&mut content, tile(10, 10, 0)).blocked_sight = Direction::East.mask()
            }
            "corner" => content.initial_state.tile = tile(12, 11, 0),
            "wall_layer" => {
                let placement = content
                    .spawns
                    .get_mut(&spawn("solid"))
                    .unwrap()
                    .placement
                    .as_mut()
                    .unwrap();
                placement.layer = ObjectLayer::Wall;
                placement.shape = 0;
            }
            "far" => content.initial_state.tile = tile(13, 10, 0),
            "plane" => {
                let target = content.spawns.get_mut(&spawn("solid")).unwrap();
                target.tile = tile(11, 10, 1);
                target.region = RegionId::new("region.synthetic.floor_1").unwrap();
            }
            _ => unreachable!(),
        }
        let (engine, mut world) = setup(content);
        error_unchanged(
            &engine,
            &mut world,
            interact("solid"),
            GameErrorCode::OutOfReach,
        );
    }
}

#[test]
fn source_rectangle_rotation_and_access_sides_apply_to_the_whole_footprint() {
    let mut content = solid_fixture();
    let object = content.objects.get_mut(&object("solid")).unwrap();
    object.size_x = 2;
    object.clip.as_mut().unwrap().access_blocked_sides = Direction::North.mask();
    content
        .spawns
        .get_mut(&spawn("solid"))
        .unwrap()
        .placement
        .as_mut()
        .unwrap()
        .quarter_turns = 1;
    let far_half = cell_mut(&mut content, tile(11, 11, 0));
    far_half.walkable = false;
    far_half.blocked_movement = u8::MAX;
    far_half.blocked_sight = u8::MAX;
    content.initial_state.tile = tile(10, 11, 0);
    let (engine, mut world) = setup(content);
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    apply(&engine, &mut world, interact("solid"));
    next(&engine, &mut world);
    state_mut(&mut world).tile = tile(12, 11, 0);
    error_unchanged(
        &engine,
        &mut world,
        interact("solid"),
        GameErrorCode::OutOfReach,
    );
}

fn private_policy() -> GroundPolicyId {
    GroundPolicyId::new("ground_policy.synthetic.owner_clock").unwrap()
}

fn drop_fixture() -> GameContent {
    let mut content = v::content();
    let ordinary = content
        .mechanics
        .ground_policies
        .get_mut(&v::ground_policy())
        .unwrap();
    ordinary.public_after = v::bound(Some(2));
    ordinary.expires_after = v::bound(Some(5));
    ordinary.clock = Some(v::bound(GroundClock::WorldTicks));
    content.mechanics.ground_policies.insert(
        private_policy(),
        GroundItemPolicy {
            id: private_policy(),
            public_after: v::bound(None),
            expires_after: v::bound(Some(5)),
            clock: Some(v::bound(GroundClock::OwnerOnlineTicks)),
            owner_can_take: true,
            source: source(),
        },
    );
    content.mechanics.player_drop = Some(PlayerDropPolicy {
        ordinary: v::bound(v::ground_policy()),
        stages: BTreeMap::new(),
        untradeable: Some(v::bound(private_policy())),
        before_playtime: Some(v::bound(PlayerDropPlaytimePolicy {
            played_ticks_below: 3,
            ground_policy: private_policy(),
        })),
        source: source(),
    });
    content.initial_state.inventory.slots[0] = Some(stack("coins", 25));
    content
}

fn drop() -> GameIntent {
    GameIntent::Drop {
        inventory_slot: 0,
        quantity: quantity(25),
    }
}

#[test]
fn fresh_manual_drop_freezes_owner_clock_and_preserves_it_through_logout_restart_rejoin() {
    let (engine, mut world) = setup(drop_fixture());
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    apply(&engine, &mut world, drop());
    let ground = world.ground_items[0].clone();
    let origin = world.runtime.ground_provenance[&ground.id].clone();
    assert!(matches!(origin.producer, GroundProducer::PlayerDrop { .. }));
    assert_eq!(origin.clock, Some(GroundClock::OwnerOnlineTicks));
    assert_eq!(ground.expires_at_tick, 5);
    assert_eq!(ground.public_at_tick, u64::MAX);
    next(&engine, &mut world);
    assert_eq!(state(&world).runtime.played_time.as_ref().unwrap().ticks, 1);
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::TransportLost)
        .unwrap();
    for _ in 0..7 {
        next(&engine, &mut world);
    }
    let json = serde_json::to_vec(&world).unwrap();
    let restored: WorldState = serde_json::from_slice(&json).unwrap();
    assert_eq!(restored, world);
    world = restored;
    engine
        .reconcile_presence(&mut world, &Default::default())
        .unwrap();
    assert_eq!(world.ground_items[0].expires_at_tick, 12);
    assert_eq!(world.ground_items[0].stack.quantity.get(), 25);
    assert_eq!(state(&world).runtime.played_time.as_ref().unwrap().ticks, 1);
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Rejoin)
        .unwrap();
    for _ in 0..3 {
        next(&engine, &mut world);
    }
    assert_eq!(world.ground_items.len(), 1);
    assert_eq!(state(&world).runtime.played_time.as_ref().unwrap().ticks, 4);
    assert_eq!(world.runtime.ground_provenance[&ground.id], origin);
    next(&engine, &mut world);
    assert!(world.ground_items.is_empty());
    assert!(world.runtime.ground_provenance.is_empty());
}

#[test]
fn playtime_boundary_selects_normal_tradeable_drops_but_not_untradeables() {
    for (played, tradable, expected) in [
        (2, true, private_policy()),
        (3, true, v::ground_policy()),
        (4, true, v::ground_policy()),
        (3, false, private_policy()),
    ] {
        let mut content = drop_fixture();
        content.items.get_mut(&item("coins")).unwrap().tradable = tradable;
        let (engine, mut world) = setup(content);
        state_mut(&mut world)
            .runtime
            .played_time
            .as_mut()
            .unwrap()
            .ticks = played;
        engine
            .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
            .unwrap();
        apply(&engine, &mut world, drop());
        let ground = &world.ground_items[0];
        assert_eq!(world.runtime.ground_provenance[&ground.id].policy, expected);
        engine
            .apply_lifecycle(&mut world, &actor(), LifecycleTransition::TransportLost)
            .unwrap();
        next(&engine, &mut world);
        assert_eq!(
            world.ground_items[0].expires_at_tick,
            if expected == private_policy() { 6 } else { 5 }
        );
    }
}

#[test]
fn tutorial_stage_override_precedes_playtime_and_untradeable_selection() {
    let mut content = drop_fixture();
    content
        .mechanics
        .player_drop
        .as_mut()
        .unwrap()
        .stages
        .insert(stage("start"), v::bound(v::ground_policy()));
    content.items.get_mut(&item("coins")).unwrap().tradable = false;
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, drop());
    assert_eq!(
        world.runtime.ground_provenance[&world.ground_items[0].id].policy,
        v::ground_policy()
    );
}

#[test]
fn unknown_legacy_playtime_and_missing_owner_presence_are_explicit_atomic_errors() {
    let (engine, mut world) = setup(drop_fixture());
    state_mut(&mut world).runtime.played_time = None;
    error_unchanged(&engine, &mut world, drop(), GameErrorCode::Unavailable);
    state_mut(&mut world).runtime.played_time = Some(PlayedTime {
        ticks: 0,
        through_world_tick: None,
    });
    let before = world.clone();
    assert_eq!(
        engine.tick(&mut world, &mut NeverDraw).unwrap_err().code,
        GameErrorCode::Unavailable
    );
    assert_eq!(world, before);
}

#[test]
fn authoritative_legacy_context_counts_playtime_once_per_processed_tick() {
    let (engine, mut world) = setup(drop_fixture());
    let context = TickContext {
        actors: BTreeMap::from([(
            actor(),
            ActorPresence {
                online: true,
                idle_milliseconds: 0,
                grave_interface: None,
            },
        )]),
    };
    engine
        .tick_with_context(&mut world, &mut NeverDraw, &context)
        .unwrap();
    engine
        .process_advanced_tick_with_context(&mut world, &mut NeverDraw, &context)
        .unwrap();
    assert_eq!(state(&world).runtime.played_time.as_ref().unwrap().ticks, 1);
    let before = state(&world).runtime.clone();
    let mut after = before.clone();
    after.played_time.as_mut().unwrap().ticks = 0;
    assert!(before.validate_ledger_successor(&after).is_err());
}

#[test]
fn old_ground_receipts_migrate_clock_without_changing_origin_or_remaining_lifetime() {
    let (engine, mut world) = setup(drop_fixture());
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    apply(&engine, &mut world, drop());
    world
        .runtime
        .ground_provenance
        .values_mut()
        .next()
        .unwrap()
        .clock = None;
    let bytes = serde_json::to_vec(&world).unwrap();
    world = serde_json::from_slice(&bytes).unwrap();
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::TransportLost)
        .unwrap();
    next(&engine, &mut world);
    let ground = &world.ground_items[0];
    assert_eq!(ground.expires_at_tick, 6);
    assert_eq!(ground.stack.quantity.get(), 25);
    assert_eq!(
        world.runtime.ground_provenance[&ground.id].clock,
        Some(GroundClock::OwnerOnlineTicks)
    );
}

#[test]
fn owner_clock_pauses_visibility_and_expiry_and_rejects_future_clock_metadata() {
    let mut content = drop_fixture();
    content
        .mechanics
        .ground_policies
        .get_mut(&private_policy())
        .unwrap()
        .public_after = v::bound(Some(2));
    let (engine, mut world) = setup(content);
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    apply(&engine, &mut world, drop());
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::TransportLost)
        .unwrap();
    next(&engine, &mut world);
    assert_eq!(world.ground_items[0].public_at_tick, 3);
    assert_eq!(world.ground_items[0].expires_at_tick, 6);
    let future = world.tick + 1;
    state_mut(&mut world)
        .runtime
        .played_time
        .as_mut()
        .unwrap()
        .through_world_tick = Some(future);
    let before = world.clone();
    assert!(engine.tick(&mut world, &mut NeverDraw).is_err());
    assert_eq!(world, before);
}

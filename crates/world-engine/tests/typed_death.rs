mod support;

use clubscape_game_types::*;
use clubscape_world_engine::{ActorPresence, TickContext};
use std::collections::BTreeMap;
use support::{v2 as v, *};

fn dying_setup(items: bool) -> (clubscape_world_engine::WorldEngine, WorldState) {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::with_death(&mut content);
    v::armed(&mut content, false);
    content.initial_state.hitpoints = 1;
    if items {
        give_initial(
            &mut content,
            &[
                stack("ore", 1),
                stack("tin", 1),
                stack("egg", 1),
                stack("milk", 1),
            ],
        );
    }
    content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
        .mechanics
        .as_mut()
        .unwrap()
        .retaliation = true;
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "accurate");
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    (engine, world)
}

fn kill_player(
    engine: &clubscape_world_engine::WorldEngine,
    world: &mut WorldState,
) -> Vec<clubscape_world_engine::ActorEvent> {
    v::tick_n(engine, world, 4, &mut v::Hits(0))
}

fn complete_office(engine: &clubscape_world_engine::WorldEngine, world: &mut WorldState) {
    apply(engine, world, interact("cook"));
    for topic in ["fees", "timer", "kept"] {
        v::tick(engine, world, &mut NeverDraw);
        apply(engine, world, select(topic));
    }
    v::tick(engine, world, &mut NeverDraw);
    apply(engine, world, interact("portal"));
}

#[test]
fn real_npc_lethal_damage_visits_private_office_only_when_items_are_lost() {
    for items_lost in [false, true] {
        let (engine, mut world) = dying_setup(items_lost);
        let before_bank = state(&world).bank.clone();
        let events = kill_player(&engine, &mut world);
        assert!(events.iter().any(|event| matches!(event.event, GameEvent::DeathOccurred { items_lost: lost, .. } if lost == items_lost)));
        assert_eq!(state(&world).hitpoints, 10);
        assert_eq!(state(&world).bank, before_bank);
        assert_eq!(
            matches!(
                state(&world).runtime.life,
                LifeState::FirstDeathOffice { .. }
            ),
            items_lost
        );
        assert_eq!(state(&world).runtime.instance.is_some(), items_lost);
        assert_eq!(world.runtime.deaths.len(), 1);
    }
}

#[test]
fn death_retains_three_valued_units_not_every_quantity_of_a_stack() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    content.initial_state.inventory = Inventory::default();
    give_initial(&mut content, &[stack("coins", 100)]);
    v::with_death(&mut content);
    content.initial_state.hitpoints = 1;
    content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
        .mechanics
        .as_mut()
        .unwrap()
        .retaliation = true;
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "accurate");
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    kill_player(&engine, &mut world);
    assert_eq!(count(&engine, &world, "coins"), 2);
    assert_eq!(state(&world).equipment[&slot("weapon")], stack("dagger", 1));
    let grave = world
        .runtime
        .deaths
        .values()
        .next()
        .unwrap()
        .grave
        .as_ref()
        .unwrap();
    assert_eq!(grave.items[0].stack, stack("coins", 98));
}

#[test]
fn office_portal_requires_all_three_actual_dialogue_topics_and_pauses_the_grave() {
    let (engine, mut world) = dying_setup(true);
    kill_player(&engine, &mut world);
    let death = state(&world).runtime.active_death.clone().unwrap();
    error_unchanged(
        &engine,
        &mut world,
        interact("portal"),
        GameErrorCode::RequirementNotMet,
    );
    apply(&engine, &mut world, interact("cook"));
    for topic in ["fees", "timer"] {
        v::tick(&engine, &mut world, &mut NeverDraw);
        apply(&engine, &mut world, select(topic));
    }
    v::tick(&engine, &mut world, &mut NeverDraw);
    error_unchanged(
        &engine,
        &mut world,
        interact("portal"),
        GameErrorCode::RequirementNotMet,
    );
    v::tick_n(&engine, &mut world, 20, &mut NeverDraw);
    assert_eq!(
        world.runtime.deaths[&death]
            .grave
            .as_ref()
            .unwrap()
            .active_ticks_remaining,
        1500
    );
    apply(&engine, &mut world, select("kept"));
    v::tick(&engine, &mut world, &mut NeverDraw);
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    apply(&engine, &mut world, interact("portal"));
    assert!(matches!(state(&world).runtime.life, LifeState::Alive));
    assert_eq!(state(&world).runtime.instance, None);
    assert!(
        world.runtime.deaths[&death]
            .grave
            .as_ref()
            .unwrap()
            .clock_started
    );
}

#[test]
fn grave_reclaim_is_owner_only_free_for_starter_values_and_exactly_once() {
    let (engine, mut world) = dying_setup(true);
    kill_player(&engine, &mut world);
    let death = state(&world).runtime.active_death.clone().unwrap();
    let entries: Vec<_> = world.runtime.deaths[&death]
        .grave
        .as_ref()
        .unwrap()
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect();
    complete_office(&engine, &mut world);
    v::tick(&engine, &mut world, &mut NeverDraw);
    let other = engine
        .character_from_initial(actor_two(), "Other recovery actor", Default::default())
        .unwrap();
    world.characters.insert(actor_two(), other);
    let before = world.clone();
    let reclaim = GameIntent::Reclaim {
        death: death.clone(),
        storage: RecoveryStorage::Grave,
        items: entries.clone(),
    };
    assert_eq!(
        engine
            .apply_intent(&mut world, &actor_two(), &reclaim, &mut NeverDraw)
            .unwrap_err()
            .code,
        GameErrorCode::NotOwned
    );
    assert_eq!(world, before);
    let events = apply(&engine, &mut world, reclaim.clone());
    assert!(
        events
            .iter()
            .any(|event| matches!(event.event, GameEvent::RecoveryCompleted { fee: 0, .. }))
    );
    assert!(
        world.runtime.deaths[&death]
            .grave
            .as_ref()
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(world.runtime.deaths[&death].reclaimed.len(), entries.len());
    v::tick(&engine, &mut world, &mut NeverDraw);
    error_unchanged(&engine, &mut world, reclaim, GameErrorCode::NotOwned);
}

#[test]
fn grave_clock_uses_authority_offline_idle_and_interface_facts_not_wall_clock() {
    let (engine, mut world) = dying_setup(true);
    kill_player(&engine, &mut world);
    let death = state(&world).runtime.active_death.clone().unwrap();
    complete_office(&engine, &mut world);
    for (index, presence) in [
        ActorPresence {
            online: false,
            idle_milliseconds: 0,
            grave_interface: None,
        },
        ActorPresence {
            online: true,
            idle_milliseconds: 10_001,
            grave_interface: None,
        },
        ActorPresence {
            online: true,
            idle_milliseconds: 0,
            grave_interface: Some(death.clone()),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let context = TickContext {
            actors: BTreeMap::from([(actor(), presence)]),
        };
        engine
            .tick_with_context(&mut world, &mut NeverDraw, &context)
            .unwrap();
        assert_eq!(
            world.runtime.deaths[&death]
                .grave
                .as_ref()
                .unwrap()
                .active_ticks_remaining,
            if index < 2 { 1500 } else { 1499 }
        );
    }
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(
        world.runtime.deaths[&death]
            .grave
            .as_ref()
            .unwrap()
            .active_ticks_remaining,
        1498
    );
}

#[test]
fn grave_expires_to_office_once_without_public_loot_or_free_replacement() {
    let (engine, mut world) = dying_setup(true);
    kill_player(&engine, &mut world);
    let death = state(&world).runtime.active_death.clone().unwrap();
    complete_office(&engine, &mut world);
    let lost = world.runtime.deaths[&death]
        .grave
        .as_ref()
        .unwrap()
        .items
        .clone();
    let carried = state(&world).inventory.clone();
    let events = v::tick_n(&engine, &mut world, 1500, &mut NeverDraw);
    assert!(world.runtime.deaths[&death].grave.is_none());
    assert_eq!(world.runtime.deaths[&death].office, lost);
    assert_eq!(state(&world).inventory, carried);
    assert!(world.ground_items.is_empty());
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.event, GameEvent::GraveExpired { .. }))
            .count(),
        1
    );
}

#[test]
fn unbound_valuation_aborts_incoming_lethal_tick_not_a_fake_nonfatal_fallback() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::with_death(&mut content);
    v::armed(&mut content, false);
    content.initial_state.hitpoints = 1;
    content
        .mechanics
        .value_providers
        .values_mut()
        .next()
        .unwrap()
        .values = v::unresolved("source valuation not observed");
    content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
        .mechanics
        .as_mut()
        .unwrap()
        .retaliation = true;
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "accurate");
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    v::tick_n(&engine, &mut world, 3, &mut NeverDraw);
    let before = world.clone();
    let context = TickContext::all_active(&world);
    assert_eq!(
        engine
            .tick_with_context(&mut world, &mut v::Hits(0), &context)
            .unwrap_err()
            .code,
        GameErrorCode::Unavailable
    );
    assert_eq!(world, before);
}

#[test]
fn source_fee_is_paid_only_for_the_reclaimed_entries_and_repeat_recovery_cannot_charge_again() {
    let (engine, mut world) = dying_setup(true);
    kill_player(&engine, &mut world);
    let death = state(&world).runtime.active_death.clone().unwrap();
    complete_office(&engine, &mut world);
    v::tick(&engine, &mut world, &mut NeverDraw);
    let grave = world
        .runtime
        .deaths
        .get_mut(&death)
        .unwrap()
        .grave
        .as_mut()
        .unwrap();
    for item in &mut grave.items {
        item.effective_unit_value = 100_000;
    }
    let entries: Vec<_> = grave.items.iter().map(|item| item.id.clone()).collect();
    state_mut(&mut world).runtime.death_coffer = 1000;
    let events = apply(
        &engine,
        &mut world,
        GameIntent::Reclaim {
            death: death.clone(),
            storage: RecoveryStorage::Grave,
            items: entries,
        },
    );
    assert_eq!(state(&world).runtime.death_coffer, 0);
    assert_eq!(world.runtime.deaths[&death].reclaimed.len(), 1);
    assert!(
        events
            .iter()
            .any(|event| matches!(event.event, GameEvent::RecoveryCompleted { fee: 1000, .. }))
    );
    assert!(
        !world.runtime.deaths[&death]
            .grave
            .as_ref()
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn grave_fee_cap_applies_to_the_whole_batch_not_each_entry() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::with_death(&mut content);
    v::armed(&mut content, false);
    give_initial(&mut content, &[stack("ore", 9)]);
    content.initial_state.hitpoints = 1;
    content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
        .mechanics
        .as_mut()
        .unwrap()
        .retaliation = true;
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "accurate");
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    kill_player(&engine, &mut world);
    complete_office(&engine, &mut world);
    v::tick(&engine, &mut world, &mut NeverDraw);
    let death = state(&world).runtime.active_death.clone().unwrap();
    let grave = world
        .runtime
        .deaths
        .get_mut(&death)
        .unwrap()
        .grave
        .as_mut()
        .unwrap();
    for item in &mut grave.items {
        item.effective_unit_value = 10_000_000;
    }
    let entries = grave.items.iter().map(|item| item.id.clone()).collect();
    state_mut(&mut world).runtime.death_coffer = 900_000;
    let events = apply(
        &engine,
        &mut world,
        GameIntent::Reclaim {
            death,
            storage: RecoveryStorage::Grave,
            items: entries,
        },
    );
    assert_eq!(state(&world).runtime.death_coffer, 400_000);
    assert!(events.iter().any(|event| matches!(
        event.event,
        GameEvent::RecoveryCompleted { fee: 500_000, .. }
    )));
}

#[test]
fn partial_recovery_keeps_an_unfitting_stack_remainder_with_its_original_identity() {
    let (engine, mut world) = dying_setup(true);
    kill_player(&engine, &mut world);
    complete_office(&engine, &mut world);
    v::tick(&engine, &mut world, &mut NeverDraw);
    let death = state(&world).runtime.active_death.clone().unwrap();
    let item = &mut world
        .runtime
        .deaths
        .get_mut(&death)
        .unwrap()
        .grave
        .as_mut()
        .unwrap()
        .items[0];
    item.stack.quantity = quantity(5);
    let id = item.id.clone();
    state_mut(&mut world).inventory = Inventory {
        slots: std::array::from_fn(|index| {
            if index == 27 {
                None
            } else {
                Some(stack("pebble", 1))
            }
        }),
    };
    let events = apply(
        &engine,
        &mut world,
        GameIntent::Reclaim {
            death: death.clone(),
            storage: RecoveryStorage::Grave,
            items: vec![id.clone()],
        },
    );
    assert_eq!(
        world.runtime.deaths[&death].grave.as_ref().unwrap().items[0]
            .stack
            .quantity
            .get(),
        4
    );
    assert_eq!(
        world.runtime.deaths[&death].grave.as_ref().unwrap().items[0].id,
        id
    );
    assert!(!world.runtime.deaths[&death].reclaimed.contains(&id));
    assert!(
        events
            .iter()
            .any(|event| matches!(event.event, GameEvent::ItemTransferred { .. }))
    );
}

#[test]
fn persisted_explicit_respawn_deadline_resumes_without_repeating_death_retention() {
    let (engine, mut world) = dying_setup(false);
    kill_player(&engine, &mut world);
    let (id, record) = world.runtime.deaths.iter().next().unwrap();
    let phase = LifeState::Respawning {
        death: id.clone(),
        destination: record.respawn.clone(),
        at_tick: world.tick + 3,
    };
    let death = id.clone();
    world.runtime.deaths.get_mut(&death).unwrap().arrival = None;
    let inventory = state(&world).inventory.clone();
    let equipment = state(&world).equipment.clone();
    state_mut(&mut world).hitpoints = 0;
    state_mut(&mut world).runtime.life = phase;
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(state(&world).hitpoints, 0);
    let events = v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(state(&world).hitpoints, 10);
    assert!(matches!(state(&world).runtime.life, LifeState::Alive));
    assert_eq!(state(&world).inventory, inventory);
    assert_eq!(state(&world).equipment, equipment);
    assert_eq!(world.runtime.deaths.len(), 1);
    assert!(!events.iter().any(|event| matches!(
        event.event,
        GameEvent::Died | GameEvent::DeathOccurred { .. }
    )));
}

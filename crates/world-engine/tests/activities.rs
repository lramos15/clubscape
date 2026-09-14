mod support;

use clubscape_game_types::*;
use clubscape_world_engine::WorldEngine;
use std::sync::Arc;
use support::*;

#[test]
fn source_copper_success_boundary_and_eight_tick_attempt_are_real() {
    for (roll, success) in [(100, true), (101, false)] {
        let (engine, mut world) = setup(content());
        apply(&engine, &mut world, interact("rock"));
        assert!(ticks(&engine, &mut world, 7, &mut NeverDraw).is_empty());
        assert_eq!(count(&engine, &world, "ore"), 0);
        let events = ticks(&engine, &mut world, 1, &mut Fixed(roll));
        assert_eq!(count(&engine, &world, "ore"), u32::from(success));
        assert_eq!(
            state(&world).skills[&skill()].xp_tenths,
            if success { 175 } else { 0 }
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.event, GameEvent::Gathered { .. }))
                .count(),
            usize::from(success)
        );
    }
}

#[test]
fn failure_retries_at_the_source_cadence_without_output_or_xp() {
    let (engine, mut world) = setup(content());
    apply(&engine, &mut world, interact("rock"));
    ticks(&engine, &mut world, 8, &mut Fixed(255));
    assert!(matches!(
        state(&world).activity,
        Activity::Gathering { next_tick: 16, .. }
    ));
    ticks(&engine, &mut world, 7, &mut NeverDraw);
    ticks(&engine, &mut world, 1, &mut Fixed(0));
    assert_eq!(count(&engine, &world, "ore"), 1);
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 175);
}

#[test]
fn action_spam_cannot_accelerate_gathering() {
    let (engine, mut world) = setup(content());
    apply(&engine, &mut world, interact("rock"));
    error_unchanged(&engine, &mut world, interact("rock"), GameErrorCode::Busy);
    for _ in 0..5 {
        next(&engine, &mut world);
        apply(&engine, &mut world, interact("rock"));
    }
    assert_eq!(count(&engine, &world, "ore"), 0);
    assert!(matches!(
        state(&world).activity,
        Activity::Gathering { next_tick: 13, .. }
    ));
}

#[test]
fn depleted_source_respawns_and_competing_actor_never_gets_duplicate_ore() {
    let (engine, mut world) = setup(content());
    world.characters.insert(
        actor_two(),
        engine
            .character_from_initial(actor_two(), "Synthetic second", Default::default())
            .unwrap(),
    );
    apply(&engine, &mut world, interact("rock"));
    engine
        .apply_intent(&mut world, &actor_two(), &interact("rock"), &mut NeverDraw)
        .unwrap();
    let events = ticks(&engine, &mut world, 8, &mut Fixed(0));
    assert_eq!(count(&engine, &world, "ore"), 1);
    assert_eq!(world.characters[&actor_two()].skills[&skill()].xp_tenths, 0);
    assert!(
        events.iter().any(|event| event.actor_id == actor_two()
            && matches!(event.event, GameEvent::Message { .. }))
    );
    error_unchanged(&engine, &mut world, interact("rock"), GameErrorCode::Busy);
    ticks(&engine, &mut world, 4, &mut NeverDraw);
    apply(&engine, &mut world, interact("rock"));
    ticks(&engine, &mut world, 8, &mut Fixed(0));
    assert_eq!(count(&engine, &world, "ore"), 2);
}

#[test]
fn invalid_rng_rolls_back_tick_entities_xp_and_items() {
    let (engine, mut world) = setup(content());
    apply(&engine, &mut world, interact("rock"));
    ticks(&engine, &mut world, 7, &mut NeverDraw);
    let before = world.clone();
    assert_eq!(
        engine.tick(&mut world, &mut Fixed(256)).unwrap_err().code,
        GameErrorCode::InvalidInput
    );
    assert_eq!(world, before);
}

#[test]
fn full_inventory_refuses_to_start_without_depleting() {
    let mut content = content();
    give_initial(&mut content, &[stack("pebble", 26)]);
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        interact("rock"),
        GameErrorCode::InventoryFull,
    );
    assert_eq!(world.entities[&spawn("rock")].available_at_tick, 0);
}

#[test]
fn equipped_tool_can_gather_and_missing_tool_cannot() {
    let (engine, mut world) = setup(content());
    apply(&engine, &mut world, GameIntent::Equip { inventory_slot: 0 });
    next(&engine, &mut world);
    apply(&engine, &mut world, interact("rock"));
    ticks(&engine, &mut world, 8, &mut Fixed(0));
    assert_eq!(count(&engine, &world, "ore"), 1);
    next(&engine, &mut world);
    state_mut(&mut world).equipment.clear();
    ticks(&engine, &mut world, 3, &mut NeverDraw);
    error_unchanged(
        &engine,
        &mut world,
        interact("rock"),
        GameErrorCode::RequirementNotMet,
    );
}

#[test]
fn source_xp_stop_level_is_not_an_exact_clamp() {
    let mut content = content();
    content
        .tutorial
        .get_mut(&stage("start"))
        .unwrap()
        .xp_stop_levels
        .insert(skill(), 3);
    content
        .tutorial
        .get_mut(&stage("start"))
        .unwrap()
        .xp_caps_tenths
        .insert(skill(), 2759);
    content
        .initial_state
        .skills
        .get_mut(&skill())
        .unwrap()
        .xp_tenths = 1700;
    content
        .initial_state
        .skills
        .get_mut(&skill())
        .unwrap()
        .current_level = 2;
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("rock"));
    ticks(&engine, &mut world, 8, &mut Fixed(0));
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 1875);
    ticks(&engine, &mut world, 4, &mut NeverDraw);
    apply(&engine, &mut world, interact("rock"));
    let events = ticks(&engine, &mut world, 8, &mut Fixed(0));
    assert_eq!(count(&engine, &world, "ore"), 2);
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 1875);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.event, GameEvent::XpGained { .. }))
    );
}

#[test]
fn exact_xp_cap_awards_only_remaining_room_once() {
    let mut content = content();
    content
        .tutorial
        .get_mut(&stage("start"))
        .unwrap()
        .xp_caps_tenths
        .insert(skill(), 100);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("rock"));
    let events = ticks(&engine, &mut world, 8, &mut Fixed(0));
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 100);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.event,
                GameEvent::XpGained {
                    amount_tenths: 100,
                    ..
                }
            ))
            .count(),
        1
    );
}

#[test]
fn production_real_inputs_ticks_tools_and_source_62_plus_125_xp() {
    let mut content = content();
    give_initial(&mut content, &[stack("ore", 1), stack("tin", 1)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, produce("bronze", Some("furnace"), 1));
    assert_eq!(count(&engine, &world, "ore"), 1);
    ticks(&engine, &mut world, 5, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "bar"), 0);
    next(&engine, &mut world);
    assert_eq!(count(&engine, &world, "bar"), 1);
    assert_eq!(count(&engine, &world, "tin"), 0);
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 62);
    apply(&engine, &mut world, produce("dagger", Some("anvil"), 1));
    ticks(&engine, &mut world, 5, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "bar"), 0);
    assert_eq!(count(&engine, &world, "dagger"), 1);
    assert_eq!(count(&engine, &world, "hammer"), 1);
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 187);
}

#[test]
fn production_is_interrupted_without_losing_reserved_inputs() {
    let mut content = content();
    give_initial(&mut content, &[stack("ore", 1), stack("tin", 1)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, produce("bronze", Some("furnace"), 3));
    next(&engine, &mut world);
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(10, 11, 0),
            running: false,
        },
    );
    ticks(&engine, &mut world, 10, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "ore"), 1);
    assert_eq!(count(&engine, &world, "tin"), 1);
    assert_eq!(count(&engine, &world, "bar"), 0);
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 0);
}

#[test]
fn production_rechecks_facility_before_consuming() {
    let mut content = content();
    give_initial(&mut content, &[stack("ore", 1), stack("tin", 1)]);
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        produce("bronze", Some("anvil"), 1),
        GameErrorCode::RequirementNotMet,
    );
    error_unchanged(
        &engine,
        &mut world,
        produce("bronze", None, 1),
        GameErrorCode::OutOfReach,
    );
    apply(&engine, &mut world, produce("bronze", Some("furnace"), 1));
    world
        .entities
        .get_mut(&spawn("furnace"))
        .unwrap()
        .available_at_tick = 100;
    let events = ticks(&engine, &mut world, 6, &mut NeverDraw);
    assert!(
        events
            .iter()
            .any(|event| matches!(event.event, GameEvent::Message { .. }))
    );
    assert_eq!(count(&engine, &world, "ore"), 1);
    assert_eq!(count(&engine, &world, "bar"), 0);
}

#[test]
fn missing_recipe_input_or_tool_is_atomic() {
    let mut content = content();
    give_initial(&mut content, &[stack("ore", 1), stack("bar", 1)]);
    content.initial_state.inventory.slots[1] = None;
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        produce("bronze", Some("furnace"), 1),
        GameErrorCode::InsufficientItems,
    );
    error_unchanged(
        &engine,
        &mut world,
        produce("dagger", Some("anvil"), 1),
        GameErrorCode::RequirementNotMet,
    );
}

#[test]
fn full_inventory_can_replace_ore_with_bar_but_not_add_dough_byproducts() {
    let mut content = content();
    give_initial(
        &mut content,
        &[
            stack("ore", 1),
            stack("tin", 1),
            stack("flour", 1),
            stack("water", 1),
            stack("pebble", 22),
        ],
    );
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        produce("dough", None, 1),
        GameErrorCode::InventoryFull,
    );
    apply(&engine, &mut world, produce("bronze", Some("furnace"), 1));
    ticks(&engine, &mut world, 6, &mut NeverDraw);
    apply(&engine, &mut world, produce("dough", None, 1));
    next(&engine, &mut world);
    assert_eq!(count(&engine, &world, "dough"), 1);
    assert_eq!(count(&engine, &world, "pot"), 1);
    assert_eq!(count(&engine, &world, "bucket"), 1);
}

#[test]
fn queue_stops_on_missing_next_input_not_infinite_free_production() {
    let mut content = content();
    give_initial(&mut content, &[stack("bar", 1)]);
    let (engine, mut world) = setup(content);
    apply(
        &engine,
        &mut world,
        produce("dagger", Some("anvil"), MAX_STACK_QUANTITY),
    );
    let events = ticks(&engine, &mut world, 10, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "dagger"), 1);
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 125);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.event, GameEvent::Produced { .. }))
            .count(),
        1
    );
    assert!(matches!(state(&world).activity, Activity::Idle));
}

#[test]
fn burnt_output_can_advance_cooking_without_double_xp_or_grants() {
    let mut content = content();
    give_initial(&mut content, &[stack("raw", 1)]);
    content.recipes.get_mut(&recipe("cook")).unwrap().success = never();
    content
        .tutorial
        .get_mut(&stage("start"))
        .unwrap()
        .transitions
        .push(ProgressTransition {
            event: "produced".into(),
            target: Some(recipe("cook").to_string()),
            guard: Guard::Always,
            effects: vec![Effect::SetTutorialStage {
                stage: stage("next"),
            }],
        });
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, produce("cook", Some("range"), 1));
    let events = ticks(&engine, &mut world, 3, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "raw"), 0);
    assert_eq!(count(&engine, &world, "burnt"), 1);
    assert_eq!(count(&engine, &world, "cooked"), 0);
    assert_eq!(state(&world).tutorial_stage, stage("next"));
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 0);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.event, GameEvent::Produced { .. }))
            .count(),
        1
    );
}

#[test]
fn item_on_item_runs_a_real_recipe_not_an_arbitrary_effect() {
    let mut content = content();
    give_initial(&mut content, &[stack("flour", 1), stack("water", 1)]);
    let (engine, mut world) = setup(content);
    let flour = locate(&world, "flour");
    let water = locate(&world, "water");
    apply(
        &engine,
        &mut world,
        GameIntent::UseItem {
            inventory_slot: flour,
            target: ItemTarget::Inventory { slot: water },
        },
    );
    assert_eq!(count(&engine, &world, "dough"), 0);
    next(&engine, &mut world);
    assert_eq!(count(&engine, &world, "dough"), 1);
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 0);
}

#[test]
fn seeded_execution_is_identical_after_serialized_restart() {
    let mut content = content();
    gather_rule(&mut content).depletion = never();
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("rock"));
    let mut rng = Seeded(777);
    ticks(&engine, &mut world, 13, &mut rng);
    let mut restored: WorldState =
        serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    let restored_engine = WorldEngine::new(Arc::new(engine.content().clone())).unwrap();
    let mut restored_rng = rng.clone();
    let a = ticks(&engine, &mut world, 100, &mut rng);
    let b = ticks(&restored_engine, &mut restored, 100, &mut restored_rng);
    assert_eq!(a, b);
    assert_eq!(world, restored);
    assert!(count(&engine, &world, "ore") > 0);
}

#[test]
fn undefined_recipe_and_unbound_multi_skill_chance_do_not_mutate() {
    let mut content = content();
    give_initial(&mut content, &[stack("raw", 1)]);
    let cook = content.recipes.get_mut(&recipe("cook")).unwrap();
    cook.success = ChanceRule {
        numerator_at_level_1: 129,
        numerator_at_level_99: 513,
        denominator: 256,
    };
    cook.requirements.push(SkillRequirement {
        skill: hp_skill(),
        level: 1,
    });
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        produce("missing", None, 1),
        GameErrorCode::UnknownContent,
    );
    error_unchanged(
        &engine,
        &mut world,
        produce("cook", Some("range"), 1),
        GameErrorCode::Unavailable,
    );
}

#[test]
fn later_actor_random_failure_rolls_back_earlier_actor_success() {
    struct Rolls(std::collections::VecDeque<u32>);
    impl clubscape_world_engine::RandomSource for Rolls {
        fn draw_below(&mut self, _: u32) -> GameResult<u32> {
            Ok(self.0.pop_front().unwrap())
        }
    }
    let mut content = content();
    gather_rule(&mut content).depletion = never();
    let (engine, mut world) = setup(content);
    world.characters.insert(
        actor_two(),
        engine
            .character_from_initial(actor_two(), "Synthetic second", Default::default())
            .unwrap(),
    );
    apply(&engine, &mut world, interact("rock"));
    engine
        .apply_intent(&mut world, &actor_two(), &interact("rock"), &mut NeverDraw)
        .unwrap();
    ticks(&engine, &mut world, 7, &mut NeverDraw);
    let before = world.clone();
    let mut rng = Rolls([0, 256].into());
    assert_eq!(
        engine.tick(&mut world, &mut rng).unwrap_err().code,
        GameErrorCode::InvalidInput
    );
    assert_eq!(world, before);
}

#[test]
fn recipe_requirements_use_boosted_current_levels_not_equipment_base_levels() {
    let mut content = content();
    content
        .initial_state
        .skills
        .get_mut(&skill())
        .unwrap()
        .current_level = 2;
    content
        .recipes
        .get_mut(&recipe("dagger"))
        .unwrap()
        .requirements[0]
        .level = 2;
    content
        .items
        .get_mut(&item("pick"))
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
        .requirements = vec![SkillRequirement {
        skill: skill(),
        level: 2,
    }];
    give_initial(&mut content, &[stack("bar", 1)]);
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::Equip { inventory_slot: 0 },
        GameErrorCode::RequirementNotMet,
    );
    apply(&engine, &mut world, produce("dagger", Some("anvil"), 1));
    ticks(&engine, &mut world, 5, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "dagger"), 1);
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 125);
    assert_eq!(state(&world).skills[&skill()].current_level, 2);
}

#[path = "../../content/tests/common/mod.rs"]
mod source;

use clubscape_game_types::*;
use clubscape_world_engine::{LifecycleTransition, RandomSource, WorldEngine};
use std::{collections::BTreeMap, sync::Arc};

struct NoRandom;
impl RandomSource for NoRandom {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        panic!("Deterministic item-on conversion must not draw an outcome")
    }
}

fn actor() -> ActorId {
    source::id("actor.test.item_on")
}
fn recipe() -> RecipeId {
    source::id("recipe.test.bar")
}
fn target() -> WorldTarget {
    WorldTarget::Spawn {
        spawn: source::id("spawn.test.furnace"),
    }
}
fn use_item() -> GameIntent {
    GameIntent::UseItem {
        inventory_slot: 0,
        target: ItemTarget::World {
            spawn: source::id("spawn.test.furnace"),
        },
    }
}

fn setup(mut content: GameContent) -> (WorldEngine, WorldState) {
    content.initial_state.inventory = Inventory::default();
    for slot in &mut content.initial_state.inventory.slots[..4] {
        *slot = Some(source::stack("item.test.ore", 1));
    }
    content.initial_state.inventory.slots[4] = Some(source::stack("item.test.pickaxe", 1));
    if content.ui.is_some() {
        for item in content.items.values_mut() {
            if item.weight.is_none() {
                item.weight = Some(source::item_on::bound(ItemWeight {
                    grams: 1,
                    inventory: WeightContribution::PerUnit,
                    equipment: WeightContribution::PerUnit,
                }));
            }
        }
    }
    let engine = WorldEngine::new(Arc::new(content)).unwrap();
    let mut world = engine.initial_world().unwrap();
    world.characters.insert(
        actor(),
        engine
            .character_from_initial(actor(), "Controlled item-on", BTreeMap::new())
            .unwrap(),
    );
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    (engine, world)
}

fn apply(
    engine: &WorldEngine,
    world: &mut WorldState,
    intent: GameIntent,
) -> GameResult<Vec<clubscape_world_engine::ActorEvent>> {
    engine.apply_intent(world, &actor(), &intent, &mut NoRandom)
}

#[test]
fn explicit_item_on_and_legacy_single_produce_execute_once_without_a_menu_or_batch_phase() {
    let actions = [
        use_item(),
        GameIntent::Produce {
            recipe: recipe(),
            target: Some(source::id("spawn.test.furnace")),
            quantity: Quantity::new(1).unwrap(),
        },
        GameIntent::ProduceAt {
            recipe: recipe(),
            target: Some(target()),
            quantity: Quantity::new(1).unwrap(),
        },
        GameIntent::ProduceSelected {
            recipe: recipe(),
            target: Some(target()),
            quantity: Quantity::new(1).unwrap(),
            mode: ProductionMode::Single,
        },
    ];
    for action in actions {
        let (engine, mut world) = setup(source::item_on::content());
        let before = world.clone();
        assert!(
            engine
                .interaction_options(&world, &actor(), &target())
                .unwrap()
                .is_empty()
        );
        assert_eq!(world, before);
        let events = apply(&engine, &mut world, action).unwrap();
        assert!(!events.iter().any(|event| matches!(
            event.event,
            GameEvent::InterfaceOpened { .. } | GameEvent::InterfacePresented { .. }
        )));
        assert_eq!(
            world.characters[&actor()].inventory,
            before.characters[&actor()].inventory
        );
        let next = match world.characters[&actor()].activity {
            Activity::Producing { next_tick, .. }
            | Activity::ProducingAt { next_tick, .. }
            | Activity::ProducingSelected { next_tick, .. } => next_tick,
            _ => panic!("one pending conversion"),
        };
        assert_eq!(next, before.tick + 1);
        let events = engine.tick(&mut world, &mut NoRandom).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.event, GameEvent::ProductionResolved { .. }))
                .count(),
            1
        );
        assert!(matches!(
            world.characters[&actor()].activity,
            Activity::Idle
        ));
        let completed = world.characters[&actor()].inventory.clone();
        assert_eq!(
            clubscape_simulation::inventory::count(
                &completed,
                &engine.content().items,
                &source::id("item.test.ore")
            )
            .unwrap(),
            2
        );
        engine.tick(&mut world, &mut NoRandom).unwrap();
        assert_eq!(world.characters[&actor()].inventory, completed);
    }
}

#[test]
fn make_x_one_all_and_legacy_multi_are_not_authorized_by_single_timing() {
    let actions = [
        GameIntent::Produce {
            recipe: recipe(),
            target: Some(source::id("spawn.test.furnace")),
            quantity: Quantity::new(2).unwrap(),
        },
        GameIntent::ProduceAt {
            recipe: recipe(),
            target: Some(target()),
            quantity: Quantity::new(2).unwrap(),
        },
        GameIntent::ProduceSelected {
            recipe: recipe(),
            target: Some(target()),
            quantity: Quantity::new(1).unwrap(),
            mode: ProductionMode::MakeX,
        },
        GameIntent::ProduceSelected {
            recipe: recipe(),
            target: Some(target()),
            quantity: Quantity::new(2).unwrap(),
            mode: ProductionMode::Single,
        },
    ];
    for action in actions {
        let (engine, mut world) = setup(source::item_on::content());
        let before = world.clone();
        assert!(apply(&engine, &mut world, action).is_err());
        assert_eq!(world, before);
    }
}

#[test]
fn absent_rule_still_requires_the_existing_menu_and_its_exact_guard() {
    let mut content = source::item_on::content();
    content.recipes.get_mut(&recipe()).unwrap().item_on_target = None;
    let (engine, mut world) = setup(content);
    let before = world.clone();
    let error = apply(&engine, &mut world, use_item()).unwrap_err();
    assert_eq!(error.code, GameErrorCode::RequirementNotMet);
    assert!(error.message.contains("Facility does not offer"));
    assert_eq!(world, before);

    let mut content = source::fixture();
    content.initial_state.tile = source::tile(1004, 1002);
    for stage in content.tutorial.values_mut() {
        stage.allowed_actions = vec!["*".into()];
    }
    content
        .spawns
        .get_mut(&source::id("spawn.test.furnace"))
        .unwrap()
        .interactions[0]
        .guard = Guard::Not {
        guard: Box::new(Guard::Always),
    };
    let (engine, mut world) = setup(content);
    let before = world.clone();
    assert_eq!(
        apply(&engine, &mut world, use_item()).unwrap_err().code,
        GameErrorCode::RequirementNotMet
    );
    assert_eq!(world, before);
}

#[test]
fn target_guard_availability_and_pending_multi_modes_cannot_be_bypassed() {
    let mut content = source::item_on::content();
    let SourceBinding::Bound { value, .. } = content
        .recipes
        .get_mut(&recipe())
        .unwrap()
        .item_on_target
        .as_mut()
        .unwrap()
    else {
        panic!("bound rule")
    };
    value.guard = Guard::Not {
        guard: Box::new(Guard::Always),
    };
    let (engine, mut world) = setup(content);
    let before = world.clone();
    assert_eq!(
        apply(&engine, &mut world, use_item()).unwrap_err().code,
        GameErrorCode::RequirementNotMet
    );
    assert_eq!(world, before);

    for change in 0..2 {
        let (engine, mut world) = setup(source::item_on::content());
        apply(&engine, &mut world, use_item()).unwrap();
        let inventory = world.characters[&actor()].inventory.clone();
        if change == 0 {
            world
                .entities
                .get_mut(&source::id("spawn.test.furnace"))
                .unwrap()
                .available_at_tick = 100;
            engine.tick(&mut world, &mut NoRandom).unwrap();
            assert!(matches!(
                world.characters[&actor()].activity,
                Activity::Idle
            ));
        } else {
            world.characters.get_mut(&actor()).unwrap().activity = Activity::ProducingSelected {
                recipe: recipe(),
                target: Some(target()),
                remaining: 2,
                next_tick: 1,
                mode: ProductionMode::MakeX,
            };
            let before = world.clone();
            assert!(engine.tick(&mut world, &mut NoRandom).is_err());
            assert_eq!(world, before);
        }
        assert_eq!(world.characters[&actor()].inventory, inventory);
    }
}

mod persisted {
    use super::*;

    fn forms(remaining: u32, next_tick: u64) -> [(&'static str, Activity); 4] {
        [
            (
                "legacy",
                Activity::Producing {
                    recipe: recipe(),
                    target: Some(source::id("spawn.test.furnace")),
                    remaining,
                    next_tick,
                },
            ),
            (
                "world_target",
                Activity::ProducingAt {
                    recipe: recipe(),
                    target: Some(target()),
                    remaining,
                    next_tick,
                },
            ),
            (
                "selected_single",
                Activity::ProducingSelected {
                    recipe: recipe(),
                    target: Some(target()),
                    remaining,
                    next_tick,
                    mode: ProductionMode::Single,
                },
            ),
            (
                "selected_make_x",
                Activity::ProducingSelected {
                    recipe: recipe(),
                    target: Some(target()),
                    remaining,
                    next_tick,
                    mode: ProductionMode::MakeX,
                },
            ),
        ]
    }

    fn deserialize_activity(world: &WorldState, activity: Activity) -> WorldState {
        let mut source = world.clone();
        source.characters.get_mut(&actor()).unwrap().activity = activity;
        let json = serde_json::to_vec(&source).unwrap();
        let restored: WorldState = serde_json::from_slice(&json).unwrap();
        assert_eq!(restored, source);
        assert_eq!(serde_json::to_vec(&restored).unwrap(), json);
        restored
    }

    #[test]
    fn serialized_item_on_counts_and_modes_are_checked_at_every_deadline() {
        let (engine, mut template) = setup(source::item_on::content());
        template.tick = 10;
        let mut mismatches = Vec::new();
        for next_tick in [9, 10, 11] {
            for remaining in [0, 1, 2] {
                for (name, activity) in forms(remaining, next_tick) {
                    let restored = deserialize_activity(&template, activity);
                    let before = restored.clone();
                    let expected = remaining == 1 && name != "selected_make_x";
                    let character =
                        restored.characters[&actor()].validate_runtime(engine.content());
                    let world = restored.validate_runtime(engine.content());
                    if character.is_ok() != expected || world.is_ok() != expected {
                        mismatches.push(format!(
                            "{name}/remaining={remaining}/deadline={next_tick}: expected_valid={expected}, character={character:?}, world={world:?}"
                        ));
                    }
                    assert_eq!(
                        restored, before,
                        "Validation must not normalize persisted work."
                    );
                }
            }
        }
        assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    }

    #[test]
    fn malformed_serialized_work_is_rejected_before_repeated_due_ticks() {
        let (engine, mut template) = setup(source::item_on::content());
        template.tick = 10;
        for (name, activity) in [
            forms(2, 10)[0].clone(),
            forms(2, 10)[1].clone(),
            forms(1, 10)[3].clone(),
        ] {
            let mut restored = deserialize_activity(&template, activity);
            let validation = restored.validate_runtime(engine.content());
            let before = restored.clone();
            let first = engine.tick(&mut restored, &mut NoRandom).unwrap_err();
            assert_eq!(restored, before);
            let repeated = engine.tick(&mut restored, &mut NoRandom).unwrap_err();
            assert_eq!(restored, before);
            assert!(
                validation.is_err(),
                "{name}: accepted serialized work then repeatedly failed its due tick: first={first:?}, repeated={repeated:?}"
            );
            assert_eq!(first.code, GameErrorCode::InvalidInput);
            assert_eq!(repeated.code, GameErrorCode::InvalidInput);
        }
    }

    #[test]
    fn ordinary_recipe_batches_retain_their_serialized_count_mode_behavior() {
        let (engine, mut template) = setup(source::fixture());
        template.tick = 10;
        for next_tick in [9, 10, 11] {
            for remaining in [0, 1, 2] {
                for (name, activity) in forms(remaining, next_tick) {
                    let restored = deserialize_activity(&template, activity);
                    let expected = remaining > 0 && (name != "selected_single" || remaining == 1);
                    assert_eq!(
                        restored.characters[&actor()]
                            .validate_runtime(engine.content())
                            .is_ok(),
                        expected,
                        "{name}/remaining={remaining}/deadline={next_tick}"
                    );
                    assert_eq!(
                        restored.validate_runtime(engine.content()).is_ok(),
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn valid_single_forms_round_trip_without_normalization_and_finish_once() {
        let (engine, mut queued) = setup(source::item_on::content());
        apply(&engine, &mut queued, use_item()).unwrap();
        for (name, activity) in forms(1, 1).into_iter().take(3) {
            let mut restored = deserialize_activity(&queued, activity);
            let before = restored.clone();
            restored.characters[&actor()]
                .validate_runtime(engine.content())
                .unwrap();
            restored.validate_runtime(engine.content()).unwrap();
            assert_eq!(restored, before, "{name}");
            let events = engine.tick(&mut restored, &mut NoRandom).unwrap();
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(event.event, GameEvent::ProductionResolved { .. }))
                    .count(),
                1,
                "{name}"
            );
            assert!(matches!(
                restored.characters[&actor()].activity,
                Activity::Idle
            ));
            let inventory = restored.characters[&actor()].inventory.clone();
            engine.tick(&mut restored, &mut NoRandom).unwrap();
            assert_eq!(restored.characters[&actor()].inventory, inventory);
        }
    }

    #[test]
    fn ordinary_serialized_batches_keep_their_original_repeat_timing() {
        let mut content = source::fixture();
        content.initial_state.tile = source::tile(1004, 1002);
        for stage in content.tutorial.values_mut() {
            stage.allowed_actions = vec!["*".into()];
        }
        let (engine, mut queued) = setup(content);
        apply(
            &engine,
            &mut queued,
            GameIntent::Produce {
                recipe: recipe(),
                target: Some(source::id("spawn.test.furnace")),
                quantity: Quantity::new(2).unwrap(),
            },
        )
        .unwrap();
        for (name, activity) in forms(2, 3)
            .into_iter()
            .filter(|(name, _)| *name != "selected_single")
        {
            let mut restored = deserialize_activity(&queued, activity);
            restored.validate_runtime(engine.content()).unwrap();
            let mut resolved_at = Vec::new();
            for _ in 0..6 {
                for event in engine.tick(&mut restored, &mut NoRandom).unwrap() {
                    if matches!(event.event, GameEvent::Produced { .. }) {
                        resolved_at.push(restored.tick);
                    }
                }
            }
            assert_eq!(resolved_at, vec![3, 6], "{name}");
            assert!(matches!(
                restored.characters[&actor()].activity,
                Activity::Idle
            ));
            assert_eq!(
                clubscape_simulation::inventory::count(
                    &restored.characters[&actor()].inventory,
                    &engine.content().items,
                    &source::id("item.test.ore"),
                )
                .unwrap(),
                0,
                "{name}"
            );
        }
    }
}

#[test]
fn ui_projection_and_forged_menu_controls_cannot_expose_item_on_only_production() {
    let mut content = source::item_on::content();
    source::ui::projection(&mut content);
    content
        .ui
        .as_mut()
        .unwrap()
        .production_interfaces
        .remove(&recipe());
    content.initial_state.interfaces = content.interfaces.keys().cloned().collect();
    let (engine, mut world) = setup(content);
    assert!(
        engine
            .ui_view(&world, &actor())
            .unwrap()
            .production
            .is_none()
    );
    apply(&engine, &mut world, use_item()).unwrap();
    assert!(
        engine
            .ui_view(&world, &actor())
            .unwrap()
            .production
            .is_none()
    );
    engine.tick(&mut world, &mut NoRandom).unwrap();
    for request in [
        GameplayUiRequest::ProductionSelectAll {
            menu_id: "menu.fake".into(),
            recipe: recipe(),
        },
        GameplayUiRequest::ProductionSelect {
            menu_id: "menu.fake".into(),
            recipe: recipe(),
            quantity: 1,
            mode: ProductionMode::Single,
        },
    ] {
        let before = world.clone();
        assert!(apply(&engine, &mut world, GameIntent::Ui { request }).is_err());
        assert_eq!(world, before);
    }
}

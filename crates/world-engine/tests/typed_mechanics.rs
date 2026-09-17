mod support;

use clubscape_game_types::*;
use clubscape_world_engine::{ActorPresence, TickContext, WorldEngine};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use support::{v2 as v, *};

#[test]
fn source_running_executes_two_steps_and_weighted_drain_not_free_movement() {
    for (grams, expected) in [(0, 59), (100_000, 126)] {
        let mut content = v::content();
        v::with_run(&mut content);
        content.items.get_mut(&item("pick")).unwrap().weight = Some(v::bound(ItemWeight {
            grams,
            inventory: WeightContribution::PerUnit,
            equipment: WeightContribution::PerUnit,
        }));
        let (engine, mut world) = setup(content);
        apply(
            &engine,
            &mut world,
            GameIntent::Walk {
                destination: tile(14, 10, 0),
                running: true,
            },
        );
        let events = v::tick(&engine, &mut world, &mut NeverDraw);
        assert_eq!(state(&world).tile, tile(12, 10, 0));
        assert_eq!(state(&world).run_energy, 10_000 - expected);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.event, GameEvent::Moved { .. }))
                .count(),
            2
        );
    }
}

#[test]
fn run_toggle_threshold_single_step_recovery_and_offline_pause() {
    let mut content = v::content();
    v::with_run(&mut content);
    content.initial_state.run_energy = 99;
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::SetSetting {
            setting: CharacterSetting::Run(true),
        },
        GameErrorCode::RequirementNotMet,
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(state(&world).run_energy, 114);
    apply(
        &engine,
        &mut world,
        GameIntent::SetSetting {
            setting: CharacterSetting::Run(true),
        },
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(11, 10, 0),
            running: false,
        },
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(state(&world).tile, tile(11, 10, 0));
    assert_eq!(state(&world).run_energy, 144);
    let context = TickContext {
        actors: BTreeMap::from([(
            actor(),
            ActorPresence {
                online: false,
                idle_milliseconds: 0,
                grave_interface: None,
            },
        )]),
    };
    engine
        .tick_with_context(&mut world, &mut NeverDraw, &context)
        .unwrap();
    assert_eq!(state(&world).run_energy, 144);
}

#[test]
fn default_tick_never_fabricates_required_presence_and_advanced_tick_matches_standalone() {
    let (engine, mut a) = setup(v::content());
    let before = a.clone();
    assert_eq!(
        engine.tick(&mut a, &mut NeverDraw).unwrap_err().code,
        GameErrorCode::Unavailable
    );
    assert_eq!(a, before);
    let mut b = a.clone();
    b.tick += 1;
    let context = TickContext::all_active(&a);
    let direct = engine
        .tick_with_context(&mut a, &mut NeverDraw, &context)
        .unwrap();
    let advanced = engine
        .process_advanced_tick_with_context(&mut b, &mut NeverDraw, &context)
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(direct, advanced);
    assert_eq!(a.tick, 1);
}

#[test]
fn typed_cadence_executes_source_single_first_repeat_without_legacy_fallback() {
    let mut content = v::content();
    v::typed_recipe(&mut content, "bronze", v::cadence(6, 4, 5));
    give_initial(&mut content, &[stack("ore", 2), stack("tin", 2)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, produce("bronze", Some("furnace"), 2));
    v::tick_n(&engine, &mut world, 3, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "bar"), 0);
    let first = v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "bar"), 1);
    assert!(first.iter().any(|event| matches!(
        event.event,
        GameEvent::ProductionResolved {
            outcome: ProductionOutcome::Success,
            ..
        }
    )));
    v::tick_n(&engine, &mut world, 4, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "bar"), 1);
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "bar"), 2);
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 124);
}

#[test]
fn unresolved_single_timing_preserves_inputs_and_does_not_block_bound_make_x() {
    let mut content = v::content();
    v::typed_recipe(&mut content, "bronze", v::cadence(6, 4, 5));
    content
        .recipes
        .get_mut(&recipe("bronze"))
        .unwrap()
        .mechanics
        .as_mut()
        .unwrap()
        .cadence
        .single = v::unresolved("single source phase unknown");
    give_initial(&mut content, &[stack("ore", 2), stack("tin", 2)]);
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        produce("bronze", Some("furnace"), 1),
        GameErrorCode::Unavailable,
    );
    apply(&engine, &mut world, produce("bronze", Some("furnace"), 2));
    v::tick_n(&engine, &mut world, 9, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "bar"), 2);
}

#[test]
fn typed_gathering_uses_source_domain_and_random_respawn() {
    let mut content = v::content();
    v::typed_gather(&mut content);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("rock"));
    let mut rng = v::Rolls::new(&[100, 4]);
    v::tick_n(&engine, &mut world, 8, &mut rng);
    assert_eq!(count(&engine, &world, "ore"), 1);
    assert_eq!(world.entities[&spawn("rock")].available_at_tick, 16);
    assert!(rng.0.is_empty());
    v::tick_n(&engine, &mut world, 7, &mut NeverDraw);
    error_unchanged(&engine, &mut world, interact("rock"), GameErrorCode::Busy);
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(&engine, &mut world, interact("rock"));
}

#[test]
fn bounded_mill_counters_and_atomic_inputs_are_not_stage_shortcuts() {
    let mut content = v::content();
    let id = v::counter("flour");
    content.mechanics.counters.insert(
        id.clone(),
        CounterDefinition {
            id: id.clone(),
            scope: CounterScope::Character,
            value_type: CounterType::Integer {
                minimum: 0,
                maximum: 30,
            },
            initial: CounterValue::Integer(0),
            source_variable: None,
            source: source(),
        },
    );
    content
        .initial_state
        .runtime
        .counters
        .insert(id.clone(), CounterValue::Integer(0));
    add_object(
        &mut content,
        "mill",
        InteractionAction::Effects {
            effects: vec![
                Effect::TakeItems {
                    items: vec![stack("ore", 1)],
                },
                Effect::AddCounter {
                    counter: id.clone(),
                    delta: 1,
                },
            ],
        },
    );
    give_initial(&mut content, &[stack("ore", 1)]);
    let (engine, mut world) = setup(content);
    let events = apply(&engine, &mut world, interact("mill"));
    assert_eq!(
        state(&world).runtime.counters[&id],
        CounterValue::Integer(1)
    );
    assert!(events.iter().any(|event| matches!(
        event.event,
        GameEvent::CounterChanged {
            value: CounterValue::Integer(1),
            ..
        }
    )));
    v::tick(&engine, &mut world, &mut NeverDraw);
    error_unchanged(
        &engine,
        &mut world,
        interact("mill"),
        GameErrorCode::InsufficientItems,
    );
}

#[test]
fn once_only_bank_grant_precedes_context_presentation_and_never_reseeds() {
    let mut content = v::content();
    let interface = InterfaceId::new("interface.synthetic.bank").unwrap();
    content.interfaces.insert(
        interface.clone(),
        InterfaceDefinition {
            id: interface.clone(),
            name: "Synthetic bank context".into(),
            access: InterfaceAccess::Contextual,
            source_ids: vec![],
            source: source(),
        },
    );
    content.initial_state.interfaces.push(interface.clone());
    let grant = v::grant_id("bank");
    let entitlement = v::entitlement("bank");
    content.mechanics.grants.insert(
        grant.clone(),
        GrantDefinition {
            id: grant.clone(),
            target: ContainerKind::Bank,
            capacity: CapacityPolicy::Atomic,
            lines: vec![GrantLine {
                item: item("coins"),
                quantity: quantity(25),
                mode: GrantMode::Add,
                ownership: OwnershipScope::Bank,
            }],
            entitlement: Some(entitlement.clone()),
            source: source(),
        },
    );
    content.mechanics.entitlements.insert(
        entitlement.clone(),
        EntitlementDefinition {
            id: entitlement.clone(),
            purpose: EntitlementPurpose::Grant {
                grant: grant.clone(),
            },
            source: source(),
        },
    );
    interaction(&mut content, "bank").action = InteractionAction::OpenBank {
        interface: interface.clone(),
        before_open: vec![Effect::Grant { grant }],
    };
    let (engine, mut world) = setup(content);
    let events = apply(&engine, &mut world, interact("bank"));
    assert_eq!(state(&world).bank.slots[0], Some(stack("coins", 25)));
    assert!(events.iter().any(|event| matches!(
        &event.event,
        GameEvent::InterfacePresented {
            context: InterfaceContext::Bank { .. },
            ..
        }
    )));
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(
        &engine,
        &mut world,
        GameIntent::BankWithdraw {
            banker: spawn("bank"),
            bank_slot: 0,
            quantity: quantity(25),
            noted: false,
        },
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    apply(&engine, &mut world, interact("bank"));
    assert!(state(&world).bank.slots.iter().all(|slot| slot.is_none()));
    assert_eq!(count(&engine, &world, "coins"), 25);
    assert!(matches!(
        state(&world).runtime.entitlements[&entitlement],
        EntitlementState::Grant { complete: true, .. }
    ));
}

#[test]
fn ordered_partial_entitlement_never_regrants_a_delivered_line_after_loss() {
    let mut content = v::content();
    give_initial(&mut content, &[stack("pebble", 25)]);
    let grant = v::grant_id("supplies");
    let entitlement = v::entitlement("supplies");
    content.mechanics.grants.insert(
        grant.clone(),
        GrantDefinition {
            id: grant.clone(),
            target: ContainerKind::Inventory,
            capacity: CapacityPolicy::OrderedPartial,
            lines: ["egg", "milk"]
                .map(|name| GrantLine {
                    item: item(name),
                    quantity: quantity(1),
                    mode: GrantMode::Add,
                    ownership: OwnershipScope::Inventory,
                })
                .to_vec(),
            entitlement: Some(entitlement.clone()),
            source: source(),
        },
    );
    content.mechanics.entitlements.insert(
        entitlement.clone(),
        EntitlementDefinition {
            id: entitlement.clone(),
            purpose: EntitlementPurpose::Grant {
                grant: grant.clone(),
            },
            source: source(),
        },
    );
    add_object(
        &mut content,
        "supply",
        InteractionAction::Effects {
            effects: vec![Effect::Grant { grant }],
        },
    );
    add_object(
        &mut content,
        "consume",
        InteractionAction::Effects {
            effects: vec![Effect::TakeItems {
                items: vec![stack("egg", 1)],
            }],
        },
    );
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("supply"));
    assert_eq!(count(&engine, &world, "egg"), 1);
    assert_eq!(count(&engine, &world, "milk"), 0);
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(&engine, &mut world, interact("consume"));
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(&engine, &mut world, interact("supply"));
    assert_eq!(count(&engine, &world, "egg"), 0);
    assert_eq!(count(&engine, &world, "milk"), 1);
    assert!(matches!(
        state(&world).runtime.entitlements[&entitlement],
        EntitlementState::Grant { complete: true, .. }
    ));
}

#[test]
fn real_fire_retains_log_on_failure_then_creates_cookable_dynamic_facility_and_expires() {
    let mut content = v::content();
    v::with_fire(&mut content);
    give_initial(&mut content, &[stack("ore", 1), stack("raw", 1)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, produce("fire", None, 1));
    assert_eq!(count(&engine, &world, "ore"), 0);
    assert_eq!(world.ground_items.len(), 1);
    v::tick_n(&engine, &mut world, 4, &mut Fixed(255));
    assert!(world.runtime.temporary_objects.is_empty());
    assert_eq!(world.ground_items.len(), 1);
    let mut rng = v::Rolls::new(&[0, 3]);
    v::tick_n(&engine, &mut world, 4, &mut rng);
    assert_eq!(world.runtime.temporary_objects.len(), 1);
    assert!(world.ground_items.is_empty());
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 400);
    let object = world
        .runtime
        .temporary_objects
        .keys()
        .next()
        .unwrap()
        .clone();
    apply(
        &engine,
        &mut world,
        GameIntent::ProduceAt {
            recipe: recipe("cook"),
            target: Some(WorldTarget::TemporaryObject {
                object: object.clone(),
            }),
            quantity: quantity(1),
        },
    );
    let events = v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "cooked"), 1);
    assert!(events.iter().any(
        |event| matches!(&event.event, GameEvent::ProductionResolved {
        facility: Some(WorldTarget::TemporaryObject { object: actual }), .. } if actual == &object)
    ));
    v::tick_n(&engine, &mut world, 7, &mut NeverDraw);
    assert!(world.runtime.temporary_objects.is_empty());
    assert!(
        world
            .ground_items
            .iter()
            .any(|ground| ground.stack.item == item("pebble"))
    );
}

#[test]
fn prayer_fractional_drain_and_source_hp_regeneration_survive_restart() {
    let mut content = v::content();
    v::with_prayer(&mut content);
    content.initial_state.hitpoints = 8;
    let (engine, mut world) = setup(content);
    apply(
        &engine,
        &mut world,
        GameIntent::SetPrayer {
            prayer: v::prayer().to_string(),
            enabled: true,
        },
    );
    v::tick_n(&engine, &mut world, 59, &mut NeverDraw);
    assert_eq!(state(&world).prayer_points, 1);
    assert_eq!(
        state(&world)
            .runtime
            .combat
            .prayer_drain
            .as_ref()
            .unwrap()
            .numerator,
        59
    );
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    let events = v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(state(&world).prayer_points, 0);
    assert!(state(&world).runtime.combat.active_prayers.is_empty());
    assert!(
        events
            .iter()
            .any(|event| matches!(event.event, GameEvent::PrayerChanged { enabled: false, .. }))
    );
    v::tick_n(&engine, &mut world, 40, &mut NeverDraw);
    assert_eq!(state(&world).hitpoints, 9);
}

#[test]
fn appearance_and_experience_choices_emit_authoritative_events_only_when_guarded() {
    let mut content = v::content();
    content.mechanics.appearance = Some(AppearanceDefinition {
        choices: BTreeMap::from([("coat".into(), BTreeSet::from([1, 2]))]),
        confirmation_guard: Guard::Always,
        source: source(),
    });
    let experience = ExperienceId::new("experience.synthetic.new").unwrap();
    content.mechanics.experiences.insert(
        experience.clone(),
        ExperienceDefinition {
            id: experience.clone(),
            name: "Synthetic new player".into(),
            selection_guard: Guard::Setting {
                setting: CharacterSetting::Run(false),
            },
            source: source(),
        },
    );
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::ConfirmAppearance {
            appearance: BTreeMap::from([("coat".into(), 3)]),
        },
        GameErrorCode::InvalidInput,
    );
    let events = apply(
        &engine,
        &mut world,
        GameIntent::ConfirmAppearance {
            appearance: BTreeMap::from([("coat".into(), 2)]),
        },
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event.event, GameEvent::AppearanceConfirmed))
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(
        &engine,
        &mut world,
        GameIntent::SelectExperience {
            experience: experience.clone(),
        },
    );
    assert_eq!(state(&world).runtime.settings.experience, Some(experience));
}

#[test]
fn checked_legacy_schedule_migrates_without_old_flag_writes_or_ledger_reset() {
    let (engine, mut world) = setup(content());
    state_mut(&mut world).runtime.engine = EngineMetadata::Legacy;
    state_mut(&mut world)
        .flags
        .insert("__world_engine.food_ready".into(), 9);
    apply(&engine, &mut world, GameIntent::CancelActivity);
    assert_eq!(state(&world).runtime.food_ready, 9);
    assert!(matches!(
        state(&world).runtime.engine,
        EngineMetadata::Typed { .. }
    ));
    assert!(
        state(&world)
            .flags
            .keys()
            .all(|key| !key.starts_with("__world_engine."))
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    state_mut(&mut world).runtime.engine = EngineMetadata::Legacy;
    state_mut(&mut world)
        .flags
        .insert("__world_engine.unknown".into(), 1);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::CancelActivity,
        GameErrorCode::Unavailable,
    );
}

#[test]
fn typed_content_roundtrip_is_not_a_mechanics_fallback() {
    let mut content = v::content();
    v::typed_gather(&mut content);
    v::typed_recipe(&mut content, "bronze", v::cadence(6, 4, 5));
    let encoded = serde_json::to_string(&content).unwrap();
    let decoded: GameContent = serde_json::from_str(&encoded).unwrap();
    assert_eq!(content, decoded);
    let engine = WorldEngine::new(Arc::new(decoded)).unwrap();
    assert_eq!(engine.content().schema_version, CONTENT_SCHEMA_VERSION);
}

#[test]
fn independent_single_production_does_not_require_or_reserve_make_x_repeat_timing() {
    let mut content = v::content();
    v::typed_recipe(&mut content, "cook", v::cadence(1, 3, 4));
    let cadence = &mut content
        .recipes
        .get_mut(&recipe("cook"))
        .unwrap()
        .mechanics
        .as_mut()
        .unwrap()
        .cadence;
    cadence.first = v::unresolved("Make-X first phase not bound in this fixture");
    cadence.repeat = v::unresolved("Make-X repeat phase not bound in this fixture");
    give_initial(&mut content, &[stack("raw", 2)]);
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        produce("cook", Some("range"), 2),
        GameErrorCode::Unavailable,
    );
    for _ in 0..2 {
        apply(&engine, &mut world, produce("cook", Some("range"), 1));
        v::tick(&engine, &mut world, &mut NeverDraw);
    }
    assert_eq!(world.tick, 2);
    assert_eq!(count(&engine, &world, "cooked"), 2);
}

#[test]
fn consume_only_source_recipe_executes_ticks_and_xp_without_fake_output_items() {
    let mut content = v::content();
    v::typed_recipe(&mut content, "dough", v::cadence(2, 2, 2));
    let recipe = content.recipes.get_mut(&recipe("dough")).unwrap();
    recipe.inputs = vec![stack("egg", 1)];
    recipe.outputs.clear();
    recipe.xp = vec![XpReward {
        skill: v::named_skill("prayer"),
        amount_tenths: 45,
    }];
    give_initial(&mut content, &[stack("egg", 1)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, produce("dough", None, 1));
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "egg"), 1);
    let events = v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "egg"), 0);
    assert_eq!(
        state(&world).skills[&v::named_skill("prayer")].xp_tenths,
        45
    );
    assert!(events.iter().any(|event| matches!(&event.event, GameEvent::ProductionResolved { outputs, .. } if outputs.is_empty())));
}

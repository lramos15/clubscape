mod support;

use clubscape_game_types::*;
use std::collections::{BTreeMap, BTreeSet};
use support::{v2 as v, *};

#[test]
fn a_real_closed_door_has_reachable_face_then_atomically_opens_collision() {
    let mut content = v::content();
    let transform = ObjectTransformId::new("transform.synthetic.door").unwrap();
    let closed = ObjectStateId::new("object_state.synthetic.closed").unwrap();
    let open = ObjectStateId::new("object_state.synthetic.open").unwrap();
    add_object(
        &mut content,
        "door",
        InteractionAction::Effects {
            effects: vec![Effect::TransformObject {
                transform: transform.clone(),
                state: open.clone(),
            }],
        },
    );
    let placement = SourceObjectPlacement {
        shape: 0,
        quarter_turns: 0,
        layer: ObjectLayer::Wall,
    };
    content.spawns.get_mut(&spawn("door")).unwrap().placement = Some(placement.clone());
    cell(&mut content, tile(10, 10, 0)).blocked_movement = Direction::East.mask();
    cell(&mut content, tile(10, 10, 0)).blocked_sight = Direction::East.mask();
    cell(&mut content, tile(11, 10, 0)).blocked_movement = Direction::West.mask();
    cell(&mut content, tile(11, 10, 0)).blocked_sight = Direction::West.mask();
    let cells = vec![
        *cell(&mut content, tile(10, 10, 0)),
        *cell(&mut content, tile(11, 10, 0)),
    ];
    let open_cells = cells
        .iter()
        .map(|cell| CollisionCell {
            blocked_movement: 0,
            blocked_sight: 0,
            ..*cell
        })
        .collect();
    content.mechanics.object_transforms.insert(
        transform.clone(),
        ObjectTransformDefinition {
            id: transform.clone(),
            scope: CounterScope::World,
            spawn: spawn("door"),
            initial: closed.clone(),
            states: BTreeMap::from([
                (
                    closed,
                    ObjectTransformState {
                        object: Some(object("door")),
                        tile: tile(11, 10, 0),
                        door: Some(DoorPosition::Closed),
                        placement: placement.clone(),
                        collision: cells,
                    },
                ),
                (
                    open.clone(),
                    ObjectTransformState {
                        object: Some(object("door")),
                        tile: tile(11, 10, 0),
                        door: Some(DoorPosition::Open),
                        placement,
                        collision: open_cells,
                    },
                ),
            ]),
            source: source(),
        },
    );
    let (engine, mut world) = setup(content);
    let events = apply(&engine, &mut world, interact("door"));
    assert_eq!(world.runtime.object_states[&transform], open);
    assert!(
        events
            .iter()
            .any(|event| matches!(event.event, GameEvent::ObjectTransformed { .. }))
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
}

#[test]
fn private_rotated_instance_maps_cells_and_actors_without_global_changes() {
    let mut content = v::content();
    content.objects.get_mut(&object("bank")).unwrap().size_x = 2;
    let template = v::template();
    content.mechanics.instances.insert(
        template.clone(),
        InstanceTemplateDefinition {
            id: template.clone(),
            chunk_size: 8,
            chunks: vec![InstanceChunkMapping {
                source_region: content.initial_state.region.clone(),
                source_origin: tile(10, 10, 0),
                destination_region: RegionId::new("region.synthetic.floor_1").unwrap(),
                destination_origin: tile(10, 10, 1),
                quarter_turns: 1,
            }],
            private_to_character: true,
            source: source(),
        },
    );
    content.mechanics.travels.insert(
        v::travel("room"),
        TravelDefinition {
            id: v::travel("room"),
            guard: Guard::Always,
            destination: v::bound(TravelDestination::Fixed {
                location: WorldLocation {
                    region: RegionId::new("region.synthetic.floor_1").unwrap(),
                    tile: tile(10, 17, 1),
                    instance: Some(template),
                },
            }),
            channel_ticks: v::bound(0),
            cooldown_ticks: v::bound(10),
            cooldown_start: v::bound(CooldownStart::Completed),
            interruptions: BTreeSet::from([InterruptionCause::Movement]),
            completion_effects: vec![],
            source: source(),
        },
    );
    add_object(
        &mut content,
        "entrance",
        InteractionAction::TravelVia {
            travel: v::travel("room"),
        },
    );
    let (engine, mut world) = setup(content);
    let original_entities = world.entities.clone();
    apply(&engine, &mut world, interact("entrance"));
    let instance = state(&world).runtime.instance.clone().unwrap();
    assert_eq!(state(&world).tile, tile(10, 17, 1));
    assert_eq!(
        world.runtime.instances[&instance].entities[&spawn("bank")].tile,
        tile(10, 15, 1)
    );
    assert_eq!(world.entities, original_entities);
    assert_eq!(world.runtime.instances[&instance].owner, Some(actor()));
    world.validate_runtime(engine.content()).unwrap();
}

#[test]
fn typed_event_predicate_requires_resolved_failure_not_old_produced_or_success() {
    let mut content = v::content();
    v::typed_recipe(&mut content, "cook", v::cadence(1, 1, 1));
    content.recipes.get_mut(&recipe("cook")).unwrap().success = never();
    give_initial(&mut content, &[stack("raw", 1)]);
    content
        .tutorial
        .get_mut(&stage("start"))
        .unwrap()
        .transitions
        .push(ProgressTransition {
            event: "production_resolved".into(),
            target: Some(recipe("cook").to_string()),
            guard: Guard::Event {
                condition: EventCondition::Production {
                    recipe: recipe("cook"),
                    method: v::method("cook"),
                    facility: Some(spawn("range")),
                    outcome: ProductionOutcome::Failure,
                    output: Some(item("burnt")),
                },
            },
            effects: vec![Effect::SetTutorialStage {
                stage: stage("next"),
            }],
        });
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, produce("cook", Some("range"), 1));
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(state(&world).tutorial_stage, stage("next"));
    assert_eq!(count(&engine, &world, "burnt"), 1);
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 0);
}

#[test]
fn valid_wind_splash_can_finish_quest_before_kill_with_once_reward_and_persistent_caps() {
    let mut content = v::content();
    v::with_combat(&mut content);
    add_quest(&mut content);
    give_initial(&mut content, &[stack("rune", 1), stack("coins", 1)]);
    let claim = v::entitlement("wind_reward");
    content.mechanics.entitlements.insert(
        claim.clone(),
        EntitlementDefinition {
            id: claim.clone(),
            purpose: EntitlementPurpose::AtomicReward,
            source: source(),
        },
    );
    content
        .quests
        .get_mut(&quest())
        .unwrap()
        .transitions
        .push(ProgressTransition {
            event: "spell_resolved".into(),
            target: Some(v::spell().to_string()),
            guard: Guard::All {
                guards: vec![
                    quest_guard("quest_not_started"),
                    Guard::Event {
                        condition: EventCondition::Spell {
                            spell: v::spell(),
                            target: Some(spawn("enemy")),
                            outcomes: BTreeSet::from([SpellOutcome::Hit, SpellOutcome::Splash]),
                        },
                    },
                ],
            },
            effects: vec![Effect::Once {
                entitlement: claim.clone(),
                effects: vec![
                    quest_effect("quest_completed"),
                    Effect::AddQuestPoints { amount: 1 },
                ],
            }],
        });
    content
        .tutorial
        .get_mut(&stage("start"))
        .unwrap()
        .xp_caps_tenths
        .insert(v::named_skill("magic"), 30);
    let (engine, mut world) = setup(content);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Cast {
                spell: v::spell().to_string(),
                target: Some(spawn("enemy")),
            },
            &mut v::Rolls::new(&[0, 0]),
        )
        .unwrap();
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(state(&world).quest_points, 1);
    assert_eq!(
        state(&world).quests[&quest()].stage,
        stage("quest_completed")
    );
    assert_eq!(state(&world).tutorial_stage, stage("start"));
    assert_eq!(state(&world).skills[&v::named_skill("magic")].xp_tenths, 30);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 5);
    assert!(matches!(
        state(&world).runtime.entitlements[&claim],
        EntitlementState::Claimed { .. }
    ));
}

#[test]
fn departure_reconciliation_occurs_on_completed_transport_not_request_or_interruption() {
    let mut content = v::content();
    let reconciliation = ReconciliationId::new("reconciliation.synthetic.departure").unwrap();
    let entitlement = v::entitlement("departure");
    content.mechanics.entitlements.insert(
        entitlement.clone(),
        EntitlementDefinition {
            id: entitlement.clone(),
            purpose: EntitlementPurpose::Reconciliation {
                reconciliation: reconciliation.clone(),
            },
            source: source(),
        },
    );
    content.mechanics.reconciliations.insert(
        reconciliation.clone(),
        ReconciliationDefinition {
            id: reconciliation.clone(),
            entitlement: entitlement.clone(),
            policies: v::bound(vec![
                ContainerReconciliation::ReplaceInventory {
                    inventory: Box::new(Inventory::default()),
                },
                ContainerReconciliation::ReplaceBank {
                    slots: vec![Some(stack("coins", 25))],
                },
            ]),
            source: source(),
        },
    );
    content.mechanics.travels.insert(
        v::travel("depart"),
        TravelDefinition {
            id: v::travel("depart"),
            guard: Guard::Always,
            destination: v::bound(TravelDestination::Fixed {
                location: WorldLocation {
                    region: content.initial_state.region.clone(),
                    tile: tile(15, 15, 0),
                    instance: None,
                },
            }),
            channel_ticks: v::bound(3),
            cooldown_ticks: v::bound(3000),
            cooldown_start: v::bound(CooldownStart::Completed),
            interruptions: BTreeSet::from([
                InterruptionCause::Movement,
                InterruptionCause::AnotherAction,
                InterruptionCause::Combat,
            ]),
            completion_effects: vec![Effect::ReconcileContainers { reconciliation }],
            source: source(),
        },
    );
    add_object(
        &mut content,
        "depart",
        InteractionAction::TravelVia {
            travel: v::travel("depart"),
        },
    );
    let (engine, mut world) = setup(content);
    let original = state(&world).inventory.clone();
    apply(&engine, &mut world, interact("depart"));
    assert_eq!(state(&world).inventory, original);
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(&engine, &mut world, GameIntent::CancelActivity);
    v::tick_n(&engine, &mut world, 3, &mut NeverDraw);
    assert_eq!(state(&world).inventory, original);
    assert!(
        !state(&world)
            .runtime
            .entitlements
            .contains_key(&entitlement)
    );
    assert!(state(&world).runtime.travel_cooldowns.is_empty());
    apply(&engine, &mut world, interact("depart"));
    v::tick_n(&engine, &mut world, 3, &mut NeverDraw);
    assert_eq!(state(&world).tile, tile(15, 15, 0));
    assert!(
        state(&world)
            .inventory
            .slots
            .iter()
            .all(|slot| slot.is_none())
    );
    assert_eq!(state(&world).bank.slots[0], Some(stack("coins", 25)));
    assert_eq!(
        state(&world).runtime.travel_cooldowns[&v::travel("depart")],
        world.tick + 3000
    );
}

#[test]
fn stock_sensitive_shop_prices_every_unit_and_tracks_since_change_restock() {
    let mut content = v::content();
    content.items.get_mut(&item("pot")).unwrap().base_value = 26;
    content.shops.get_mut(&shop()).unwrap().stock[0].mechanics = Some(ShopLineMechanics {
        pricing: ShopPricing::StockSensitive {
            buy: StockPriceFormula {
                base_per_mille: 1300,
                change_per_stock: 30,
                minimum_per_mille: 300,
                maximum_per_mille: 6300,
                minimum_price: 1,
                rounding: IntegerRounding::Floor,
            },
            sell: StockPriceFormula {
                base_per_mille: 400,
                change_per_stock: 30,
                minimum_per_mille: 100,
                maximum_per_mille: 1400,
                minimum_price: 0,
                rounding: IntegerRounding::Floor,
            },
            overstock: v::bound(OverstockPricing::LinearToClamp),
        },
        restock: StockRestockRule {
            interval_ticks: 10,
            amount: quantity(1),
            phase: v::bound(RestockPhase::SinceLastStockChange),
        },
    });
    give_initial(&mut content, &[stack("pot", 2)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("store"));
    v::tick(&engine, &mut world, &mut NeverDraw);
    let slot = locate(&world, "pot");
    apply(
        &engine,
        &mut world,
        GameIntent::ShopSell {
            shop: shop(),
            inventory_slot: slot,
            quantity: quantity(2),
        },
    );
    assert_eq!(count(&engine, &world, "coins"), 19);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 7);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("pot")], 11);
    v::tick_n(&engine, &mut world, 9, &mut NeverDraw);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 7);
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 6);
}

#[test]
fn selected_source_choice_and_counter_effect_are_atomic_under_full_inventory_failure() {
    let mut content = v::content();
    add_dialogue(&mut content);
    let counter = v::counter("bounded");
    content.mechanics.counters.insert(
        counter.clone(),
        CounterDefinition {
            id: counter.clone(),
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
        .insert(counter.clone(), CounterValue::Integer(0));
    give_initial(&mut content, &[stack("pebble", 26)]);
    content.dialogues.get_mut(&dialogue()).unwrap().nodes[0].choices[0].effects = vec![
        Effect::AddCounter { counter, delta: 1 },
        Effect::GiveItems {
            items: vec![stack("egg", 1)],
        },
    ];
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("cook"));
    v::tick(&engine, &mut world, &mut NeverDraw);
    error_unchanged(
        &engine,
        &mut world,
        select("continue"),
        GameErrorCode::InventoryFull,
    );
}

#[test]
fn world_epoch_restock_does_not_fire_early_after_an_old_clock_sat_at_base_stock() {
    let (engine, mut world) = setup(content());
    state_mut(&mut world).inventory.slots[2] = Some(stack("coins", 20));
    apply(&engine, &mut world, interact("store"));
    next(&engine, &mut world);
    apply(
        &engine,
        &mut world,
        GameIntent::ShopBuy {
            shop: shop(),
            item_index: 0,
            quantity: quantity(1),
        },
    );
    ticks(&engine, &mut world, 9, &mut NeverDraw);
    assert_eq!(world.tick, 10);
    apply(
        &engine,
        &mut world,
        GameIntent::ShopBuy {
            shop: shop(),
            item_index: 0,
            quantity: quantity(1),
        },
    );
    next(&engine, &mut world);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 4);
    next(&engine, &mut world);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 5);
}

#[test]
fn blocked_object_side_is_not_a_valid_interaction_even_with_clear_los() {
    let mut content = v::content();
    content.objects.get_mut(&object("bank")).unwrap().clip = Some(ObjectClipDefinition {
        blocks_movement: false,
        blocks_projectiles: false,
        access_blocked_sides: Direction::West.mask(),
    });
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        interact("bank"),
        GameErrorCode::OutOfReach,
    );
}

#[test]
fn dormant_offline_players_do_not_receive_elapsed_regeneration_on_reconnect() {
    let mut content = v::content();
    content.initial_state.hitpoints = 5;
    let (engine, mut world) = setup(content);
    let context = clubscape_world_engine::TickContext {
        actors: BTreeMap::from([(
            actor(),
            clubscape_world_engine::ActorPresence {
                online: false,
                idle_milliseconds: 0,
                grave_interface: None,
            },
        )]),
    };
    for _ in 0..100 {
        engine
            .tick_with_context(&mut world, &mut NeverDraw, &context)
            .unwrap();
    }
    assert_eq!(state(&world).hitpoints, 5);
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(state(&world).hitpoints, 5);
}

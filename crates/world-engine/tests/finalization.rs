//! Execution tests for the final typed selectors. All worlds/policies are synthetic.
mod support;

use clubscape_game_types::*;
use clubscape_world_engine::{ActorPresence, TickContext};
use std::collections::{BTreeMap, BTreeSet};
use support::{v2 as v, *};

fn drops(content: &mut GameContent) {
    content.mechanics.player_drop = Some(PlayerDropPolicy {
        ordinary: v::bound(v::ground_policy()),
        stages: BTreeMap::new(),
        untradeable: None,
        before_playtime: None,
        source: source(),
    });
}

fn player_combat(content: &mut GameContent) {
    let mut unarmed = content.mechanics.combat_styles[&v::style("accurate")].clone();
    unarmed.id = v::style("unarmed");
    content
        .mechanics
        .combat_styles
        .insert(unarmed.id.clone(), unarmed);
    content.mechanics.player_combat = Some(PlayerCombatPolicy {
        unarmed: v::bound(WeaponDefinition {
            styles: vec![v::style("unarmed")],
            default_style: v::style("unarmed"),
            ammunition: None,
        }),
        engagement: v::bound(PlayerEngagementPolicy {
            combat_state_ticks: 4,
            logout_lock_ticks: 6,
            travel_lock_ticks: 8,
        }),
        source: source(),
    });
}

fn recovery_interfaces(content: &mut GameContent) {
    let grave = InterfaceId::new("interface.synthetic.grave").unwrap();
    let office = InterfaceId::new("interface.synthetic.office").unwrap();
    for id in [&grave, &office] {
        content.interfaces.insert(
            id.clone(),
            InterfaceDefinition {
                id: id.clone(),
                name: id.to_string(),
                access: InterfaceAccess::Contextual,
                source_ids: vec![],
                source: source(),
            },
        );
        content.initial_state.interfaces.push(id.clone());
    }
    content.mechanics.death.as_mut().unwrap().interfaces =
        Some(RecoveryInterfaces { grave, office });
}

fn dying_world(timing: DeathTiming) -> (clubscape_world_engine::WorldEngine, WorldState) {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::with_death(&mut content);
    v::armed(&mut content, false);
    recovery_interfaces(&mut content);
    content.mechanics.death.as_mut().unwrap().timing = v::bound(timing);
    content.initial_state.hitpoints = 1;
    give_initial(&mut content, &[stack("ore", 1), stack("egg", 1)]);
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
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    v::tick_n(&engine, &mut world, 4, &mut v::Hits(0));
    (engine, world)
}

fn leave_office(engine: &clubscape_world_engine::WorldEngine, world: &mut WorldState) {
    apply(engine, world, interact("cook"));
    for topic in ["fees", "timer", "kept"] {
        v::tick(engine, world, &mut NeverDraw);
        apply(engine, world, select(topic));
    }
    v::tick(engine, world, &mut NeverDraw);
    apply(engine, world, interact("portal"));
}

#[test]
fn ordinary_drop_uses_explicit_policy_and_persisted_identity_without_duplicating_items() {
    let mut content = v::content();
    drops(&mut content);
    give_initial(&mut content, &[stack("coins", 5)]);
    let (engine, mut world) = setup(content);
    let selected = locate(&world, "coins");
    let events = apply(
        &engine,
        &mut world,
        GameIntent::Drop {
            inventory_slot: selected,
            quantity: quantity(3),
        },
    );
    assert_eq!(count(&engine, &world, "coins"), 2);
    let ground = &world.ground_items[0];
    let id = ground.id.clone();
    assert_eq!(ground.stack, stack("coins", 3));
    assert_eq!(ground.public_at_tick, 100);
    assert_eq!(ground.expires_at_tick, 200);
    assert!(
        matches!(&world.runtime.ground_provenance[&id].producer, GroundProducer::PlayerDrop { actor: owner, at_tick: 0 } if owner == &actor())
    );
    assert!(events.iter().any(|event| matches!(
        event.event,
        GameEvent::ItemTransferred {
            from: ContainerKind::Inventory,
            to: ContainerKind::Ground,
            ..
        }
    )));
    v::tick(&engine, &mut world, &mut NeverDraw);
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    apply(
        &engine,
        &mut world,
        GameIntent::TakeGroundItem {
            ground_item_id: id.clone(),
        },
    );
    assert_eq!(count(&engine, &world, "coins"), 5);
    assert!(!world.runtime.ground_provenance.contains_key(&id));
    v::tick(&engine, &mut world, &mut NeverDraw);
    let selected = locate(&world, "coins");
    apply(
        &engine,
        &mut world,
        GameIntent::Drop {
            inventory_slot: selected,
            quantity: quantity(1),
        },
    );
    assert_ne!(world.ground_items[0].id, id);
}

#[test]
fn stage_drop_override_is_private_and_expires_without_falling_back_to_ordinary_policy() {
    let mut content = v::content();
    drops(&mut content);
    let private = GroundPolicyId::new("ground_policy.synthetic.tutorial").unwrap();
    content.mechanics.ground_policies.insert(
        private.clone(),
        GroundItemPolicy {
            id: private.clone(),
            public_after: v::bound(None),
            expires_after: v::bound(Some(3)),
            clock: None,
            owner_can_take: true,
            source: source(),
        },
    );
    content
        .mechanics
        .player_drop
        .as_mut()
        .unwrap()
        .stages
        .insert(stage("start"), v::bound(private));
    let (engine, mut world) = setup(content);
    world.characters.insert(
        actor_two(),
        engine
            .character_from_initial(actor_two(), "Other", BTreeMap::new())
            .unwrap(),
    );
    apply(
        &engine,
        &mut world,
        GameIntent::Drop {
            inventory_slot: 0,
            quantity: quantity(1),
        },
    );
    let id = world.ground_items[0].id.clone();
    let before = world.clone();
    assert_eq!(
        engine
            .apply_intent(
                &mut world,
                &actor_two(),
                &GameIntent::TakeGroundItem { ground_item_id: id },
                &mut NeverDraw
            )
            .unwrap_err()
            .code,
        GameErrorCode::NotOwned
    );
    assert_eq!(world, before);
    v::tick_n(&engine, &mut world, 3, &mut NeverDraw);
    assert!(world.ground_items.is_empty());
    assert!(world.runtime.ground_provenance.is_empty());
}

#[test]
fn unresolved_stage_drop_selector_is_not_replaced_by_the_bound_ordinary_selector() {
    let mut content = v::content();
    drops(&mut content);
    content
        .mechanics
        .player_drop
        .as_mut()
        .unwrap()
        .stages
        .insert(
            stage("start"),
            v::unresolved("source stage policy not observed"),
        );
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::Drop {
            inventory_slot: 0,
            quantity: quantity(1),
        },
        GameErrorCode::Unavailable,
    );
}

#[test]
fn real_npc_defeat_materializes_guaranteed_primary_and_tertiary_loot_once() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    let npc = content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap();
    npc.hitpoints = 1;
    let entry = |name, qty| LootEntry {
        item: item(name),
        minimum: quantity(qty),
        maximum: quantity(qty),
    };
    npc.mechanics.as_mut().unwrap().loot = vec![
        LootPool::Guaranteed {
            items: vec![entry("egg", 1)],
        },
        LootPool::Exclusive {
            total_weight: 4,
            entries: vec![
                WeightedLoot {
                    weight: 2,
                    items: vec![entry("coins", 3)],
                },
                WeightedLoot {
                    weight: 1,
                    items: vec![entry("hammer", 1)],
                },
                WeightedLoot {
                    weight: 1,
                    items: vec![],
                },
            ],
        },
        LootPool::Independent {
            chance: Ratio {
                numerator: 1,
                denominator: 5,
            },
            items: vec![entry("arrow", 1)],
        },
    ];
    let (engine, mut world) = setup(content);
    let events = engine
        .apply_intent(
            &mut world,
            &actor(),
            &interact("enemy"),
            &mut v::Rolls::new(&[816, 0, 1, 0, 0]),
        )
        .unwrap();
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 0);
    assert_eq!(world.ground_items.len(), 3);
    assert!(world.ground_items.iter().all(|ground| ground.owner.as_ref() == Some(&actor()) && ground.tile == tile(11, 10, 0)));
    assert!(
        world
            .ground_items
            .iter()
            .any(|ground| ground.stack == stack("coins", 3))
    );
    assert!(
        !world
            .ground_items
            .iter()
            .any(|ground| ground.stack.item == item("hammer"))
    );
    assert!(
        world
            .runtime
            .ground_provenance
            .values()
            .all(|origin| matches!(&origin.producer,
        GroundProducer::NpcLoot { spawn: source, life: 0, .. } if source == &spawn("enemy")))
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.event, GameEvent::NpcKilled { credited: true, .. }))
            .count(),
        1
    );
    let before_items = world.ground_items.clone();
    v::tick(&engine, &mut world, &mut NeverDraw);
    error_unchanged(&engine, &mut world, interact("enemy"), GameErrorCode::Busy);
    assert_eq!(world.ground_items, before_items);
}

#[test]
fn source_method_guard_rejects_melee_but_accepts_magic_before_spending_resources() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    give_initial(&mut content, &[stack("rune", 1), stack("coins", 1)]);
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
        .eligibility = v::bound(vec![AttackEligibility {
        method: AttackMethod::Magic,
        style: Some(v::style("magic")),
        guard: Guard::Always,
    }]);
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        interact("enemy"),
        GameErrorCode::RequirementNotMet,
    );
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
    assert_eq!(count(&engine, &world, "rune"), 0);
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
}

#[test]
fn source_unarmed_and_weapon_defaults_are_selected_without_guessing_a_registry_entry() {
    let mut content = v::content();
    v::with_combat(&mut content);
    player_combat(&mut content);
    give_initial(&mut content, &[stack("dagger", 1)]);
    let (engine, mut world) = setup(content);
    assert_eq!(
        state(&world).runtime.combat.style,
        Some(v::style("unarmed"))
    );
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    v::tick(&engine, &mut world, &mut NeverDraw);
    let selected = locate(&world, "dagger");
    apply(
        &engine,
        &mut world,
        GameIntent::Equip {
            inventory_slot: selected,
        },
    );
    assert_eq!(
        state(&world).runtime.combat.style,
        Some(v::style("accurate"))
    );
    assert_eq!(state(&world).runtime.combat.attack_ready, 4);
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(
        &engine,
        &mut world,
        GameIntent::Unequip {
            slot: slot("weapon"),
        },
    );
    assert_eq!(
        state(&world).runtime.combat.style,
        Some(v::style("unarmed"))
    );
    assert_eq!(state(&world).runtime.combat.attack_ready, 4);
}

#[test]
fn mixed_method_kill_credits_the_winning_contributors_method_and_ground_ownership() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    content
        .mechanics
        .combat_styles
        .get_mut(&v::style("accurate"))
        .unwrap()
        .maximum_hit = v::bound(MaximumHitFormula::Fixed { hit: 2 });
    let npc = content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap();
    npc.hitpoints = 3;
    let mechanics = npc.mechanics.as_mut().unwrap();
    mechanics.attribution = v::bound(KillMethodPolicy::MostDamageThenFirstMethod);
    mechanics.loot = vec![LootPool::Guaranteed {
        items: vec![LootEntry {
            item: item("egg"),
            minimum: quantity(1),
            maximum: quantity(1),
        }],
    }];
    let (engine, mut world) = setup(content);
    let mut other = engine
        .character_from_initial(actor_two(), "Ranged finisher", BTreeMap::new())
        .unwrap();
    other.equipment.insert(slot("weapon"), stack("pick", 1));
    other.equipment.insert(slot("ammo"), stack("arrow", 50));
    other.runtime.combat.style = Some(v::style("ranged"));
    world.characters.insert(actor_two(), other);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &interact("enemy"),
            &mut v::Rolls::new(&[816, 0, 2]),
        )
        .unwrap();
    engine
        .apply_intent(
            &mut world,
            &actor_two(),
            &interact("enemy"),
            &mut v::Rolls::new(&[768, 0, 1, 0]),
        )
        .unwrap();
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    let events = v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert!(events.iter().any(|event| event.actor_id == actor()
        && matches!(
            event.event,
            GameEvent::NpcKilled {
                credited: true,
                method: AttackMethod::Melee,
                ..
            }
        )));
    assert!(events.iter().any(|event| event.actor_id == actor_two()
        && matches!(
            event.event,
            GameEvent::NpcKilled {
                credited: false,
                method: AttackMethod::Ranged,
                ..
            }
        )));
    assert_eq!(world.ground_items.len(), 1);
    assert_eq!(world.ground_items[0].owner, Some(actor()));
    let contribution = &world.entities[&spawn("enemy")].runtime.contributions[&actor()];
    assert!(contribution.methods_complete);
    assert_eq!(contribution.methods[&AttackMethod::Melee].damage, 2);
}

#[test]
fn engagement_timeout_and_source_logout_lock_survive_cancellation() {
    let mut content = v::content();
    v::with_combat(&mut content);
    player_combat(&mut content);
    v::armed(&mut content, false);
    let mechanics = content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
        .mechanics
        .as_mut()
        .unwrap();
    mechanics.retaliation = true;
    mechanics.engagement = v::bound(NpcEngagementPolicy {
        leash_range: 5,
        inactivity_ticks: 3,
        reacquire_delay_ticks: 4,
        acquire_delay_ticks: 0,
        return_to_spawn: true,
        reset_life_on_return: true,
        aggression: None,
    });
    let (engine, mut world) = setup(content);
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(&engine, &mut world, GameIntent::CancelActivity);
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert!(
        world.entities[&spawn("enemy")]
            .runtime
            .retaliation_target
            .is_none()
    );
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 5);
    assert_eq!(world.entities[&spawn("enemy")].runtime.life, 1);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::RequestLogout,
        GameErrorCode::Busy,
    );
    v::tick_n(&engine, &mut world, 3, &mut NeverDraw);
    apply(&engine, &mut world, GameIntent::RequestLogout);
}

#[test]
fn bound_aggression_uses_actor_eligibility_and_declared_acquisition_delay() {
    let mut content = v::content();
    v::with_combat(&mut content);
    player_combat(&mut content);
    content
        .initial_state
        .flags
        .insert("aggression_allowed".into(), 0);
    let npc = content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap();
    npc.aggressive = true;
    npc.mechanics.as_mut().unwrap().engagement = v::bound(NpcEngagementPolicy {
        leash_range: 5,
        inactivity_ticks: 20,
        reacquire_delay_ticks: 5,
        acquire_delay_ticks: 1,
        return_to_spawn: false,
        reset_life_on_return: false,
        aggression: Some(AggressionPolicy {
            acquisition_range: 3,
            require_line_of_sight: true,
            guard: Guard::Flag {
                name: "aggression_allowed".into(),
                equals: 1,
            },
        }),
    });
    let (engine, mut world) = setup(content);
    let mut other = engine
        .character_from_initial(actor_two(), "Eligible target", BTreeMap::new())
        .unwrap();
    other.tile = tile(12, 10, 0);
    other.flags.insert("aggression_allowed".into(), 1);
    world.characters.insert(actor_two(), other);
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(
        world.entities[&spawn("enemy")].runtime.retaliation_target,
        Some(actor_two())
    );
    assert_eq!(world.characters[&actor_two()].hitpoints, 10);
    v::tick(&engine, &mut world, &mut v::Hits(0));
    assert_eq!(world.characters[&actor_two()].hitpoints, 9);
    assert_eq!(state(&world).hitpoints, 10);
}

#[test]
fn make_x_of_one_uses_first_phase_not_unresolved_single_and_survives_restart() {
    let mut content = v::content();
    v::typed_recipe(&mut content, "cook", v::cadence(1, 3, 4));
    content
        .recipes
        .get_mut(&recipe("cook"))
        .unwrap()
        .mechanics
        .as_mut()
        .unwrap()
        .cadence
        .single = v::unresolved("single source phase unknown");
    give_initial(&mut content, &[stack("raw", 1)]);
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        produce("cook", Some("range"), 1),
        GameErrorCode::Unavailable,
    );
    apply(
        &engine,
        &mut world,
        GameIntent::ProduceSelected {
            recipe: recipe("cook"),
            target: Some(WorldTarget::Spawn {
                spawn: spawn("range"),
            }),
            quantity: quantity(1),
            mode: ProductionMode::MakeX,
        },
    );
    assert!(matches!(
        state(&world).activity,
        Activity::ProducingSelected {
            mode: ProductionMode::MakeX,
            next_tick: 3,
            ..
        }
    ));
    v::tick(&engine, &mut world, &mut NeverDraw);
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "cooked"), 0);
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(count(&engine, &world, "cooked"), 1);
}

#[test]
fn source_death_phase_deadlines_pause_actions_then_arrive_once_without_relosing_items() {
    let (engine, mut world) = dying_world(DeathTiming {
        dying_ticks: 2,
        respawn_ticks: 3,
    });
    let death = state(&world).runtime.active_death.clone().unwrap();
    assert!(matches!(
        state(&world).runtime.life,
        LifeState::Dying { at_tick: 6, .. }
    ));
    assert_eq!(state(&world).hitpoints, 0);
    let kept = state(&world).inventory.clone();
    let lost = world.runtime.deaths[&death]
        .grave
        .as_ref()
        .unwrap()
        .items
        .clone();
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert!(matches!(
        state(&world).runtime.life,
        LifeState::Respawning { at_tick: 9, .. }
    ));
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(state(&world).hitpoints, 0);
    let events = v::tick(&engine, &mut world, &mut NeverDraw);
    assert!(matches!(
        state(&world).runtime.life,
        LifeState::FirstDeathOffice { .. }
    ));
    assert_eq!(state(&world).hitpoints, 10);
    assert_eq!(state(&world).inventory, kept);
    assert_eq!(
        world.runtime.deaths[&death].grave.as_ref().unwrap().items,
        lost
    );
    assert_eq!(
        world.runtime.deaths[&death]
            .arrival
            .as_ref()
            .unwrap()
            .completed_at_tick,
        Some(9)
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.event, GameEvent::DeathOccurred { .. }))
    );
}

#[test]
fn actual_grave_opening_not_presence_assertion_controls_pause_and_movement_closes_it() {
    let (engine, mut world) = dying_world(DeathTiming {
        dying_ticks: 0,
        respawn_ticks: 0,
    });
    let death = state(&world).runtime.active_death.clone().unwrap();
    leave_office(&engine, &mut world);
    let grave_interface = engine
        .content()
        .mechanics
        .death
        .as_ref()
        .unwrap()
        .interfaces
        .as_ref()
        .unwrap()
        .grave
        .clone();
    v::tick(&engine, &mut world, &mut NeverDraw);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::OpenInterface {
            interface: grave_interface.clone(),
        },
        GameErrorCode::Unavailable,
    );
    let spoof = TickContext {
        actors: BTreeMap::from([(
            actor(),
            ActorPresence {
                online: true,
                idle_milliseconds: 0,
                grave_interface: Some(death.clone()),
            },
        )]),
    };
    engine
        .tick_with_context(&mut world, &mut NeverDraw, &spoof)
        .unwrap();
    let remaining = world.runtime.deaths[&death]
        .grave
        .as_ref()
        .unwrap()
        .active_ticks_remaining;
    let events = apply(
        &engine,
        &mut world,
        GameIntent::OpenGrave {
            death: death.clone(),
        },
    );
    assert!(events.iter().any(|event| matches!(&event.event, GameEvent::InterfacePresented { interface, context: InterfaceContext::Grave { death: id } } if interface == &grave_interface && id == &death)));
    v::tick_n(&engine, &mut world, 5, &mut NeverDraw);
    assert_eq!(
        world.runtime.deaths[&death]
            .grave
            .as_ref()
            .unwrap()
            .active_ticks_remaining,
        remaining
    );
    let events = apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(10, 11, 0),
            running: false,
        },
    );
    assert!(events.iter().any(|event| matches!(&event.event, GameEvent::InterfaceClosed { interface } if interface == &grave_interface)));
    engine
        .tick_with_context(&mut world, &mut NeverDraw, &spoof)
        .unwrap();
    assert_eq!(
        world.runtime.deaths[&death]
            .grave
            .as_ref()
            .unwrap()
            .active_ticks_remaining,
        remaining - 1
    );
}

#[test]
fn unstocked_shop_since_change_clock_is_real_and_validated_after_serialization() {
    let mut content = v::content();
    content.items.get_mut(&item("pebble")).unwrap().base_value = 26;
    content.shops.get_mut(&shop()).unwrap().unstocked = Some(UnstockedShopPolicy::Accept {
        maximum_lines: 4,
        base_stock: 0,
        rule: ShopLineMechanics {
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
                interval_ticks: 3,
                amount: quantity(1),
                phase: v::bound(RestockPhase::SinceLastStockChange),
            },
        },
    });
    give_initial(&mut content, &[stack("pebble", 2)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("store"));
    v::tick(&engine, &mut world, &mut NeverDraw);
    let selected = locate(&world, "pebble");
    apply(
        &engine,
        &mut world,
        GameIntent::ShopSell {
            shop: shop(),
            inventory_slot: selected,
            quantity: quantity(2),
        },
    );
    assert_eq!(count(&engine, &world, "coins"), 19);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("pebble")], 4);
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    world.validate_runtime(engine.content()).unwrap();
    v::tick_n(&engine, &mut world, 3, &mut NeverDraw);
    assert_eq!(world.shops[&shop()].stock[&item("pebble")], 1);
}

#[test]
fn second_actor_cannot_cross_an_open_shared_gate_or_a_forbidden_diagonal_route() {
    let mut content = v::content();
    v::with_run(&mut content);
    content
        .initial_state
        .flags
        .insert("exit_permission".into(), 0);
    let transform = ObjectTransformId::new("transform.synthetic.shared_gate").unwrap();
    let closed = ObjectStateId::new("object_state.synthetic.closed").unwrap();
    let open = ObjectStateId::new("object_state.synthetic.open").unwrap();
    let placement = SourceObjectPlacement {
        shape: 0,
        quarter_turns: 0,
        layer: ObjectLayer::Wall,
    };
    for cell in content
        .regions
        .get_mut(&content.initial_state.region)
        .unwrap()
        .cells
        .iter_mut()
    {
        if cell.tile.y() != 10 {
            cell.walkable = false;
        }
    }
    cell(&mut content, tile(11, 10, 0)).blocked_movement = Direction::East.mask();
    cell(&mut content, tile(12, 10, 0)).blocked_movement = Direction::West.mask();
    let initial = vec![
        *cell(&mut content, tile(11, 10, 0)),
        *cell(&mut content, tile(12, 10, 0)),
    ];
    add_object(
        &mut content,
        "gate",
        InteractionAction::Effects {
            effects: vec![Effect::TransformObject {
                transform: transform.clone(),
                state: open.clone(),
            }],
        },
    );
    interaction(&mut content, "gate").guard = Guard::Flag {
        name: "exit_permission".into(),
        equals: 1,
    };
    content.mechanics.object_transforms.insert(
        transform.clone(),
        ObjectTransformDefinition {
            id: transform.clone(),
            scope: CounterScope::World,
            spawn: spawn("gate"),
            initial: closed.clone(),
            states: BTreeMap::from([
                (
                    closed,
                    ObjectTransformState {
                        object: Some(object("gate")),
                        tile: tile(11, 10, 0),
                        door: Some(DoorPosition::Closed),
                        placement: placement.clone(),
                        collision: initial.clone(),
                    },
                ),
                (
                    open.clone(),
                    ObjectTransformState {
                        object: Some(object("gate")),
                        tile: tile(11, 10, 0),
                        door: Some(DoorPosition::Open),
                        placement,
                        collision: initial
                            .iter()
                            .map(|cell| CollisionCell {
                                blocked_movement: 0,
                                ..*cell
                            })
                            .collect(),
                    },
                ),
            ]),
            source: source(),
        },
    );
    let traversal = TraversalId::new("traversal.synthetic.gate_permission").unwrap();
    content.mechanics.traversal.insert(
        traversal.clone(),
        TraversalDefinition {
            id: traversal,
            scope: CounterScope::World,
            edges: vec![TraversalEdge {
                from: tile(11, 10, 0),
                to: tile(12, 10, 0),
                bidirectional: true,
            }],
            guard: Guard::Flag {
                name: "exit_permission".into(),
                equals: 1,
            },
            source: source(),
        },
    );
    let (engine, mut world) = setup(content);
    state_mut(&mut world)
        .flags
        .insert("exit_permission".into(), 1);
    world.characters.insert(
        actor_two(),
        engine
            .character_from_initial(actor_two(), "Locked actor", BTreeMap::new())
            .unwrap(),
    );
    apply(&engine, &mut world, interact("gate"));
    engine
        .apply_intent(
            &mut world,
            &actor_two(),
            &GameIntent::Walk {
                destination: tile(13, 10, 0),
                running: false,
            },
            &mut NeverDraw,
        )
        .unwrap();
    v::tick_n(&engine, &mut world, 3, &mut NeverDraw);
    assert_eq!(world.runtime.object_states[&transform], open);
    assert_eq!(world.characters[&actor_two()].tile, tile(11, 10, 0));
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(13, 10, 0),
            running: true,
        },
    );
    state_mut(&mut world)
        .flags
        .insert("exit_permission".into(), 0);
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(
        state(&world).tile,
        tile(10, 10, 0),
        "a forbidden second running step rolls back the prefix"
    );

    let mut content = v::content();
    content
        .initial_state
        .flags
        .insert("exit_permission".into(), 0);
    content.mechanics.traversal = engine.content().mechanics.traversal.clone();
    let (engine, mut world) = setup(content);
    state_mut(&mut world).tile = tile(11, 10, 0);
    state_mut(&mut world).activity = Activity::Walking {
        path: vec![tile(12, 11, 0)],
        running: false,
    };
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(
        state(&world).tile,
        tile(11, 10, 0),
        "both diagonal cardinal routes enforce traversal guards"
    );
}

#[test]
fn combined_collision_states_keep_two_overlapping_doors_open_without_last_writer_loss() {
    let mut content = v::content();
    let a = ObjectTransformId::new("transform.synthetic.a").unwrap();
    let b = ObjectTransformId::new("transform.synthetic.b").unwrap();
    let closed = ObjectStateId::new("object_state.synthetic.closed").unwrap();
    let open = ObjectStateId::new("object_state.synthetic.open").unwrap();
    cell(&mut content, tile(11, 10, 0)).blocked_movement =
        Direction::East.mask() | Direction::North.mask();
    cell(&mut content, tile(12, 10, 0)).blocked_movement = Direction::West.mask();
    cell(&mut content, tile(11, 11, 0)).blocked_movement = Direction::South.mask();
    let initial = [tile(11, 10, 0), tile(12, 10, 0), tile(11, 11, 0)]
        .map(|tile| *cell(&mut content, tile))
        .to_vec();
    let cells = |east_open: bool, north_open: bool| {
        initial
            .iter()
            .map(|cell| {
                let mut cell = *cell;
                if east_open {
                    if cell.tile == tile(11, 10, 0) {
                        cell.blocked_movement &= !Direction::East.mask();
                    } else if cell.tile == tile(12, 10, 0) {
                        cell.blocked_movement &= !Direction::West.mask();
                    }
                }
                if north_open {
                    if cell.tile == tile(11, 10, 0) {
                        cell.blocked_movement &= !Direction::North.mask();
                    } else if cell.tile == tile(11, 11, 0) {
                        cell.blocked_movement &= !Direction::South.mask();
                    }
                }
                cell
            })
            .collect::<Vec<_>>()
    };
    for (name, id, east) in [("door_a", a.clone(), true), ("door_b", b.clone(), false)] {
        add_object(
            &mut content,
            name,
            InteractionAction::Effects {
                effects: vec![Effect::TransformObject {
                    transform: id.clone(),
                    state: open.clone(),
                }],
            },
        );
        let state = |collision, door| ObjectTransformState {
            object: Some(object(name)),
            tile: tile(11, 10, 0),
            door: Some(door),
            placement: SourceObjectPlacement {
                shape: 0,
                quarter_turns: 0,
                layer: ObjectLayer::Wall,
            },
            collision,
        };
        content.mechanics.object_transforms.insert(
            id.clone(),
            ObjectTransformDefinition {
                id,
                scope: CounterScope::World,
                spawn: spawn(name),
                initial: closed.clone(),
                states: BTreeMap::from([
                    (closed.clone(), state(initial.clone(), DoorPosition::Closed)),
                    (open.clone(), state(cells(east, !east), DoorPosition::Open)),
                ]),
                source: source(),
            },
        );
    }
    let group = CollisionGroupId::new("collision_group.synthetic.doors").unwrap();
    content.mechanics.collision_groups.insert(
        group.clone(),
        CollisionGroupDefinition {
            id: group,
            scope: CounterScope::World,
            transforms: BTreeSet::from([a.clone(), b.clone()]),
            states: v::bound(
                [(false, false), (false, true), (true, false), (true, true)]
                    .into_iter()
                    .map(|(east, north)| CombinedCollisionState {
                        selection: BTreeMap::from([
                            (a.clone(), if east { open.clone() } else { closed.clone() }),
                            (b.clone(), if north { open.clone() } else { closed.clone() }),
                        ]),
                        collision: cells(east, north),
                    })
                    .collect(),
            ),
            source: source(),
        },
    );
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("door_a"));
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(&engine, &mut world, interact("door_b"));
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
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(12, 11, 0),
            running: false,
        },
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(state(&world).tile, tile(12, 11, 0));
}

#[test]
fn geometry_changing_morph_updates_source_identity_and_collision_in_one_counter_transaction() {
    let mut content = v::content();
    let counter = v::counter("gate_morph");
    content.mechanics.counters.insert(
        counter.clone(),
        CounterDefinition {
            id: counter.clone(),
            scope: CounterScope::World,
            value_type: CounterType::Boolean,
            initial: CounterValue::Boolean(false),
            source_variable: None,
            source: source(),
        },
    );
    let transform = ObjectTransformId::new("transform.synthetic.morph").unwrap();
    let closed = ObjectStateId::new("object_state.synthetic.closed").unwrap();
    let open = ObjectStateId::new("object_state.synthetic.open").unwrap();
    add_object(
        &mut content,
        "morph_gate",
        InteractionAction::Effects {
            effects: vec![Effect::SetCounter {
                counter: counter.clone(),
                value: CounterValue::Boolean(true),
            }],
        },
    );
    content.objects.get_mut(&object("range")).unwrap().size_x = 2;
    cell(&mut content, tile(11, 10, 0)).walkable = false;
    let original = vec![
        *cell(&mut content, tile(11, 10, 0)),
        *cell(&mut content, tile(12, 10, 0)),
    ];
    content.mechanics.object_transforms.insert(
        transform.clone(),
        ObjectTransformDefinition {
            id: transform.clone(),
            scope: CounterScope::World,
            spawn: spawn("morph_gate"),
            initial: closed.clone(),
            source: source(),
            states: BTreeMap::from([
                (
                    closed.clone(),
                    ObjectTransformState {
                        object: Some(object("morph_gate")),
                        tile: tile(11, 10, 0),
                        door: None,
                        placement: SourceObjectPlacement {
                            shape: 10,
                            quarter_turns: 0,
                            layer: ObjectLayer::GameObject,
                        },
                        collision: original.clone(),
                    },
                ),
                (
                    open.clone(),
                    ObjectTransformState {
                        object: Some(object("range")),
                        tile: tile(11, 10, 0),
                        door: None,
                        placement: SourceObjectPlacement {
                            shape: 10,
                            quarter_turns: 0,
                            layer: ObjectLayer::GameObject,
                        },
                        collision: original
                            .iter()
                            .map(|cell| CollisionCell {
                                walkable: true,
                                ..*cell
                            })
                            .collect(),
                    },
                ),
            ]),
        },
    );
    content
        .objects
        .get_mut(&object("morph_gate"))
        .unwrap()
        .morph = Some(SourceObjectMorph {
        counter: counter.clone(),
        variants: BTreeMap::from([(0, Some(object("morph_gate"))), (1, Some(object("range")))]),
        fallback: Some(object("morph_gate")),
        collision: Some(v::bound(ObjectMorphCollision {
            placements: BTreeMap::from([(spawn("morph_gate"), transform.clone())]),
            variants: vec![
                ObjectMorphCollisionCase {
                    value: 0,
                    state: closed.clone(),
                },
                ObjectMorphCollisionCase {
                    value: 1,
                    state: open.clone(),
                },
            ],
            fallback: closed,
        })),
    });
    let (engine, mut world) = setup(content);
    let events = apply(&engine, &mut world, interact("morph_gate"));
    assert_eq!(
        world.runtime.counters[&counter],
        CounterValue::Boolean(true)
    );
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
            destination: tile(12, 10, 0),
            running: false,
        },
    );
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(state(&world).tile, tile(12, 10, 0));
}

#[test]
fn full_ground_capacity_rolls_back_npc_defeat_xp_and_all_loot_producers() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    let npc = content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap();
    npc.hitpoints = 1;
    npc.mechanics.as_mut().unwrap().loot = vec![LootPool::Guaranteed {
        items: vec![LootEntry {
            item: item("egg"),
            minimum: quantity(1),
            maximum: quantity(1),
        }],
    }];
    let (engine, mut world) = setup(content);
    world.ground_items = (0..32_768)
        .map(|index| GroundItem {
            id: format!("fixture.capacity.{index}"),
            tile: tile(10, 10, 0),
            stack: stack("pebble", 1),
            owner: None,
            public_at_tick: 0,
            expires_at_tick: u64::MAX,
            instance: None,
        })
        .collect();
    let before = world.clone();
    let error = engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap_err();
    assert_eq!(error.code, GameErrorCode::InventoryFull);
    assert!(world == before);
}

#[test]
fn declared_style_filter_rejects_another_style_of_the_same_method() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, true);
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
        .eligibility = v::bound(vec![AttackEligibility {
        method: AttackMethod::Ranged,
        style: Some(v::style("ranged")),
        guard: Guard::Always,
    }]);
    let (engine, mut world) = setup(content);
    apply(
        &engine,
        &mut world,
        GameIntent::SetCombatStyle {
            style: v::style("rapid").to_string(),
        },
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    error_unchanged(
        &engine,
        &mut world,
        interact("enemy"),
        GameErrorCode::RequirementNotMet,
    );
    assert_eq!(state(&world).equipment[&slot("ammo")].quantity.get(), 50);
}

#[test]
fn leash_disengagement_and_home_travel_lock_use_declared_deadlines() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    player_combat(&mut content);
    v::with_run(&mut content);
    let mechanics = content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
        .mechanics
        .as_mut()
        .unwrap();
    mechanics.retaliation = true;
    mechanics.engagement = v::bound(NpcEngagementPolicy {
        leash_range: 2,
        inactivity_ticks: 100,
        reacquire_delay_ticks: 4,
        acquire_delay_ticks: 0,
        return_to_spawn: true,
        reset_life_on_return: true,
        aggression: None,
    });
    content.mechanics.travels.insert(
        v::travel("home"),
        TravelDefinition {
            id: v::travel("home"),
            guard: Guard::Always,
            destination: v::bound(TravelDestination::Fixed {
                location: WorldLocation {
                    region: content.initial_state.region.clone(),
                    tile: tile(20, 20, 0),
                    instance: None,
                },
            }),
            channel_ticks: v::bound(0),
            cooldown_ticks: v::bound(0),
            cooldown_start: v::bound(CooldownStart::Completed),
            interruptions: BTreeSet::from([InterruptionCause::Combat]),
            completion_effects: vec![],
            source: source(),
        },
    );
    add_object(
        &mut content,
        "home",
        InteractionAction::TravelVia {
            travel: v::travel("home"),
        },
    );
    set_position(&mut content, "home", tile(14, 11, 0));
    let (engine, mut world) = setup(content);
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(14, 10, 0),
            running: true,
        },
    );
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert!(
        world.entities[&spawn("enemy")]
            .runtime
            .retaliation_target
            .is_none()
    );
    error_unchanged(&engine, &mut world, interact("home"), GameErrorCode::Busy);
    v::tick_n(&engine, &mut world, 5, &mut NeverDraw);
    apply(&engine, &mut world, interact("home"));
    assert_eq!(state(&world).tile, tile(20, 20, 0));
}

#[test]
fn traversal_permissions_map_into_a_rotated_shared_instance_per_actor() {
    let mut content = v::content();
    content.initial_state.flags.insert("may_cross".into(), 0);
    for cell in &mut content
        .regions
        .get_mut(&content.initial_state.region)
        .unwrap()
        .cells
    {
        if cell.tile.y() != 10 {
            cell.walkable = false;
        }
    }
    let template = InstanceTemplateId::new("instance_template.synthetic.shared").unwrap();
    content.mechanics.instances.insert(
        template.clone(),
        InstanceTemplateDefinition {
            id: template.clone(),
            chunk_size: 8,
            private_to_character: false,
            source: source(),
            chunks: vec![InstanceChunkMapping {
                source_region: content.initial_state.region.clone(),
                source_origin: tile(10, 10, 0),
                destination_region: RegionId::new("region.synthetic.floor_1").unwrap(),
                destination_origin: tile(10, 10, 1),
                quarter_turns: 1,
            }],
        },
    );
    content.mechanics.travels.insert(
        v::travel("shared"),
        TravelDefinition {
            id: v::travel("shared"),
            guard: Guard::Always,
            destination: v::bound(TravelDestination::Fixed {
                location: WorldLocation {
                    region: RegionId::new("region.synthetic.floor_1").unwrap(),
                    tile: tile(10, 17, 1),
                    instance: Some(template),
                },
            }),
            channel_ticks: v::bound(0),
            cooldown_ticks: v::bound(0),
            cooldown_start: v::bound(CooldownStart::Completed),
            interruptions: BTreeSet::new(),
            completion_effects: vec![],
            source: source(),
        },
    );
    add_object(
        &mut content,
        "shared",
        InteractionAction::TravelVia {
            travel: v::travel("shared"),
        },
    );
    let id = TraversalId::new("traversal.synthetic.instance").unwrap();
    content.mechanics.traversal.insert(
        id.clone(),
        TraversalDefinition {
            id,
            scope: CounterScope::Instance,
            edges: vec![TraversalEdge {
                from: tile(11, 10, 0),
                to: tile(12, 10, 0),
                bidirectional: true,
            }],
            guard: Guard::Flag {
                name: "may_cross".into(),
                equals: 1,
            },
            source: source(),
        },
    );
    let (engine, mut world) = setup(content);
    state_mut(&mut world).flags.insert("may_cross".into(), 1);
    world.characters.insert(
        actor_two(),
        engine
            .character_from_initial(actor_two(), "Same instance", BTreeMap::new())
            .unwrap(),
    );
    apply(&engine, &mut world, interact("shared"));
    engine
        .apply_intent(
            &mut world,
            &actor_two(),
            &interact("shared"),
            &mut NeverDraw,
        )
        .unwrap();
    assert_eq!(
        state(&world).runtime.instance,
        world.characters[&actor_two()].runtime.instance
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    let walk = GameIntent::Walk {
        destination: tile(10, 14, 1),
        running: false,
    };
    apply(&engine, &mut world, walk.clone());
    engine
        .apply_intent(&mut world, &actor_two(), &walk, &mut NeverDraw)
        .unwrap();
    v::tick_n(&engine, &mut world, 4, &mut NeverDraw);
    assert_eq!(state(&world).tile, tile(10, 14, 1));
    assert_eq!(world.characters[&actor_two()].tile, tile(10, 16, 1));
}

#[test]
fn grave_open_request_rejects_other_owners_and_remote_positions_atomically() {
    let (engine, mut world) = dying_world(DeathTiming {
        dying_ticks: 0,
        respawn_ticks: 0,
    });
    let death = state(&world).runtime.active_death.clone().unwrap();
    leave_office(&engine, &mut world);
    world.characters.insert(
        actor_two(),
        engine
            .character_from_initial(actor_two(), "Not the owner", BTreeMap::new())
            .unwrap(),
    );
    let request = GameIntent::OpenGrave {
        death: death.clone(),
    };
    let before = world.clone();
    assert_eq!(
        engine
            .apply_intent(&mut world, &actor_two(), &request, &mut NeverDraw)
            .unwrap_err()
            .code,
        GameErrorCode::NotOwned
    );
    assert_eq!(world, before);
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(24, 24, 0),
            running: false,
        },
    );
    v::tick_n(&engine, &mut world, 14, &mut NeverDraw);
    error_unchanged(&engine, &mut world, request, GameErrorCode::OutOfReach);
}

//! Engine-owned projections/lifecycle; no network authorization or source content is mocked in.
mod support;

use clubscape_game_types::*;
use clubscape_world_engine::{ContextView, LifecycleTransition, TickContext};
use std::collections::{BTreeMap, BTreeSet};
use support::{v2 as v, *};

fn tracked_combat(content: &mut GameContent) {
    v::with_combat(content);
    content.mechanics.player_combat = Some(PlayerCombatPolicy {
        unarmed: v::bound(WeaponDefinition {
            styles: vec![v::style("accurate")],
            default_style: v::style("accurate"),
            ammunition: None,
        }),
        engagement: v::bound(PlayerEngagementPolicy {
            combat_state_ticks: 3,
            logout_lock_ticks: 3,
            travel_lock_ticks: 3,
        }),
        source: source(),
    });
}

#[test]
fn source_conditional_vital_gain_preserves_drained_and_boosted_values() {
    for vital in [Vital::Hitpoints, Vital::Prayer] {
        for (current, expected) in [(5, 5), (10, 11), (15, 15)] {
            let mut content = v::content();
            let skill_id = if vital == Vital::Hitpoints {
                hp_skill()
            } else {
                v::named_skill("prayer")
            };
            content
                .skills
                .get_mut(&skill_id)
                .unwrap()
                .xp_thresholds_tenths = (0..12).map(|index| index * 100).collect();
            content.initial_state.skills.insert(
                skill_id.clone(),
                SkillState {
                    xp_tenths: 900,
                    current_level: 10,
                },
            );
            content.mechanics.vitals.as_mut().unwrap().level_up =
                v::bound(LevelUpVitalPolicy::RaiseIfAtOldBaseOtherwisePreserve);
            add_object(
                &mut content,
                "gain",
                InteractionAction::Effects {
                    effects: vec![Effect::AwardXp {
                        rewards: vec![XpReward {
                            skill: skill_id.clone(),
                            amount_tenths: 100,
                        }],
                    }],
                },
            );
            let (engine, mut world) = setup(content);
            state_mut(&mut world)
                .skills
                .get_mut(&skill_id)
                .unwrap()
                .current_level = current;
            if vital == Vital::Hitpoints {
                state_mut(&mut world).hitpoints = current;
            } else {
                state_mut(&mut world).prayer_points = current;
            }
            apply(&engine, &mut world, interact("gain"));
            assert_eq!(state(&world).skills[&skill_id].xp_tenths, 1000);
            assert_eq!(state(&world).skills[&skill_id].current_level, expected);
            assert_eq!(
                if vital == Vital::Hitpoints {
                    state(&world).hitpoints
                } else {
                    state(&world).prayer_points
                },
                expected
            );
        }
    }
}

#[test]
fn cooking_or_mining_gain_does_not_require_unresolved_vital_level_up_policy() {
    let mut content = v::content();
    content.mechanics.vitals.as_mut().unwrap().level_up =
        v::unresolved("unneeded HP/Prayer policy");
    add_object(
        &mut content,
        "cook_reward",
        InteractionAction::Effects {
            effects: vec![Effect::AwardXp {
                rewards: vec![XpReward {
                    skill: skill(),
                    amount_tenths: 3000,
                }],
            }],
        },
    );
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("cook_reward"));
    assert_eq!(state(&world).skills[&skill()].xp_tenths, 3000);
    assert_eq!(state(&world).hitpoints, 10);
    assert_eq!(state(&world).prayer_points, 1);
}

#[test]
fn lifecycle_join_logout_rejoin_preserves_progress_and_cancels_offline_gathering() {
    let (engine, mut world) = setup(v::content());
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    apply(&engine, &mut world, interact("rock"));
    let revision = world.revision;
    let sequence = state(&world).last_command_sequence;
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::TransportLost)
        .unwrap();
    assert!(matches!(
        state(&world).runtime.presence,
        PresenceState::Offline { .. }
    ));
    for _ in 0..10 {
        engine.tick(&mut world, &mut NeverDraw).unwrap();
    }
    assert_eq!(count(&engine, &world, "ore"), 0);
    error_unchanged(
        &engine,
        &mut world,
        interact("rock"),
        GameErrorCode::SessionConflict,
    );
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Rejoin)
        .unwrap();
    assert!(
        engine
            .presence_view(&world, &actor())
            .unwrap()
            .accepts_input
    );
    assert_eq!(world.revision, revision);
    assert_eq!(state(&world).last_command_sequence, sequence);
    assert_eq!(state(&world).tutorial_stage, stage("start"));
}

#[test]
fn authentication_revocation_does_not_make_an_engaged_body_disappear() {
    let mut content = v::content();
    tracked_combat(&mut content);
    v::with_death(&mut content);
    v::armed(&mut content, false);
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
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    engine
        .apply_lifecycle(
            &mut world,
            &actor(),
            LifecycleTransition::AuthenticationRevoked,
        )
        .unwrap();
    let presence = engine.presence_view(&world, &actor()).unwrap();
    assert!(!presence.connected && !presence.accepts_input && presence.present_in_world);
    assert_eq!(
        world.entities[&spawn("enemy")].runtime.retaliation_target,
        Some(actor())
    );
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::CancelActivity,
        GameErrorCode::SessionConflict,
    );
    for _ in 0..3 {
        engine.tick(&mut world, &mut NeverDraw).unwrap();
    }
    let events = engine.tick(&mut world, &mut v::Hits(0)).unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event.event, GameEvent::DeathOccurred { .. }))
    );
    assert!(
        !engine
            .presence_view(&world, &actor())
            .unwrap()
            .present_in_world
    );
    assert_eq!(world.runtime.deaths.len(), 1);
}

#[test]
fn requested_logout_stays_guarded_and_reconcile_does_not_refresh_idle_on_polls() {
    let mut content = v::content();
    tracked_combat(&mut content);
    v::armed(&mut content, false);
    let (engine, mut world) = setup(content);
    engine
        .reconcile_presence(&mut world, &BTreeSet::from([actor()]))
        .unwrap();
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    let before = world.clone();
    assert_eq!(
        engine
            .apply_lifecycle(&mut world, &actor(), LifecycleTransition::RequestedLogout)
            .unwrap_err()
            .code,
        GameErrorCode::Busy
    );
    assert_eq!(world, before);
    engine.tick(&mut world, &mut NeverDraw).unwrap();
    engine
        .reconcile_presence(&mut world, &BTreeSet::from([actor()]))
        .unwrap();
    assert_eq!(
        engine.tick_context(&world).unwrap().actors[&actor()].idle_milliseconds,
        600
    );
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Activity)
        .unwrap();
    assert_eq!(
        engine.tick_context(&world).unwrap().actors[&actor()].idle_milliseconds,
        0
    );
}

#[test]
fn coordinator_restart_reconciliation_never_forces_all_persisted_actors_online() {
    let (engine, mut world) = setup(v::content());
    world.characters.insert(
        actor_two(),
        engine
            .character_from_initial(actor_two(), "Offline", BTreeMap::new())
            .unwrap(),
    );
    engine
        .reconcile_presence(&mut world, &BTreeSet::from([actor()]))
        .unwrap();
    assert!(engine.presence_view(&world, &actor()).unwrap().connected);
    assert!(
        !engine
            .presence_view(&world, &actor_two())
            .unwrap()
            .present_in_world
    );
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    engine
        .reconcile_presence(&mut world, &BTreeSet::new())
        .unwrap();
    assert!(
        !engine
            .presence_view(&world, &actor())
            .unwrap()
            .present_in_world
    );
    engine.tick(&mut world, &mut NeverDraw).unwrap();
}

#[test]
fn bank_and_shop_queries_are_read_only_and_quotes_match_the_actual_transfers() {
    let mut content = v::content();
    give_initial(&mut content, &[stack("coins", 10), stack("ore", 3)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("bank"));
    let before = world.clone();
    let bank = engine.bank_view(&world, &actor()).unwrap();
    assert_eq!(bank.banker, spawn("bank"));
    let slot = locate(&world, "ore");
    let quote = engine
        .bank_deposit_quote(&world, &actor(), slot, quantity(10))
        .unwrap();
    assert_eq!(quote.transferred, stack("ore", 3));
    assert_eq!(world, before);
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(
        &engine,
        &mut world,
        GameIntent::BankDeposit {
            banker: spawn("bank"),
            inventory_slot: slot,
            quantity: quantity(10),
        },
    );
    assert_eq!(state(&world).bank.slots[0], Some(quote.transferred));
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(&engine, &mut world, interact("store"));
    let before = world.clone();
    let shop = engine.shop_view(&world, &actor()).unwrap();
    assert_eq!(shop.lines[0].buy_price, 2);
    let quote = engine
        .shop_buy_quote(&world, &actor(), &shop.shop, 0, quantity(50), None)
        .unwrap();
    assert_eq!(quote.quantity, 5);
    assert_eq!(quote.total_price, 10);
    assert_eq!(world, before);
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(
        &engine,
        &mut world,
        GameIntent::ShopBuy {
            shop: shop.shop,
            item_index: 0,
            quantity: quantity(50),
            expected_item: None,
        },
    );
    assert_eq!(count(&engine, &world, "pot"), quote.quantity);
    assert_eq!(count(&engine, &world, "coins"), 0);
}

#[test]
fn dialogue_query_returns_only_eligible_choices_from_the_real_opened_node() {
    let mut content = v::content();
    add_dialogue(&mut content);
    content
        .initial_state
        .flags
        .insert("choice_locked".into(), 0);
    content.dialogues.get_mut(&dialogue()).unwrap().nodes[0]
        .choices
        .push(DialogueChoice {
            id: "locked".into(),
            text: "Hidden locked choice".into(),
            guard: Guard::Flag {
                name: "choice_locked".into(),
                equals: 1,
            },
            effects: vec![],
            next_node: None,
        });
    let (engine, mut world) = setup(content);
    assert!(engine.dialogue_view(&world, &actor()).unwrap().is_none());
    apply(&engine, &mut world, interact("cook"));
    let before = world.clone();
    let view = engine.dialogue_view(&world, &actor()).unwrap().unwrap();
    assert_eq!(view.node, "entry");
    assert_eq!(view.choices.len(), 1);
    assert_eq!(view.choices[0].id, "continue");
    assert!(matches!(
        engine.context_view(&world, &actor()).unwrap(),
        ContextView::Dialogue { .. }
    ));
    assert_eq!(world, before);
}

#[test]
fn ground_and_interaction_views_use_actual_reach_ownership_tools_and_capacity() {
    let mut content = v::content();
    add_item_spawn(&mut content);
    let (engine, mut world) = setup(content);
    let before = world.clone();
    let view = engine
        .target_view(
            &world,
            &actor(),
            &WorldTarget::Spawn {
                spawn: spawn("rock"),
            },
        )
        .unwrap()
        .unwrap();
    assert!(view.interactions[0].permission.allowed);
    let ground = engine.ground_item_views(&world, &actor()).unwrap();
    assert_eq!(ground.len(), 1);
    assert!(ground[0].can_take.allowed);
    assert_eq!(world, before);
    state_mut(&mut world).inventory.slots[0] = None;
    let view = engine
        .target_view(
            &world,
            &actor(),
            &WorldTarget::Spawn {
                spawn: spawn("rock"),
            },
        )
        .unwrap()
        .unwrap();
    assert!(!view.interactions[0].permission.allowed);
    world.ground_items[0].owner = Some(actor_two());
    world.ground_items[0].public_at_tick = 100;
    assert!(
        engine
            .ground_item_views(&world, &actor())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn static_and_dynamic_views_resolve_source_morphs_instead_of_returning_base_names() {
    let mut content = v::content();
    let counter = v::counter("visual");
    content.mechanics.counters.insert(
        counter.clone(),
        CounterDefinition {
            id: counter.clone(),
            scope: CounterScope::Character,
            value_type: CounterType::Integer {
                minimum: 0,
                maximum: 1,
            },
            initial: CounterValue::Integer(1),
            source_variable: None,
            source: source(),
        },
    );
    content
        .initial_state
        .runtime
        .counters
        .insert(counter.clone(), CounterValue::Integer(1));
    content.objects.get_mut(&object("bank")).unwrap().morph = Some(SourceObjectMorph {
        counter,
        variants: BTreeMap::from([(1, Some(object("range")))]),
        fallback: Some(object("bank")),
        collision: None,
    });
    let (engine, world) = setup(content);
    let view = engine
        .target_view(
            &world,
            &actor(),
            &WorldTarget::Spawn {
                spawn: spawn("bank"),
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(view.object, Some(object("range")));
    assert_eq!(view.name, engine.content().objects[&object("range")].name);
}

#[test]
fn tracked_offline_state_cannot_be_overridden_by_an_all_online_tick_context() {
    let (engine, mut world) = setup(v::content());
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::RequestedLogout)
        .unwrap();
    state_mut(&mut world).hitpoints = 5;
    let forged_context = TickContext::all_active(&world);
    for _ in 0..100 {
        engine
            .tick_with_context(&mut world, &mut NeverDraw, &forged_context)
            .unwrap();
    }
    assert_eq!(state(&world).hitpoints, 5);
}

fn recovery_world() -> (clubscape_world_engine::WorldEngine, WorldState, DeathId) {
    let mut content = v::content();
    tracked_combat(&mut content);
    v::with_death(&mut content);
    v::armed(&mut content, false);
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
    let grave = InterfaceId::new("interface.synthetic.grave_panel").unwrap();
    let office = InterfaceId::new("interface.synthetic.office_panel").unwrap();
    for id in [&grave, &office] {
        content.interfaces.insert(
            id.clone(),
            InterfaceDefinition {
                id: id.clone(),
                name: "Synthetic recovery".into(),
                access: InterfaceAccess::Contextual,
                source_ids: vec![],
                source: source(),
            },
        );
        content.initial_state.interfaces.push(id.clone());
    }
    content.mechanics.death.as_mut().unwrap().interfaces =
        Some(RecoveryInterfaces { grave, office });
    let (engine, mut world) = setup(content);
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    for _ in 0..4 {
        engine.tick(&mut world, &mut v::Hits(0)).unwrap();
    }
    let death = state(&world).runtime.active_death.clone().unwrap();
    (engine, world, death)
}

fn leave_recovery_office(engine: &clubscape_world_engine::WorldEngine, world: &mut WorldState) {
    apply(engine, world, interact("cook"));
    for topic in ["fees", "timer", "kept"] {
        engine.tick(world, &mut NeverDraw).unwrap();
        apply(engine, world, select(topic));
    }
    engine.tick(world, &mut NeverDraw).unwrap();
    apply(engine, world, interact("portal"));
}

#[test]
fn recovery_views_and_quotes_are_owned_guarded_and_side_effect_free() {
    let (engine, mut world, death) = recovery_world();
    let before = world.clone();
    let office = engine
        .recovery_view(&world, &actor(), &death, RecoveryStorage::DeathOffice)
        .unwrap();
    assert!(
        !office.entries.is_empty(),
        "Office can quote legitimate remote grave collection"
    );
    assert!(
        office
            .entries
            .iter()
            .all(|entry| entry.current_storage == RecoveryStorage::Grave)
    );
    let selected = office
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        engine
            .recovery_quote(
                &world,
                &actor(),
                &death,
                RecoveryStorage::DeathOffice,
                &selected
            )
            .unwrap()
            .full_selection_fee,
        0
    );
    assert_eq!(world, before);
    leave_recovery_office(&engine, &mut world);
    engine.tick(&mut world, &mut NeverDraw).unwrap();
    apply(
        &engine,
        &mut world,
        GameIntent::OpenGrave {
            death: death.clone(),
        },
    );
    let before = world.clone();
    assert!(matches!(
        engine.context_view(&world, &actor()).unwrap(),
        ContextView::Recovery { .. }
    ));
    assert_eq!(
        engine
            .recovery_view(&world, &actor(), &death, RecoveryStorage::Grave)
            .unwrap()
            .entries
            .len(),
        selected.len()
    );
    assert_eq!(world, before);
    world.characters.insert(
        actor_two(),
        engine
            .character_from_initial(actor_two(), "Not owner", BTreeMap::new())
            .unwrap(),
    );
    assert_eq!(
        engine
            .recovery_view(&world, &actor_two(), &death, RecoveryStorage::Grave)
            .unwrap_err()
            .code,
        GameErrorCode::NotOwned
    );
}

#[test]
fn acknowledged_activity_drives_idle_grave_pause_while_polled_rejoins_do_not_reset_it() {
    let (engine, mut world, death) = recovery_world();
    leave_recovery_office(&engine, &mut world);
    let initial = world.runtime.deaths[&death]
        .grave
        .as_ref()
        .unwrap()
        .active_ticks_remaining;
    for _ in 0..17 {
        engine
            .reconcile_presence(&mut world, &BTreeSet::from([actor()]))
            .unwrap();
        engine.tick(&mut world, &mut NeverDraw).unwrap();
    }
    assert_eq!(
        world.runtime.deaths[&death]
            .grave
            .as_ref()
            .unwrap()
            .active_ticks_remaining,
        initial - 16
    );
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Activity)
        .unwrap();
    engine.tick(&mut world, &mut NeverDraw).unwrap();
    assert_eq!(
        world.runtime.deaths[&death]
            .grave
            .as_ref()
            .unwrap()
            .active_ticks_remaining,
        initial - 17
    );
}

#[test]
fn office_capacity_counts_ordinary_merged_keys_not_duplicate_recovery_layout_entries() {
    let (engine, mut world, death) = recovery_world();
    let mut content = engine.content().clone();
    content.mechanics.death.as_mut().unwrap().office_capacity = 1;
    content.mechanics.death.as_mut().unwrap().office_overflow =
        v::unresolved("unreachable for one ordinary key");
    let engine = clubscape_world_engine::WorldEngine::new(std::sync::Arc::new(content)).unwrap();
    let entry = world.runtime.deaths[&death].grave.as_ref().unwrap().items[0].clone();
    let record = world.runtime.deaths.get_mut(&death).unwrap();
    record.office = (0..130)
        .map(|index| RecoveryItem {
            id: RecoveryItemId::new(format!("recovery_item.synthetic.repeat_{index}")).unwrap(),
            ..entry.clone()
        })
        .collect();
    world.validate_runtime(engine.content()).unwrap();
    assert_eq!(
        engine
            .recovery_view(&world, &actor(), &death, RecoveryStorage::DeathOffice)
            .unwrap()
            .entries
            .iter()
            .filter(|entry| entry.current_storage == RecoveryStorage::DeathOffice)
            .count(),
        130
    );
}

#[test]
fn source_death_supplies_pause_their_active_lifetime_while_owner_is_offline() {
    let mut content = v::content();
    tracked_combat(&mut content);
    v::with_death(&mut content);
    v::armed(&mut content, false);
    content.initial_state.hitpoints = 1;
    content.initial_state.runtime.settings.death_supply_piles = Some(true);
    give_initial(&mut content, &[stack("ore", 1), stack("cooked", 1)]);
    content
        .mechanics
        .ground_policies
        .get_mut(&v::ground_policy())
        .unwrap()
        .public_after = v::bound(None);
    content
        .mechanics
        .ground_policies
        .get_mut(&v::ground_policy())
        .unwrap()
        .expires_after = v::bound(Some(10));
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
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    for _ in 0..4 {
        engine.tick(&mut world, &mut v::Hits(0)).unwrap();
    }
    let supply = world
        .ground_items
        .iter()
        .find(|item| item.stack.item == support::item("cooked"))
        .unwrap()
        .id
        .clone();
    assert!(matches!(
        world.runtime.ground_provenance[&supply].producer,
        GroundProducer::DeathSupply { .. }
    ));
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::RequestedLogout)
        .unwrap();
    for _ in 0..20 {
        engine.tick(&mut world, &mut NeverDraw).unwrap();
    }
    assert!(world.ground_items.iter().any(|item| item.id == supply));
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Rejoin)
        .unwrap();
    for _ in 0..10 {
        engine.tick(&mut world, &mut NeverDraw).unwrap();
    }
    assert!(!world.ground_items.iter().any(|item| item.id == supply));
}

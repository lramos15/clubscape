mod support;

use clubscape_game_types::*;
use clubscape_world_engine::TickContext;
use support::{v2 as v, *};

#[test]
fn melee_executes_accuracy_damage_xp_cooldown_and_source_respawn() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "accurate");
    let events = engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 4);
    assert_eq!(
        state(&world).skills[&v::named_skill("attack")].xp_tenths,
        40
    );
    assert_eq!(state(&world).skills[&hp_skill()].xp_tenths, 913);
    assert!(events.iter().any(|event| matches!(
        event.event,
        GameEvent::CombatResolved {
            damage: 1,
            outcome: CombatOutcome::Hit,
            ..
        }
    )));
    error_unchanged(&engine, &mut world, interact("enemy"), GameErrorCode::Busy);
    v::tick_n(&engine, &mut world, 3, &mut NeverDraw);
    let old_hp = world.entities[&spawn("enemy")].hitpoints;
    assert_eq!(old_hp, 4);
    let events = v::tick_n(&engine, &mut world, 13, &mut v::Hits(0));
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 0);
    assert_eq!(world.entities[&spawn("enemy")].available_at_tick, 52);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.event, GameEvent::NpcKilled { credited: true, .. }))
            .count(),
        1
    );
    let life = world.entities[&spawn("enemy")].runtime.life;
    v::tick_n(&engine, &mut world, 35, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 5);
    assert_eq!(world.entities[&spawn("enemy")].runtime.life, life + 1);
    assert!(
        world.entities[&spawn("enemy")]
            .runtime
            .contributions
            .is_empty()
    );
}

#[test]
fn a_real_accuracy_miss_does_not_damage_or_award_damage_xp() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "accurate");
    let events = engine
        .apply_intent(
            &mut world,
            &actor(),
            &interact("enemy"),
            &mut v::Rolls::new(&[0, 0]),
        )
        .unwrap();
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 5);
    assert_eq!(state(&world).skills[&v::named_skill("attack")].xp_tenths, 0);
    assert!(events.iter().any(|event| matches!(
        event.event,
        GameEvent::CombatResolved {
            damage: 0,
            outcome: CombatOutcome::Miss,
            ..
        }
    )));
    assert_eq!(state(&world).runtime.combat.attack_ready, 5);
}

#[test]
fn combat_cancellation_and_target_switch_do_not_reset_cooldowns() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "accurate");
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(&engine, &mut world, GameIntent::CancelActivity);
    v::tick(&engine, &mut world, &mut NeverDraw);
    error_unchanged(&engine, &mut world, interact("enemy"), GameErrorCode::Busy);
    assert_eq!(state(&world).runtime.combat.attack_ready, 5);
}

#[test]
fn ranged_consumes_equipped_arrow_and_resolves_delayed_projectile_without_respends() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, true);
    set_position(&mut content, "enemy", tile(17, 10, 0));
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "ranged");
    let events = engine
        .apply_intent(
            &mut world,
            &actor(),
            &interact("enemy"),
            &mut v::Rolls::new(&[767, 0, 1, 4]),
        )
        .unwrap();
    assert_eq!(state(&world).equipment[&slot("ammo")].quantity.get(), 49);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 5);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.event, GameEvent::CombatResolved { .. }))
    );
    assert_eq!(world.runtime.projectiles.len(), 1);
    assert_eq!(
        world
            .ground_items
            .iter()
            .filter(|item| item.stack.item == support::item("arrow"))
            .count(),
        1
    );
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    v::tick(&engine, &mut world, &mut NeverDraw);
    let events = v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 4);
    assert_eq!(state(&world).equipment[&slot("ammo")].quantity.get(), 49);
    assert!(world.runtime.projectiles.is_empty());
    assert!(events.iter().any(|event| matches!(
        event.event,
        GameEvent::CombatResolved {
            method: AttackMethod::Ranged,
            damage: 1,
            ..
        }
    )));
}

#[test]
fn ranged_miss_still_spends_ammo_and_twenty_percent_break_boundary_is_independent() {
    for (break_roll, recovered) in [(0, false), (1, true)] {
        let mut content = v::content();
        v::with_combat(&mut content);
        v::armed(&mut content, true);
        let (engine, mut world) = setup(content);
        v::select_style(&engine, &mut world, "rapid");
        engine
            .apply_intent(
                &mut world,
                &actor(),
                &interact("enemy"),
                &mut v::Rolls::new(&[0, 0, break_roll]),
            )
            .unwrap();
        assert_eq!(state(&world).equipment[&slot("ammo")].quantity.get(), 49);
        assert_eq!(
            world
                .ground_items
                .iter()
                .any(|ground| ground.stack.item == item("arrow")),
            recovered
        );
        assert_eq!(state(&world).runtime.combat.attack_ready, 4);
        v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
        assert_eq!(world.entities[&spawn("enemy")].hitpoints, 5);
        assert_eq!(state(&world).skills[&v::named_skill("ranged")].xp_tenths, 0);
    }
}

#[test]
fn ranged_range_ammo_and_los_are_checked_before_consumption() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, true);
    set_position(&mut content, "enemy", tile(19, 10, 0));
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "ranged");
    error_unchanged(
        &engine,
        &mut world,
        interact("enemy"),
        GameErrorCode::OutOfReach,
    );
    apply(
        &engine,
        &mut world,
        GameIntent::SetCombatStyle {
            style: v::style("longrange").to_string(),
        },
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &interact("enemy"),
            &mut v::Rolls::new(&[0, 0, 0]),
        )
        .unwrap();
    assert_eq!(state(&world).equipment[&slot("ammo")].quantity.get(), 49);
}

#[test]
fn backpack_arrows_do_not_satisfy_equipped_ammunition() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, true);
    content.initial_state.equipment.remove(&slot("ammo"));
    give_initial(&mut content, &[stack("arrow", 50)]);
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "ranged");
    error_unchanged(
        &engine,
        &mut world,
        interact("enemy"),
        GameErrorCode::InsufficientItems,
    );
    assert_eq!(count(&engine, &world, "arrow"), 50);
}

#[test]
fn real_magic_splash_spends_two_rune_types_and_awards_base_55_xp_once() {
    let mut content = v::content();
    v::with_combat(&mut content);
    give_initial(&mut content, &[stack("rune", 2), stack("coins", 2)]);
    let (engine, mut world) = setup(content);
    let intent = GameIntent::Cast {
        spell: v::spell().to_string(),
        target: Some(spawn("enemy")),
    };
    engine
        .apply_intent(&mut world, &actor(), &intent, &mut v::Rolls::new(&[0, 0]))
        .unwrap();
    assert_eq!(count(&engine, &world, "rune"), 1);
    assert_eq!(count(&engine, &world, "coins"), 1);
    assert_eq!(state(&world).skills[&v::named_skill("magic")].xp_tenths, 55);
    let events = v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 5);
    assert!(events.iter().any(|event| matches!(
        event.event,
        GameEvent::SpellResolved {
            outcome: SpellOutcome::Splash,
            damage: 0,
            ..
        }
    )));
    assert_eq!(state(&world).skills[&v::named_skill("magic")].xp_tenths, 55);
    error_unchanged(&engine, &mut world, intent, GameErrorCode::Busy);
}

#[test]
fn missing_runes_and_unresolved_projectile_source_roll_back_everything() {
    let mut content = v::content();
    v::with_combat(&mut content);
    give_initial(&mut content, &[stack("rune", 2)]);
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::Cast {
            spell: v::spell().to_string(),
            target: Some(spawn("enemy")),
        },
        GameErrorCode::InsufficientItems,
    );
    let mut content = v::content();
    v::with_combat(&mut content);
    give_initial(&mut content, &[stack("rune", 2), stack("coins", 2)]);
    content
        .mechanics
        .projectiles
        .get_mut(&v::projectile())
        .unwrap()
        .timing = v::unresolved("Exact projectile launch/impact is not observed");
    let (engine, mut world) = setup(content);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::Cast {
            spell: v::spell().to_string(),
            target: Some(spawn("enemy")),
        },
        GameErrorCode::Unavailable,
    );
}

#[test]
fn tutorial_combat_caps_damage_xp_hp_xp_and_incoming_lethal_damage() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    content.initial_state.hitpoints = 1;
    let stage = content.tutorial.get_mut(&stage("start")).unwrap();
    stage.nonfatal_combat = true;
    stage.xp_caps_tenths.insert(v::named_skill("attack"), 20);
    stage.xp_caps_tenths.insert(hp_skill(), 900);
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
    v::tick_n(&engine, &mut world, 4, &mut v::Hits(0));
    assert_eq!(state(&world).hitpoints, 1);
    assert_eq!(
        state(&world).skills[&v::named_skill("attack")].xp_tenths,
        20
    );
    assert_eq!(state(&world).skills[&hp_skill()].xp_tenths, 900);
    assert!(world.runtime.deaths.is_empty());
}

#[test]
fn pending_projectile_invalidates_an_old_npc_life_without_repaying_runes() {
    let mut content = v::content();
    v::with_combat(&mut content);
    give_initial(&mut content, &[stack("rune", 1), stack("coins", 1)]);
    let (engine, mut world) = setup(content);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Cast {
                spell: v::spell().to_string(),
                target: Some(spawn("enemy")),
            },
            &mut v::Hits(0),
        )
        .unwrap();
    world
        .entities
        .get_mut(&spawn("enemy"))
        .unwrap()
        .runtime
        .life += 1;
    let events = v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 5);
    assert_eq!(count(&engine, &world, "rune"), 0);
    assert!(events.iter().any(|event| matches!(
        event.event,
        GameEvent::SpellResolved {
            outcome: SpellOutcome::Invalidated,
            ..
        }
    )));
}

#[test]
fn unresolved_loot_supplement_aborts_actual_defeat_instead_of_bones_only_fallback() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    let combat = content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap();
    combat.hitpoints = 1;
    combat.mechanics.as_mut().unwrap().loot = vec![
        LootPool::Guaranteed {
            items: vec![LootEntry {
                item: item("egg"),
                minimum: quantity(1),
                maximum: quantity(1),
            }],
        },
        LootPool::Unresolved {
            reason: "source goblin supplement has no observed weight".into(),
            source: source(),
        },
    ];
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "accurate");
    let before = world.clone();
    let error = engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap_err();
    assert_eq!(error.code, GameErrorCode::Unavailable);
    assert!(error.message.contains("supplement"));
    assert_eq!(world, before);
}

#[test]
fn first_contributor_wins_equal_damage_credit_and_other_actor_events_are_routed() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
        .hitpoints = 2;
    let (engine, mut world) = setup(content);
    let mut other = engine
        .character_from_initial(
            actor_two(),
            "Synthetic second contributor",
            Default::default(),
        )
        .unwrap();
    other.runtime.combat.style = Some(v::style("accurate"));
    world.characters.insert(actor_two(), other);
    v::select_style(&engine, &mut world, "accurate");
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    let events = engine
        .apply_intent(
            &mut world,
            &actor_two(),
            &interact("enemy"),
            &mut v::Hits(0),
        )
        .unwrap();
    assert!(events.iter().any(|event| event.actor_id == actor()
        && matches!(event.event, GameEvent::NpcKilled { credited: true, .. })));
    assert!(events.iter().any(|event| event.actor_id == actor_two()
        && matches!(
            event.event,
            GameEvent::NpcKilled {
                credited: false,
                ..
            }
        )));
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 0);
}

#[test]
fn invalid_trusted_combat_rng_aborts_resources_and_tick_metadata() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "accurate");
    let before = world.clone();
    let error = engine
        .apply_intent(
            &mut world,
            &actor(),
            &interact("enemy"),
            &mut Fixed(u32::MAX),
        )
        .unwrap_err();
    assert_eq!(error.code, GameErrorCode::InvalidInput);
    assert_eq!(world, before);
    let context = TickContext::all_active(&world);
    engine
        .tick_with_context(&mut world, &mut NeverDraw, &context)
        .unwrap();
}

#[test]
fn delayed_launch_awards_spell_xp_at_launch_and_impact_only_once_after_restart() {
    let mut content = v::content();
    v::with_combat(&mut content);
    give_initial(&mut content, &[stack("rune", 2), stack("coins", 2)]);
    let SourceBinding::Bound { value: timing, .. } = &mut content
        .mechanics
        .projectiles
        .get_mut(&v::projectile())
        .unwrap()
        .timing
    else {
        unreachable!()
    };
    timing.launch_delay_ticks = 2;
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
    assert_eq!(state(&world).skills[&v::named_skill("magic")].xp_tenths, 0);
    assert_eq!(count(&engine, &world, "rune"), 1);
    v::tick(&engine, &mut world, &mut NeverDraw);
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(state(&world).skills[&v::named_skill("magic")].xp_tenths, 55);
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    let events = v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(state(&world).skills[&v::named_skill("magic")].xp_tenths, 55);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event.event, GameEvent::SpellResolved { .. }))
            .count(),
        1
    );
}

#[test]
fn damage_on_launch_projectile_does_not_apply_damage_again_on_visual_impact() {
    let mut content = v::content();
    v::with_combat(&mut content);
    give_initial(&mut content, &[stack("rune", 2), stack("coins", 2)]);
    let SourceBinding::Bound { value: timing, .. } = &mut content
        .mechanics
        .projectiles
        .get_mut(&v::projectile())
        .unwrap()
        .timing
    else {
        unreachable!()
    };
    timing.damage_on_launch = true;
    let (engine, mut world) = setup(content);
    let launch = engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Cast {
                spell: v::spell().to_string(),
                target: Some(spawn("enemy")),
            },
            &mut v::Hits(0),
        )
        .unwrap();
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 3);
    assert_eq!(
        launch
            .iter()
            .filter(|event| matches!(event.event, GameEvent::SpellResolved { .. }))
            .count(),
        1
    );
    let later = v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 3);
    assert!(
        !later
            .iter()
            .any(|event| matches!(event.event, GameEvent::SpellResolved { .. }))
    );
}

#[test]
fn source_impact_recheck_keeps_a_live_target_even_after_it_moves_out_of_old_range() {
    let mut content = v::content();
    v::with_combat(&mut content);
    give_initial(&mut content, &[stack("rune", 1), stack("coins", 1)]);
    let (engine, mut world) = setup(content);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Cast {
                spell: v::spell().to_string(),
                target: Some(spawn("enemy")),
            },
            &mut v::Hits(0),
        )
        .unwrap();
    world.entities.get_mut(&spawn("enemy")).unwrap().tile = tile(24, 24, 0);
    let events = v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 3);
    assert_eq!(count(&engine, &world, "rune"), 0);
    assert!(events.iter().any(|event| matches!(
        event.event,
        GameEvent::SpellResolved {
            outcome: SpellOutcome::Hit,
            ..
        }
    )));
}

#[test]
fn allowing_combat_style_settings_does_not_unlock_repeated_attacks_in_a_later_stage() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
    content
        .tutorial
        .get_mut(&stage("next"))
        .unwrap()
        .allowed_actions = vec!["set_combat_style".into()];
    content
        .tutorial
        .get_mut(&stage("start"))
        .unwrap()
        .transitions
        .push(ProgressTransition {
            event: "combat_resolved".into(),
            target: None,
            guard: Guard::Always,
            effects: vec![Effect::SetTutorialStage {
                stage: stage("next"),
            }],
        });
    let (engine, mut world) = setup(content);
    v::select_style(&engine, &mut world, "accurate");
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    assert_eq!(state(&world).tutorial_stage, stage("next"));
    let events = v::tick_n(&engine, &mut world, 4, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 4);
    assert!(matches!(state(&world).activity, Activity::Idle));
    assert!(
        events
            .iter()
            .any(|event| matches!(event.event, GameEvent::Message { .. }))
    );
}

struct ExactDraws(std::collections::VecDeque<(u32, u32)>);
impl clubscape_world_engine::RandomSource for ExactDraws {
    fn draw_below(&mut self, upper: u32) -> GameResult<u32> {
        let (expected, value) = self.0.pop_front().expect("unexpected RNG boundary");
        assert_eq!(upper, expected);
        Ok(value)
    }
}

#[test]
fn spell_defence_selector_uses_npc_magic_not_its_different_physical_defence_stat() {
    let mut content = v::content();
    v::with_combat(&mut content);
    content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
        .defence = 100;
    give_initial(&mut content, &[stack("rune", 1), stack("coins", 1)]);
    let (engine, mut world) = setup(content);
    let mut rng = ExactDraws([(577, 576), (491, 0), (3, 2)].into());
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Cast {
                spell: v::spell().to_string(),
                target: Some(spawn("enemy")),
            },
            &mut rng,
        )
        .unwrap();
    assert!(rng.0.is_empty());
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 3);
    assert_eq!(state(&world).skills[&v::named_skill("magic")].xp_tenths, 95);
}

#[test]
fn prayer_modifier_changes_the_actual_npc_retaliation_defence_roll_boundary() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::with_prayer(&mut content);
    v::armed(&mut content, false);
    content
        .initial_state
        .skills
        .get_mut(&v::named_skill("defence"))
        .unwrap()
        .current_level = 20;
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
    apply(
        &engine,
        &mut world,
        GameIntent::SetPrayer {
            prayer: v::prayer().to_string(),
            enabled: true,
        },
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    engine
        .apply_intent(&mut world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(&engine, &mut world, GameIntent::CancelActivity);
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    let mut rng = ExactDraws([(431, 430), (1857, 0), (2, 1)].into());
    v::tick(&engine, &mut world, &mut rng);
    assert!(rng.0.is_empty());
    assert_eq!(state(&world).hitpoints, 9);
}

#[test]
fn ordinary_one_tile_melee_cannot_hit_diagonally_or_from_inside_the_target() {
    for location in [tile(11, 11, 0), tile(10, 10, 0)] {
        let mut content = v::content();
        v::with_combat(&mut content);
        v::armed(&mut content, false);
        set_position(&mut content, "enemy", location);
        let (engine, mut world) = setup(content);
        v::select_style(&engine, &mut world, "accurate");
        error_unchanged(
            &engine,
            &mut world,
            interact("enemy"),
            GameErrorCode::OutOfReach,
        );
    }
}

#[test]
fn retaliating_npc_routes_to_a_cardinal_edge_without_overlapping_the_actor() {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, false);
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
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(10, 11, 0),
            running: false,
        },
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(state(&world).tile, tile(10, 11, 0));
    assert_eq!(world.entities[&spawn("enemy")].tile, tile(10, 10, 0));
    assert_eq!(state(&world).hitpoints, 10);
}

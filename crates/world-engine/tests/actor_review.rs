//! Review regressions executed in synthetic worlds, not M1 journey acceptance.
mod support;

use std::collections::{BTreeMap, BTreeSet};

use clubscape_game_types::*;
use clubscape_world_engine::{ActorPresence, LifecycleTransition, TickContext, WorldEngine};
use support::{v2 as v, *};

fn combat_content(ranged: bool) -> GameContent {
    let mut content = v::content();
    v::with_combat(&mut content);
    v::armed(&mut content, ranged);
    content.initial_state.inventory = Inventory::default();
    give_initial(
        &mut content,
        &[
            stack(if ranged { "dagger" } else { "pick" }, 1),
            stack("ore", 1),
            stack("cooked", 1),
        ],
    );
    content
        .initial_state
        .equipment
        .insert(slot("ammo"), stack("arrow", 50));
    content
        .mechanics
        .vitals
        .as_mut()
        .unwrap()
        .regeneration
        .clear();
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
    content
        .npcs
        .get_mut(&v::npc())
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
        .hitpoints = 30;
    content
}

#[derive(Clone, Copy, Debug)]
enum PresenceCase {
    Offline,
    Connected,
    Disconnecting,
    LegacyPresent,
    LegacyOffline,
    LegacyAbsent,
}

impl PresenceCase {
    fn collides(self) -> bool {
        matches!(
            self,
            Self::Connected | Self::Disconnecting | Self::LegacyPresent
        )
    }
}

fn presence(
    engine: &WorldEngine,
    world: &mut WorldState,
    subject: &ActorId,
    case: PresenceCase,
) -> TickContext {
    if matches!(
        case,
        PresenceCase::Offline | PresenceCase::Connected | PresenceCase::Disconnecting
    ) {
        engine
            .apply_lifecycle(world, subject, LifecycleTransition::Join)
            .unwrap();
    }
    if matches!(case, PresenceCase::Disconnecting) {
        engine
            .apply_intent(world, subject, &interact("enemy"), &mut v::Hits(0))
            .unwrap();
    }
    if matches!(case, PresenceCase::Offline | PresenceCase::Disconnecting) {
        engine
            .apply_lifecycle(world, subject, LifecycleTransition::TransportLost)
            .unwrap();
        assert_eq!(
            engine
                .presence_view(world, subject)
                .unwrap()
                .present_in_world,
            case.collides()
        );
    }
    let online = match case {
        // Supplied legacy facts must not override acknowledged tracked presence.
        PresenceCase::Offline | PresenceCase::LegacyPresent => Some(true),
        PresenceCase::Connected | PresenceCase::Disconnecting | PresenceCase::LegacyOffline => {
            Some(false)
        }
        PresenceCase::LegacyAbsent => None,
    };
    TickContext {
        actors: online
            .map(|online| {
                (
                    subject.clone(),
                    ActorPresence {
                        online,
                        idle_milliseconds: 0,
                        grave_interface: None,
                    },
                )
            })
            .into_iter()
            .collect(),
    }
}

fn wandering_content(size: u8) -> GameContent {
    let mut content = combat_content(false);
    set_position(&mut content, "enemy", tile(14, 14, 0));
    content.initial_state.tile = tile(14 + u16::from(size), 14, 0);
    let npc = content.npcs.get_mut(&v::npc()).unwrap();
    npc.size = size;
    npc.navigation = NpcNavigation::Mobile {
        wander_radius: 1,
        step_ticks: v::bound(1),
        clip: NpcClipPolicy::MovementAndActors,
    };
    for x in 13..=15 + u16::from(size) {
        for y in 13..=14 + u16::from(size) {
            cell(&mut content, tile(x, y, 0)).walkable =
                x >= 14 && x <= 14 + u16::from(size) && y >= 14 && y < 14 + u16::from(size);
        }
    }
    content
}

fn round_trip(world: &WorldState) -> WorldState {
    serde_json::from_str(&serde_json::to_string(world).unwrap()).unwrap()
}

#[test]
fn wandering_uses_mechanical_presence_in_both_tick_entry_points() {
    for case in [
        PresenceCase::Offline,
        PresenceCase::Connected,
        PresenceCase::Disconnecting,
        PresenceCase::LegacyPresent,
        PresenceCase::LegacyOffline,
        PresenceCase::LegacyAbsent,
    ] {
        let (engine, mut world) = setup(wandering_content(1));
        let context = presence(&engine, &mut world, &actor(), case);
        let before = state(&world).clone();
        world = round_trip(&world);
        let mut advanced = world.clone();
        advanced.tick += 1;
        let mut draws = v::Rolls::new(&[0]);
        engine
            .tick_with_context(&mut world, &mut draws, &context)
            .unwrap();
        engine
            .process_advanced_tick_with_context(&mut advanced, &mut v::Rolls::new(&[0]), &context)
            .unwrap();
        assert_eq!(world, advanced, "{case:?}");
        assert_eq!(
            world.entities[&spawn("enemy")].tile,
            tile(if case.collides() { 14 } else { 15 }, 14, 0),
            "{case:?}"
        );
        assert_eq!(draws.0.len(), usize::from(case.collides()), "{case:?}");
        assert_eq!(state(&world).tile, before.tile);
        assert_eq!(state(&world).inventory, before.inventory);
        assert_eq!(state(&world).equipment, before.equipment);
        assert_eq!(state(&world).bank, before.bank);
        assert_eq!(state(&world).skills, before.skills);
        assert_eq!(world.revision, 0);
        assert_eq!(state(&world).last_command_sequence, 0);
    }
}

#[test]
fn rejoin_restores_collision_without_moving_or_recreating_retained_characters() {
    for case in [PresenceCase::Offline, PresenceCase::LegacyAbsent] {
        let (engine, mut world) = setup(wandering_content(1));
        let context = presence(&engine, &mut world, &actor(), case);
        let inventory = state(&world).inventory.clone();
        world = round_trip(&world);
        for expected_x in [15, 14] {
            engine
                .tick_with_context(&mut world, &mut v::Rolls::new(&[0]), &context)
                .unwrap();
            assert_eq!(
                world.entities[&spawn("enemy")].tile,
                tile(expected_x, 14, 0),
                "{case:?}"
            );
        }
        world = round_trip(&world);
        engine
            .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Rejoin)
            .unwrap();
        engine.tick(&mut world, &mut NeverDraw).unwrap();
        assert_eq!(world.entities[&spawn("enemy")].tile, tile(14, 14, 0));
        engine
            .apply_lifecycle(&mut world, &actor(), LifecycleTransition::RequestedLogout)
            .unwrap();
        world = round_trip(&world);
        engine.tick(&mut world, &mut v::Rolls::new(&[0])).unwrap();
        assert_eq!(world.entities[&spawn("enemy")].tile, tile(15, 14, 0));
        assert_eq!(state(&world).tile, tile(15, 14, 0));
        assert_eq!(state(&world).inventory, inventory);
    }
}

fn chasing_content() -> GameContent {
    let mut content = combat_content(false);
    content.initial_state.tile = tile(21, 20, 0);
    set_position(&mut content, "enemy", tile(20, 20, 0));
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
        leash_range: 50,
        inactivity_ticks: 4,
        reacquire_delay_ticks: 4,
        acquire_delay_ticks: 0,
        return_to_spawn: true,
        reset_life_on_return: false,
        aggression: None,
    });
    content
}

fn begin_chase(engine: &WorldEngine, world: &mut WorldState) {
    engine
        .apply_lifecycle(world, &actor(), LifecycleTransition::Join)
        .unwrap();
    engine
        .apply_intent(world, &actor(), &interact("enemy"), &mut v::Hits(0))
        .unwrap();
    engine.tick(world, &mut NeverDraw).unwrap();
    apply(
        engine,
        world,
        GameIntent::Walk {
            destination: tile(23, 20, 0),
            running: false,
        },
    );
}

fn add_blocker(engine: &WorldEngine, world: &mut WorldState) {
    world.characters.insert(
        actor_two(),
        engine
            .character_from_initial(actor_two(), "Retained blocker", BTreeMap::new())
            .unwrap(),
    );
}

#[test]
fn chasing_checks_trusted_presence_without_overlapping_the_detached_target() {
    for case in [
        PresenceCase::Offline,
        PresenceCase::Connected,
        PresenceCase::LegacyPresent,
        PresenceCase::LegacyOffline,
        PresenceCase::LegacyAbsent,
    ] {
        let (engine, mut world) = setup(chasing_content());
        begin_chase(&engine, &mut world);
        add_blocker(&engine, &mut world);
        let context = presence(&engine, &mut world, &actor_two(), case);
        world = round_trip(&world);
        engine
            .tick_with_context(&mut world, &mut NeverDraw, &context)
            .unwrap();
        assert_eq!(state(&world).tile, tile(22, 20, 0));
        let enemy = &world.entities[&spawn("enemy")];
        assert_eq!(
            enemy.tile,
            tile(if case.collides() { 20 } else { 21 }, 20, 0),
            "{case:?}"
        );
        assert_ne!(enemy.tile, state(&world).tile);
        assert_eq!(enemy.runtime.retaliation_target, Some(actor()));
        assert_eq!(enemy.runtime.attack_ready, 4);
    }
}

#[test]
fn returning_to_spawn_uses_the_same_presence_facts_as_chasing_and_wandering() {
    for case in [
        PresenceCase::Offline,
        PresenceCase::Connected,
        PresenceCase::LegacyPresent,
        PresenceCase::LegacyOffline,
        PresenceCase::LegacyAbsent,
    ] {
        let (engine, mut world) = setup(chasing_content());
        begin_chase(&engine, &mut world);
        engine.tick(&mut world, &mut NeverDraw).unwrap();
        engine.tick(&mut world, &mut NeverDraw).unwrap();
        assert_eq!(world.entities[&spawn("enemy")].tile, tile(22, 20, 0));
        add_blocker(&engine, &mut world);
        let context = presence(&engine, &mut world, &actor_two(), case);
        apply(
            &engine,
            &mut world,
            GameIntent::Walk {
                destination: tile(24, 20, 0),
                running: false,
            },
        );
        world = round_trip(&world);
        engine
            .tick_with_context(&mut world, &mut NeverDraw, &context)
            .unwrap();
        let enemy = &world.entities[&spawn("enemy")];
        assert!(enemy.runtime.returning_to_spawn);
        assert_eq!(enemy.runtime.retaliation_target, None);
        assert_eq!(
            enemy.tile,
            tile(if case.collides() { 22 } else { 21 }, 20, 0),
            "{case:?}"
        );
        assert_eq!(enemy.runtime.next_movement_tick, Some(5));
        assert_eq!(enemy.runtime.attack_ready, 4);
        assert_eq!(enemy.hitpoints, 29);
    }
}

#[test]
fn presence_filter_preserves_full_npc_footprints_and_plane_scoping() {
    for case in [PresenceCase::Offline, PresenceCase::Connected] {
        let (engine, mut world) = setup(wandering_content(2));
        let context = presence(&engine, &mut world, &actor(), case);
        engine
            .tick_with_context(&mut world, &mut v::Rolls::new(&[0]), &context)
            .unwrap();
        assert_eq!(
            world.entities[&spawn("enemy")].tile,
            tile(if case.collides() { 14 } else { 15 }, 14, 0),
            "{case:?}"
        );
    }
    let mut content = wandering_content(1);
    content.initial_state.region = RegionId::new("region.synthetic.floor_1").unwrap();
    content.initial_state.tile = tile(15, 14, 1);
    let (engine, mut world) = setup(content);
    presence(&engine, &mut world, &actor(), PresenceCase::Connected);
    engine.tick(&mut world, &mut v::Rolls::new(&[0])).unwrap();
    assert_eq!(world.entities[&spawn("enemy")].tile, tile(15, 14, 0));
    assert_eq!(state(&world).tile, tile(15, 14, 1));
}

#[test]
fn identical_world_and_instance_tiles_use_separate_mechanical_occupancy() {
    for case in [PresenceCase::Connected, PresenceCase::Offline] {
        let mut content = wandering_content(1);
        let region = content.initial_state.region.clone();
        content.mechanics.instances.insert(
            v::template(),
            InstanceTemplateDefinition {
                id: v::template(),
                chunk_size: 8,
                private_to_character: true,
                source: source(),
                chunks: vec![InstanceChunkMapping {
                    source_region: region.clone(),
                    source_origin: tile(10, 10, 0),
                    destination_region: region.clone(),
                    destination_origin: tile(10, 10, 0),
                    quarter_turns: 0,
                }],
            },
        );
        content.mechanics.travels.insert(
            v::travel("private"),
            TravelDefinition {
                id: v::travel("private"),
                guard: Guard::Always,
                destination: v::bound(TravelDestination::Fixed {
                    location: WorldLocation {
                        region,
                        tile: tile(15, 14, 0),
                        instance: Some(v::template()),
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
        let spell = content.mechanics.spells.get_mut(&v::spell()).unwrap();
        spell.action = SpellAction::Teleport {
            travel: v::travel("private"),
        };
        spell.runes.clear();
        spell.launch_xp.clear();
        let (engine, mut world) = setup(content);
        apply(
            &engine,
            &mut world,
            GameIntent::Cast {
                spell: v::spell().to_string(),
                target: None,
            },
        );
        let instance = state(&world).runtime.instance.clone().unwrap();
        let context = presence(&engine, &mut world, &actor(), case);
        world = round_trip(&world);
        let mut draws = v::Rolls::new(&[0, 0]);
        engine
            .tick_with_context(&mut world, &mut draws, &context)
            .unwrap();
        assert_eq!(world.entities[&spawn("enemy")].tile, tile(15, 14, 0));
        assert_eq!(
            world.runtime.instances[&instance].entities[&spawn("enemy")].tile,
            tile(if case.collides() { 14 } else { 15 }, 14, 0),
            "{case:?}"
        );
        assert_eq!(draws.0.len(), usize::from(case.collides()));
        assert_eq!(state(&world).runtime.instance, Some(instance));
        assert_eq!(state(&world).tile, tile(15, 14, 0));
    }
}

#[test]
fn presence_filter_preserves_movement_only_walls_and_other_npc_clipping() {
    let mut content = wandering_content(1);
    let NpcNavigation::Mobile { clip, .. } =
        &mut content.npcs.get_mut(&v::npc()).unwrap().navigation
    else {
        unreachable!()
    };
    *clip = NpcClipPolicy::MovementOnly;
    let (engine, mut world) = setup(content);
    presence(&engine, &mut world, &actor(), PresenceCase::Connected);
    engine.tick(&mut world, &mut v::Rolls::new(&[0])).unwrap();
    assert_eq!(world.entities[&spawn("enemy")].tile, tile(15, 14, 0));

    for other_npc in [false, true] {
        let mut content = wandering_content(1);
        if other_npc {
            let mut guard = content.npcs[&v::npc()].clone();
            guard.id = NpcId::new("npc.synthetic.guard").unwrap();
            guard.combat = None;
            guard.navigation = NpcNavigation::Stationary {
                anchor: StationaryAnchor::Walkable,
            };
            let mut placement = content.spawns[&spawn("enemy")].clone();
            placement.id = spawn("guard");
            placement.tile = tile(15, 14, 0);
            placement.kind = SpawnKind::Npc {
                npc: guard.id.clone(),
            };
            placement.interactions.clear();
            content.spawns.insert(placement.id.clone(), placement);
            content.npcs.insert(guard.id.clone(), guard);
        } else {
            cell(&mut content, tile(14, 14, 0)).blocked_movement |= 2;
            cell(&mut content, tile(15, 14, 0)).blocked_movement |= 8;
        }
        let (engine, mut world) = setup(content);
        presence(&engine, &mut world, &actor(), PresenceCase::Offline);
        engine.tick(&mut world, &mut NeverDraw).unwrap();
        assert_eq!(world.entities[&spawn("enemy")].tile, tile(14, 14, 0));
    }
}

fn fighting(world: &WorldState, style: &str, queued: u64, ready: u64) {
    assert_eq!(
        state(world).activity,
        Activity::Fighting {
            target: spawn("enemy"),
            style: v::style(style).to_string(),
            next_tick: queued,
        }
    );
    assert_eq!(state(world).runtime.combat.style, Some(v::style(style)));
    assert_eq!(state(world).runtime.combat.target, Some(spawn("enemy")));
    assert_eq!(state(world).runtime.combat.attack_ready, ready);
}

fn attack(engine: &WorldEngine, world: &mut WorldState, ranged: bool) {
    let mut draws = v::Rolls::new(if ranged {
        &[767, 0, 1, 0]
    } else {
        &[815, 0, 1]
    });
    engine
        .apply_intent(world, &actor(), &interact("enemy"), &mut draws)
        .unwrap();
    assert!(draws.0.is_empty());
}

#[test]
fn weapon_changes_synchronize_queued_style_without_resetting_any_combat_clock() {
    for ranged in [false, true] {
        let mut content = combat_content(ranged);
        let npc = content
            .npcs
            .get_mut(&v::npc())
            .unwrap()
            .combat
            .as_mut()
            .unwrap();
        npc.attack_speed_ticks = 6;
        npc.mechanics.as_mut().unwrap().retaliation = true;
        let (engine, mut world) = setup(content);
        attack(&engine, &mut world, ranged);
        v::tick(&engine, &mut world, &mut NeverDraw);
        let old_combat = state(&world).runtime.combat.clone();
        let old_npc = world.entities[&spawn("enemy")].clone();
        let inventory_slot = locate(&world, if ranged { "dagger" } else { "pick" });
        apply(&engine, &mut world, GameIntent::Equip { inventory_slot });
        let selected = if ranged { "accurate" } else { "ranged" };
        fighting(&world, selected, 4, 4);
        let mut expected_combat = old_combat;
        expected_combat.style = Some(v::style(selected));
        assert_eq!(state(&world).runtime.combat, expected_combat);
        assert_eq!(world.entities[&spawn("enemy")], old_npc);
        world = round_trip(&world);
        v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
        assert_eq!(world.entities[&spawn("enemy")].hitpoints, 29);
        fighting(&world, selected, 4, 4);
        assert_eq!(state(&world).hitpoints, 10);
        let mut draws = v::Rolls::new(if ranged {
            &[815, 0, 1]
        } else {
            &[767, 0, 1, 0]
        });
        v::tick(&engine, &mut world, &mut draws);
        assert!(draws.0.is_empty());
        fighting(&world, selected, 8, 8);
        assert_eq!(world.entities[&spawn("enemy")].runtime.attack_ready, 6);
        assert_eq!(state(&world).equipment[&slot("ammo")].quantity.get(), 49);
        v::tick(&engine, &mut world, &mut NeverDraw);
        assert_eq!(state(&world).hitpoints, 10);
        v::tick(&engine, &mut world, &mut v::Hits(0));
        assert_eq!(world.entities[&spawn("enemy")].hitpoints, 28);
        assert_eq!(state(&world).hitpoints, 9);
        assert_eq!(world.entities[&spawn("enemy")].runtime.attack_ready, 12);
        fighting(&world, selected, 8, 8);
    }
}

#[test]
fn unequipping_uses_unarmed_default_without_cancelling_the_pending_swing() {
    let (engine, mut world) = setup(combat_content(false));
    attack(&engine, &mut world, false);
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(
        &engine,
        &mut world,
        GameIntent::Unequip {
            slot: slot("weapon"),
        },
    );
    fighting(&world, "unarmed", 4, 4);
    world = round_trip(&world);
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 29);
    v::tick(&engine, &mut world, &mut v::Rolls::new(&[767, 0, 1]));
    fighting(&world, "unarmed", 8, 8);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 28);
}

#[test]
fn equipping_preserves_an_offered_style_instead_of_forcing_the_new_default() {
    let mut content = combat_content(false);
    let mut replacement = content.items[&item("dagger")].equipment.clone().unwrap();
    replacement.weapon = Some(WeaponDefinition {
        styles: vec![v::style("accurate"), v::style("unarmed")],
        default_style: v::style("unarmed"),
        ammunition: None,
    });
    content.items.get_mut(&item("pick")).unwrap().equipment = Some(replacement);
    let (engine, mut world) = setup(content);
    attack(&engine, &mut world, false);
    v::tick(&engine, &mut world, &mut NeverDraw);
    let inventory_slot = locate(&world, "pick");
    apply(&engine, &mut world, GameIntent::Equip { inventory_slot });
    fighting(&world, "accurate", 4, 4);
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    v::tick(&engine, &mut world, &mut v::Rolls::new(&[815, 0, 1]));
    fighting(&world, "accurate", 8, 8);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 28);
}

#[test]
fn rapid_selection_preserves_the_queued_four_tick_swing_then_uses_three_ticks() {
    let (engine, mut world) = setup(combat_content(true));
    attack(&engine, &mut world, true);
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(
        &engine,
        &mut world,
        GameIntent::SetCombatStyle {
            style: v::style("rapid").to_string(),
        },
    );
    fighting(&world, "rapid", 4, 4);
    world = round_trip(&world);
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 29);
    assert_eq!(state(&world).equipment[&slot("ammo")].quantity.get(), 49);
    v::tick(&engine, &mut world, &mut v::Rolls::new(&[575, 0, 1, 0]));
    fighting(&world, "rapid", 7, 7);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 29);
    assert_eq!(state(&world).equipment[&slot("ammo")].quantity.get(), 48);
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 29);
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 28);
    v::tick(&engine, &mut world, &mut v::Rolls::new(&[575, 0, 1, 0]));
    fighting(&world, "rapid", 10, 10);
    assert_eq!(state(&world).equipment[&slot("ammo")].quantity.get(), 47);
}

#[test]
fn weapon_changes_preserve_distinct_food_delayed_attack_and_spell_clocks() {
    let (engine, mut world) = setup(combat_content(false));
    attack(&engine, &mut world, false);
    v::tick(&engine, &mut world, &mut NeverDraw);
    let inventory_slot = locate(&world, "cooked");
    apply(&engine, &mut world, GameIntent::Eat { inventory_slot });
    fighting(&world, "accurate", 4, 7);
    assert_eq!(state(&world).runtime.combat.spell_ready, 4);
    v::tick(&engine, &mut world, &mut NeverDraw);
    let inventory_slot = locate(&world, "pick");
    apply(&engine, &mut world, GameIntent::Equip { inventory_slot });
    fighting(&world, "ranged", 4, 7);
    assert_eq!(state(&world).runtime.combat.spell_ready, 4);
    world = round_trip(&world);
    v::tick_n(&engine, &mut world, 4, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 29);
    assert_eq!(state(&world).equipment[&slot("ammo")].quantity.get(), 50);
    v::tick(&engine, &mut world, &mut v::Rolls::new(&[767, 0, 1, 0]));
    fighting(&world, "ranged", 11, 11);
    assert_eq!(state(&world).runtime.combat.spell_ready, 4);
    assert_eq!(state(&world).equipment[&slot("ammo")].quantity.get(), 49);
}

#[test]
fn unsupported_style_and_equipment_requirements_fail_atomically_midcombat() {
    let mut content = combat_content(false);
    content
        .items
        .get_mut(&item("pick"))
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
        .requirements
        .push(SkillRequirement {
            skill: v::named_skill("ranged"),
            level: 5,
            basis: SkillLevelBasis::Base,
        });
    let (engine, mut world) = setup(content);
    attack(&engine, &mut world, false);
    v::tick(&engine, &mut world, &mut NeverDraw);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::SetCombatStyle {
            style: v::style("rapid").to_string(),
        },
        GameErrorCode::RequirementNotMet,
    );
    let inventory_slot = locate(&world, "pick");
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::Equip { inventory_slot },
        GameErrorCode::RequirementNotMet,
    );
    fighting(&world, "accurate", 4, 4);
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    v::tick(&engine, &mut world, &mut v::Rolls::new(&[815, 0, 1]));
    fighting(&world, "accurate", 8, 8);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 28);
}

#[test]
fn removing_required_ammo_still_stops_combat_without_spending_or_rescheduling() {
    let (engine, mut world) = setup(combat_content(true));
    attack(&engine, &mut world, true);
    v::tick(&engine, &mut world, &mut NeverDraw);
    apply(
        &engine,
        &mut world,
        GameIntent::Unequip { slot: slot("ammo") },
    );
    fighting(&world, "ranged", 4, 4);
    v::tick_n(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 29);
    let before = state(&world).inventory.clone();
    let events = v::tick(&engine, &mut world, &mut NeverDraw);
    assert!(matches!(state(&world).activity, Activity::Idle));
    assert_eq!(state(&world).runtime.combat.attack_ready, 4);
    assert_eq!(state(&world).inventory, before);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 29);
    assert!(events.iter().any(|event| matches!(
        &event.event,
        GameEvent::Message { text } if text.contains("ammunition")
    )));
}

#[test]
fn saved_pre_fix_fighting_style_is_reconciled_before_its_unchanged_deadline() {
    let (engine, mut world) = setup(combat_content(false));
    attack(&engine, &mut world, false);
    v::tick(&engine, &mut world, &mut NeverDraw);
    let inventory_slot = locate(&world, "pick");
    apply(&engine, &mut world, GameIntent::Equip { inventory_slot });
    // Represent an acknowledged schedule written by the reviewed pre-fix engine.
    let Activity::Fighting { style, .. } = &mut state_mut(&mut world).activity else {
        unreachable!()
    };
    *style = v::style("accurate").to_string();
    world = round_trip(&world);
    v::tick(&engine, &mut world, &mut NeverDraw);
    fighting(&world, "ranged", 4, 4);
    assert_eq!(world.entities[&spawn("enemy")].hitpoints, 29);
    v::tick(&engine, &mut world, &mut NeverDraw);
    v::tick(&engine, &mut world, &mut v::Rolls::new(&[767, 0, 1, 0]));
    fighting(&world, "ranged", 8, 8);
}

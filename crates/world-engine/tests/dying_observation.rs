use std::{collections::BTreeMap, io::Read, sync::Arc};

use clubscape_content::{ValidationMode, load_compiled};
use clubscape_game_types::*;
use clubscape_world_engine::{LifecycleTransition, RandomSource, WorldEngine};

struct TickRandom;
impl RandomSource for TickRandom {
    fn draw_below(&mut self, upper: u32) -> GameResult<u32> {
        assert!(upper > 0);
        Ok(0)
    }
}

fn actor() -> ActorId {
    ActorId::new("actor.source.dying.view").unwrap()
}

fn stack(item: &str, quantity: u32) -> ItemStack {
    ItemStack {
        item: ItemId::new(item).unwrap(),
        quantity: Quantity::new(quantity).unwrap(),
        instance: None,
    }
}

fn setup(lose_weapon: bool) -> (WorldEngine, WorldState, CombatStyleId, SlotId) {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(&include_bytes!("../../../content/m1/game-content.csc.gz")[..])
        .read_to_end(&mut raw)
        .unwrap();
    let compiled = load_compiled(&raw, ValidationMode::Runtime).unwrap();
    let content = compiled.definition().clone();
    let weapon = content.items[&ItemId::new("item.sword.bronze").unwrap()]
        .equipment
        .as_ref()
        .unwrap()
        .clone();
    let original_style = weapon.weapon.as_ref().unwrap().default_style.clone();
    let engine = WorldEngine::new(Arc::new(content)).unwrap();
    let mut world = engine.initial_world().unwrap();
    let mut character = engine
        .character_from_initial(
            actor(),
            "Controlled dying observation",
            BTreeMap::from([("body_type".into(), 0)]),
        )
        .unwrap();
    // Controlled death-boundary preconditions, never a seeded player journey.
    character.tutorial_stage = StageId::new("stage.tutorial.mainland").unwrap();
    character.runtime.settings.experience =
        Some(ExperienceId::new("experience.brand_new").unwrap());
    character.runtime.settings.death_supply_piles = Some(false);
    character
        .equipment
        .insert(weapon.slot.clone(), stack("item.sword.bronze", 1));
    character.runtime.combat.style = Some(original_style.clone());
    if lose_weapon {
        character.inventory.slots[0] = Some(stack("item.fishing_net.small", 1));
        character.inventory.slots[1] = Some(stack("item.axe.bronze", 1));
        character.inventory.slots[2] = Some(stack("item.dagger.bronze", 1));
    }
    character.runtime.ui.as_mut().unwrap().active_interface = None;
    world.characters.insert(actor(), character);
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    assert!(engine.ui_view(&world, &actor()).is_ok());
    world.characters.get_mut(&actor()).unwrap().hitpoints = 0;
    engine.tick(&mut world, &mut TickRandom).unwrap();
    (engine, world, original_style, weapon.slot)
}

fn unarmed(engine: &WorldEngine) -> CombatStyleId {
    engine
        .content()
        .mechanics
        .player_combat
        .as_ref()
        .unwrap()
        .unarmed
        .require()
        .unwrap()
        .default_style
        .clone()
}

#[test]
fn a_source_death_that_loses_the_weapon_remains_observable() {
    let (engine, mut world, _, weapon_slot) = setup(true);
    let character = &world.characters[&actor()];
    assert!(matches!(character.runtime.life, LifeState::Dying { .. }));
    assert!(
        !character.equipment.contains_key(&weapon_slot),
        "The controlled source death must actually lose the equipped weapon"
    );
    let death = &world.runtime.deaths[character.runtime.active_death.as_ref().unwrap()];
    assert!(
        death
            .grave
            .as_ref()
            .unwrap()
            .items
            .iter()
            .any(|entry| { entry.stack.item.as_str() == "item.sword.bronze" })
    );
    let before = world.clone();
    let result = engine.ui_view(&world, &actor());
    assert!(
        result.is_ok(),
        "Dying source UI must remain readable after equipment loss: {:?}; style {:?}",
        result.err(),
        character.runtime.combat.style
    );
    assert!(
        world == before,
        "A dying-state observation must be read-only"
    );
    assert_eq!(
        world.characters[&actor()].runtime.combat.style,
        Some(unarmed(&engine))
    );
    let death_id = world.characters[&actor()]
        .runtime
        .active_death
        .clone()
        .unwrap();
    let arrival = world.runtime.deaths[&death_id].arrival.clone().unwrap();
    let inventory = world.characters[&actor()].inventory.clone();
    let skills = world.characters[&actor()].skills.clone();
    let lost_items = world.runtime.deaths[&death_id]
        .grave
        .as_ref()
        .unwrap()
        .items
        .clone();
    while world.tick < arrival.arrives_at_tick {
        let before_input = world.clone();
        let destination = world.characters[&actor()].tile;
        let denied = engine.apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Walk {
                destination,
                running: false,
            },
            &mut TickRandom,
        );
        assert!(
            denied.is_err(),
            "Dying/respawning must not accept ordinary input"
        );
        assert!(
            world == before_input,
            "A denied input must preserve death progression"
        );
        engine.tick(&mut world, &mut TickRandom).unwrap();
        let before_query = world.clone();
        assert!(engine.ui_view(&world, &actor()).is_ok());
        assert!(world == before_query);
    }
    assert!(matches!(
        world.characters[&actor()].runtime.life,
        LifeState::FirstDeathOffice { .. }
    ));
    assert_eq!(world.characters[&actor()].inventory, inventory);
    assert_eq!(world.characters[&actor()].skills, skills);
    assert_eq!(
        world.runtime.deaths[&death_id]
            .grave
            .as_ref()
            .unwrap()
            .items,
        lost_items
    );
    assert_eq!(
        world.runtime.deaths[&death_id]
            .arrival
            .as_ref()
            .unwrap()
            .completed_at_tick,
        Some(arrival.arrives_at_tick)
    );
}

#[test]
fn source_retention_of_the_weapon_keeps_its_valid_selected_style() {
    let (engine, world, original_style, weapon_slot) = setup(false);
    assert!(
        world.characters[&actor()]
            .equipment
            .contains_key(&weapon_slot)
    );
    assert_eq!(
        world.characters[&actor()].runtime.combat.style,
        Some(original_style)
    );
    let before = world.clone();
    assert!(engine.ui_view(&world, &actor()).is_ok());
    assert!(world == before);
}

#[test]
fn trusted_rejoin_repairs_only_derived_style_in_a_serialized_old_dying_state() {
    let (engine, mut world, stale_style, _) = setup(true);
    world
        .characters
        .get_mut(&actor())
        .unwrap()
        .runtime
        .combat
        .style = Some(stale_style);
    let bytes = serde_json::to_vec(&world).unwrap();
    let mut restored: WorldState = serde_json::from_slice(&bytes).unwrap();
    assert!(engine.ui_view(&restored, &actor()).is_err());
    let mut expected = restored.clone();
    expected
        .characters
        .get_mut(&actor())
        .unwrap()
        .runtime
        .combat
        .style = Some(unarmed(&engine));
    engine
        .apply_lifecycle(&mut restored, &actor(), LifecycleTransition::Rejoin)
        .unwrap();
    assert!(
        restored == expected,
        "Derived style reconciliation must not change items, XP, clocks, death records or other state"
    );
    assert!(engine.ui_view(&restored, &actor()).is_ok());
    engine
        .apply_lifecycle(&mut restored, &actor(), LifecycleTransition::Rejoin)
        .unwrap();
    assert!(
        restored == expected,
        "Repeated rejoin must not repeat a death or advance its phase"
    );
}

#[test]
fn coordinator_restart_preserves_death_receipts_and_phase_times_while_repairing_style() {
    let (engine, mut world, stale_style, _) = setup(true);
    let character = world.characters.get_mut(&actor()).unwrap();
    character.runtime.combat.style = Some(stale_style);
    let before = character.clone();
    let deaths = world.runtime.deaths.clone();
    let tick = world.tick;
    engine
        .reconcile_presence(&mut world, &Default::default())
        .unwrap();
    let character = &world.characters[&actor()];
    assert_eq!(character.runtime.combat.style, Some(unarmed(&engine)));
    assert_eq!(character.runtime.life, before.runtime.life);
    assert_eq!(character.runtime.active_death, before.runtime.active_death);
    assert_eq!(character.inventory, before.inventory);
    assert_eq!(character.equipment, before.equipment);
    assert_eq!(character.bank, before.bank);
    assert_eq!(character.skills, before.skills);
    assert_eq!(character.runtime.entitlements, before.runtime.entitlements);
    assert_eq!(
        character.runtime.combat.attack_ready,
        before.runtime.combat.attack_ready
    );
    assert_eq!(
        character.runtime.combat.spell_ready,
        before.runtime.combat.spell_ready
    );
    assert!(world.runtime.deaths == deaths);
    assert_eq!(world.tick, tick);
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Rejoin)
        .unwrap();
    assert!(engine.ui_view(&world, &actor()).is_ok());
    assert!(world.runtime.deaths == deaths);
    assert_eq!(world.tick, tick);
}

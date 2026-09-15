use std::collections::BTreeSet;

use clubscape_game_types::*;
use clubscape_world_engine::WorldEngine;
use serde_json::{Value, json};

use crate::{
    NoRandom,
    probes::{ProbeRandom, mainland_fixture},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn apply(
    engine: &WorldEngine,
    world: &mut WorldState,
    actor: &ActorId,
    request: GameplayUiRequest,
) -> Result<()> {
    engine.apply_intent(world, actor, &GameIntent::Ui { request }, &mut NoRandom)?;
    Ok(())
}

pub fn run(engine: &WorldEngine) -> Result<Value> {
    let (base, actor) = mainland_fixture(engine, "ui_source")?;
    let mut checked = 0;
    for stage in engine.content().tutorial.keys() {
        let mut world = base.clone();
        world.characters.get_mut(&actor).unwrap().tutorial_stage = stage.clone();
        let before = world.clone();
        let view = engine.ui_view(&world, &actor)?;
        if view.version != 1
            || view.interfaces.len() != engine.content().interfaces.len()
            || world != before
        {
            return Err(
                "A semantic source UI state is missing or mutated the queried world".into(),
            );
        }
        let body = &view.appearance.choices["body_type"];
        if body
            .iter()
            .map(|choice| choice.value)
            .collect::<BTreeSet<_>>()
            != BTreeSet::from([0, 1])
            || view
                .appearance
                .base
                .as_ref()
                .is_none_or(|base| base.source_npc != 2063)
        {
            return Err(
                "Source UI invented an appearance option or replaced the approved base".into(),
            );
        }
        checked += 1;
    }

    let mut world = base.clone();
    let character = world.characters.get_mut(&actor).unwrap();
    character.run_energy = 1000;
    character.inventory.slots[0] = Some(ItemStack {
        item: ItemId::new("item.energy_potion.four_dose")?,
        quantity: Quantity::new(1)?,
        instance: None,
    });
    character.inventory.slots[1] = character.inventory.slots[0].clone();
    let other_dose = character.inventory.slots[1].clone();
    let mut random = ProbeRandom::new();
    for dose in ["four_dose", "three_dose", "two_dose", "one_dose"] {
        let before = world.characters[&actor].run_energy;
        let food = world.characters[&actor].runtime.food_ready;
        let attack = world.characters[&actor].runtime.combat.attack_ready;
        apply(
            engine,
            &mut world,
            &actor,
            GameplayUiRequest::ItemAction {
                inventory_slot: 0,
                expected_item: ItemId::new(format!("item.energy_potion.{dose}"))?,
                expected_instance: None,
                action: "drink".into(),
            },
        )?;
        let character = &world.characters[&actor];
        if character.run_energy != before + 1500
            || character.inventory.slots[1] != other_dose
            || character.runtime.food_ready != food
            || character.runtime.combat.attack_ready != attack
        {
            return Err(
                "Native potion UI changed a different slot, wrong energy or food/attack delay"
                    .into(),
            );
        }
        for _ in 0..3 {
            engine.tick(&mut world, &mut random)?;
        }
    }
    if world.characters[&actor].inventory.slots[0]
        .as_ref()
        .unwrap()
        .item
        .as_str()
        != "item.vial"
    {
        return Err("Final original dose did not leave its original vial".into());
    }
    world.characters.get_mut(&actor).unwrap().inventory.slots[2] = Some(ItemStack {
        item: ItemId::new("item.bones")?,
        quantity: Quantity::new(1)?,
        instance: None,
    });
    let prayer = SkillId::new("skill.prayer")?;
    let xp = world.characters[&actor].skills[&prayer].xp_tenths;
    apply(
        engine,
        &mut world,
        &actor,
        GameplayUiRequest::ItemAction {
            inventory_slot: 2,
            expected_item: ItemId::new("item.bones")?,
            expected_instance: None,
            action: "bury".into(),
        },
    )?;
    engine.tick(&mut world, &mut random)?;
    if world.characters[&actor].inventory.slots[2].is_none() {
        return Err("UI burial consumed before its source phase".into());
    }
    engine.tick(&mut world, &mut random)?;
    if world.characters[&actor].inventory.slots[2].is_some()
        || world.characters[&actor].skills[&prayer].xp_tenths != xp + 45
    {
        return Err("UI burial did not execute exactly one source45XP-tenths outcome".into());
    }

    let mut modes = Vec::new();
    for (mode, delay) in [(ProductionMode::Single, 1), (ProductionMode::MakeX, 3)] {
        let mut world = base.clone();
        let recipe = RecipeId::new("recipe.cooking.bread.lumbridge_range")?;
        let character = world.characters.get_mut(&actor).unwrap();
        character.tile = Tile::new(3211, 3215, 0)?;
        character.inventory.slots[0] = Some(engine.content().recipes[&recipe].inputs[0].clone());
        character
            .interfaces
            .push(InterfaceId::new("interface.cooking")?);
        character
            .quests
            .get_mut(&QuestId::new("quest.cooks_assistant")?)
            .unwrap()
            .stage = StageId::new("stage.cooks.completed")?;
        let target = SpawnId::new("spawn.range.lumbridge.3212.3215.p0.t10.r2")?;
        let opened = engine.apply_intent(
            &mut world,
            &actor,
            &GameIntent::Interact {
                target: target.clone(),
                action: "Cook".into(),
            },
            &mut NoRandom,
        )?;
        if !opened.iter().any(|event| matches!(&event.event, GameEvent::InterfaceOpened { interface } if interface.as_str() == "interface.cooking")) {
            return Err("Source production context did not emit its actual interface-open event".into());
        }
        let menu = engine
            .ui_view(&world, &actor)?
            .production
            .ok_or("Missing actual cooking menu")?;
        if menu.target != Some(WorldTarget::Spawn { spawn: target }) {
            return Err("Cooking menu invented a target".into());
        }
        engine.tick(&mut world, &mut random)?;
        let now = world.tick;
        apply(
            engine,
            &mut world,
            &actor,
            GameplayUiRequest::ProductionSelect {
                menu_id: menu.id,
                recipe,
                quantity: 1,
                mode,
            },
        )?;
        if !matches!(world.characters[&actor].activity, Activity::ProducingSelected { next_tick, mode: stored, .. } if next_tick == now + delay && stored == mode)
        {
            return Err("Actual source menu collapsed Single and Make-X-of-one".into());
        }
        modes.push(json!({"mode": mode, "delay_ticks": delay}));
    }
    let mut world = base.clone();
    let recipe = RecipeId::new("recipe.cooking.dough")?;
    let inputs = &engine.content().recipes[&recipe].inputs;
    let character = world.characters.get_mut(&actor).unwrap();
    character
        .interfaces
        .push(InterfaceId::new("interface.cooking")?);
    for (slot, input) in [
        (0, &inputs[0]),
        (1, &inputs[1]),
        (8, &inputs[0]),
        (9, &inputs[1]),
    ] {
        character.inventory.slots[slot] = Some(input.clone());
    }
    engine.apply_intent(
        &mut world,
        &actor,
        &GameIntent::UseItem {
            inventory_slot: 8,
            target: ItemTarget::Inventory { slot: 9 },
        },
        &mut NoRandom,
    )?;
    let menu = engine
        .ui_view(&world, &actor)?
        .production
        .ok_or("Actual source dough item-use did not open its menu")?;
    if menu.target.is_some()
        || !menu
            .recipes
            .iter()
            .any(|choice| choice.recipe == recipe && choice.single.allowed)
    {
        return Err(
            "Inventory-only source dough menu invented a facility or denied its actual inputs"
                .into(),
        );
    }
    world = serde_json::from_slice(&serde_json::to_vec(&world)?)?;
    engine.tick(&mut world, &mut random)?;
    apply(
        engine,
        &mut world,
        &actor,
        GameplayUiRequest::ProductionSelect {
            menu_id: menu.id,
            recipe: recipe.clone(),
            quantity: 1,
            mode: ProductionMode::Single,
        },
    )?;
    engine.tick(&mut world, &mut random)?;
    let inventory = &world.characters[&actor].inventory;
    if inventory.slots[0] != Some(inputs[0].clone())
        || inventory.slots[1] != Some(inputs[1].clone())
    {
        return Err(
            "Dough UI consumed a different matching input instead of its selected slots".into(),
        );
    }
    for output in &engine.content().recipes[&recipe].outputs {
        let count: u32 = inventory
            .slots
            .iter()
            .flatten()
            .filter(|stack| stack.item == output.item)
            .map(|stack| stack.quantity.get())
            .sum();
        if count != output.quantity.get() {
            return Err(
                "Source dough UI lost or duplicated its declared dough/container output".into(),
            );
        }
    }
    Ok(json!({
        "passed": true, "semantic_immutable_ui_states": checked,
        "appearance": {"body_type": [0, 1], "approved_source_npc": 2063},
        "selected_original_potion_doses": 4, "run_restoration_units_per_dose": 1500, "independent_drink_ticks": 3,
        "selected_bury_ticks": 2, "selected_bury_xp_tenths": 45, "real_source_production_menu_modes": modes,
        "inventory_only_dough": {"passed": true, "target": null, "selected_slots": [8, 9], "earlier_matching_copies_preserved": true,
            "original_dough_and_empty_containers_preserved": true},
        "scope": "Actual unmodified strict content4 and real engine control execution on explicit source-precondition component states. Not a fresh-account journey, source capture or presentation acceptance."
    }))
}

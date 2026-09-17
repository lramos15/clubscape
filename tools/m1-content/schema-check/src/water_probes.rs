use std::{collections::BTreeMap, sync::Arc};

use clubscape_game_types::*;
use clubscape_world_engine::WorldEngine;
use serde_json::{Value, json};

use crate::{
    NoRandom,
    probes::{ProbeRandom, mainland_fixture},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SINK: &str = "spawn.water_source.3205.3215.p0.t10.r0";
const RECIPE: &str = "recipe.water.bucket";

fn count(character: &CharacterState, item: &str) -> u32 {
    character
        .inventory
        .slots
        .iter()
        .flatten()
        .filter(|stack| stack.item.as_str() == item)
        .map(|stack| stack.quantity.get())
        .sum()
}

fn use_bucket() -> Result<GameIntent> {
    Ok(GameIntent::UseItem {
        inventory_slot: 0,
        target: ItemTarget::World {
            spawn: SpawnId::new(SINK)?,
        },
    })
}

fn at_sink(engine: &WorldEngine, tile: Tile, full: bool) -> Result<(WorldState, ActorId)> {
    let (mut world, actor) = mainland_fixture(engine, "water")?;
    let character = world.characters.get_mut(&actor).unwrap();
    character.tile = tile;
    if !engine
        .content()
        .regions
        .values()
        .flat_map(|region| &region.cells)
        .any(|cell| cell.tile == tile && cell.walkable)
    {
        return Err("Water native fixture attempted to stand on blocked source terrain".into());
    }
    character.inventory.slots[0] = Some(ItemStack {
        item: ItemId::new("item.bucket")?,
        quantity: Quantity::new(1)?,
        instance: None,
    });
    if full {
        for slot in &mut character.inventory.slots[1..] {
            *slot = Some(ItemStack {
                item: ItemId::new("item.logs.normal")?,
                quantity: Quantity::new(1)?,
                instance: None,
            });
        }
    }
    world.validate_runtime(engine.content())?;
    Ok((world, actor))
}

fn positive(engine: &WorldEngine, tile: Tile, full: bool, primitive: bool) -> Result<Value> {
    let (mut world, actor) = at_sink(engine, tile, full)?;
    let before = world.clone();
    let target = WorldTarget::Spawn {
        spawn: SpawnId::new(SINK)?,
    };
    let menu = engine.interaction_options(&world, &actor, &target)?;
    if !menu.is_empty() || world != before {
        return Err(
            "Water binding manufactured an object-menu action or mutated a source query".into(),
        );
    }
    let intent = if primitive {
        GameIntent::Produce {
            recipe: RecipeId::new(RECIPE)?,
            target: Some(SpawnId::new(SINK)?),
            quantity: Quantity::new(1)?,
        }
    } else {
        use_bucket()?
    };
    if let Err(error) = engine.apply_intent(&mut world, &actor, &intent, &mut NoRandom) {
        if world != before {
            return Err("Rejected water input changed source-owned state".into());
        }
        return Ok(json!({
            "tile": tile, "full_inventory": full, "passed": false, "phase": "item_on_facility_admission",
            "error": error, "no_object_menu_manufactured": true, "atomic_refusal": true,
            "expected": {"empty_bucket_consumed": 1, "water_bucket_produced": 1, "xp_tenths": 0, "ticks": 1},
            "cadence_and_conversion_executed": false, "duplicate_outcome_check_executed": false,
        }));
    }
    let character = &world.characters[&actor];
    let next_tick = match character.activity {
        Activity::Producing { next_tick, .. }
        | Activity::ProducingAt { next_tick, .. }
        | Activity::ProducingSelected { next_tick, .. } => next_tick,
        _ => {
            return Err("Accepted water input did not queue the declared source conversion".into());
        }
    };
    if next_tick != before.tick + 1 || character.inventory != before.characters[&actor].inventory {
        return Err("Water conversion did not retain the declared one-tick admission phase".into());
    }
    if character
        .runtime
        .ui
        .as_ref()
        .is_some_and(|ui| ui.production.is_some())
    {
        return Err("Item-on-only water created a production menu".into());
    }
    let restored: WorldState = serde_json::from_slice(&serde_json::to_vec(&world)?)?;
    if restored != world {
        return Err("Pending water conversion changed in a state round trip".into());
    }
    world = restored;
    let mut random = ProbeRandom::new();
    let events = engine.tick(&mut world, &mut random)?;
    if events.iter().filter(|event| matches!(&event.event, GameEvent::ProductionResolved { recipe, .. } if recipe.as_str() == RECIPE)).count() != 1 {
        return Err("Water did not emit exactly one committed conversion event".into());
    }
    let character = &world.characters[&actor];
    let prior = &before.characters[&actor];
    if count(character, "item.bucket") != 0
        || count(character, "item.water.bucket") != 1
        || count(character, "item.logs.normal") != count(prior, "item.logs.normal")
        || character.skills != prior.skills
        || character.quests != prior.quests
        || character.quest_points != prior.quest_points
        || character.bank != prior.bank
        || character.equipment != prior.equipment
        || character.inventory.slots.iter().flatten().count()
            != prior.inventory.slots.iter().flatten().count()
    {
        return Err(
            "Water failed exact replacement, zero-XP or unrelated-state conservation".into(),
        );
    }
    let after = character.inventory.clone();
    engine.tick(&mut world, &mut random)?;
    let before_repeated = world.clone();
    let repeated = engine.apply_intent(&mut world, &actor, &intent, &mut NoRandom);
    if repeated.is_ok() || world != before_repeated || world.characters[&actor].inventory != after {
        return Err("Consumed empty-bucket input was accepted again or duplicated output".into());
    }
    Ok(json!({
        "tile": tile, "full_inventory": full, "passed": true, "no_object_menu_manufactured": true,
        "input_kind": if primitive { "produce_single" } else { "use_item" },
        "cadence_and_conversion_executed": true, "ticks": 1, "empty_bucket_consumed": 1,
        "water_bucket_produced": 1, "all_skill_xp_unchanged": true, "unrelated_owned_state_unchanged": true,
        "duplicate_outcome_check_executed": true, "consumed_input_rejected": true,
        "pending_state_roundtrip": true, "exactly_one_resolution_event": true,
        "duplicate_scope": "A repeated consumed-input command is refused; no durable server operation or receipt was executed.",
        "server_receipt_replay_claimed": false,
    }))
}

fn rejected(
    engine: &WorldEngine,
    mut world: WorldState,
    actor: &ActorId,
    name: &str,
    intent: GameIntent,
) -> Result<Value> {
    world.validate_runtime(engine.content())?;
    let before = world.clone();
    let result = engine.apply_intent(&mut world, actor, &intent, &mut NoRandom);
    let (expected_code, expected_message) = match name {
        "wrong_target" => (GameErrorCode::RequirementNotMet, "Wrong source facility."),
        "wrong_plane" => (GameErrorCode::OutOfReach, "another plane"),
        "bucket_only_in_bank" | "bucket_owned_by_other_actor" => {
            (GameErrorCode::NotOwned, "Inventory slot 0 is empty.")
        }
        "wrong_item" => (
            GameErrorCode::Unavailable,
            "No source item-use rule matches.",
        ),
        "valid_office_instance_cannot_use_overworld_sink" => {
            (GameErrorCode::UnknownContent, "in this instance")
        }
        "blocked_west_contact" | "far_from_source" => (GameErrorCode::OutOfReach, "blocked"),
        "absent_ingredients" => (GameErrorCode::InsufficientItems, "item.bucket"),
        "missing_facility" => (GameErrorCode::OutOfReach, "source facility"),
        "legacy_multi_conversion" | "make_x_one" => {
            (GameErrorCode::Unavailable, "one single conversion")
        }
        "single_mode_with_two_conversions" => {
            (GameErrorCode::InvalidInput, "exactly one operation")
        }
        "make_all_without_a_menu" => (GameErrorCode::StaleCommand, "menu is no longer open"),
        "non_mainland" => (GameErrorCode::RequirementNotMet, "source interaction guard"),
        _ => return Err("Native water negative case has no explicit expected guard".into()),
    };
    let error = result.err();
    let guard_exercised = error.as_ref().is_some_and(|error| {
        error.code == expected_code && error.message.contains(expected_message)
    });
    Ok(json!({
        "case": name, "passed": guard_exercised && world == before, "atomic_refusal": world == before,
        "expected_code": expected_code, "expected_message_fragment": expected_message,
        "intended_guard_exercised": guard_exercised, "error": error,
    }))
}

fn pending_refusal(engine: &WorldEngine, unavailable: bool) -> Result<Value> {
    let (mut world, actor) = at_sink(engine, Tile::new(3205, 3214, 0)?, false)?;
    engine.apply_intent(&mut world, &actor, &use_bucket()?, &mut NoRandom)?;
    if unavailable {
        world
            .entities
            .get_mut(&SpawnId::new(SINK)?)
            .ok_or("Missing source sink")?
            .available_at_tick = world.tick + 100;
    } else {
        world.characters.get_mut(&actor).unwrap().tile = Tile::new(3204, 3216, 0)?;
    }
    let before = world.characters[&actor].clone();
    let events = engine.tick(&mut world, &mut ProbeRandom::new())?;
    let after = &world.characters[&actor];
    let passed = after.inventory == before.inventory
        && after.skills == before.skills
        && after.bank == before.bank
        && matches!(after.activity, Activity::Idle)
        && !events.iter().any(|event| matches!(&event.event, GameEvent::ProductionResolved { recipe, .. } if recipe.as_str() == RECIPE));
    Ok(json!({
        "case": if unavailable { "target_unavailable_before_resolution" } else { "blocked_contact_before_resolution" },
        "passed": passed, "queued_before_target_change": true,
        "no_input_consumed_or_output_xp_granted": passed,
        "scope": "Controlled source state change between admission and the pending tick, not a player action or migration.",
    }))
}

fn rule_variants(engine: &WorldEngine) -> Result<Vec<Value>> {
    let mut results = Vec::new();
    let mut without = engine.content().clone();
    without
        .recipes
        .get_mut(&RecipeId::new(RECIPE)?)
        .unwrap()
        .item_on_target = None;
    let without = WorldEngine::new(Arc::new(without))?;
    let (mut world, actor) = at_sink(&without, Tile::new(3205, 3214, 0)?, false)?;
    let before = world.clone();
    let error = without
        .apply_intent(&mut world, &actor, &use_bucket()?, &mut NoRandom)
        .err();
    results.push(json!({
        "case": "absent_rule_retains_menu_admission",
        "passed": world == before && error.as_ref().is_some_and(|error| error.code == GameErrorCode::RequirementNotMet && error.message.contains("Facility does not offer")),
        "error": error, "controlled_definition_variant": true,
    }));
    let mut unresolved = engine.content().clone();
    let recipe = unresolved.recipes.get_mut(&RecipeId::new(RECIPE)?).unwrap();
    recipe.item_on_target = Some(SourceBinding::Unresolved {
        reason: "Controlled absent source item-on target authority".into(),
        source: recipe.source.clone(),
    });
    let compiler_rejected = clubscape_content::compile_content(
        unresolved.clone(),
        clubscape_content::ValidationMode::Runtime,
    )
    .is_err();
    let error = WorldEngine::new(Arc::new(unresolved)).err();
    results.push(json!({
        "case": "unresolved_rule_is_not_executable",
        "passed": compiler_rejected && error.as_ref().is_some_and(|error| error.code == GameErrorCode::Unavailable),
        "compiler_rejected": compiler_rejected, "error": error, "controlled_definition_variant": true,
    }));
    Ok(results)
}

fn office_world(engine: &WorldEngine) -> Result<(WorldState, ActorId)> {
    let (mut world, actor) = mainland_fixture(engine, "water_instance")?;
    let entrance = SpawnId::new("spawn.death.entrance.3238.3192.p0.t10.r2")?;
    let target = WorldTarget::Spawn {
        spawn: entrance.clone(),
    };
    let mut found = false;
    let candidates: Vec<_> = engine
        .content()
        .regions
        .values()
        .flat_map(|region| &region.cells)
        .filter(|cell| {
            cell.walkable
                && cell
                    .tile
                    .distance(engine.content().spawns[&entrance].tile)
                    .is_some_and(|distance| distance <= 3)
        })
        .map(|cell| cell.tile)
        .collect();
    for tile in candidates {
        let character = world.characters.get_mut(&actor).unwrap();
        character.tile = tile;
        character.region = engine.content().spawns[&entrance].region.clone();
        if engine
            .interaction_options(&world, &actor, &target)?
            .iter()
            .any(|option| option.name == "Enter" && option.permission.allowed)
        {
            found = true;
            break;
        }
    }
    if !found {
        return Err("Existing source Office entry has no valid native fixture contact".into());
    }
    engine.apply_intent(
        &mut world,
        &actor,
        &GameIntent::Interact {
            target: entrance,
            action: "Enter".into(),
        },
        &mut NoRandom,
    )?;
    engine.tick(&mut world, &mut ProbeRandom::new())?;
    let character = world.characters.get_mut(&actor).unwrap();
    if character.runtime.instance.is_none() {
        return Err("Existing source Office travel did not create a valid owned instance".into());
    }
    character.inventory.slots[0] = Some(ItemStack {
        item: ItemId::new("item.bucket")?,
        quantity: Quantity::new(1)?,
        instance: None,
    });
    Ok((world, actor))
}

pub fn run(engine: &WorldEngine) -> Result<Value> {
    let recipe = &engine.content().recipes[&RecipeId::new(RECIPE)?];
    if recipe.inputs.len() != 1
        || recipe.inputs[0].item.as_str() != "item.bucket"
        || recipe.outputs.len() != 1
        || recipe.outputs[0].item.as_str() != "item.water.bucket"
        || !recipe.xp.is_empty()
    {
        return Err("Native water probe did not receive the declared source recipe".into());
    }
    let south = Tile::new(3205, 3214, 0)?;
    let mut positives = Vec::new();
    for (tile, full) in [
        (south, false),
        (Tile::new(3205, 3217, 0)?, false),
        (Tile::new(3206, 3216, 0)?, false),
        (south, true),
    ] {
        positives.push(positive(engine, tile, full, false)?);
    }
    let mut negatives = Vec::new();
    let (world, actor) = at_sink(engine, south, false)?;
    negatives.push(rejected(
        engine,
        world.clone(),
        &actor,
        "wrong_target",
        GameIntent::UseItem {
            inventory_slot: 0,
            target: ItemTarget::World {
                spawn: SpawnId::new("spawn.range.lumbridge.3212.3215.p0.t10.r2")?,
            },
        },
    )?);
    let (plane, _) = at_sink(engine, Tile::new(3205, 3217, 1)?, false)?;
    negatives.push(rejected(
        engine,
        plane,
        &actor,
        "wrong_plane",
        use_bucket()?,
    )?);
    for (name, tile) in [
        ("blocked_west_contact", Tile::new(3204, 3216, 0)?),
        ("far_from_source", Tile::new(3209, 3213, 0)?),
    ] {
        let (world, actor) = at_sink(engine, tile, false)?;
        negatives.push(rejected(engine, world, &actor, name, use_bucket()?)?);
    }
    let mut empty = world.clone();
    empty.characters.get_mut(&actor).unwrap().inventory.slots[0] = None;
    negatives.push(rejected(
        engine,
        empty,
        &actor,
        "absent_ingredients",
        GameIntent::Produce {
            recipe: RecipeId::new(RECIPE)?,
            target: Some(SpawnId::new(SINK)?),
            quantity: Quantity::new(1)?,
        },
    )?);
    for (name, intent) in [
        (
            "missing_facility",
            GameIntent::Produce {
                recipe: RecipeId::new(RECIPE)?,
                target: None,
                quantity: Quantity::new(1)?,
            },
        ),
        (
            "legacy_multi_conversion",
            GameIntent::Produce {
                recipe: RecipeId::new(RECIPE)?,
                target: Some(SpawnId::new(SINK)?),
                quantity: Quantity::new(2)?,
            },
        ),
        (
            "make_x_one",
            GameIntent::ProduceSelected {
                recipe: RecipeId::new(RECIPE)?,
                target: Some(WorldTarget::Spawn {
                    spawn: SpawnId::new(SINK)?,
                }),
                quantity: Quantity::new(1)?,
                mode: ProductionMode::MakeX,
            },
        ),
        (
            "single_mode_with_two_conversions",
            GameIntent::ProduceSelected {
                recipe: RecipeId::new(RECIPE)?,
                target: Some(WorldTarget::Spawn {
                    spawn: SpawnId::new(SINK)?,
                }),
                quantity: Quantity::new(2)?,
                mode: ProductionMode::Single,
            },
        ),
        (
            "make_all_without_a_menu",
            GameIntent::Ui {
                request: GameplayUiRequest::ProductionSelectAll {
                    menu_id: "ui.unavailable.water".into(),
                    recipe: RecipeId::new(RECIPE)?,
                },
            },
        ),
    ] {
        negatives.push(rejected(engine, world.clone(), &actor, name, intent)?);
    }
    let mut tutorial = world.clone();
    tutorial.characters.get_mut(&actor).unwrap().tutorial_stage =
        StageId::new("stage.tutorial.make_dough")?;
    negatives.push(rejected(
        engine,
        tutorial,
        &actor,
        "non_mainland",
        use_bucket()?,
    )?);
    let mut banked = world.clone();
    let character = banked.characters.get_mut(&actor).unwrap();
    character
        .bank
        .slots
        .push(character.inventory.slots[0].take());
    character.runtime.ui.as_mut().unwrap().bank = GameplayUiRuntime::from_legacy(
        &character.bank,
        &engine
            .content()
            .ui
            .as_ref()
            .ok_or("Missing source bank UI")?
            .bank,
    )?
    .bank;
    negatives.push(rejected(
        engine,
        banked,
        &actor,
        "bucket_only_in_bank",
        use_bucket()?,
    )?);
    let mut foreign = world.clone();
    let bucket = foreign.characters.get_mut(&actor).unwrap().inventory.slots[0].take();
    let other = ActorId::new("actor.content.water_other")?;
    let mut character = engine.character_from_initial(
        other.clone(),
        "Separate controlled owner",
        BTreeMap::from([("body_type".to_owned(), 0)]),
    )?;
    character.inventory.slots[0] = bucket;
    foreign.characters.insert(other, character);
    negatives.push(rejected(
        engine,
        foreign,
        &actor,
        "bucket_owned_by_other_actor",
        use_bucket()?,
    )?);
    let mut wrong_item = world.clone();
    wrong_item
        .characters
        .get_mut(&actor)
        .unwrap()
        .inventory
        .slots[0]
        .as_mut()
        .unwrap()
        .item = ItemId::new("item.coins")?;
    negatives.push(rejected(
        engine,
        wrong_item,
        &actor,
        "wrong_item",
        use_bucket()?,
    )?);
    let (office, office_actor) = office_world(engine)?;
    negatives.push(rejected(
        engine,
        office,
        &office_actor,
        "valid_office_instance_cannot_use_overworld_sink",
        use_bucket()?,
    )?);
    let positive_count = positives.iter().filter(|row| row["passed"] == true).count();
    let negative_count = negatives.iter().filter(|row| row["passed"] == true).count();
    let primitive = positive(engine, south, false, true)?;
    let pending = [
        pending_refusal(engine, false)?,
        pending_refusal(engine, true)?,
    ];
    let variants = rule_variants(engine)?;
    let passed = positive_count == positives.len()
        && negative_count == negatives.len()
        && primitive["passed"] == true
        && pending
            .iter()
            .chain(&variants)
            .all(|row| row["passed"] == true);
    Ok(json!({
        "passed": passed,
        "source_recipe": RECIPE, "source_sink": SINK,
        "positive_cases": positives, "negative_cases": negatives,
        "positive_passed": positive_count, "negative_passed": negative_count,
        "legacy_single_primitive": primitive, "pending_guard_cases": pending,
        "controlled_rule_variants": variants,
        "source_object_menu_operations": [],
        "scope": "Controlled native source states against the exact Runtime artifact, plus separately labeled absent/unresolved definition variants. No private account/checkpoint, network pump, migration or gameplay acceptance.",
        "source_motion": "Unverified numeric dispatch, not deliberate silence or a guessed sequence.",
        "remaining_seam": if passed { None } else { Some("Inspect the exact failing admission/guard/cadence result; no source menu, reach or batching fallback is permitted.") },
        "migration_admitted": false,
    }))
}

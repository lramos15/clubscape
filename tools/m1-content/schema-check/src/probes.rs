use std::collections::BTreeMap;

use clubscape_game_types::*;
use clubscape_world_engine::{LifecycleTransition, RandomSource, WorldEngine};
use serde_json::{Value, json};

use crate::NoRandom;

type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;

struct ProbeRandom {
    state: u64,
    draws: u32,
}

impl ProbeRandom {
    fn new() -> Self {
        Self {
            state: 0x6d315f636f6e7465,
            draws: 0,
        }
    }
}

impl RandomSource for ProbeRandom {
    fn draw_below(&mut self, upper: u32) -> GameResult<u32> {
        if upper == 0 {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Native policy probe requested an empty random domain.",
            ));
        }
        let threshold = upper.wrapping_neg() % upper;
        loop {
            self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
            let mut mixed = self.state;
            mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d049bb133111eb);
            let draw = ((mixed ^ (mixed >> 31)) >> 32) as u32;
            if draw >= threshold {
                self.draws += 1;
                return Ok(draw % upper);
            }
        }
    }
}

// Isolated component states are never written into canonical creation content.
fn mainland_fixture(engine: &WorldEngine, name: &str) -> ProbeResult<(WorldState, ActorId)> {
    let mut world = engine.initial_world()?;
    let actor = ActorId::new(format!("actor.content.{name}"))?;
    let mut character = engine.character_from_initial(
        actor.clone(),
        "Isolated source-policy probe",
        BTreeMap::from([("body_type".to_owned(), 0)]),
    )?;
    let location = engine
        .content()
        .mechanics
        .death
        .as_ref()
        .ok_or("Missing source death policy")?
        .respawn
        .require()?;
    character.region = location.region.clone();
    character.tile = location.tile;
    character.tutorial_stage = StageId::new("stage.tutorial.mainland")?;
    world.characters.insert(actor.clone(), character);
    engine.apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)?;
    Ok((world, actor))
}

fn consume_only(engine: &WorldEngine) -> ProbeResult<Value> {
    let (mut world, actor) = mainland_fixture(engine, "burial")?;
    let recipe = RecipeId::new("recipe.prayer.bones.ordinary")?;
    let definition = &engine.content().recipes[&recipe];
    let skill = SkillId::new("skill.prayer")?;
    world.characters.get_mut(&actor).unwrap().inventory.slots[0] =
        Some(definition.inputs[0].clone());
    let before_xp = world.characters[&actor].skills[&skill].xp_tenths;
    let ground_before = world.ground_items.len();
    let mut random = ProbeRandom::new();
    engine.apply_intent(
        &mut world,
        &actor,
        &GameIntent::ProduceSelected {
            recipe,
            target: None,
            quantity: Quantity::new(1)?,
            mode: ProductionMode::Single,
        },
        &mut NoRandom,
    )?;
    engine.tick(&mut world, &mut random)?;
    if world.characters[&actor].inventory.slots[0].is_none() {
        return Err("Source burial consumed before its two-tick deadline".into());
    }
    engine.tick(&mut world, &mut random)?;
    let character = &world.characters[&actor];
    if character.inventory.slots.iter().any(Option::is_some)
        || character.skills[&skill].xp_tenths != before_xp + 45
        || world.ground_items.len() != ground_before
    {
        return Err("Native ConsumeOnly did not consume once/award45 without fake output".into());
    }
    Ok(
        json!({"passed": true, "ticks": 2, "xp_tenths": 45, "direct_output_items": 0,
              "controlled_component_random_draws": random.draws}),
    )
}

fn production_modes(engine: &WorldEngine) -> ProbeResult<Value> {
    let mut results = Vec::new();
    for (mode, expected_tick) in [(ProductionMode::Single, 1), (ProductionMode::MakeX, 3)] {
        let (mut world, actor) = mainland_fixture(engine, "production")?;
        let recipe = RecipeId::new("recipe.cooking.bread.lumbridge_range")?;
        let definition = &engine.content().recipes[&recipe];
        let character = world.characters.get_mut(&actor).unwrap();
        character.tile = Tile::new(3211, 3215, 0)?;
        character.inventory.slots[0] = Some(definition.inputs[0].clone());
        character
            .quests
            .get_mut(&QuestId::new("quest.cooks_assistant")?)
            .ok_or("Missing source Cook quest")?
            .stage = StageId::new("stage.cooks.completed")?;
        let before = world.clone();
        let response = engine.apply_intent(
            &mut world,
            &actor,
            &GameIntent::ProduceSelected {
                recipe,
                target: Some(WorldTarget::Spawn {
                    spawn: SpawnId::new("spawn.range.lumbridge.3212.3215.p0.t10.r2")?,
                }),
                quantity: Quantity::new(1)?,
                mode,
            },
            &mut NoRandom,
        );
        if let Err(error) = response {
            if error.code != GameErrorCode::OutOfReach {
                return Err(error.into());
            }
            if world != before {
                return Err("Rejected source facility input mutated the world".into());
            }
            results.push(json!({
                "passed": false,
                "mode": mode,
                "quantity": 1,
                "actor_tile": before.characters[&actor].tile,
                "target": "spawn.range.lumbridge.3212.3215.p0.t10.r2",
                "expected_next_tick": expected_tick,
                "error": error,
                "atomic_refusal": true,
                "required_contract": "Contact a solid source facility from its allowed side without requiring movement/center-to-center sight through its own clipped footprint; keep real intervening walls and source access masks.",
            }));
            continue;
        }
        match world.characters[&actor].activity {
            Activity::ProducingSelected {
                remaining: 1,
                mode: actual,
                next_tick,
                ..
            } if actual == mode && next_tick == expected_tick => {
                results.push(
                    json!({"passed": true, "mode": mode, "quantity": 1, "next_tick": next_tick}),
                );
            }
            _ => return Err("Native production collapsed Single and Make-X-of-one".into()),
        }
    }
    Ok(json!({"passed": results.iter().all(|row| row["passed"] == true), "cases": results}))
}

fn fresh_drop_clock(engine: &WorldEngine) -> ProbeResult<Value> {
    let (mut world, actor) = mainland_fixture(engine, "fresh_drop")?;
    world.characters.get_mut(&actor).unwrap().inventory.slots[0] = Some(ItemStack {
        item: ItemId::new("item.coins")?,
        quantity: Quantity::new(25)?,
        instance: None,
    });
    engine.apply_intent(
        &mut world,
        &actor,
        &GameIntent::Drop {
            inventory_slot: 0,
            quantity: Quantity::new(25)?,
        },
        &mut NoRandom,
    )?;
    let ground = world
        .ground_items
        .iter()
        .find(|item| item.owner.as_ref() == Some(&actor))
        .ok_or("Native drop lost the original owner")?
        .clone();
    let provenance = world
        .runtime
        .ground_provenance
        .get(&ground.id)
        .ok_or("Native drop omitted origin/policy provenance")?
        .clone();
    if ground.public_at_tick != u64::MAX
        || ground.expires_at_tick != 300
        || provenance.policy.as_str() != "ground_policy.player_drop.m1_fresh"
        || provenance.clock != Some(GroundClock::OwnerOnlineTicks)
        || !matches!(provenance.producer, GroundProducer::PlayerDrop { .. })
    {
        return Err("Native drop did not consume the explicit fresh-profile policy".into());
    }
    engine.apply_lifecycle(&mut world, &actor, LifecycleTransition::TransportLost)?;
    let mut random = ProbeRandom::new();
    engine.tick(&mut world, &mut random)?;
    let after = world
        .ground_items
        .iter()
        .find(|item| item.id == ground.id)
        .ok_or("Fresh manual drop expired before its source deadline")?;
    let expected_deadline = ground.expires_at_tick + 1;
    Ok(json!({
        "passed": after.expires_at_tick == expected_deadline,
        "origin": "player_drop",
        "item": "item.coins",
        "quantity_conserved": after.stack.quantity.get() == 25,
        "actual_policy": provenance.policy,
        "frozen_clock": provenance.clock,
        "public_after": null,
        "before_logout_expiry": ground.expires_at_tick,
        "offline_ticks_advanced": 1,
        "controlled_component_random_draws": random.draws,
        "expected_expiry_after_offline_tick": expected_deadline,
        "actual_expiry_after_offline_tick": after.expires_at_tick,
        "source": "research/m1-bindings/runtime3-policy.json#fresh_normal_manual_drop_profile",
        "required_contract": "Fresh manual drops use an explicit frozen owner-online clock without changing their PlayerDrop origin.",
    }))
}

fn played_time_selection(engine: &WorldEngine) -> ProbeResult<Value> {
    let mut results = Vec::new();
    for (ticks, policy, public_at) in [
        (119999, "ground_policy.player_drop.m1_fresh", u64::MAX),
        (120000, "ground_policy.player_drop.ordinary", 100),
    ] {
        let (mut world, actor) = mainland_fixture(engine, "played_time")?;
        let character = world.characters.get_mut(&actor).unwrap();
        character
            .runtime
            .played_time
            .as_mut()
            .ok_or("New source character has no played-time clock")?
            .ticks = ticks;
        character.inventory.slots[0] = Some(ItemStack {
            item: ItemId::new("item.coins")?,
            quantity: Quantity::new(25)?,
            instance: None,
        });
        engine.apply_intent(
            &mut world,
            &actor,
            &GameIntent::Drop {
                inventory_slot: 0,
                quantity: Quantity::new(25)?,
            },
            &mut NoRandom,
        )?;
        let ground = world
            .ground_items
            .iter()
            .find(|item| item.owner.as_ref() == Some(&actor))
            .ok_or("Source manual drop is missing")?;
        let provenance = &world.runtime.ground_provenance[&ground.id];
        let passed = provenance.policy.as_str() == policy
            && ground.public_at_tick == public_at
            && ground.expires_at_tick == 300
            && ground.stack.quantity.get() == 25;
        results.push(
            json!({"passed": passed, "played_ticks": ticks, "policy": provenance.policy,
            "clock": provenance.clock, "public_at_tick": ground.public_at_tick}),
        );
    }
    Ok(
        json!({"passed": results.iter().all(|row| row["passed"] == true), "cases": results,
        "classification": "Native conformance to the source-supported20-hour conditional, not live-source observation."}),
    )
}

pub fn run(engine: &WorldEngine) -> ProbeResult<Value> {
    let burial = consume_only(engine)?;
    let production = production_modes(engine)?;
    let drop = fresh_drop_clock(engine)?;
    let played = played_time_selection(engine)?;
    Ok(json!({
        "scope": "Isolated native component probes against unmodified strictly compiled source content. Controlled runtime inventory/stage boundaries and reproducible test-only randomness are not a fresh journey, observed odds or presentation acceptance.",
        "consume_only": burial,
        "single_and_make_x_one": production,
        "fresh_manual_drop_offline_clock": drop,
        "played_time_drop_selection": played,
        "passed": burial["passed"] == true && production["passed"] == true && drop["passed"] == true && played["passed"] == true,
    }))
}

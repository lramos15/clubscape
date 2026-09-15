use std::{collections::BTreeMap, env, fs, sync::Arc};

use clubscape_content::{ValidationMode, load_compiled, sha256};
use clubscape_game_types::*;
use clubscape_world_engine::{LifecycleTransition, RandomSource, WorldEngine};
use serde_json::json;

struct NoDraw;
impl RandomSource for NoDraw {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        Err(GameError::new(
            GameErrorCode::InvalidInput,
            "Dialogue must not roll randomness.",
        ))
    }
}

struct LowestTickDraw;
impl RandomSource for LowestTickDraw {
    fn draw_below(&mut self, upper: u32) -> GameResult<u32> {
        if upper == 0 {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Empty source random domain.",
            ));
        }
        Ok(0)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: dialogue_entries RAW_RUNTIME_ARTIFACT")?;
    let bytes = fs::read(path)?;
    let compiled = load_compiled(&bytes, ValidationMode::Runtime)?;
    let source = compiled.definition();
    let engine = WorldEngine::new(Arc::new(source.clone()))?;
    let speaker = SpawnId::new("spawn.survival_expert")?;
    let actor = ActorId::new("actor.dialogue.source")?;
    let mut results = Vec::new();
    for missing in [false, true] {
        let mut world = engine.initial_world()?;
        let mut character = engine.character_from_initial(
            actor.clone(),
            "Isolated source dialogue",
            BTreeMap::from([("body_type".into(), 0)]),
        )?;
        let anchor = world.entities[&speaker].tile;
        character.tile = anchor;
        character.region = source.spawns[&speaker].region.clone();
        character.tutorial_stage = StageId::new(if missing {
            "stage.tutorial.catch_shrimp"
        } else {
            "stage.tutorial.survival_tools"
        })?;
        character.runtime.counters.insert(
            CounterId::new("counter.tutorial.net.completed")?,
            CounterValue::Boolean(true),
        );
        character.runtime.entitlements.insert(
            EntitlementId::new("entitlement.tutorial.net")?,
            EntitlementState::Grant {
                delivered: BTreeMap::from([(ItemId::new("item.fishing_net.small")?, 1)]),
                satisfied: [ItemId::new("item.fishing_net.small")?].into(),
                complete: true,
            },
        );
        if !missing {
            character.inventory.slots[0] = Some(ItemStack {
                item: ItemId::new("item.fishing_net.small")?,
                quantity: Quantity::new(1)?,
                instance: None,
            });
            character.inventory.slots[1] = Some(ItemStack {
                item: ItemId::new("item.shrimps.raw")?,
                quantity: Quantity::new(1)?,
                instance: None,
            });
        }
        world.characters.insert(actor.clone(), character);
        engine.apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)?;
        let before = world.clone();
        let options = engine.interaction_options(
            &world,
            &actor,
            &WorldTarget::Spawn {
                spawn: speaker.clone(),
            },
        )?;
        if world != before {
            return Err("A source dialogue query mutated state".into());
        }
        engine.apply_intent(
            &mut world,
            &actor,
            &GameIntent::Interact {
                target: speaker.clone(),
                action: "Talk-to".into(),
            },
            &mut NoDraw,
        )?;
        let dialogue = engine
            .dialogue_view(&world, &actor)?
            .ok_or("No dialogue opened")?;
        let expected = if missing {
            "replace.net"
        } else {
            "woodcutting_firemaking_intro"
        };
        let admitted = dialogue.choices.iter().any(|choice| choice.id == expected);
        let unchanged = world.characters[&actor].inventory == before.characters[&actor].inventory
            && world.characters[&actor].skills == before.characters[&actor].skills;
        engine.tick(&mut world, &mut LowestTickDraw)?;
        engine.apply_intent(
            &mut world,
            &actor,
            &GameIntent::SelectDialogue {
                speaker: speaker.clone(),
                choice: expected.into(),
            },
            &mut NoDraw,
        )?;
        let player = &world.characters[&actor];
        let count = |name: &str| {
            player
                .inventory
                .slots
                .iter()
                .flatten()
                .filter(|stack| stack.item.as_str() == name)
                .map(|stack| stack.quantity.get())
                .sum::<u32>()
        };
        let grants_correct = count("item.fishing_net.small") == 1
            && count("item.axe.bronze") == u32::from(!missing)
            && count("item.tinderbox") == u32::from(!missing);
        let stage_correct = player.tutorial_stage.as_str()
            == if missing {
                "stage.tutorial.catch_shrimp"
            } else {
                "stage.tutorial.cut_logs"
            };
        results.push(
            json!({"missing_net": missing, "expected_choice": expected, "admitted": admitted,
            "ownership_unchanged_before_selection": unchanged, "options": options,
            "actual_selected_grants_correct": grants_correct, "actual_stage_correct": stage_correct,
            "passed": admitted && unchanged && grants_correct && stage_correct}),
        );
    }
    let passed = results.iter().all(|row| row["passed"] == true);
    println!(
        "{}",
        json!({"artifact_sha256": sha256(&bytes), "cases": results, "passed": passed,
        "classification": "Controlled source-native component states, not legitimate progression or player acquisition.",
        "full_journey_executed": false, "milestone_accepted": false})
    );
    if !passed {
        return Err("Source lesson/recovery entry conformance failed".into());
    }
    Ok(())
}

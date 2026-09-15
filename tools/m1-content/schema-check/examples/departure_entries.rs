use std::{collections::BTreeMap, env, fs, sync::Arc};

use clubscape_content::{ValidationMode, load_compiled, sha256};
use clubscape_game_types::*;
use clubscape_world_engine::{LifecycleTransition, RandomSource, WorldEngine};
use serde_json::json;

struct InputRandom;
impl RandomSource for InputRandom {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        Err(GameError::new(
            GameErrorCode::InvalidInput,
            "Dialogue choices must not sample gameplay randomness.",
        ))
    }
}

struct TickRandom;
impl RandomSource for TickRandom {
    fn draw_below(&mut self, upper: u32) -> GameResult<u32> {
        if upper == 0 {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Empty random domain.",
            ));
        }
        Ok(0)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: departure_entries RAW_RUNTIME_ARTIFACT")?;
    let bytes = fs::read(path)?;
    let compiled = load_compiled(&bytes, ValidationMode::Runtime)?;
    let source = compiled.definition();
    let engine = WorldEngine::new(Arc::new(source.clone()))?;
    let actor = ActorId::new("actor.departure.source")?;
    let speaker = SpawnId::new("spawn.magic_instructor")?;
    let learning = QuestId::new("quest.learning_the_ropes")?;
    let confirmation = StageId::new("stage.tutorial.departure_confirmation")?;
    let choices = [
        "stay_on_island",
        "ask_ironman_tutor",
        "confirm_normal_mainland",
    ];
    let mut results = Vec::new();
    for (choice, start, expected, authorized) in [
        (
            "offer_mainland",
            "departure_offer",
            "departure_confirmation",
            false,
        ),
        (
            "stay_on_island",
            "departure_confirmation",
            "departure_offer",
            false,
        ),
        (
            "ask_ironman_tutor",
            "departure_confirmation",
            "departure_offer",
            false,
        ),
        (
            "confirm_normal_mainland",
            "departure_confirmation",
            "home_teleport",
            true,
        ),
    ] {
        let mut world = engine.initial_world()?;
        let mut character = engine.character_from_initial(
            actor.clone(),
            "Source departure fixture",
            BTreeMap::from([("body_type".into(), 0)]),
        )?;
        character.tile = world.entities[&speaker].tile;
        character.region = source.spawns[&speaker].region.clone();
        character.tutorial_stage = StageId::new(format!("stage.tutorial.{start}"))?;
        character.runtime.settings.experience = Some(ExperienceId::new("experience.brand_new")?);
        character
            .quests
            .get_mut(&learning)
            .ok_or("Missing source quest")?
            .stage = StageId::new("stage.learning_the_ropes.completed")?;
        character.quest_points = 1;
        world.characters.insert(actor.clone(), character);
        engine.apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)?;
        let original = world.characters[&actor].clone();
        engine.apply_intent(
            &mut world,
            &actor,
            &GameIntent::Interact {
                target: speaker.clone(),
                action: "Talk-to".into(),
            },
            &mut InputRandom,
        )?;
        let dialogue = engine
            .dialogue_view(&world, &actor)?
            .ok_or("No source dialogue")?;
        let offered: Vec<_> = dialogue.choices.iter().map(|row| row.id.as_str()).collect();
        if original.tutorial_stage == confirmation && offered != choices {
            return Err("Source confirmation does not preserve all three choices in order".into());
        }
        if !offered.contains(&choice) {
            return Err("The intended source branch was not offered".into());
        }
        engine.tick(&mut world, &mut TickRandom)?;
        if choice == "offer_mainland" {
            let before = world.clone();
            if engine
                .apply_intent(
                    &mut world,
                    &actor,
                    &GameIntent::SelectDialogue {
                        speaker: speaker.clone(),
                        choice: "confirm_normal_mainland".into(),
                    },
                    &mut InputRandom,
                )
                .is_ok()
                || world != before
            {
                return Err("Wrong-stage confirmation was accepted or mutated state".into());
            }
        }
        engine.apply_intent(
            &mut world,
            &actor,
            &GameIntent::SelectDialogue {
                speaker: speaker.clone(),
                choice: choice.into(),
            },
            &mut InputRandom,
        )?;
        let player = &world.characters[&actor];
        let authorized_actual = player.runtime.counters
            [&CounterId::new("counter.tutorial.departure_authorized")?]
            == CounterValue::Boolean(true);
        let unchanged = player.inventory == original.inventory
            && player.equipment == original.equipment
            && player.bank == original.bank
            && player.skills == original.skills
            && player.quest_points == 1
            && player.quests == original.quests
            && player.runtime.entitlements == original.runtime.entitlements
            && player.runtime.settings.experience == original.runtime.settings.experience
            && player.runtime.pending_travel.is_none()
            && player.region == original.region
            && player.tile == original.tile;
        let passed = player.tutorial_stage.as_str() == format!("stage.tutorial.{expected}")
            && authorized_actual == authorized
            && unchanged;
        let before_query = world.clone();
        engine.interaction_options(
            &world,
            &actor,
            &WorldTarget::Spawn {
                spawn: speaker.clone(),
            },
        )?;
        if world != before_query {
            return Err("A source menu query mutated the world".into());
        }
        results.push(json!({
            "choice": choice, "expected_stage": expected,
            "authorized": authorized_actual,
            "inventory_xp_qp_entitlements_mode_and_location_unchanged": unchanged,
            "passed": passed,
        }));
    }
    let passed = results.iter().all(|row| row["passed"] == true);
    println!(
        "{}",
        json!({
            "artifact_sha256": sha256(&bytes), "cases": results, "passed": passed,
            "classification": "Controlled source-native dialogue preconditions, not legitimate progression or acquisition.",
            "full_journey_executed": false, "milestone_accepted": false,
        })
    );
    if !passed {
        return Err("Source departure menu conformance failed".into());
    }
    Ok(())
}

use std::{collections::BTreeMap, env, fs, sync::Arc, time::Instant};

use clubscape_content::{ValidationMode, load_compiled, sha256};
use clubscape_game_types::*;
use clubscape_world_engine::{LifecycleTransition, RandomSource, WorldEngine};
use serde_json::json;

struct LowestDraw;

impl RandomSource for LowestDraw {
    fn draw_below(&mut self, upper: u32) -> GameResult<u32> {
        if upper == 0 {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "A profile cannot draw from an empty source domain.",
            ));
        }
        Ok(0)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: collision_profile ARTIFACT MAX_TICK_MS SAMPLES (1..10)".into());
    }
    let maximum_ms: f64 = args[1].parse()?;
    let samples: usize = args[2].parse()?;
    if !maximum_ms.is_finite()
        || maximum_ms <= 0.0
        || maximum_ms > 5000.0
        || !(1..=10).contains(&samples)
    {
        return Err("profile bounds must be positive, at most5000ms and1..10 samples".into());
    }
    let bytes = fs::read(&args[0])?;
    let compiled = load_compiled(&bytes, ValidationMode::Runtime)?;
    let definition = compiled.definition();
    let spawn = SpawnId::new("spawn.tutorial.start_door.3098.3107.p0.t0.r0")?;
    let transform = definition
        .mechanics
        .object_transforms
        .values()
        .find(|transform| transform.spawn == spawn)
        .ok_or("The original starting-door transform is missing")?;
    let open = transform
        .states
        .iter()
        .find(|(_, state)| state.door == Some(DoorPosition::Open))
        .map(|(id, _)| id.clone())
        .ok_or("The original starting-door open state is missing")?;
    let transform_id = transform.id.clone();
    let mut results = Vec::new();
    for actor_count in [1, 5] {
        let engine = WorldEngine::new(Arc::new(definition.clone()))?;
        let mut base = engine.initial_world()?;
        for index in 0..actor_count {
            let actor = ActorId::new(format!("actor.collision_profile.player_{index}"))?;
            let character = engine.character_from_initial(
                actor.clone(),
                "Isolated source profile",
                BTreeMap::from([("body_type".into(), 0)]),
            )?;
            base.characters.insert(actor.clone(), character);
            engine.apply_lifecycle(&mut base, &actor, LifecycleTransition::Join)?;
        }
        for opened in [false, true] {
            let mut fixture = base.clone();
            if opened {
                fixture
                    .runtime
                    .object_states
                    .insert(transform_id.clone(), open.clone());
            }
            fixture.validate_runtime(definition)?;
            for sample in 0..samples {
                let mut world = fixture.clone();
                let mut random = LowestDraw;
                let started = Instant::now();
                let events = engine.tick(&mut world, &mut random)?;
                let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
                let row = json!({
                    "actors": actor_count, "door_open": opened, "sample": sample,
                    "elapsed_ms": elapsed_ms, "within_limit": elapsed_ms <= maximum_ms,
                    "world_sha256": sha256(&serde_json::to_vec(&world)?),
                    "events_sha256": sha256(&serde_json::to_vec(&events)?),
                    "entities": world.entities.len(), "tick": world.tick,
                });
                println!("{}", row);
                results.push(row);
            }
        }
    }
    let passed = results.iter().all(|row| row["within_limit"] == true);
    println!(
        "{}",
        json!({
            "classification": "Controlled native source-content component profile. The physical door state is selected directly only in this isolated fixture; no legitimate journey or final server-performance acceptance.",
            "artifact_sha256": sha256(&bytes),
            "content_revision": definition.revision,
            "runtime_cells": definition.regions.values().map(|region| region.cells.len()).sum::<usize>(),
            "maximum_tick_ms": maximum_ms, "samples_per_case": samples,
            "passed_component_limit": passed,
            "full_journey_executed": false,
            "milestone_accepted": false,
        })
    );
    if !passed {
        return Err("the measured source component exceeds the declared profiling limit".into());
    }
    Ok(())
}

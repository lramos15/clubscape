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
            "Contact admission must not roll a catch.",
        ))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: nonwalking_contact RAW_RUNTIME_ARTIFACT")?;
    let bytes = fs::read(path)?;
    let compiled = load_compiled(&bytes, ValidationMode::Runtime)?;
    let content = compiled.definition();
    let engine = WorldEngine::new(Arc::new(content.clone()))?;
    let target = SpawnId::new("spawn.tutorial.fishing_spot.3099.3090.p0")?;
    let mut results = Vec::new();
    for tile in [Tile::new(3098, 3090, 0)?, Tile::new(3099, 3089, 0)?] {
        let mut world = engine.initial_world()?;
        let actor = ActorId::new("actor.contact.source")?;
        let mut character = engine.character_from_initial(
            actor.clone(),
            "Isolated source contact",
            BTreeMap::from([("body_type".into(), 0)]),
        )?;
        character.tile = tile;
        character.tutorial_stage = StageId::new("stage.tutorial.catch_shrimp")?;
        character.inventory.slots[0] = Some(ItemStack {
            item: ItemId::new("item.fishing_net.small")?,
            quantity: Quantity::new(1)?,
            instance: None,
        });
        world.characters.insert(actor.clone(), character);
        engine.apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)?;
        let before = world.clone();
        let options = engine.interaction_options(
            &world,
            &actor,
            &WorldTarget::Spawn {
                spawn: target.clone(),
            },
        )?;
        if world != before {
            return Err("A source contact query mutated the world".into());
        }
        let allowed = options
            .iter()
            .any(|option| option.name == "Net" && option.permission.allowed);
        let result = engine.apply_intent(
            &mut world,
            &actor,
            &GameIntent::Interact {
                target: target.clone(),
                action: "Net".into(),
            },
            &mut NoDraw,
        );
        let queued = matches!(&world.characters[&actor].activity, Activity::Gathering { target: actual, .. } if actual == &target);
        let passed = allowed
            && result.is_ok()
            && queued
            && world.characters[&actor].inventory == before.characters[&actor].inventory
            && world.characters[&actor].skills == before.characters[&actor].skills;
        results.push(
            json!({"tile": tile, "allowed": allowed, "admitted": result.is_ok(),
            "gathering_queued": queued, "passed": passed, "error": result.err()}),
        );
    }
    let passed = results.iter().all(|row| row["passed"] == true);
    println!(
        "{}",
        json!({
            "scope": "Controlled native component states against unchanged strict source artifact; not legitimate player acquisition, a catch, XP, full journey or acceptance.",
            "artifact_sha256": sha256(&bytes), "source_npc": 3317,
            "source_geometry_unchanged": engine.content().regions == content.regions,
            "cases": results, "passed": passed, "milestone_accepted": false,
        })
    );
    if !passed {
        return Err("Declared source fishing land contact is rejected".into());
    }
    Ok(())
}

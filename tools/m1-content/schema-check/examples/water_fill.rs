use std::{env, fs, sync::Arc};

use clubscape_content::{ValidationMode, load_compiled, sha256};
use clubscape_game_types::{GameError, GameErrorCode, GameResult};
use clubscape_world_engine::{RandomSource, WorldEngine};
use serde_json::json;

#[path = "../src/probes.rs"]
#[allow(dead_code)]
mod probes;
#[path = "../src/water_probes.rs"]
mod water_probes;

struct NoRandom;
impl RandomSource for NoRandom {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        Err(GameError::new(
            GameErrorCode::InvalidInput,
            "Water admission must not draw a production outcome.",
        ))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: water_fill RAW_RUNTIME_ARTIFACT")?;
    let data = fs::read(path)?;
    let compiled = load_compiled(&data, ValidationMode::Runtime)?;
    let engine = WorldEngine::new(Arc::new(compiled.definition().clone()))?;
    let result = water_probes::run(&engine)?;
    println!(
        "{}",
        json!({
            "artifact_sha256": sha256(&data), "revision": compiled.definition().revision,
            "runtime_reloaded": true, "engine_constructed": true, "water": result,
        })
    );
    if result["passed"] != true {
        return Err(
            "Source water-fill native conformance failed; no missing menu action is synthesized."
                .into(),
        );
    }
    Ok(())
}

use std::{collections::BTreeMap, env, fs, sync::Arc};

use clubscape_content::{
    ValidationMode, compile_content, encode_compiled, load_compiled, read_content_json, sha256,
};
use clubscape_game_types::{
    ActorId, GameError, GameErrorCode, GameIntent, GameResult, MaximumHitFormula, SkillId,
    SkillLevelBasis, SourceBinding, WorldTarget,
};
use clubscape_world_engine::{LifecycleTransition, RandomSource, WorldEngine};

mod probes;
mod ui_probes;

struct NoRandom;

impl RandomSource for NoRandom {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        Err(GameError::new(
            GameErrorCode::InvalidInput,
            "Non-random construction/API validation unexpectedly requested randomness.",
        ))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("expected a GameContent JSON path")?;
    let data = fs::read(path)?;
    let content = read_content_json(&data)?;
    let compiled = compile_content(content, ValidationMode::Runtime)?;
    let bytes = encode_compiled(&compiled)?;
    let reloaded = load_compiled(&bytes, ValidationMode::Runtime)?;
    if compiled.definition() != reloaded.definition() {
        return Err("compiled content changed during artifact roundtrip".into());
    }
    let content = reloaded.definition();
    let mut numeric_probe = content.clone();
    let style = numeric_probe
        .mechanics
        .combat_styles
        .get_mut(&clubscape_game_types::CombatStyleId::new(
            "style.magic.wind_strike",
        )?)
        .ok_or("missing source Wind Strike style")?;
    style.maximum_hit = SourceBinding::Bound {
        value: MaximumHitFormula::LevelTable {
            skill: SkillId::new("skill.magic")?,
            basis: SkillLevelBasis::Current,
            hits: [(1, 2), (5, 4), (9, 6), (13, 8)].into(),
        },
        source: style.maximum_hit.source().to_vec(),
    };
    let numeric_result = read_content_json(&serde_json::to_vec(&numeric_probe)?);
    let numeric_error = numeric_result.as_ref().err().map(ToString::to_string);
    if numeric_result.is_err() {
        return Err("the parent-bound source Wind Strike table must decode".into());
    }
    let engine = WorldEngine::new(Arc::new(content.clone()))?;
    let mut world = engine.initial_world()?;
    let actor = ActorId::new("actor.content.validation")?;
    let character = engine.character_from_initial(
        actor.clone(),
        "Content validation",
        BTreeMap::from([("body_type".to_owned(), 0)]),
    )?;
    if character.quest_points != 0
        || character.inventory.slots.iter().any(Option::is_some)
        || character.runtime.settings.appearance_confirmed
    {
        return Err("engine creation seeded source progress or possessions".into());
    }
    world.characters.insert(actor.clone(), character);
    engine.apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)?;
    world.validate_runtime(content)?;
    let before_query = world.clone();
    engine.context_view(&world, &actor)?;
    let initial_ui = engine.ui_view(&world, &actor)?;
    if initial_ui.version != 1
        || initial_ui.appearance.confirmed
        || initial_ui.public_chat.permission.allowed
    {
        return Err(
            "Source initial UI fabricated confirmation or mainland chat availability".into(),
        );
    }
    engine.presence_view(&world, &actor)?;
    engine.target_view(
        &world,
        &actor,
        &WorldTarget::Spawn {
            spawn: clubscape_game_types::SpawnId::new("spawn.gielinor_guide")?,
        },
    )?;
    if world != before_query {
        return Err("read-only engine projections mutated the source state".into());
    }
    engine.apply_intent(
        &mut world,
        &actor,
        &GameIntent::ConfirmAppearance {
            appearance: BTreeMap::from([("body_type".to_owned(), 0)]),
        },
        &mut NoRandom,
    )?;
    if world.characters[&actor].tutorial_stage.as_str() != "stage.tutorial.experience" {
        return Err("actual source appearance input did not follow its declared edge".into());
    }
    let native = probes::run(&engine)?;
    let ui_native = ui_probes::run(&engine)?;
    println!(
        "{}",
        serde_json::json!({
            "canonical_game_content_deserialized": true,
            "runtime_compiled": true,
            "artifact_reloaded": true,
            "artifact_sha256": sha256(&bytes),
            "unresolved_bindings": reloaded.report().unresolved_bindings,
            "source_numeric_hit_table_json_supported": numeric_result.is_ok(),
            "source_numeric_hit_table_json_error": numeric_error,
            "engine_constructed": true,
            "source_initial_world_and_character_validated": true,
            "read_only_engine_views_preserved_state": true,
            "real_source_appearance_request_passed": true,
            "native_source_policy_probes": native,
            "native_ui_control_probes": ui_native,
            "engine_api_scope": "Construction, authenticated-lifecycle boundary, read-only views and first source input only; not a full journey or browser/presentation acceptance.",
            "gameplay_executed": false,
            "revision": content.revision,
            "items": content.items.len(),
            "skills": content.skills.len(),
            "regions": content.regions.len(),
            "spawns": content.spawns.len(),
            "interfaces": content.interfaces.len(),
            "recipes": content.recipes.len(),
        })
    );
    if native["passed"] != true || ui_native["passed"] != true {
        return Err("Native source-policy conformance failed; see the exact probe result.".into());
    }
    Ok(())
}

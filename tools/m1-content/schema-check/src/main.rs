use std::{env, fs};

use clubscape_content::{
    ValidationMode, compile_content, encode_compiled, load_compiled, read_content_json, sha256,
};
use clubscape_game_types::{MaximumHitFormula, SkillId, SkillLevelBasis, SourceBinding};

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
    Ok(())
}

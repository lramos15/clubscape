use std::{env, fs, io};

use clubscape_content::{ValidationMode, compile_content, read_content_json, sha256};
use clubscape_game_types::{CombatStyleId, LevelUpVitalPolicy, MaximumHitFormula, SourceBinding};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "expected candidate JSON path")
    })?;
    let bytes = fs::read(path)?;
    let definition = read_content_json(&bytes)?;
    let compiled = compile_content(definition, ValidationMode::Runtime)?;
    let style = &compiled.definition().mechanics.combat_styles
        [&CombatStyleId::new("style.magic.wind_strike")?];
    let MaximumHitFormula::LevelTable { hits, .. } = style.maximum_hit.require()? else {
        return Err(
            io::Error::other("Wind Strike was not rebound to the known level table").into(),
        );
    };
    for (level, expected) in [(1, 2), (5, 4), (9, 6), (13, 8)] {
        if hits.get(&level) != Some(&expected) {
            return Err(io::Error::other("Wind Strike source table changed").into());
        }
    }
    let unsupported = json!({
        "status": "bound",
        "value": "raise_if_at_old_base_otherwise_preserve",
        "source": []
    });
    let conditional_enum_rejected =
        serde_json::from_value::<SourceBinding<LevelUpVitalPolicy>>(unsupported).is_err();
    if !conditional_enum_rejected {
        return Err(io::Error::other(
            "Parent enum changed: refresh the conditional-vital disposition",
        )
        .into());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "mode": "runtime",
            "candidate_sha256": sha256(&bytes),
            "content_revision": compiled.definition().revision,
            "strict_candidate_compile_passed": true,
            "wind_strike_numeric_key_table_passed": true,
            "unimplemented_conditional_vital_enum_rejected": conditional_enum_rejected,
            "remaining_unresolved_bindings": compiled.report().unresolved_bindings,
            "remaining_unresolved_count": compiled.report().unresolved_bindings.len(),
            "source_verification_performed_by_compiler": compiled.report().source_verification_performed,
            "gameplay_executed": false,
            "presentation_approved": false
        }))?
    );
    Ok(())
}

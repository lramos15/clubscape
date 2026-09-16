use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

use super::{observation, source::Tile};

pub(super) const AUTHORITY: &str = "db74895cd5d9f109d292eea20f07f5c2a57e3343";

pub(super) struct ExistingDeath {
    pub baseline: Value,
    pub owned_before: BTreeMap<String, u64>,
    pub origin: Tile,
    pub id: String,
}

pub(super) fn observed_boundary(capsule: &Value, report: &Value) -> Result<Value> {
    let player = &report["last_snapshot"]["player"];
    ensure!(
        report["status"] == "observed"
            && report["observation_only"] == true
            && report["full_journey_passed"] == false
            && report["checks_passed"] == 321
            && report["observation_checks_passed"] == 3
            && report["dying_observation"]["successful_stop"] == true
            && report["dying_observation"]["public_poll_succeeded"] == true
            && report["dying_observation"]["world_inputs_emitted"] == 0
            && report["last_snapshot"]["next_sequence"] == 302
            && report["last_snapshot"]["tick"] == 1622
            && player["actor_id"] == "actor.05a9c9bae95142fabe29d3ec35e8cf9e"
            && player["region"] == "region.osrs.12633"
            && player["tile"] == json!({"x": 3174, "y": 5726, "plane": 0})
            && player["instance"] == "instance.engine.1615.0"
            && player["active_death"] == "death.engine.1615.0"
            && capsule["source_identity"]["content_artifact"]["uncompressed_sha256"]
                == observation::SOURCE,
        "Not the explicitly authorized observed-success Office checkpoint"
    );
    let control = observation::validate_control(&capsule["latest_control_request"], false)?;
    ensure!(
        control["decoded_command"] == "poll_world"
            && control["observed_http_status"] == 200
            && control["failed_control_exception_used"] == false,
        "Observed-success continuation requires the actual successful typed poll"
    );
    Ok(
        json!({"authority": AUTHORITY, "decoded_original_control": control,
        "historical_source_checks": 321, "separate_observation_checks": 3,
        "new_account_or_death": false}),
    )
}

pub(super) fn predeath_baseline(state: &Value) -> Result<ExistingDeath> {
    let player = &state["player"];
    ensure!(
        state["tick"] == 1613
            && state["revision"] == 1924
            && state["next_sequence"] == 302
            && player["actor_id"] == "actor.05a9c9bae95142fabe29d3ec35e8cf9e"
            && player["tutorial_stage"] == "stage.tutorial.mainland"
            && player["active_death"].is_null()
            && player["instance"].is_null()
            && player["vitals"]["hitpoints"] == 1,
        "Pre-death conservation baseline is not the original public observation"
    );
    let mut counts = BTreeMap::new();
    let inventory = player["inventory"]
        .as_array()
        .context("Historical inventory missing")?;
    let equipment = player["equipment"]
        .as_object()
        .context("Historical equipment missing")?;
    for stack in inventory
        .iter()
        .map(|slot| &slot["stack"])
        .chain(equipment.values())
    {
        if stack.is_null() {
            continue;
        }
        let item = stack["item"]
            .as_str()
            .context("Historical item identity missing")?;
        let quantity = stack["quantity"]
            .as_u64()
            .context("Historical quantity missing")?;
        *counts.entry(item.to_owned()).or_insert(0) += quantity;
    }
    ensure!(
        counts.len() > 3,
        "Historical item-losing death ownership is incomplete"
    );
    Ok(ExistingDeath {
        baseline: player.clone(),
        owned_before: counts,
        origin: serde_json::from_value(player["tile"].clone())?,
        id: "death.engine.1615.0".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn death_recovery_uses_exact_public_history_not_a_recreated_inventory() {
        let state = json!({
            "tick":1613,"revision":1924,"next_sequence":302,
            "player":{"actor_id":"actor.05a9c9bae95142fabe29d3ec35e8cf9e",
                "tutorial_stage":"stage.tutorial.mainland","active_death":null,"instance":null,
                "vitals":{"hitpoints":1},"tile":{"x":3245,"y":3235,"plane":0},
                "inventory":[{"stack":{"item":"item.coins","quantity":8}},
                    {"stack":{"item":"item.bones","quantity":1}},
                    {"stack":{"item":"item.bucket","quantity":1}}],
                "equipment":{"slot.weapon":{"item":"item.sword.bronze","quantity":1}}}
        });
        let value = predeath_baseline(&state).unwrap();
        assert_eq!(value.owned_before["item.coins"], 8);
        assert_eq!(value.owned_before["item.sword.bronze"], 1);
        assert_eq!(value.origin, Tile::new(3245, 3235, 0));
        let mut changed = state;
        changed["tick"] = json!(1622);
        assert!(predeath_baseline(&changed).is_err());
    }
}

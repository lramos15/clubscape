use clubscape_protocol::game;
use serde_json::{Map, Value, json};

use crate::{BridgeError, gameplay_ui, ui_wire};

pub(crate) const CAPABILITY: &str = "game.observer.v1";

fn action(value: &game::ActorAction) -> Result<Value, BridgeError> {
    if value.version != 1 || value.id.is_empty() || value.activity.is_empty() {
        return Err(BridgeError::protocol(
            "The actor action observer has an invalid version or identity.",
        ));
    }
    for tick in [
        &value.started_at_tick,
        &value.cycle_started_at_tick,
        &value.observed_at_tick,
    ] {
        gameplay_ui::decimal(tick, false)?;
    }
    if let Some(tick) = &value.next_action_tick {
        gameplay_ui::decimal(tick, false)?;
    }
    Ok(json!({
        "version":value.version,"id":value.id,"activity":value.activity,"actionId":value.action_id,
        "target":value.target.as_ref().map(ui_wire::target).transpose()?,
        "recipeId":value.recipe_id,"styleId":value.style_id,"spellId":value.spell_id,"animation":value.animation,
        "startedAtTick":value.started_at_tick,"cycleStartedAtTick":value.cycle_started_at_tick,
        "nextActionTick":value.next_action_tick,"observedAtTick":value.observed_at_tick,
    }))
}

pub(crate) fn fields(
    running: Option<bool>,
    movement_tick: &Option<String>,
    active: Option<&game::ActorAction>,
) -> Result<Map<String, Value>, BridgeError> {
    let mut result = Map::new();
    if let Some(running) = running {
        result.insert("running".into(), json!(running));
    }
    if let Some(tick) = movement_tick {
        gameplay_ui::decimal(tick, false)?;
    }
    if running.is_some() || movement_tick.is_some() {
        result.insert("movementTick".into(), json!(movement_tick));
    }
    if running.is_some() || active.is_some() {
        result.insert(
            "action".into(),
            active.map(action).transpose()?.unwrap_or(Value::Null),
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_movement_and_action_identity_remain_exact_not_derived_from_activity() {
        assert!(fields(None, &None, None).unwrap().is_empty());
        assert_eq!(
            Value::Object(fields(Some(false), &None, None).unwrap()),
            json!({"running":false,"movementTick":null,"action":null})
        );
        let action = game::ActorAction {
            version: 1,
            id: "action-instance.fixture".into(),
            activity: "producing".into(),
            action_id: Some("action.fixture".into()),
            target: None,
            recipe_id: Some("recipe.fixture".into()),
            style_id: Some("style.fixture".into()),
            spell_id: None,
            animation: Some("animation.source.897".into()),
            started_at_tick: "9007199254740993".into(),
            cycle_started_at_tick: "9007199254740994".into(),
            next_action_tick: Some("9007199254740995".into()),
            observed_at_tick: "9007199254740994".into(),
        };
        let value = Value::Object(
            fields(
                Some(true),
                &Some("18446744073709551615".into()),
                Some(&action),
            )
            .unwrap(),
        );
        assert_eq!(value["running"], true);
        assert_eq!(value["movementTick"], "18446744073709551615");
        assert_eq!(value["action"]["id"], action.id);
        assert_eq!(value["action"]["recipeId"], "recipe.fixture");
        assert_eq!(value["action"]["startedAtTick"], action.started_at_tick);
        assert!(value["action"]["target"].is_null() && value["action"]["spellId"].is_null());
        let mut bad = action;
        bad.version = 2;
        assert!(fields(Some(false), &None, Some(&bad)).is_err());
    }
}

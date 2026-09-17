use std::{collections::BTreeMap, sync::OnceLock};

use clubscape_game_types::{ActorActionView, ActorObserverView, GameContent};
use clubscape_protocol::game;
use serde::Deserialize;

use crate::error::ApiError;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceBindings {
    version: u32,
    baseline: String,
    action_sequences: BTreeMap<String, u32>,
    recipe_sequences: BTreeMap<String, u32>,
    movement_sequences: BTreeMap<String, u32>,
    source: Vec<BindingSource>,
    unknown_policy: String,
    acceptance: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingSource {
    reference: String,
    revision: String,
    #[serde(default)]
    sha256: Option<String>,
    qualification: String,
}

fn bindings() -> Result<&'static SourceBindings, ApiError> {
    static BINDINGS: OnceLock<Result<SourceBindings, ()>> = OnceLock::new();
    BINDINGS
        .get_or_init(|| {
            let value: SourceBindings = serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../research/interface-contracts/actor-observer-bindings.json"
            )))
            .map_err(|_| ())?;
            if value.version != 1
                || value.baseline.is_empty()
                || value.source.is_empty()
                || value.unknown_policy.is_empty()
                || value.acceptance
                || !value.movement_sequences.contains_key("walk")
                || !value.movement_sequences.contains_key("run")
                || value.source.iter().any(|source| {
                    source.reference.is_empty()
                        || source.revision.is_empty()
                        || source.qualification.is_empty()
                        || source.sha256.as_ref().is_some_and(|sha| sha.len() != 64)
                })
            {
                return Err(());
            }
            Ok(value)
        })
        .as_ref()
        .map_err(|_| ApiError::internal("source_observer_bindings"))
}

pub(super) fn validate() -> Result<(), ApiError> {
    bindings().map(|_| ())
}

pub(super) fn project(
    content: &GameContent,
    mut observed: ActorObserverView,
) -> Result<(String, ActorObserverView), ApiError> {
    let sources = bindings()?;
    let bound_profile = content.baseline == sources.baseline;
    if let Some(action) = &mut observed.action
        && action.animation.is_none()
        && bound_profile
    {
        let sequence = action
            .recipe_id
            .as_ref()
            .and_then(|id| sources.recipe_sequences.get(id.as_str()))
            .or_else(|| {
                action
                    .action_id
                    .as_ref()
                    .and_then(|id| sources.action_sequences.get(id.as_str()))
            });
        action.animation = sequence.map(ToString::to_string);
    }
    let animation = if let Some(action) = &observed.action {
        action
            .animation
            .clone()
            .or_else(|| action.recipe_id.as_ref().map(ToString::to_string))
            .or_else(|| action.action_id.as_ref().map(ToString::to_string))
            .or_else(|| action.spell_id.as_ref().map(ToString::to_string))
            .or_else(|| action.style_id.as_ref().map(ToString::to_string))
            .unwrap_or_default()
    } else if observed.movement_tick.is_some() && bound_profile {
        sources.movement_sequences[if observed.running { "run" } else { "walk" }].to_string()
    } else {
        String::new()
    };
    Ok((animation, observed))
}

pub(super) fn action(value: ActorActionView) -> game::ActorAction {
    game::ActorAction {
        version: value.version,
        id: value.id,
        activity: value.activity,
        action_id: value.action_id.map(|id| id.to_string()),
        target: value.target.map(|target| game::WorldTarget {
            target: Some(match target {
                clubscape_game_types::WorldTarget::Spawn { spawn } => {
                    game::world_target::Target::Spawn(spawn.to_string())
                }
                clubscape_game_types::WorldTarget::TemporaryObject { object } => {
                    game::world_target::Target::TemporaryObject(object.to_string())
                }
            }),
        }),
        recipe_id: value.recipe_id.map(|id| id.to_string()),
        style_id: value.style_id.map(|id| id.to_string()),
        spell_id: value.spell_id.map(|id| id.to_string()),
        animation: value.animation,
        started_at_tick: value.started_at_tick,
        cycle_started_at_tick: value.cycle_started_at_tick,
        next_action_tick: value.next_action_tick,
        observed_at_tick: value.observed_at_tick,
    }
}

use serde::{Deserialize, Serialize};

use crate::{
    ActionId, CombatStyleId, GameError, GameErrorCode, GameResult, InstanceId, InstanceTemplateId,
    RecipeId, RegionId, SpellId, Tile, WorldTarget,
};

pub const ACTOR_OBSERVER_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentSceneView {
    pub region: RegionId,
    pub instance: Option<InstanceId>,
    pub instance_template: Option<InstanceTemplateId>,
}

/// Read-only identity and phase of the actual authoritative action, not scene-neighbour inference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorActionView {
    pub version: u32,
    pub id: String,
    pub activity: String,
    pub action_id: Option<ActionId>,
    pub target: Option<WorldTarget>,
    pub recipe_id: Option<RecipeId>,
    pub style_id: Option<CombatStyleId>,
    pub spell_id: Option<SpellId>,
    /// Explicit bound sequence/asset identity; absent when only the source action is known.
    pub animation: Option<String>,
    pub started_at_tick: String,
    pub cycle_started_at_tick: String,
    pub next_action_tick: Option<String>,
    pub observed_at_tick: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorObserverView {
    pub running: bool,
    pub movement_tick: Option<String>,
    pub action: Option<ActorActionView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorObservation {
    pub version: u32,
    pub next_id: u64,
    pub movement: Option<MovementObservation>,
    pub action: Option<ActionObservation>,
}

impl Default for ActorObservation {
    fn default() -> Self {
        Self {
            version: ACTOR_OBSERVER_VERSION,
            next_id: 1,
            movement: None,
            action: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovementObservation {
    pub tick: u64,
    pub from: Tile,
    pub to: Tile,
    pub instance: Option<InstanceId>,
    pub running: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedAction {
    pub activity: String,
    pub action_id: Option<ActionId>,
    pub target: Option<WorldTarget>,
    pub recipe_id: Option<RecipeId>,
    pub style_id: Option<CombatStyleId>,
    pub spell_id: Option<SpellId>,
    pub animation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionObservation {
    pub ordinal: u64,
    pub identity: ObservedAction,
    pub started_at_tick: u64,
    pub cycle_started_at_tick: u64,
    pub next_action_tick: Option<u64>,
    /// A completed/instant action remains observable only in its actual commit tick.
    pub completed_at_tick: Option<u64>,
}

impl ActorObservation {
    pub fn validate(&self, tick: u64) -> GameResult<()> {
        if self.version != ACTOR_OBSERVER_VERSION
            || self.next_id == 0
            || self.next_id > i64::MAX as u64
            || self.movement.as_ref().is_some_and(|motion| {
                motion.tick > tick
                    || motion.from == motion.to
                    || motion.from.plane() != motion.to.plane()
                    || motion
                        .from
                        .distance(motion.to)
                        .is_none_or(|distance| distance > if motion.running { 2 } else { 1 })
            })
            || self.action.as_ref().is_some_and(|action| {
                action.ordinal == 0
                    || action.ordinal >= self.next_id
                    || action.started_at_tick > action.cycle_started_at_tick
                    || action.cycle_started_at_tick > tick
                    || action
                        .next_action_tick
                        .is_some_and(|next| next > i64::MAX as u64)
                    || action.completed_at_tick.is_some_and(|at| at > tick)
                    || action.identity.activity.is_empty()
                    || action.identity.activity.len() > 64
                    || action.identity.animation.as_ref().is_some_and(|name| {
                        name.is_empty() || name.len() > 256 || name.chars().any(char::is_control)
                    })
            })
        {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Invalid authoritative actor observation.",
            ));
        }
        Ok(())
    }
}

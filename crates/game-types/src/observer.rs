use serde::{Deserialize, Serialize};

use crate::{ActionId, CombatStyleId, RecipeId, SpellId, WorldTarget};

pub const ACTOR_OBSERVER_VERSION: u32 = 1;

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

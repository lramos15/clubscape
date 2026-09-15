use crate::{ActionId, CombatStyleId, RecipeId, SourceRecord, SpellId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorAnimationDefinition {
    pub version: u32,
    pub sequences: BTreeMap<u32, ActorSequenceDefinition>,
    pub actions: BTreeMap<ActionId, ActorAnimationBinding>,
    pub recipes: BTreeMap<RecipeId, ActorAnimationBinding>,
    pub styles: BTreeMap<CombatStyleId, ActorAnimationBinding>,
    pub spells: BTreeMap<SpellId, ActorAnimationBinding>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorSequenceDefinition {
    pub duration_cycles: u32,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorAnimationBinding {
    pub rule: ActorAnimationRule,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ActorAnimationRule {
    Sequence {
        sequence: u32,
    },
    Channel {
        duration_ticks: u32,
        phases: Vec<ActorAnimationPhase>,
    },
    Unverified {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorAnimationPhase {
    pub at_tick: u32,
    pub sequence: u32,
}

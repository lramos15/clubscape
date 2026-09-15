//! Typed source selectors and policies for the M1 executor. No numeric source defaults.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerDropPolicy {
    pub ordinary: SourceBinding<GroundPolicyId>,
    pub stages: BTreeMap<StageId, SourceBinding<GroundPolicyId>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub untradeable: Option<SourceBinding<GroundPolicyId>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_playtime: Option<SourceBinding<PlayerDropPlaytimePolicy>>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerDropPlaytimePolicy {
    pub played_ticks_below: u64,
    pub ground_policy: GroundPolicyId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraversalEdge {
    pub from: Tile,
    pub to: Tile,
    pub bidirectional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraversalDefinition {
    pub id: TraversalId,
    pub scope: CounterScope,
    pub edges: Vec<TraversalEdge>,
    /// Evaluated for the moving actor, separately from an opener/key interaction.
    pub guard: Guard,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombinedCollisionState {
    pub selection: BTreeMap<ObjectTransformId, ObjectStateId>,
    pub collision: Vec<CollisionCell>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollisionGroupDefinition {
    pub id: CollisionGroupId,
    pub scope: CounterScope,
    pub transforms: BTreeSet<ObjectTransformId>,
    pub states: SourceBinding<Vec<CombinedCollisionState>>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectMorphCollision {
    pub placements: BTreeMap<SpawnId, ObjectTransformId>,
    pub variants: Vec<ObjectMorphCollisionCase>,
    pub fallback: ObjectStateId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectMorphCollisionCase {
    pub value: i64,
    pub state: ObjectStateId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttackEligibility {
    pub method: AttackMethod,
    /// None explicitly permits every style of the declared method.
    pub style: Option<CombatStyleId>,
    pub guard: Guard,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerEngagementPolicy {
    pub combat_state_ticks: u32,
    pub logout_lock_ticks: u32,
    pub travel_lock_ticks: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerCombatPolicy {
    pub unarmed: SourceBinding<WeaponDefinition>,
    pub engagement: SourceBinding<PlayerEngagementPolicy>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AggressionPolicy {
    pub acquisition_range: u16,
    pub require_line_of_sight: bool,
    pub guard: Guard,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NpcEngagementPolicy {
    pub leash_range: u16,
    pub inactivity_ticks: u32,
    pub reacquire_delay_ticks: u32,
    pub acquire_delay_ticks: u32,
    pub return_to_spawn: bool,
    pub reset_life_on_return: bool,
    pub aggression: Option<AggressionPolicy>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KillMethodPolicy {
    FinishingAttack,
    FirstContributingMethod,
    LastContributingMethod,
    MostDamageThenFirstMethod,
    MostDamageThenLastMethod,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionMode {
    Single,
    MakeX,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeathTiming {
    /// Explicit zero means no mechanical delay in that phase.
    pub dying_ticks: u32,
    pub respawn_ticks: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryInterfaces {
    pub grave: InterfaceId,
    pub office: InterfaceId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MethodContribution {
    pub damage: u64,
    pub first_hit_tick: u64,
    pub first_hit_order: u32,
    pub last_hit_tick: u64,
    pub last_hit_order: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KillResolution {
    pub life: u64,
    pub at_tick: u64,
    pub credited: ActorId,
    pub method: AttackMethod,
    pub npc: NpcId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GroundProducer {
    PlayerDrop {
        actor: ActorId,
        at_tick: u64,
    },
    Activity {
        actor: ActorId,
        at_tick: u64,
    },
    DeathSupply {
        actor: ActorId,
        at_tick: u64,
    },
    NpcLoot {
        spawn: SpawnId,
        life: u64,
        instance: Option<InstanceId>,
        ordinal: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundProvenance {
    pub policy: GroundPolicyId,
    pub producer: GroundProducer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<GroundClock>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeathArrival {
    pub destination: RuntimeLocation,
    pub first_office: bool,
    pub dying_until_tick: u64,
    pub arrives_at_tick: u64,
    pub completed_at_tick: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionLoss {
    TransportLost,
    AuthenticationRevoked,
    CoordinatorRestart,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PresenceState {
    /// Legacy callers must supply authoritative TickContext; this is not online.
    #[default]
    Untracked,
    Connected {
        joined_at_tick: u64,
    },
    Disconnecting {
        reason: ConnectionLoss,
        since_tick: u64,
    },
    Offline {
        since_tick: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectileTargetSnapshot {
    pub npc: NpcId,
    pub location: RuntimeLocation,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecoverySlotKey {
    Ordinary(ItemId),
    Instance(ItemInstanceId),
}

pub fn recovery_slot_key(stack: &ItemStack) -> RecoverySlotKey {
    stack.instance.as_ref().map_or_else(
        || RecoverySlotKey::Ordinary(stack.item.clone()),
        |instance| RecoverySlotKey::Instance(instance.id.clone()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_contributions_preserve_damage_without_fabricated_method_history() {
        let legacy = r#"{"damage":2,"first_hit_tick":4,"first_hit_order":0,"last_hit_tick":8,"last_hit_order":1}"#;
        let contribution: DamageContribution = serde_json::from_str(legacy).unwrap();
        assert_eq!(contribution.damage, 2);
        assert!(!contribution.methods_complete);
        assert!(contribution.methods.is_empty());
        let actor = ActorId::new("actor.fixture.legacy").unwrap();
        let mut runtime = EntityRuntime {
            contributions: BTreeMap::from([(actor.clone(), contribution)]),
            ..EntityRuntime::default()
        };
        runtime.validate_shape().unwrap();
        runtime
            .contributions
            .get_mut(&actor)
            .unwrap()
            .methods_complete = true;
        assert!(runtime.validate_shape().is_err());
    }

    #[test]
    fn old_runtime_defaults_are_unbound_metadata_not_new_claims_or_fresh_contact() {
        let mut json = serde_json::to_value(WorldRuntime::default()).unwrap();
        json.as_object_mut().unwrap().remove("next_ground_id");
        json.as_object_mut().unwrap().remove("ground_provenance");
        let world: WorldRuntime = serde_json::from_value(json).unwrap();
        assert_eq!(world.next_ground_id, 0);
        assert!(world.ground_provenance.is_empty());
        let mut json = serde_json::to_value(CharacterCombatState::default()).unwrap();
        json.as_object_mut().unwrap().remove("last_combat_tick");
        let combat: CharacterCombatState = serde_json::from_value(json).unwrap();
        assert_eq!(combat.last_combat_tick, None);
    }

    #[test]
    fn new_wire_variants_are_requests_and_do_not_accept_outcome_setters() {
        for intent in [
            GameIntent::OpenGrave {
                death: DeathId::new("death.fixture.one").unwrap(),
            },
            GameIntent::OpenDeathOffice,
            GameIntent::ProduceSelected {
                recipe: RecipeId::new("recipe.fixture.one").unwrap(),
                target: None,
                quantity: Quantity::new(1).unwrap(),
                mode: ProductionMode::MakeX,
            },
        ] {
            assert_eq!(
                serde_json::from_str::<GameIntent>(&serde_json::to_string(&intent).unwrap())
                    .unwrap(),
                intent
            );
        }
        assert!(
            serde_json::from_str::<GameIntent>(r#"{"kind":"set_counter","value":true}"#).is_err()
        );
        assert!(serde_json::from_str::<GameIntent>(r#"{"kind":"give_items","items":[]}"#).is_err());
    }
}

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct InitialRuntimeDefinition {
    pub settings: CharacterSettings,
    pub counters: BTreeMap<CounterId, CounterValue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CharacterSettings {
    /// None means not bound; it is not an implicit false source setting.
    pub run_enabled: Option<bool>,
    pub auto_retaliate: Option<bool>,
    pub death_auto_equip: Option<bool>,
    pub death_supply_piles: Option<bool>,
    pub experience: Option<ExperienceId>,
    pub appearance_confirmed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "setting",
    content = "enabled",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum CharacterSetting {
    Run(bool),
    AutoRetaliate(bool),
    DeathAutoEquip(bool),
    DeathSupplyPiles(bool),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionSession {
    pub spawn: SpawnId,
    /// The legacy one-based index is preserved, including content-revision dependence.
    pub interaction: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContainerSession {
    Bank {
        session: InteractionSession,
    },
    Shop {
        session: InteractionSession,
    },
    Grave {
        death: DeathId,
        interface: InterfaceId,
    },
    DeathOffice {
        interface: InterfaceId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct EngineSchedule {
    pub command_seen: bool,
    pub gather_interaction: Option<u32>,
    pub dialogue_interaction: Option<u32>,
    pub access: Option<ContainerSession>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EngineMetadata {
    /// Existing flags remain authoritative until an explicit, checked migration.
    #[default]
    Legacy,
    Typed {
        schedule: EngineSchedule,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FractionalAccumulator {
    pub numerator: u64,
    pub denominator: u64,
}

impl FractionalAccumulator {
    pub fn validate(&self) -> GameResult<()> {
        if self.denominator == 0 || self.numerator >= self.denominator {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Fractional accumulator must contain a proper nonnegative remainder.",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CharacterCombatState {
    pub style: Option<CombatStyleId>,
    pub target: Option<SpawnId>,
    pub attack_ready: u64,
    pub spell_ready: u64,
    pub last_attacker: Option<SpawnId>,
    pub active_prayers: BTreeSet<PrayerId>,
    pub prayer_drain: Option<FractionalAccumulator>,
    #[serde(default)]
    pub last_combat_tick: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EntitlementState {
    Claimed {
        at_tick: u64,
    },
    Grant {
        delivered: BTreeMap<ItemId, u32>,
        /// Includes missing/top-up lines already satisfied by legitimately owned items.
        satisfied: BTreeSet<ItemId>,
        complete: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifePhase {
    Alive,
    Dying,
    FirstDeathOffice,
    Respawning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LifeState {
    #[default]
    Legacy,
    Alive,
    Dying {
        death: DeathId,
        at_tick: u64,
    },
    FirstDeathOffice {
        death: DeathId,
        instance: InstanceId,
    },
    Respawning {
        death: DeathId,
        destination: RuntimeLocation,
        at_tick: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingTravel {
    pub travel: TravelId,
    pub origin: RuntimeLocation,
    pub destination: RuntimeLocation,
    pub started_at_tick: u64,
    pub completes_at_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingFire {
    pub recipe: RecipeId,
    pub ground_item: String,
    pub tile: Tile,
    pub next_attempt_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterRuntime {
    pub schema_version: u32,
    pub engine: EngineMetadata,
    pub settings: CharacterSettings,
    pub counters: BTreeMap<CounterId, CounterValue>,
    pub entitlements: BTreeMap<EntitlementId, EntitlementState>,
    pub combat: CharacterCombatState,
    pub food_ready: u64,
    pub life: LifeState,
    pub death_topics: BTreeSet<DeathTopic>,
    pub first_item_loss_seen: bool,
    pub active_death: Option<DeathId>,
    pub previous_respawn: Option<RuntimeLocation>,
    pub instance: Option<InstanceId>,
    pub pending_travel: Option<PendingTravel>,
    pub pending_fire: Option<PendingFire>,
    pub travel_cooldowns: BTreeMap<TravelId, u64>,
    pub action_cooldowns: BTreeMap<ActionId, u64>,
    pub regeneration_deadlines: BTreeMap<Vital, u64>,
    pub death_coffer: u64,
    pub last_active_tick: Option<u64>,
    #[serde(default)]
    pub presence: PresenceState,
}

impl Default for CharacterRuntime {
    fn default() -> Self {
        Self {
            schema_version: RUNTIME_SCHEMA_VERSION,
            engine: EngineMetadata::Legacy,
            settings: CharacterSettings::default(),
            counters: BTreeMap::new(),
            entitlements: BTreeMap::new(),
            combat: CharacterCombatState::default(),
            food_ready: 0,
            life: LifeState::Legacy,
            death_topics: BTreeSet::new(),
            first_item_loss_seen: false,
            active_death: None,
            previous_respawn: None,
            instance: None,
            pending_travel: None,
            pending_fire: None,
            travel_cooldowns: BTreeMap::new(),
            action_cooldowns: BTreeMap::new(),
            regeneration_deadlines: BTreeMap::new(),
            death_coffer: 0,
            last_active_tick: None,
            presence: PresenceState::Untracked,
        }
    }
}

impl CharacterRuntime {
    pub fn from_initial(content: &GameContent) -> Self {
        Self::from_initial_definition(&content.initial_state.runtime)
    }

    pub fn from_initial_definition(initial: &InitialRuntimeDefinition) -> Self {
        Self {
            settings: initial.settings.clone(),
            counters: initial.counters.clone(),
            life: LifeState::Alive,
            ..Self::default()
        }
    }

    pub fn validate_shape(&self) -> GameResult<()> {
        let tick = |tick: u64| tick <= i64::MAX as u64;
        match self.presence {
            PresenceState::Connected { joined_at_tick }
                if !tick(joined_at_tick) || self.last_active_tick.is_none() =>
            {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    "Connected presence requires a valid join/input clock.",
                ));
            }
            PresenceState::Disconnecting { since_tick, .. }
            | PresenceState::Offline { since_tick }
                if !tick(since_tick) =>
            {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    "Presence deadline is out of range.",
                ));
            }
            _ => {}
        }
        let valid = self.schema_version == RUNTIME_SCHEMA_VERSION
            && self.counters.len() <= 2048
            && self.entitlements.len() <= 2048
            && self.action_cooldowns.len() <= 2048
            && self.travel_cooldowns.len() <= 2048
            && self.combat.active_prayers.len() <= 256
            && tick(self.combat.attack_ready)
            && tick(self.combat.spell_ready)
            && self.combat.last_combat_tick.is_none_or(tick)
            && tick(self.food_ready)
            && self.action_cooldowns.values().all(|value| tick(*value))
            && self.travel_cooldowns.values().all(|value| tick(*value))
            && self
                .regeneration_deadlines
                .values()
                .all(|value| tick(*value))
            && self.last_active_tick.is_none_or(tick)
            && self.pending_travel.as_ref().is_none_or(|travel| {
                travel.started_at_tick <= travel.completes_at_tick && tick(travel.completes_at_tick)
            })
            && self.pending_fire.as_ref().is_none_or(|fire| {
                !fire.ground_item.is_empty()
                    && fire.ground_item.len() <= 160
                    && tick(fire.next_attempt_tick)
            })
            && self.entitlements.values().all(|claim| match claim {
                EntitlementState::Claimed { at_tick } => tick(*at_tick),
                EntitlementState::Grant {
                    delivered,
                    satisfied,
                    ..
                } => {
                    delivered.len() <= 2048
                        && satisfied.len() <= 2048
                        && delivered
                            .values()
                            .all(|value| *value > 0 && *value <= MAX_STACK_QUANTITY)
                }
            });
        if !valid {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Invalid character runtime shape.",
            ));
        }
        if let EngineMetadata::Typed { schedule } = &self.engine {
            schedule.validate()?;
        }
        if let Some(remainder) = &self.combat.prayer_drain {
            remainder.validate()?;
        }
        match &self.life {
            LifeState::Dying { at_tick, .. } | LifeState::Respawning { at_tick, .. }
                if *at_tick > i64::MAX as u64 =>
            {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    "Life-state deadline overflow.",
                ));
            }
            _ => {}
        }
        if self.death_coffer > i64::MAX as u64 {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Death coffer exceeds currency bounds.",
            ));
        }
        Ok(())
    }
}

impl EngineSchedule {
    fn validate(&self) -> GameResult<()> {
        let index = |value: Option<u32>| value.is_none_or(|value| value > 0);
        let valid = index(self.gather_interaction)
            && index(self.dialogue_interaction)
            && self.access.as_ref().is_none_or(|access| match access {
                ContainerSession::Bank { session } | ContainerSession::Shop { session } => {
                    session.interaction > 0
                }
                ContainerSession::Grave { .. } | ContainerSession::DeathOffice { .. } => true,
            });
        if valid {
            Ok(())
        } else {
            Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Invalid engine schedule.",
            ))
        }
    }
}

impl CharacterState {
    /// Atomic opt-in migration. Inventory, XP, progression, Activity and OpenDialogue are untouched.
    /// Unknown keys, conflicting typed metadata and invalid pending indices fail without mutation.
    pub fn migrate_engine_metadata(&mut self, content: &GameContent) -> GameResult<()> {
        const PREFIX: &str = "__world_engine.";
        if !matches!(self.runtime.engine, EngineMetadata::Legacy) {
            if self.flags.keys().any(|key| key.starts_with(PREFIX)) {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    "Both typed and legacy engine metadata are present.",
                ));
            }
            return self.runtime.validate_shape();
        }
        let mut schedule = EngineSchedule::default();
        self.runtime.validate_shape()?;
        let mut food_ready = self.runtime.food_ready;
        let mut attack_ready = self.runtime.combat.attack_ready;
        let merge_deadline = |current: &mut u64, legacy: u64| -> GameResult<()> {
            if *current != 0 && *current != legacy {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    "Typed and legacy action deadlines conflict.",
                ));
            }
            *current = legacy;
            Ok(())
        };
        for (key, value) in self.flags.iter().filter(|(key, _)| key.starts_with(PREFIX)) {
            let number = u64::try_from(*value).map_err(|_| {
                GameError::new(
                    GameErrorCode::InvalidInput,
                    "Negative legacy engine metadata.",
                )
            })?;
            let index = || {
                u32::try_from(number)
                    .ok()
                    .filter(|index| *index > 0)
                    .ok_or_else(|| {
                        GameError::new(
                            GameErrorCode::InvalidInput,
                            "Invalid legacy interaction index.",
                        )
                    })
            };
            match key.as_str() {
                "__world_engine.command_seen" if number == 1 => schedule.command_seen = true,
                "__world_engine.food_ready" => merge_deadline(&mut food_ready, number)?,
                "__world_engine.attack_ready" => merge_deadline(&mut attack_ready, number)?,
                "__world_engine.gather_interaction" => schedule.gather_interaction = Some(index()?),
                "__world_engine.dialogue_interaction" => {
                    schedule.dialogue_interaction = Some(index()?)
                }
                _ => {
                    let (kind, spawn) = key
                        .strip_prefix("__world_engine.access.")
                        .and_then(|key| key.split_once(':'))
                        .ok_or_else(|| {
                            GameError::new(
                                GameErrorCode::Unavailable,
                                format!("Unmapped legacy key {key}."),
                            )
                        })?;
                    let session = InteractionSession {
                        spawn: SpawnId::new(spawn)?,
                        interaction: index()?,
                    };
                    let action = migration_interaction(content, &session)?;
                    let access = match (kind, &action.action) {
                        ("bank", InteractionAction::Bank | InteractionAction::OpenBank { .. }) => {
                            ContainerSession::Bank { session }
                        }
                        (
                            "shop",
                            InteractionAction::Shop { .. } | InteractionAction::OpenShop { .. },
                        ) => ContainerSession::Shop { session },
                        _ => {
                            return Err(GameError::new(
                                GameErrorCode::InvalidInput,
                                "Legacy access kind changed.",
                            ));
                        }
                    };
                    if schedule.access.replace(access).is_some() {
                        return Err(GameError::new(
                            GameErrorCode::InvalidInput,
                            "Multiple legacy access sessions.",
                        ));
                    }
                }
            }
        }
        if let Some(interaction) = schedule.gather_interaction {
            let Activity::Gathering { target, .. } = &self.activity else {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    "Orphaned gathering metadata.",
                ));
            };
            let action = migration_interaction(
                content,
                &InteractionSession {
                    spawn: target.clone(),
                    interaction,
                },
            )?;
            if !matches!(action.action, InteractionAction::Gather { .. }) {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    "Gathering interaction changed.",
                ));
            }
        }
        if let Some(interaction) = schedule.dialogue_interaction {
            let dialogue = self.dialogue.as_ref().ok_or_else(|| {
                GameError::new(GameErrorCode::InvalidInput, "Orphaned dialogue metadata.")
            })?;
            let action = migration_interaction(
                content,
                &InteractionSession {
                    spawn: dialogue.speaker.clone(),
                    interaction,
                },
            )?;
            if !matches!(&action.action, InteractionAction::Dialogue { dialogue: id } if id == &dialogue.id)
            {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    "Dialogue interaction changed.",
                ));
            }
        }
        if matches!(self.activity, Activity::Gathering { .. })
            && schedule.gather_interaction.is_none()
        {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Pending gathering metadata is missing.",
            ));
        }
        if self.dialogue.is_some() && schedule.dialogue_interaction.is_none() {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Pending dialogue metadata is missing.",
            ));
        }
        schedule.validate()?;
        self.flags.retain(|key, _| !key.starts_with(PREFIX));
        self.runtime.engine = EngineMetadata::Typed { schedule };
        self.runtime.food_ready = food_ready;
        self.runtime.combat.attack_ready = attack_ready;
        Ok(())
    }
}

fn migration_interaction<'a>(
    content: &'a GameContent,
    session: &InteractionSession,
) -> GameResult<&'a InteractionDefinition> {
    content
        .spawns
        .get(&session.spawn)
        .and_then(|spawn| {
            spawn
                .interactions
                .get(session.interaction.saturating_sub(1) as usize)
        })
        .ok_or_else(|| {
            GameError::new(
                GameErrorCode::InvalidInput,
                "Legacy interaction no longer exists.",
            )
        })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageContribution {
    pub damage: u64,
    pub first_hit_tick: u64,
    pub first_hit_order: u32,
    pub last_hit_tick: u64,
    pub last_hit_order: u32,
    #[serde(default)]
    pub methods: BTreeMap<AttackMethod, MethodContribution>,
    #[serde(default)]
    pub methods_complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct EntityRuntime {
    pub life: u64,
    pub attack_ready: u64,
    pub retaliation_target: Option<ActorId>,
    pub contributions: BTreeMap<ActorId, DamageContribution>,
    pub loot_resolved: bool,
    pub next_movement_tick: Option<u64>,
    #[serde(default)]
    pub last_combat_tick: Option<u64>,
    #[serde(default)]
    pub aggression_ready: u64,
    #[serde(default)]
    pub returning_to_spawn: bool,
    #[serde(default)]
    pub kill: Option<KillResolution>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Combatant {
    Player {
        actor: ActorId,
    },
    Npc {
        spawn: SpawnId,
        life: u64,
        instance: Option<InstanceId>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingProjectile {
    pub id: u64,
    pub definition: ProjectileId,
    pub source: Combatant,
    pub target: Combatant,
    pub style: CombatStyleId,
    pub spell: Option<SpellId>,
    pub launched_at_tick: u64,
    pub impacts_at_tick: u64,
    pub outcome: CombatOutcome,
    pub damage: u16,
    pub resources_spent: Vec<ItemStack>,
    #[serde(default)]
    pub target_snapshot: Option<ProjectileTargetSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicObject {
    pub definition: TemporaryObjectId,
    pub owner: ActorId,
    pub location: RuntimeLocation,
    pub created_at_tick: u64,
    pub expires_at_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceState {
    pub template: InstanceTemplateId,
    pub owner: Option<ActorId>,
    pub counters: BTreeMap<CounterId, CounterValue>,
    pub entities: BTreeMap<SpawnId, EntityState>,
    pub object_states: BTreeMap<ObjectTransformId, ObjectStateId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeLocation {
    pub region: RegionId,
    pub tile: Tile,
    pub instance: Option<InstanceId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ItemLayout {
    Inventory { slot: u8 },
    Equipment { slot: SlotId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryItem {
    pub id: RecoveryItemId,
    pub stack: ItemStack,
    pub layout: ItemLayout,
    pub effective_unit_value: u64,
    pub fee_paid: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraveState {
    pub location: RuntimeLocation,
    pub active_ticks_remaining: u32,
    pub clock_started: bool,
    #[serde(default)]
    pub started_at_tick: Option<u64>,
    pub paused: BTreeSet<ClockPause>,
    pub items: Vec<RecoveryItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeathRecord {
    pub owner: ActorId,
    pub occurred_at_tick: u64,
    pub origin: RuntimeLocation,
    pub respawn: RuntimeLocation,
    pub value_provider: ValueProviderId,
    pub value_revision: String,
    /// References to retained ownership/layout, not another spendable container.
    pub retained: Vec<RecoveryItem>,
    pub grave: Option<GraveState>,
    pub office: Vec<RecoveryItem>,
    pub reclaimed: BTreeSet<RecoveryItemId>,
    #[serde(default)]
    pub arrival: Option<DeathArrival>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldRuntime {
    pub schema_version: u32,
    /// None is a legacy/unbound world, not an implicit free or members world.
    pub members: Option<bool>,
    pub counters: BTreeMap<CounterId, CounterValue>,
    pub object_states: BTreeMap<ObjectTransformId, ObjectStateId>,
    pub temporary_objects: BTreeMap<DynamicObjectId, DynamicObject>,
    pub instances: BTreeMap<InstanceId, InstanceState>,
    pub projectiles: Vec<PendingProjectile>,
    pub deaths: BTreeMap<DeathId, DeathRecord>,
    pub stock_deadlines: BTreeMap<ShopId, BTreeMap<ItemId, u64>>,
    #[serde(default)]
    pub next_ground_id: u64,
    #[serde(default)]
    pub ground_provenance: BTreeMap<String, GroundProvenance>,
}

impl Default for WorldRuntime {
    fn default() -> Self {
        Self {
            schema_version: RUNTIME_SCHEMA_VERSION,
            members: None,
            counters: BTreeMap::new(),
            object_states: BTreeMap::new(),
            temporary_objects: BTreeMap::new(),
            instances: BTreeMap::new(),
            projectiles: Vec::new(),
            deaths: BTreeMap::new(),
            stock_deadlines: BTreeMap::new(),
            next_ground_id: 0,
            ground_provenance: BTreeMap::new(),
        }
    }
}

impl WorldRuntime {
    pub fn from_initial(content: &GameContent) -> Self {
        Self {
            members: content.mechanics.world_members,
            counters: initial_counters(content, CounterScope::World),
            object_states: content
                .mechanics
                .object_transforms
                .iter()
                .filter(|(_, definition)| definition.scope == CounterScope::World)
                .map(|(id, definition)| (id.clone(), definition.initial.clone()))
                .collect(),
            ..Self::default()
        }
    }
}

fn initial_counters(
    content: &GameContent,
    scope: CounterScope,
) -> BTreeMap<CounterId, CounterValue> {
    content
        .mechanics
        .counters
        .iter()
        .filter(|(_, definition)| definition.scope == scope)
        .map(|(id, definition)| (id.clone(), definition.initial))
        .collect()
}

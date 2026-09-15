use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::*;

/// An unresolved source input is data, never zero, a guarantee, or runtime permission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceBinding<T> {
    Bound {
        value: T,
        source: Vec<SourceRecord>,
    },
    Unresolved {
        reason: String,
        source: Vec<SourceRecord>,
    },
}

impl<T> SourceBinding<T> {
    pub fn require(&self) -> GameResult<&T> {
        match self {
            Self::Bound { value, .. } => Ok(value),
            Self::Unresolved { reason, .. } => {
                Err(GameError::new(GameErrorCode::Unavailable, reason.clone()))
            }
        }
    }

    pub fn source(&self) -> &[SourceRecord] {
        match self {
            Self::Bound { source, .. } | Self::Unresolved { source, .. } => source,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillLevelBasis {
    Base,
    Current,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelDomain {
    pub minimum: u16,
    pub maximum: u16,
    pub basis: SkillLevelBasis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChanceDomain {
    Constant,
    /// Endpoints are success counts at levels 1 and 99, including the source +1.
    Skill {
        levels: LevelDomain,
    },
}

impl ChanceRule {
    pub fn constant(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator_at_level_1: numerator,
            numerator_at_level_99: numerator,
            denominator,
            domain: ChanceDomain::Constant,
        }
    }

    pub fn source_skilling(low: u32, high: u32, levels: LevelDomain) -> GameResult<Self> {
        let increment = |value: u32| {
            value.checked_add(1).ok_or_else(|| {
                GameError::new(
                    GameErrorCode::InvalidContent,
                    "Source chance endpoint overflow.",
                )
            })
        };
        Ok(Self {
            numerator_at_level_1: increment(low)?,
            numerator_at_level_99: increment(high)?,
            denominator: 256,
            domain: ChanceDomain::Skill { levels },
        })
    }

    pub fn validate(&self) -> GameResult<()> {
        let valid = self.denominator != 0
            && match self.domain {
                ChanceDomain::Constant => {
                    self.numerator_at_level_1 == self.numerator_at_level_99
                        && self.numerator_at_level_1 <= self.denominator
                }
                ChanceDomain::Skill { levels } => {
                    levels.minimum >= 1 && levels.minimum <= levels.maximum && levels.maximum <= 99
                }
            };
        if !valid {
            return Err(GameError::new(
                GameErrorCode::InvalidContent,
                "Invalid chance domain.",
            ));
        }
        Ok(())
    }

    pub fn numerator(&self, level: u16) -> GameResult<u32> {
        self.validate()?;
        if let ChanceDomain::Skill { levels } = self.domain {
            if !(levels.minimum..=levels.maximum).contains(&level) {
                return Err(GameError::new(
                    GameErrorCode::Unavailable,
                    "Chance is not bound for this skill level.",
                ));
            }
            let level = u64::from(level);
            let count = (u64::from(self.numerator_at_level_1) * (99 - level)
                + u64::from(self.numerator_at_level_99) * (level - 1)
                + 49)
                / 98;
            return Ok(count.min(u64::from(self.denominator)) as u32);
        }
        Ok(self.numerator_at_level_1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TickDuration {
    Fixed { ticks: u32 },
    UniformInclusive { minimum: u32, maximum: u32 },
}

impl TickDuration {
    pub fn fixed(&self) -> GameResult<u32> {
        match self {
            Self::Fixed { ticks } if *ticks > 0 => Ok(*ticks),
            Self::Fixed { .. } => Err(GameError::new(
                GameErrorCode::InvalidContent,
                "Duration must be positive.",
            )),
            Self::UniformInclusive { .. } => Err(GameError::new(
                GameErrorCode::Unavailable,
                "This operation requires a bound random-duration scheduler.",
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionCadence {
    pub single: SourceBinding<u32>,
    pub first: SourceBinding<u32>,
    pub repeat: SourceBinding<u32>,
    /// Zero explicitly means no extra menu delay.
    pub menu_delay: SourceBinding<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnershipScope {
    Inventory,
    Equipment,
    InventoryAndEquipment,
    Bank,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolCadence {
    pub tool: ItemId,
    pub location: OwnershipScope,
    pub cadence: ActionCadence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatherAlternative {
    pub requirement: SkillRequirement,
    pub chance: ChanceRule,
    pub output: ItemStack,
    pub xp_tenths: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatherMechanics {
    pub method: ActionId,
    pub levels: LevelDomain,
    pub cadence: ActionCadence,
    pub tool_cadences: Vec<ToolCadence>,
    pub respawn: SourceBinding<TickDuration>,
    /// Evaluated in order; the original output is the final fallback roll.
    pub alternatives: Vec<GatherAlternative>,
    pub relocation: Option<SourceBinding<RelocationPolicy>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelocationPolicy {
    pub locations: Vec<SpawnId>,
    pub interval: TickDuration,
    pub exclude_current: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeMechanics {
    pub method: ActionId,
    pub guard: Guard,
    pub chance_skill: Option<SkillId>,
    pub cadence: ActionCadence,
    pub tool_ownership: OwnershipScope,
    pub failed_xp: Vec<XpReward>,
    pub success_effects: Vec<Effect>,
    pub failure_effects: Vec<Effect>,
    pub lifecycle: RecipeLifecycle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecipeLifecycle {
    InventoryConversion,
    ConsumeOnly,
    Firemaking {
        ground_input: ItemId,
        fire: TemporaryObjectId,
        step_priority: Vec<Direction>,
        retain_ground_input_on_failure: bool,
    },
}

/// Ordinary bools keep their existing wire representation. Mode 2 is never coerced to a bool.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Stackability {
    Simple(bool),
    Conditional {
        source_mode: u8,
        rule: SourceBinding<ConditionalStackRule>,
    },
}

impl Stackability {
    pub fn fixed(&self) -> GameResult<bool> {
        match self {
            Self::Simple(value) => Ok(*value),
            Self::Conditional { .. } => Err(GameError::new(
                GameErrorCode::Unavailable,
                "Conditional item stackability requires its source context.",
            )),
        }
    }

    pub fn is_always(&self) -> bool {
        matches!(self, Self::Simple(true))
    }

    pub fn is_never(&self) -> bool {
        matches!(self, Self::Simple(false))
    }
}

impl From<bool> for Stackability {
    fn from(value: bool) -> Self {
        Self::Simple(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerKind {
    Inventory,
    Equipment,
    Bank,
    Ground,
    Grave,
    DeathOffice,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConditionalStackRule {
    pub stack_in: BTreeSet<ContainerKind>,
    pub require_same_origin: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemWeight {
    pub grams: i32,
    pub inventory: WeightContribution,
    pub equipment: WeightContribution,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightContribution {
    None,
    OncePerStack,
    PerUnit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeDefinition {
    pub kind: ChargeKindId,
    pub maximum: u32,
    pub empty_variant: ItemId,
    pub charged_variant: ItemId,
    pub trade_with_charges: bool,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemInstance {
    pub id: ItemInstanceId,
    pub charges: Option<ItemCharges>,
    pub origin: Option<ItemOrigin>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemCharges {
    pub kind: ChargeKindId,
    pub remaining: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeSelection {
    FirstEligibleInventorySlot,
    InteractedInventorySlot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemOrigin {
    pub actor: ActorId,
    pub npc: Option<SpawnId>,
    pub life: Option<u64>,
    pub acquired_at_tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterScope {
    Character,
    World,
    Instance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum CounterValue {
    Boolean(bool),
    Integer(i64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CounterType {
    Boolean,
    Integer { minimum: i64, maximum: i64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceVariable {
    Varp { id: u32 },
    Varbit { id: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CounterDefinition {
    pub id: CounterId,
    pub scope: CounterScope,
    pub value_type: CounterType,
    pub initial: CounterValue,
    pub source_variable: Option<SourceVariable>,
    pub source: Vec<SourceRecord>,
}

impl CounterDefinition {
    pub fn validate_value(&self, value: CounterValue) -> GameResult<()> {
        let valid = match (self.value_type, value) {
            (CounterType::Boolean, CounterValue::Boolean(_)) => true,
            (CounterType::Integer { minimum, maximum }, CounterValue::Integer(value)) => {
                minimum <= maximum && (minimum..=maximum).contains(&value)
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(GameError::new(
                GameErrorCode::InvalidInput,
                format!("Counter {} violates its declared type or bounds.", self.id),
            ))
        }
    }

    pub fn checked_add(&self, value: CounterValue, delta: i64) -> GameResult<CounterValue> {
        self.validate_value(value)?;
        let CounterValue::Integer(value) = value else {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Cannot add to a boolean.",
            ));
        };
        let value = value.checked_add(delta).ok_or_else(|| {
            GameError::new(GameErrorCode::InvalidInput, "Counter arithmetic overflow.")
        })?;
        let result = CounterValue::Integer(value);
        self.validate_value(result)?;
        Ok(result)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CounterPredicate {
    Equals { value: CounterValue },
    IntegerRange { minimum: i64, maximum: i64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantMode {
    Add,
    MissingOnly,
    TopUp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapacityPolicy {
    Atomic,
    OrderedPartial,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantLine {
    pub item: ItemId,
    pub quantity: Quantity,
    pub mode: GrantMode,
    pub ownership: OwnershipScope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantDefinition {
    pub id: GrantId,
    pub target: ContainerKind,
    pub capacity: CapacityPolicy,
    pub lines: Vec<GrantLine>,
    pub entitlement: Option<EntitlementId>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EntitlementPurpose {
    AtomicReward,
    Grant { grant: GrantId },
    Reconciliation { reconciliation: ReconciliationId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntitlementDefinition {
    pub id: EntitlementId,
    pub purpose: EntitlementPurpose,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContainerReconciliation {
    Preserve,
    ReplaceInventory {
        inventory: Box<Inventory>,
    },
    ReplaceEquipment {
        equipment: BTreeMap<SlotId, ItemStack>,
    },
    ReplaceBank {
        slots: Vec<Option<ItemStack>>,
    },
    RemoveItems {
        container: ContainerKind,
        items: Vec<ItemId>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationDefinition {
    pub id: ReconciliationId,
    pub entitlement: EntitlementId,
    pub policies: SourceBinding<Vec<ContainerReconciliation>>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NpcNavigation {
    Mobile {
        wander_radius: u16,
        step_ticks: SourceBinding<u32>,
        clip: NpcClipPolicy,
    },
    Stationary {
        anchor: StationaryAnchor,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcClipPolicy {
    MovementAndActors,
    MovementOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum StationaryAnchor {
    Walkable,
    NonWalkingResource {
        access_tiles: Vec<Tile>,
    },
    /// E.g. a seated/scenery-bound actor, not permission to erase walls.
    SceneryBound {
        object: ObjectId,
        access_tiles: Vec<Tile>,
    },
    ScriptedActor {
        access_tiles: Vec<Tile>,
        source: Vec<SourceRecord>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceObjectPlacement {
    pub shape: u8,
    pub quarter_turns: u8,
    pub layer: ObjectLayer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectLayer {
    Wall,
    WallDecoration,
    GameObject,
    FloorDecoration,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectClipDefinition {
    pub blocks_movement: bool,
    pub blocks_projectiles: bool,
    pub access_blocked_sides: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceObjectMorph {
    pub counter: CounterId,
    pub variants: BTreeMap<i64, Option<ObjectId>>,
    pub fallback: Option<ObjectId>,
    pub collision: Option<SourceBinding<ObjectMorphCollision>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceNpcMorph {
    pub counter: CounterId,
    pub variants: BTreeMap<i64, Option<NpcId>>,
    pub fallback: Option<NpcId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoorPosition {
    Open,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectTransformState {
    pub object: Option<ObjectId>,
    pub tile: Tile,
    pub door: Option<DoorPosition>,
    pub placement: SourceObjectPlacement,
    /// Complete replacements for these explicit cells only, never an empty-map fallback.
    pub collision: Vec<CollisionCell>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectTransformDefinition {
    pub id: ObjectTransformId,
    pub scope: CounterScope,
    pub spawn: SpawnId,
    pub initial: ObjectStateId,
    pub states: BTreeMap<ObjectStateId, ObjectTransformState>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporaryObjectDefinition {
    pub id: TemporaryObjectId,
    pub object: ObjectId,
    pub lifetime: SourceBinding<TickDuration>,
    pub placement_guard: Guard,
    pub interactions: Vec<InteractionDefinition>,
    pub owner_only_use: bool,
    pub blocks_movement: bool,
    pub blocks_projectiles: bool,
    pub expired_items: Vec<ItemStack>,
    pub ground_policy: GroundPolicyId,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroundItemPolicy {
    pub id: GroundPolicyId,
    pub public_after: SourceBinding<Option<u32>>,
    pub expires_after: SourceBinding<Option<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<SourceBinding<GroundClock>>,
    pub owner_can_take: bool,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroundClock {
    WorldTicks,
    OwnerOnlineTicks,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceChunkMapping {
    pub source_region: RegionId,
    pub source_origin: Tile,
    pub destination_region: RegionId,
    pub destination_origin: Tile,
    pub quarter_turns: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstanceTemplateDefinition {
    pub id: InstanceTemplateId,
    pub chunk_size: u8,
    pub chunks: Vec<InstanceChunkMapping>,
    pub private_to_character: bool,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldLocation {
    pub region: RegionId,
    pub tile: Tile,
    pub instance: Option<InstanceTemplateId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TravelDestination {
    Fixed {
        location: WorldLocation,
    },
    Experience {
        branches: BTreeMap<ExperienceId, WorldLocation>,
    },
    PreviousRespawn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterruptionCause {
    Movement,
    Combat,
    AnotherAction,
    Logout,
    TargetLost,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CooldownStart {
    Accepted,
    Launched,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TravelDefinition {
    pub id: TravelId,
    pub guard: Guard,
    pub destination: SourceBinding<TravelDestination>,
    pub channel_ticks: SourceBinding<u32>,
    pub cooldown_ticks: SourceBinding<u32>,
    pub cooldown_start: SourceBinding<CooldownStart>,
    pub interruptions: BTreeSet<InterruptionCause>,
    pub completion_effects: Vec<Effect>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExperienceDefinition {
    pub id: ExperienceId,
    pub name: String,
    pub selection_guard: Guard,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppearanceDefinition {
    pub choices: BTreeMap<String, BTreeSet<u32>>,
    pub confirmation_guard: Guard,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackType {
    Stab,
    Slash,
    Crush,
    Ranged,
    Magic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackMethod {
    Melee,
    Ranged,
    Magic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ratio {
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveLevelFormula {
    pub skill: SkillId,
    pub basis: SkillLevelBasis,
    pub style_bonus: i16,
    pub constant_bonus: i16,
    pub prayer_before_style: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccuracyFormula {
    InclusiveOpposedRolls,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NegativeRollPolicy {
    ClampToZero,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum MaximumHitFormula {
    Strength {
        level: EffectiveLevelFormula,
        equipment_offset: i16,
        additive: u32,
        divisor: u32,
    },
    LevelTable {
        skill: SkillId,
        basis: SkillLevelBasis,
        #[serde(deserialize_with = "crate::numeric_keys::level_hits")]
        hits: BTreeMap<u16, u16>,
    },
    Fixed {
        hit: u16,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamagePolicy {
    pub successful_minimum: u16,
    pub cap_to_remaining_hitpoints: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageXp {
    pub skill: SkillId,
    pub tenths_per_damage: Ratio,
    pub rounding: IntegerRounding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegerRounding {
    Floor,
    NearestTiesUp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatStyleDefinition {
    pub id: CombatStyleId,
    pub method: AttackMethod,
    pub attack_type: AttackType,
    pub attack: EffectiveLevelFormula,
    pub defence: EffectiveLevelFormula,
    pub accuracy: SourceBinding<AccuracyFormula>,
    pub negative_rolls: SourceBinding<NegativeRollPolicy>,
    pub maximum_hit: SourceBinding<MaximumHitFormula>,
    pub damage: SourceBinding<DamagePolicy>,
    pub cycle_ticks: SourceBinding<u32>,
    pub reach: u16,
    pub damage_xp: Vec<DamageXp>,
    pub projectile: Option<ProjectileId>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmmunitionRequirement {
    pub slot: SlotId,
    pub compatible_items: Vec<ItemId>,
    pub per_attack: Quantity,
    pub break_chance: SourceBinding<Ratio>,
    pub ground_policy: GroundPolicyId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponDefinition {
    pub styles: Vec<CombatStyleId>,
    pub default_style: CombatStyleId,
    pub ammunition: Option<AmmunitionRequirement>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SpellAction {
    Combat {
        style: CombatStyleId,
        projectile: ProjectileId,
    },
    Teleport {
        travel: TravelId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpellDefinition {
    pub id: SpellId,
    pub interface: InterfaceId,
    pub requirements: Vec<SkillRequirement>,
    pub guard: Guard,
    pub runes: Vec<ItemStack>,
    pub launch_xp: Vec<XpReward>,
    pub action: SpellAction,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectileTiming {
    pub launch_delay_ticks: u32,
    pub base_flight_ticks: u32,
    pub ticks_per_tile: Ratio,
    pub rounding: IntegerRounding,
    pub damage_on_launch: bool,
    pub recheck_target_on_impact: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectileDefinition {
    pub id: ProjectileId,
    pub timing: SourceBinding<ProjectileTiming>,
    pub asset: Option<AssetId>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KillCreditPolicy {
    MostDamageThenFirstContributor,
    MostDamageThenLastContributor,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NpcCombatMechanics {
    pub attack_type: AttackType,
    pub attack_stat: NpcCombatStat,
    pub defence_stats: BTreeMap<AttackType, NpcCombatStat>,
    pub effective_level_bonus: SourceBinding<i16>,
    pub retaliation: bool,
    pub reach: u16,
    pub accuracy: SourceBinding<AccuracyFormula>,
    pub negative_rolls: SourceBinding<NegativeRollPolicy>,
    pub damage: SourceBinding<DamagePolicy>,
    pub respawn: SourceBinding<TickDuration>,
    pub credit: SourceBinding<KillCreditPolicy>,
    pub attribution: SourceBinding<KillMethodPolicy>,
    pub eligibility: SourceBinding<Vec<AttackEligibility>>,
    pub engagement: SourceBinding<NpcEngagementPolicy>,
    pub loot_ground_policy: SourceBinding<GroundPolicyId>,
    pub loot: Vec<LootPool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpcCombatStat {
    Attack,
    Strength,
    Defence,
    Ranged,
    Magic,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LootEntry {
    pub item: ItemId,
    pub minimum: Quantity,
    pub maximum: Quantity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedLoot {
    pub weight: u32,
    /// An explicit empty list is the no-drop outcome.
    pub items: Vec<LootEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LootPool {
    Guaranteed {
        items: Vec<LootEntry>,
    },
    Exclusive {
        total_weight: u32,
        entries: Vec<WeightedLoot>,
    },
    Independent {
        chance: Ratio,
        items: Vec<LootEntry>,
    },
    Conditional {
        guard: Guard,
        pools: Vec<LootPool>,
    },
    Unresolved {
        reason: String,
        source: Vec<SourceRecord>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillModifier {
    pub skill: SkillId,
    pub multiplier: Ratio,
    pub rounding: IntegerRounding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrayerDrain {
    pub points_per_tick: Ratio,
    /// Effective period multiplier is (offset + prayer bonus) / divisor.
    pub bonus_offset: u32,
    pub bonus_divisor: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrayerDefinition {
    pub id: PrayerId,
    pub interface: InterfaceId,
    pub requirements: Vec<SkillRequirement>,
    pub modifiers: Vec<SkillModifier>,
    pub drain: SourceBinding<PrayerDrain>,
    pub exclusive_with: Vec<PrayerId>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vital {
    Hitpoints,
    Prayer,
    RunEnergy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum VitalRestoration {
    ToBaseMaximum,
    Amount { amount: u16 },
    Set { amount: u16 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClockPause {
    Offline,
    Idle,
    GraveInterface,
    FirstDeathOffice,
    Running,
    Combat,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegenerationPolicy {
    pub vital: Vital,
    pub interval_ticks: SourceBinding<u32>,
    pub amount: u16,
    pub pauses: BTreeSet<ClockPause>,
    pub idle_after_milliseconds: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelUpVitalPolicy {
    PreserveCurrent,
    IncreaseByBaseDifference,
    RestoreToBase,
    RaiseIfAtOldBaseOtherwisePreserve,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VitalPolicy {
    pub hitpoints_skill: SkillId,
    pub prayer_skill: SkillId,
    pub regeneration: Vec<RegenerationPolicy>,
    pub level_up: SourceBinding<LevelUpVitalPolicy>,
    pub food_delay_ticks: SourceBinding<u32>,
    pub food_attack_delay_ticks: SourceBinding<u32>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunDrainFormula {
    pub base: u32,
    pub weight_scale: u32,
    pub weight_minimum_grams: i32,
    pub weight_maximum_grams: i32,
    pub agility_scale: u16,
    pub floor_weight_term_before_agility: bool,
    pub rounding: IntegerRounding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRegenerationFormula {
    pub skill_divisor: u16,
    pub additive_units: u16,
    pub pauses: BTreeSet<ClockPause>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunPolicy {
    pub agility: SkillId,
    pub levels: LevelDomain,
    pub activation_minimum: u16,
    pub disable_on_exhaustion: bool,
    pub drain: SourceBinding<RunDrainFormula>,
    pub regeneration: SourceBinding<RunRegenerationFormula>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StockPriceFormula {
    pub base_per_mille: u32,
    pub change_per_stock: u32,
    pub minimum_per_mille: u32,
    pub maximum_per_mille: u32,
    pub minimum_price: u32,
    pub rounding: IntegerRounding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ShopPricing {
    Fixed,
    StockSensitive {
        buy: StockPriceFormula,
        sell: StockPriceFormula,
        overstock: SourceBinding<OverstockPricing>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverstockPricing {
    LinearToClamp,
    BasePriceAboveBaseStock,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RestockPhase {
    WorldEpoch,
    SinceLastStockChange,
    Explicit { first_tick: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StockRestockRule {
    pub interval_ticks: u32,
    pub amount: Quantity,
    pub phase: SourceBinding<RestockPhase>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShopLineMechanics {
    pub pricing: ShopPricing,
    pub restock: StockRestockRule,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UnstockedShopPolicy {
    Reject,
    Accept {
        maximum_lines: u16,
        rule: ShopLineMechanics,
        base_stock: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathTopic {
    Fees,
    Timer,
    KeptItems,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathValueMethod {
    MaximumExchangeAndAlchemy,
    FixedSourceTable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeathValueProvider {
    pub id: ValueProviderId,
    pub method: DeathValueMethod,
    pub revision: String,
    pub values: SourceBinding<BTreeMap<ItemId, u64>>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionTiePolicy {
    InventoryThenEquipment,
    EquipmentThenInventory,
    StableItemIdThenOriginalSlot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeeBand {
    pub minimum_value: u64,
    pub fee: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RecoveryFee {
    Bands {
        bands: Vec<FeeBand>,
        maximum_total: u64,
    },
    Percentage {
        free_below: u64,
        rate: Ratio,
        rounding: IntegerRounding,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeeSource {
    Coffer,
    Bank,
    Inventory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryOverflow {
    RejectTransfer,
    DeleteOldest,
    DeleteLowestValue,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepeatDeathPolicy {
    pub keep_old_grave_location: bool,
    pub refresh_timer_if_contents_change: bool,
    pub old_unstackable_per_item_limit: u16,
    pub old_items_to_office: Vec<ItemId>,
    pub supply_items: Vec<ItemId>,
    pub supply_ground_policy: GroundPolicyId,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeathPolicy {
    pub domain: DeathDomain,
    pub timing: SourceBinding<DeathTiming>,
    pub interfaces: Option<RecoveryInterfaces>,
    pub value_provider: ValueProviderId,
    pub retained_unskulled: u16,
    pub protect_item_extra: u16,
    pub ties: SourceBinding<RetentionTiePolicy>,
    pub respawn: SourceBinding<WorldLocation>,
    pub first_office: SourceBinding<WorldLocation>,
    pub restoration: SourceBinding<DeathVitalRestoration>,
    pub required_topics: BTreeSet<DeathTopic>,
    pub grave_active_ticks: u32,
    pub grave_pauses: BTreeSet<ClockPause>,
    pub idle_after_milliseconds: u32,
    pub reclaim_range: u16,
    pub require_line_of_sight: bool,
    pub grave_capacity: u16,
    pub office_capacity: u16,
    pub office_overflow: SourceBinding<RecoveryOverflow>,
    pub grave_fee: SourceBinding<RecoveryFee>,
    pub office_fee: SourceBinding<RecoveryFee>,
    pub payment_order: Vec<FeeSource>,
    pub currency: ItemId,
    pub repeat: SourceBinding<RepeatDeathPolicy>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathDomain {
    NormalUnsafeNonPvp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeathVitalRestoration {
    pub on_arrival: BTreeMap<Vital, VitalRestoration>,
    pub on_first_office_exit: BTreeMap<Vital, VitalRestoration>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct MechanicsDefinition {
    pub world_members: Option<bool>,
    pub player_drop: Option<PlayerDropPolicy>,
    pub player_combat: Option<PlayerCombatPolicy>,
    pub traversal: BTreeMap<TraversalId, TraversalDefinition>,
    pub collision_groups: BTreeMap<CollisionGroupId, CollisionGroupDefinition>,
    pub counters: BTreeMap<CounterId, CounterDefinition>,
    pub grants: BTreeMap<GrantId, GrantDefinition>,
    pub entitlements: BTreeMap<EntitlementId, EntitlementDefinition>,
    pub reconciliations: BTreeMap<ReconciliationId, ReconciliationDefinition>,
    pub object_transforms: BTreeMap<ObjectTransformId, ObjectTransformDefinition>,
    pub temporary_objects: BTreeMap<TemporaryObjectId, TemporaryObjectDefinition>,
    pub ground_policies: BTreeMap<GroundPolicyId, GroundItemPolicy>,
    pub instances: BTreeMap<InstanceTemplateId, InstanceTemplateDefinition>,
    pub travels: BTreeMap<TravelId, TravelDefinition>,
    pub experiences: BTreeMap<ExperienceId, ExperienceDefinition>,
    pub appearance: Option<AppearanceDefinition>,
    pub combat_styles: BTreeMap<CombatStyleId, CombatStyleDefinition>,
    pub spells: BTreeMap<SpellId, SpellDefinition>,
    pub projectiles: BTreeMap<ProjectileId, ProjectileDefinition>,
    pub prayers: BTreeMap<PrayerId, PrayerDefinition>,
    pub run: Option<RunPolicy>,
    pub vitals: Option<VitalPolicy>,
    pub death: Option<DeathPolicy>,
    pub value_providers: BTreeMap<ValueProviderId, DeathValueProvider>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels() -> LevelDomain {
        LevelDomain {
            minimum: 1,
            maximum: 99,
            basis: SkillLevelBasis::Current,
        }
    }

    #[test]
    fn source_chance_keeps_unclamped_endpoints_rounding_and_exact_domains() {
        let copper = ChanceRule::source_skilling(100, 350, levels()).unwrap();
        assert_eq!(
            (
                copper.numerator_at_level_1,
                copper.numerator_at_level_99,
                copper.denominator
            ),
            (101, 351, 256)
        );
        let shrimp = ChanceRule::source_skilling(48, 256, levels()).unwrap();
        assert_eq!(
            (
                shrimp.numerator_at_level_1,
                shrimp.numerator_at_level_99,
                shrimp.denominator
            ),
            (49, 257, 256)
        );
        for (rule, low, high) in [(&copper, 100_u64, 350_u64), (&shrimp, 48, 256)] {
            for level in 1..=99 {
                let expected = (1
                    + (low * u64::from(99 - level) + high * u64::from(level - 1) + 49) / 98)
                    .min(256);
                assert_eq!(u64::from(rule.numerator(level).unwrap()), expected);
            }
            assert!(rule.numerator(0).is_err());
            assert!(rule.numerator(100).is_err());
        }
        assert_eq!(copper.numerator(2).unwrap(), 104);
        assert_eq!(shrimp.numerator(1).unwrap(), 49);
        assert!(ChanceRule::constant(257, 256).validate().is_err());
        assert!(ChanceRule::source_skilling(u32::MAX, 1, levels()).is_err());
        let mut limited = shrimp;
        limited.domain = ChanceDomain::Skill {
            levels: LevelDomain {
                maximum: 14,
                ..levels()
            },
        };
        assert!(limited.numerator(15).is_err());
        assert_eq!(ChanceRule::constant(0, 256).numerator(0).unwrap(), 0);
    }

    #[test]
    fn counters_enforce_type_scope_bounds_and_checked_arithmetic() {
        let counter = CounterDefinition {
            id: CounterId::new("counter.mill.flour").unwrap(),
            scope: CounterScope::Character,
            value_type: CounterType::Integer {
                minimum: 0,
                maximum: 30,
            },
            initial: CounterValue::Integer(0),
            source_variable: None,
            source: vec![],
        };
        assert_eq!(
            counter.checked_add(CounterValue::Integer(29), 1).unwrap(),
            CounterValue::Integer(30)
        );
        assert!(counter.checked_add(CounterValue::Integer(30), 1).is_err());
        assert!(counter.checked_add(CounterValue::Integer(0), -1).is_err());
        assert!(
            counter
                .validate_value(CounterValue::Boolean(false))
                .is_err()
        );
        let wide = CounterDefinition {
            value_type: CounterType::Integer {
                minimum: i64::MIN,
                maximum: i64::MAX,
            },
            ..counter
        };
        assert!(
            wide.checked_add(CounterValue::Integer(i64::MAX), 1)
                .is_err()
        );
        assert!(
            wide.checked_add(CounterValue::Integer(i64::MIN), -1)
                .is_err()
        );
    }

    #[test]
    fn conditional_stackability_is_not_a_bool_and_unresolved_bindings_fail_explicitly() {
        let conditional = Stackability::Conditional {
            source_mode: 2,
            rule: SourceBinding::Unresolved {
                reason: "Source mode-2 contexts remain unbound.".into(),
                source: vec![],
            },
        };
        assert_eq!(
            serde_json::from_str::<Stackability>("true").unwrap(),
            Stackability::Simple(true)
        );
        let encoded = serde_json::to_string(&conditional).unwrap();
        assert_eq!(
            serde_json::from_str::<Stackability>(&encoded).unwrap(),
            conditional
        );
        assert_eq!(
            conditional.fixed().unwrap_err().code,
            GameErrorCode::Unavailable
        );
        let missing: SourceBinding<u32> = SourceBinding::Unresolved {
            reason: "Source timer not captured.".into(),
            source: vec![],
        };
        assert_eq!(
            missing.require().unwrap_err().code,
            GameErrorCode::Unavailable
        );
    }
}

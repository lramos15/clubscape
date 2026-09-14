use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    VerifiedReference,
    Inference,
    ApprovedAdaptation,
    TestFixture,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRecord {
    pub reference: String,
    pub revision: String,
    pub status: EvidenceStatus,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRequirement {
    pub skill: SkillId,
    pub level: u16,
    pub basis: SkillLevelBasis,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CombatBonuses {
    pub attack: BTreeMap<AttackType, i16>,
    pub defence: BTreeMap<AttackType, i16>,
    pub melee_strength: i16,
    pub ranged_strength: i16,
    pub magic_damage_percent: i16,
    pub prayer: i16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentDefinition {
    pub slot: SlotId,
    pub occupied_slots: Vec<SlotId>,
    pub requirements: Vec<SkillRequirement>,
    pub bonuses: CombatBonuses,
    pub attack_speed_ticks: Option<u16>,
    pub attack_styles: Vec<String>,
    pub weapon: Option<WeaponDefinition>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemDefinition {
    pub id: ItemId,
    pub name: String,
    pub source_id: Option<u32>,
    pub stackable: Stackability,
    pub tradable: bool,
    pub base_value: u32,
    pub equipment: Option<EquipmentDefinition>,
    pub noted_variant: Option<ItemId>,
    pub unnoted_variant: Option<ItemId>,
    pub healing: Option<u16>,
    pub weight: Option<SourceBinding<ItemWeight>>,
    pub charges: Option<ChargeDefinition>,
    pub asset: Option<AssetId>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub id: SkillId,
    pub name: String,
    pub source_id: u16,
    pub xp_thresholds_tenths: Vec<u64>,
    pub maximum_xp_tenths: u64,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollisionCell {
    pub tile: Tile,
    pub height: i32,
    pub walkable: bool,
    pub blocked_movement: u8,
    pub blocked_sight: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionDefinition {
    pub id: RegionId,
    pub name: String,
    pub min: Tile,
    pub max: Tile,
    pub cells: Vec<CollisionCell>,
    pub source_map_squares: Vec<u32>,
    pub scene_asset: Option<AssetId>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpawnKind {
    Npc {
        npc: NpcId,
    },
    Object {
        object: ObjectId,
    },
    Item {
        stack: ItemStack,
        respawn_ticks: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnDefinition {
    pub id: SpawnId,
    pub region: RegionId,
    pub tile: Tile,
    pub facing: u8,
    pub placement: Option<SourceObjectPlacement>,
    pub kind: SpawnKind,
    pub interactions: Vec<InteractionDefinition>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractionDefinition {
    pub name: String,
    pub reach: u16,
    pub guard: Guard,
    pub action: InteractionAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InteractionAction {
    Effects {
        effects: Vec<Effect>,
    },
    Dialogue {
        dialogue: DialogueId,
    },
    Gather {
        rule: Box<GatherRule>,
    },
    Production {
        recipes: Vec<RecipeId>,
    },
    Bank,
    Shop {
        shop: ShopId,
    },
    Attack,
    Travel {
        destination: Tile,
        region: RegionId,
    },
    OpenBank {
        interface: InterfaceId,
        before_open: Vec<Effect>,
    },
    OpenShop {
        shop: ShopId,
        interface: InterfaceId,
        before_open: Vec<Effect>,
    },
    TravelVia {
        travel: TravelId,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatherRule {
    pub skill: SkillId,
    pub required_level: u16,
    pub tools: Vec<ItemId>,
    pub output: ItemStack,
    pub xp_tenths: u64,
    /// Legacy uniform cadence; must be None when mechanics is present.
    pub attempt_ticks: Option<u16>,
    pub success: ChanceRule,
    pub depletion: ChanceRule,
    pub respawn_ticks: Option<u32>,
    pub mechanics: Option<GatherMechanics>,
    pub animation: Option<AssetId>,
    pub sound: Option<AssetId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChanceRule {
    pub numerator_at_level_1: u32,
    pub numerator_at_level_99: u32,
    pub denominator: u32,
    pub domain: ChanceDomain,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeDefinition {
    pub id: RecipeId,
    pub name: String,
    pub inputs: Vec<ItemStack>,
    pub outputs: Vec<ItemStack>,
    pub failed_outputs: Vec<ItemStack>,
    #[serde(default)]
    pub tools: Vec<ItemId>,
    pub requirements: Vec<SkillRequirement>,
    pub xp: Vec<XpReward>,
    /// Legacy uniform cadence; must be None when mechanics is present.
    pub ticks: Option<u16>,
    pub success: ChanceRule,
    pub target_objects: Vec<ObjectId>,
    pub mechanics: Option<RecipeMechanics>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XpReward {
    pub skill: SkillId,
    pub amount_tenths: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Guard {
    Always,
    All {
        guards: Vec<Guard>,
    },
    Any {
        guards: Vec<Guard>,
    },
    Not {
        guard: Box<Guard>,
    },
    Flag {
        name: String,
        equals: i64,
    },
    TutorialStage {
        stage: StageId,
    },
    QuestStage {
        quest: QuestId,
        stage: StageId,
    },
    HasItems {
        items: Vec<ItemStack>,
    },
    Equipped {
        item: ItemId,
    },
    SkillAtLeast {
        requirement: SkillRequirement,
    },
    InterfaceUnlocked {
        interface: InterfaceId,
    },
    Within {
        tile: Tile,
        distance: u16,
    },
    FreeCapacity {
        container: ContainerKind,
        slots: u16,
    },
    OwnsItems {
        items: Vec<ItemStack>,
        scope: OwnershipScope,
    },
    Counter {
        counter: CounterId,
        predicate: CounterPredicate,
    },
    EntitlementClaimed {
        entitlement: EntitlementId,
    },
    Event {
        condition: EventCondition,
    },
    Experience {
        experience: ExperienceId,
    },
    Setting {
        setting: CharacterSetting,
    },
    Life {
        phase: LifePhase,
    },
    DeathTopics {
        topics: std::collections::BTreeSet<DeathTopic>,
    },
    Charges {
        item: ItemId,
        charge_kind: ChargeKindId,
        minimum: u32,
    },
    MembersWorld,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    GiveItems {
        items: Vec<ItemStack>,
    },
    TakeItems {
        items: Vec<ItemStack>,
    },
    AwardXp {
        rewards: Vec<XpReward>,
    },
    SetFlag {
        name: String,
        value: i64,
    },
    UnlockInterface {
        interface: InterfaceId,
    },
    SetTutorialStage {
        stage: StageId,
    },
    SetQuestStage {
        quest: QuestId,
        stage: StageId,
    },
    AddQuestPoints {
        amount: u16,
    },
    Travel {
        region: RegionId,
        tile: Tile,
    },
    Message {
        text: String,
    },
    Conditional {
        guard: Guard,
        effects: Vec<Effect>,
    },
    Grant {
        grant: GrantId,
    },
    Once {
        entitlement: EntitlementId,
        effects: Vec<Effect>,
    },
    RestoreVital {
        vital: Vital,
        restoration: VitalRestoration,
    },
    SetCounter {
        counter: CounterId,
        value: CounterValue,
    },
    AddCounter {
        counter: CounterId,
        delta: i64,
    },
    TransformObject {
        transform: ObjectTransformId,
        state: ObjectStateId,
    },
    CreateTemporaryObject {
        definition: TemporaryObjectId,
    },
    TravelVia {
        travel: TravelId,
    },
    ReconcileContainers {
        reconciliation: ReconciliationId,
    },
    CompleteDeathTopic {
        topic: DeathTopic,
    },
    ConsumeCharges {
        item: ItemId,
        charge_kind: ChargeKindId,
        amount: u32,
        selection: ChargeSelection,
    },
    Inspect {
        target: SpawnId,
        explanation: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogueChoice {
    pub id: String,
    pub text: String,
    pub guard: Guard,
    pub effects: Vec<Effect>,
    pub next_node: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogueNode {
    pub id: String,
    pub text: String,
    pub guard: Guard,
    pub choices: Vec<DialogueChoice>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogueDefinition {
    pub id: DialogueId,
    pub nodes: Vec<DialogueNode>,
    pub entry_nodes: Vec<String>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressTransition {
    pub event: String,
    pub target: Option<String>,
    pub guard: Guard,
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TutorialStageDefinition {
    pub id: StageId,
    pub instruction: String,
    pub allowed_actions: Vec<String>,
    pub xp_caps_tenths: BTreeMap<SkillId, u64>,
    pub xp_stop_levels: BTreeMap<SkillId, u16>,
    pub nonfatal_combat: bool,
    pub transitions: Vec<ProgressTransition>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestDefinition {
    pub id: QuestId,
    pub name: String,
    pub initial_stage: StageId,
    pub completed_stage: StageId,
    pub journal: BTreeMap<StageId, String>,
    pub transitions: Vec<ProgressTransition>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopItem {
    pub item: ItemId,
    pub base_stock: u32,
    pub restock_ticks: u32,
    pub buy_price: u32,
    pub sell_price: u32,
    pub mechanics: Option<ShopLineMechanics>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopDefinition {
    pub id: ShopId,
    pub name: String,
    pub currency: ItemId,
    pub stock: Vec<ShopItem>,
    pub accepts_general_items: bool,
    pub unstocked: Option<UnstockedShopPolicy>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectDefinition {
    pub id: ObjectId,
    pub name: String,
    pub source_id: u32,
    pub size_x: u8,
    pub size_y: u8,
    pub clip: Option<ObjectClipDefinition>,
    pub morph: Option<SourceObjectMorph>,
    pub asset: Option<AssetId>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcDefinition {
    pub id: NpcId,
    pub name: String,
    pub source_id: u32,
    pub size: u8,
    pub navigation: NpcNavigation,
    pub morph: Option<SourceNpcMorph>,
    pub combat: Option<NpcCombatDefinition>,
    pub asset: Option<AssetId>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcCombatDefinition {
    pub hitpoints: u16,
    pub attack: u16,
    pub strength: u16,
    pub defence: u16,
    pub ranged: u16,
    pub magic: u16,
    pub attack_speed_ticks: u16,
    pub max_hit: u16,
    pub bonuses: CombatBonuses,
    pub respawn_ticks: Option<u32>,
    pub aggressive: bool,
    pub drops: Vec<DropDefinition>,
    /// When present, legacy independent drops must be empty.
    pub mechanics: Option<NpcCombatMechanics>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DropDefinition {
    pub item: ItemId,
    pub minimum_quantity: u32,
    pub maximum_quantity: u32,
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitialStateDefinition {
    pub region: RegionId,
    pub tile: Tile,
    pub inventory: Inventory,
    pub equipment: BTreeMap<SlotId, ItemStack>,
    pub bank: Bank,
    pub skills: BTreeMap<SkillId, SkillState>,
    pub hitpoints: u16,
    pub prayer_points: u16,
    pub run_energy: u16,
    pub tutorial_stage: StageId,
    pub quest_points: u32,
    pub quests: BTreeMap<QuestId, QuestState>,
    pub flags: BTreeMap<String, i64>,
    pub interfaces: Vec<InterfaceId>,
    pub runtime: InitialRuntimeDefinition,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameContent {
    pub schema_version: u32,
    pub revision: String,
    pub baseline: String,
    pub items: BTreeMap<ItemId, ItemDefinition>,
    pub skills: BTreeMap<SkillId, SkillDefinition>,
    pub regions: BTreeMap<RegionId, RegionDefinition>,
    pub spawns: BTreeMap<SpawnId, SpawnDefinition>,
    pub objects: BTreeMap<ObjectId, ObjectDefinition>,
    pub npcs: BTreeMap<NpcId, NpcDefinition>,
    pub recipes: BTreeMap<RecipeId, RecipeDefinition>,
    pub dialogues: BTreeMap<DialogueId, DialogueDefinition>,
    pub tutorial: BTreeMap<StageId, TutorialStageDefinition>,
    pub quests: BTreeMap<QuestId, QuestDefinition>,
    pub shops: BTreeMap<ShopId, ShopDefinition>,
    #[serde(default)]
    pub interfaces: BTreeMap<InterfaceId, InterfaceDefinition>,
    pub equipment_slots: Vec<SlotId>,
    pub initial_state: InitialStateDefinition,
    pub mechanics: MechanicsDefinition,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceDefinition {
    pub id: InterfaceId,
    pub name: String,
    pub access: InterfaceAccess,
    pub source_ids: Vec<u32>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterfaceAccess {
    Tab,
    Contextual,
}

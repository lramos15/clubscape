use serde::{Deserialize, Serialize};

use crate::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GameIntent {
    Walk {
        destination: Tile,
        running: bool,
    },
    Interact {
        target: SpawnId,
        action: String,
    },
    SelectDialogue {
        speaker: SpawnId,
        choice: String,
    },
    OpenInterface {
        interface: InterfaceId,
    },
    CloseInterface,
    Equip {
        inventory_slot: u8,
    },
    Unequip {
        slot: SlotId,
    },
    Drop {
        inventory_slot: u8,
        quantity: Quantity,
    },
    TakeGroundItem {
        ground_item_id: String,
    },
    UseItem {
        inventory_slot: u8,
        target: ItemTarget,
    },
    MoveInventory {
        from: u8,
        to: u8,
    },
    Eat {
        inventory_slot: u8,
    },
    Produce {
        recipe: RecipeId,
        target: Option<SpawnId>,
        quantity: Quantity,
    },
    ProduceAt {
        recipe: RecipeId,
        target: Option<WorldTarget>,
        quantity: Quantity,
    },
    ProduceSelected {
        recipe: RecipeId,
        target: Option<WorldTarget>,
        quantity: Quantity,
        mode: ProductionMode,
    },
    InteractWith {
        target: WorldTarget,
        action: String,
    },
    BankDeposit {
        banker: SpawnId,
        inventory_slot: u8,
        quantity: Quantity,
    },
    BankWithdraw {
        banker: SpawnId,
        bank_slot: u16,
        quantity: Quantity,
        noted: bool,
    },
    ShopBuy {
        shop: ShopId,
        item_index: u16,
        quantity: Quantity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_item: Option<ItemId>,
    },
    ShopSell {
        shop: ShopId,
        inventory_slot: u8,
        quantity: Quantity,
    },
    SetCombatStyle {
        style: String,
    },
    Cast {
        spell: String,
        target: Option<SpawnId>,
    },
    SetPrayer {
        prayer: String,
        enabled: bool,
    },
    SetSetting {
        setting: CharacterSetting,
    },
    ConfirmAppearance {
        appearance: std::collections::BTreeMap<String, u32>,
    },
    SelectExperience {
        experience: ExperienceId,
    },
    Reclaim {
        death: DeathId,
        storage: RecoveryStorage,
        items: Vec<RecoveryItemId>,
    },
    OpenGrave {
        death: DeathId,
    },
    OpenDeathOffice,
    CancelActivity,
    RequestLogout,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemTarget {
    Inventory { slot: u8 },
    World { spawn: SpawnId },
    TemporaryObject { object: DynamicObjectId },
    Ground { ground_item_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorldTarget {
    Spawn { spawn: SpawnId },
    TemporaryObject { object: DynamicObjectId },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GameEvent {
    Moved {
        tile: Tile,
    },
    Interacted {
        target: SpawnId,
        action: String,
    },
    DialogueSelected {
        speaker: SpawnId,
        choice: String,
    },
    InterfaceOpened {
        interface: InterfaceId,
    },
    Gathered {
        target: SpawnId,
        stack: ItemStack,
    },
    Produced {
        recipe: RecipeId,
        outputs: Vec<ItemStack>,
    },
    Equipped {
        slot: SlotId,
        stack: ItemStack,
    },
    XpGained {
        skill: SkillId,
        amount_tenths: u64,
    },
    Hit {
        target: SpawnId,
        damage: u16,
        style: String,
    },
    Defeated {
        target: SpawnId,
        style: String,
    },
    Died,
    Recovered,
    TutorialAdvanced {
        stage: StageId,
    },
    QuestAdvanced {
        quest: QuestId,
        stage: StageId,
    },
    Message {
        text: String,
    },
    Sound {
        asset: String,
    },
    Animation {
        target: String,
        animation: String,
    },
    AppearanceConfirmed,
    ExperienceSelected {
        experience: ExperienceId,
    },
    InterfaceClosed {
        interface: InterfaceId,
    },
    InterfacePresented {
        interface: InterfaceId,
        context: InterfaceContext,
    },
    SettingChanged {
        setting: CharacterSetting,
    },
    Inspected {
        target: SpawnId,
        explanation: String,
    },
    ProductionResolved {
        recipe: RecipeId,
        method: ActionId,
        facility: Option<WorldTarget>,
        outcome: ProductionOutcome,
        outputs: Vec<ItemStack>,
    },
    CombatResolved {
        target: SpawnId,
        life: u64,
        style: CombatStyleId,
        method: AttackMethod,
        outcome: CombatOutcome,
        damage: u16,
    },
    NpcKilled {
        target: SpawnId,
        npc: NpcId,
        life: u64,
        method: AttackMethod,
        credited: bool,
        tile: Tile,
    },
    SpellResolved {
        spell: SpellId,
        target: SpawnId,
        outcome: SpellOutcome,
        damage: u16,
        tile: Tile,
    },
    Teleport {
        travel: TravelId,
        phase: TeleportPhase,
    },
    ItemTransferred {
        from: ContainerKind,
        to: ContainerKind,
        items: Vec<ItemStack>,
    },
    FoodEaten {
        item: ItemId,
        healed: u16,
    },
    PrayerChanged {
        prayer: PrayerId,
        enabled: bool,
    },
    TemporaryObjectCreated {
        object: DynamicObjectId,
        definition: TemporaryObjectId,
        tile: Tile,
    },
    ObjectTransformed {
        transform: ObjectTransformId,
        state: ObjectStateId,
    },
    CounterChanged {
        counter: CounterId,
        value: CounterValue,
    },
    DeathOccurred {
        death: DeathId,
        items_lost: bool,
    },
    DeathTopicCompleted {
        topic: DeathTopic,
    },
    RecoveryCompleted {
        death: DeathId,
        storage: RecoveryStorage,
        items: Vec<RecoveryItemId>,
        fee: u64,
    },
    GraveExpired {
        death: DeathId,
    },
}

impl GameEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Moved { .. } => "moved",
            Self::Interacted { .. } => "interacted",
            Self::DialogueSelected { .. } => "dialogue_selected",
            Self::InterfaceOpened { .. } => "interface_opened",
            Self::Gathered { .. } => "gathered",
            Self::Produced { .. } => "produced",
            Self::Equipped { .. } => "equipped",
            Self::XpGained { .. } => "xp_gained",
            Self::Hit { .. } => "hit",
            Self::Defeated { .. } => "defeated",
            Self::Died => "died",
            Self::Recovered => "recovered",
            Self::TutorialAdvanced { .. } => "tutorial_advanced",
            Self::QuestAdvanced { .. } => "quest_advanced",
            Self::Message { .. } => "message",
            Self::Sound { .. } => "sound",
            Self::Animation { .. } => "animation",
            Self::AppearanceConfirmed => "appearance_confirmed",
            Self::ExperienceSelected { .. } => "experience_selected",
            Self::InterfaceClosed { .. } => "interface_closed",
            Self::InterfacePresented { .. } => "interface_presented",
            Self::SettingChanged { .. } => "setting_changed",
            Self::Inspected { .. } => "inspected",
            Self::ProductionResolved { .. } => "production_resolved",
            Self::CombatResolved { .. } => "combat_resolved",
            Self::NpcKilled { .. } => "npc_killed",
            Self::SpellResolved { .. } => "spell_resolved",
            Self::Teleport { .. } => "teleport",
            Self::ItemTransferred { .. } => "item_transferred",
            Self::FoodEaten { .. } => "food_eaten",
            Self::PrayerChanged { .. } => "prayer_changed",
            Self::TemporaryObjectCreated { .. } => "temporary_object_created",
            Self::ObjectTransformed { .. } => "object_transformed",
            Self::CounterChanged { .. } => "counter_changed",
            Self::DeathOccurred { .. } => "death_occurred",
            Self::DeathTopicCompleted { .. } => "death_topic_completed",
            Self::RecoveryCompleted { .. } => "recovery_completed",
            Self::GraveExpired { .. } => "grave_expired",
        }
    }

    pub fn primary_target(&self) -> Option<&str> {
        match self {
            Self::Interacted { target, .. }
            | Self::Gathered { target, .. }
            | Self::Hit { target, .. }
            | Self::Defeated { target, .. } => Some(target.as_str()),
            Self::DialogueSelected { speaker, .. } => Some(speaker.as_str()),
            Self::InterfaceOpened { interface } => Some(interface.as_str()),
            Self::Produced { recipe, .. } => Some(recipe.as_str()),
            Self::Equipped { slot, .. } => Some(slot.as_str()),
            Self::XpGained { skill, .. } => Some(skill.as_str()),
            Self::TutorialAdvanced { stage } => Some(stage.as_str()),
            Self::QuestAdvanced { quest, .. } => Some(quest.as_str()),
            Self::Sound { asset } => Some(asset),
            Self::Animation { target, .. } => Some(target),
            Self::ExperienceSelected { experience } => Some(experience.as_str()),
            Self::InterfaceClosed { interface } | Self::InterfacePresented { interface, .. } => {
                Some(interface.as_str())
            }
            Self::Inspected { target, .. }
            | Self::CombatResolved { target, .. }
            | Self::NpcKilled { target, .. } => Some(target.as_str()),
            Self::ProductionResolved { recipe, .. } => Some(recipe.as_str()),
            Self::SpellResolved { spell, .. } => Some(spell.as_str()),
            Self::Teleport { travel, .. } => Some(travel.as_str()),
            Self::FoodEaten { item, .. } => Some(item.as_str()),
            Self::PrayerChanged { prayer, .. } => Some(prayer.as_str()),
            Self::TemporaryObjectCreated { definition, .. } => Some(definition.as_str()),
            Self::ObjectTransformed { transform, .. } => Some(transform.as_str()),
            Self::CounterChanged { counter, .. } => Some(counter.as_str()),
            Self::Moved { .. }
            | Self::Died
            | Self::Recovered
            | Self::Message { .. }
            | Self::AppearanceConfirmed
            | Self::SettingChanged { .. }
            | Self::ItemTransferred { .. }
            | Self::DeathOccurred { .. }
            | Self::DeathTopicCompleted { .. }
            | Self::RecoveryCompleted { .. }
            | Self::GraveExpired { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionOutcome {
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CombatOutcome {
    Hit,
    Miss,
    Invalidated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellOutcome {
    Hit,
    Splash,
    Invalidated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStorage {
    Grave,
    DeathOffice,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InterfaceContext {
    Tab,
    Bank { banker: SpawnId },
    Shop { spawn: SpawnId, shop: ShopId },
    Grave { death: DeathId },
    DeathOffice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeleportPhaseKind {
    Started,
    Interrupted,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TeleportPhase {
    Started,
    Interrupted {
        reason: InterruptionCause,
    },
    Completed {
        origin: RuntimeLocation,
        destination: RuntimeLocation,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EventCondition {
    Interaction {
        target: SpawnId,
        action: String,
    },
    DialogueChoice {
        speaker: SpawnId,
        choice: String,
    },
    Production {
        recipe: RecipeId,
        method: ActionId,
        facility: Option<SpawnId>,
        outcome: ProductionOutcome,
        output: Option<ItemId>,
    },
    Combat {
        style: CombatStyleId,
        outcome: CombatOutcome,
    },
    Kill {
        npc: NpcId,
        method: AttackMethod,
        credited: bool,
    },
    Spell {
        spell: SpellId,
        target: Option<SpawnId>,
        outcomes: std::collections::BTreeSet<SpellOutcome>,
    },
    Interface {
        interface: InterfaceId,
        context: InterfaceContext,
    },
    Teleport {
        travel: TravelId,
        phase: TeleportPhaseKind,
    },
    Setting {
        setting: CharacterSetting,
    },
    Inspection {
        target: SpawnId,
        explanation: String,
    },
    Death {
        items_lost: bool,
    },
    Recovery {
        storage: RecoveryStorage,
    },
    DeathTopic {
        topic: DeathTopic,
    },
}

impl EventCondition {
    pub fn primary_target(&self) -> Option<&str> {
        match self {
            Self::Interaction { target, .. } | Self::Inspection { target, .. } => {
                Some(target.as_str())
            }
            Self::DialogueChoice { speaker, .. } => Some(speaker.as_str()),
            Self::Production { recipe, .. } => Some(recipe.as_str()),
            Self::Spell { spell, .. } => Some(spell.as_str()),
            Self::Interface { interface, .. } => Some(interface.as_str()),
            Self::Teleport { travel, .. } => Some(travel.as_str()),
            _ => None,
        }
    }

    pub fn event_kind(&self) -> &'static str {
        match self {
            Self::Interaction { .. } => "interacted",
            Self::DialogueChoice { .. } => "dialogue_selected",
            Self::Production { .. } => "production_resolved",
            Self::Combat { .. } => "combat_resolved",
            Self::Kill { .. } => "npc_killed",
            Self::Spell { .. } => "spell_resolved",
            Self::Interface { .. } => "interface_presented",
            Self::Teleport { .. } => "teleport",
            Self::Setting { .. } => "setting_changed",
            Self::Inspection { .. } => "inspected",
            Self::Death { .. } => "death_occurred",
            Self::Recovery { .. } => "recovery_completed",
            Self::DeathTopic { .. } => "death_topic_completed",
        }
    }

    pub fn matches(&self, event: &GameEvent) -> bool {
        match (self, event) {
            (
                Self::Interaction { target, action },
                GameEvent::Interacted {
                    target: actual,
                    action: name,
                },
            ) => target == actual && action == name,
            (
                Self::DialogueChoice { speaker, choice },
                GameEvent::DialogueSelected {
                    speaker: actual,
                    choice: name,
                },
            ) => speaker == actual && choice == name,
            (
                Self::Production {
                    recipe,
                    method,
                    facility,
                    outcome,
                    output,
                },
                GameEvent::ProductionResolved {
                    recipe: actual,
                    method: actual_method,
                    facility: actual_facility,
                    outcome: result,
                    outputs,
                },
            ) => {
                recipe == actual
                    && method == actual_method
                    && facility
                        .as_ref()
                        .is_none_or(|facility| matches!(actual_facility, Some(WorldTarget::Spawn { spawn }) if spawn == facility))
                    && outcome == result
                    && output
                        .as_ref()
                        .is_none_or(|item| outputs.iter().any(|stack| &stack.item == item))
            }
            (
                Self::Combat { style, outcome },
                GameEvent::CombatResolved {
                    style: actual,
                    outcome: result,
                    ..
                },
            ) => style == actual && outcome == result,
            (
                Self::Kill {
                    npc,
                    method,
                    credited,
                },
                GameEvent::NpcKilled {
                    npc: actual,
                    method: actual_method,
                    credited: actual_credit,
                    ..
                },
            ) => npc == actual && method == actual_method && credited == actual_credit,
            (
                Self::Spell {
                    spell,
                    target,
                    outcomes,
                },
                GameEvent::SpellResolved {
                    spell: actual,
                    target: actual_target,
                    outcome,
                    ..
                },
            ) => {
                spell == actual
                    && target.as_ref().is_none_or(|target| target == actual_target)
                    && outcomes.contains(outcome)
            }
            (
                Self::Interface { interface, context },
                GameEvent::InterfacePresented {
                    interface: actual,
                    context: actual_context,
                },
            ) => interface == actual && context == actual_context,
            (
                Self::Teleport { travel, phase },
                GameEvent::Teleport {
                    travel: actual,
                    phase: actual_phase,
                },
            ) => {
                travel == actual
                    && matches!(
                        (phase, actual_phase),
                        (TeleportPhaseKind::Started, TeleportPhase::Started)
                            | (
                                TeleportPhaseKind::Interrupted,
                                TeleportPhase::Interrupted { .. }
                            )
                            | (
                                TeleportPhaseKind::Completed,
                                TeleportPhase::Completed { .. }
                            )
                    )
            }
            (Self::Setting { setting }, GameEvent::SettingChanged { setting: actual }) => {
                setting == actual
            }
            (
                Self::Inspection {
                    target,
                    explanation,
                },
                GameEvent::Inspected {
                    target: actual,
                    explanation: actual_explanation,
                },
            ) => target == actual && explanation == actual_explanation,
            (
                Self::Death { items_lost },
                GameEvent::DeathOccurred {
                    items_lost: actual, ..
                },
            ) => items_lost == actual,
            (
                Self::Recovery { storage },
                GameEvent::RecoveryCompleted {
                    storage: actual, ..
                },
            ) => storage == actual,
            (Self::DeathTopic { topic }, GameEvent::DeathTopicCompleted { topic: actual }) => {
                topic == actual
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tile;

    #[test]
    fn shop_buy_absent_or_null_identity_preserves_legacy_json_bytes() {
        let legacy = r#"{"kind":"shop_buy","shop":"shop.example","item_index":2,"quantity":50}"#;
        let intent = GameIntent::ShopBuy {
            shop: ShopId::new("shop.example").unwrap(),
            item_index: 2,
            quantity: Quantity::new(50).unwrap(),
            expected_item: None,
        };
        assert_eq!(serde_json::from_str::<GameIntent>(legacy).unwrap(), intent);
        assert_eq!(serde_json::to_vec(&intent).unwrap(), legacy.as_bytes());

        let mut with_null: serde_json::Value = serde_json::from_str(legacy).unwrap();
        with_null["expected_item"] = serde_json::Value::Null;
        let restored: GameIntent = serde_json::from_value(with_null).unwrap();
        assert_eq!(restored, intent);
        assert_eq!(serde_json::to_vec(&restored).unwrap(), legacy.as_bytes());
    }

    #[test]
    fn shop_buy_expected_identity_roundtrips_and_rejects_invalid_item_ids() {
        let json = r#"{"kind":"shop_buy","shop":"shop.example","item_index":2,"quantity":1,"expected_item":"item.tin"}"#;
        let intent: GameIntent = serde_json::from_str(json).unwrap();
        assert!(matches!(
            &intent,
            GameIntent::ShopBuy { expected_item: Some(item), .. } if item.as_str() == "item.tin"
        ));
        assert_eq!(serde_json::to_vec(&intent).unwrap(), json.as_bytes());
        for invalid in ["", "spawn.tin", "item.invalid id"] {
            let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
            value["expected_item"] = invalid.into();
            assert!(serde_json::from_value::<GameIntent>(value).is_err());
        }
    }

    #[test]
    fn transition_identity_is_shared_and_unambiguous() {
        let event = GameEvent::DialogueSelected {
            speaker: SpawnId::new("spawn.tutorial.guide").unwrap(),
            choice: "continue".to_owned(),
        };
        assert_eq!(event.kind(), "dialogue_selected");
        assert_eq!(event.primary_target(), Some("spawn.tutorial.guide"));
        let event = GameEvent::Moved {
            tile: Tile::new(1, 1, 0).unwrap(),
        };
        assert_eq!(event.kind(), "moved");
        assert_eq!(event.primary_target(), None);
        let event = GameEvent::QuestAdvanced {
            quest: QuestId::new("quest.cooks_assistant").unwrap(),
            stage: StageId::new("stage.quest.completed").unwrap(),
        };
        assert_eq!(event.primary_target(), Some("quest.cooks_assistant"));
    }
}

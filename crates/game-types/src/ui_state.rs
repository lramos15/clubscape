use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const UI_STATE_VERSION: u32 = 1;
pub const BANK_LAYOUT_AMOUNT_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameplayUiDefinition {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_authority: Option<AudioAuthorityDefinition>,
    pub production_interfaces: BTreeMap<RecipeId, InterfaceId>,
    pub direct_production: BTreeSet<RecipeId>,
    pub quest_rewards: BTreeMap<QuestId, QuestUiDefinition>,
    pub level_up: LevelUpUiDefinition,
    pub stage_interfaces: BTreeMap<StageId, Vec<InterfaceUiRule>>,
    pub stage_overlays: BTreeMap<StageId, Option<InterfaceId>>,
    pub equipment_stats_interface: InterfaceId,
    pub death_preview_interface: InterfaceId,
    pub ability_names: BTreeMap<String, String>,
    pub weapon_style_names: BTreeMap<ItemId, BTreeMap<CombatStyleId, String>>,
    pub unarmed_style_names: BTreeMap<CombatStyleId, String>,
    pub item_actions: BTreeMap<ItemId, Vec<ItemUiDefinition>>,
    pub bank: BankUiDefinition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery: Option<RecoveryUiDefinition>,
    pub coffer: SourceBinding<CofferUiDefinition>,
    pub chat: ChatUiDefinition,
    pub appearance_base: SourceBinding<PenguinBaseUiView>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestUiDefinition {
    pub entitlement: EntitlementId,
    pub interface: InterfaceId,
    pub title: String,
    pub lines: Vec<String>,
    pub items: Vec<ItemStack>,
    pub xp: Vec<XpReward>,
    pub quest_points: u32,
    pub source: Vec<SourceRecord>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelUpUiDefinition {
    pub interface: InterfaceId,
    pub title: String,
    pub line: String,
    pub source: Vec<SourceRecord>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceUiRule {
    pub interface: InterfaceId,
    pub visibility: UiVisibility,
    pub highlighted: bool,
    pub guard: Guard,
    pub unavailable_reason: Option<String>,
    pub source: Vec<SourceRecord>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemUiDefinition {
    pub id: String,
    pub label: String,
    pub guard: Guard,
    pub action: ItemUiAction,
    pub source: Vec<SourceRecord>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ItemUiAction {
    ConsumeRecipe {
        recipe: RecipeId,
    },
    Drink {
        replacement: ItemId,
        cooldown: ActionId,
        delay_ticks: SourceBinding<u32>,
        restore: BTreeMap<Vital, VitalRestoration>,
        skills: Vec<UiSkillAdjustment>,
    },
    Empty {
        replacement: ItemId,
    },
    Read {
        interface: InterfaceId,
        title: String,
        pages: Vec<String>,
        map_asset: Option<AssetId>,
        native_map: bool,
    },
    Unavailable {
        reason: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiSkillAdjustment {
    pub skill: SkillId,
    pub basis: SkillLevelBasis,
    pub percent: Ratio,
    pub additive: u16,
    pub drain: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BankUiDefinition {
    pub maximum_tabs: u8,
    pub initial_insert: bool,
    pub initial_placeholders: bool,
    pub unavailable_containers: Vec<UnavailableUiControl>,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnavailableUiControl {
    pub id: String,
    pub label: String,
    pub reason: String,
    pub source: Vec<SourceRecord>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CofferUiDefinition {
    pub eligible_items: BTreeSet<ItemId>,
    pub exchange_values: BTreeMap<ItemId, u64>,
    pub minimum_value: u64,
    pub credit: Ratio,
    pub maximum_balance: u64,
    pub source: Vec<SourceRecord>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatUiDefinition {
    pub guard: Guard,
    pub maximum_bytes: u16,
    pub radius: u16,
    /// An infrastructure admission bound, not a gameplay action cooldown.
    pub messages_per_window: u16,
    pub window_ticks: u32,
    pub source: Vec<SourceRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameplayUiRuntime {
    pub version: u32,
    pub next_id: u64,
    pub active_tab: Option<InterfaceId>,
    pub active_interface: Option<InterfaceId>,
    pub production: Option<ProductionUiSession>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub production_input: Option<ProductionInventorySelection>,
    pub rewards: Vec<RewardUiView>,
    pub confirmation: Option<UiConfirmation>,
    pub document: Option<DocumentUiView>,
    pub bank: BankLayout,
    pub death_preview: bool,
    pub chat_ticks: Vec<u64>,
    pub chat_messages: Vec<PublicChatLine>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionUiSession {
    pub id: String,
    pub interface: InterfaceId,
    pub target: Option<WorldTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inventory_selection: Option<ProductionInventorySelection>,
    pub instance: Option<InstanceId>,
    pub recipes: Vec<RecipeId>,
    pub action: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionInventorySelection {
    pub used_slot: u8,
    pub used: ItemStack,
    pub target_slot: u8,
    pub target: ItemStack,
}

impl ProductionInventorySelection {
    pub fn validate_shape(&self) -> GameResult<()> {
        if self.used_slot >= 28 || self.target_slot >= 28 || self.used_slot == self.target_slot {
            return Err(ui_state_error());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BankLayout {
    pub version: u32,
    pub revision: u64,
    pub next_entry: u64,
    pub entries: Vec<BankLayoutEntry>,
    pub selected_tab: u8,
    pub insert: bool,
    pub placeholders: bool,
    pub amount: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_all: Option<bool>,
    pub noted: bool,
}

impl BankLayout {
    pub fn amount_selection(&self) -> GameResult<UiAmount> {
        let quantity = Quantity::new(self.amount)?;
        match (self.version, self.amount_all) {
            (1, None) | (BANK_LAYOUT_AMOUNT_VERSION, Some(false)) => {
                Ok(UiAmount::Quantity { quantity })
            }
            (BANK_LAYOUT_AMOUNT_VERSION, Some(true)) => Ok(UiAmount::All {}),
            _ => Err(ui_state_error()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BankLayoutEntry {
    pub id: u64,
    pub slot: u16,
    pub tab: u8,
    pub item: ItemId,
    pub instance: Option<ItemInstanceId>,
    pub placeholder: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiConfirmation {
    pub view: ConfirmationUiView,
    pub action: UiConfirmationAction,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiConfirmationAction {
    Discard {
        death: DeathId,
        storage: RecoveryStorage,
        items: Vec<UiRecoverySelection>,
    },
    Coffer {
        slot: u8,
        item: ItemId,
        instance: Option<ItemInstanceId>,
        quantity: u32,
        credit: u64,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiRecoverySelection {
    pub id: RecoveryItemId,
    pub quantity: u32,
    pub storage: RecoveryStorage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentUiView {
    pub id: String,
    pub interface: InterfaceId,
    pub title: String,
    pub pages: Vec<String>,
    pub page: u16,
    pub map_asset: Option<AssetId>,
    pub native_map: bool,
}

impl GameplayUiRuntime {
    pub fn from_legacy(bank: &Bank, definition: &BankUiDefinition) -> GameResult<Self> {
        if bank.slots.len() > usize::from(bank.capacity) {
            return Err(ui_state_error());
        }
        let entries = bank
            .slots
            .iter()
            .enumerate()
            .filter_map(|(slot, stack)| {
                stack.as_ref().map(|stack| BankLayoutEntry {
                    id: slot as u64 + 1,
                    slot: slot as u16,
                    tab: 0,
                    item: stack.item.clone(),
                    instance: stack.instance.as_ref().map(|instance| instance.id.clone()),
                    placeholder: false,
                })
            })
            .collect();
        Ok(Self {
            version: UI_STATE_VERSION,
            next_id: 1,
            active_tab: None,
            active_interface: None,
            production: None,
            production_input: None,
            rewards: Vec::new(),
            confirmation: None,
            document: None,
            death_preview: false,
            chat_ticks: Vec::new(),
            chat_messages: Vec::new(),
            bank: BankLayout {
                version: 1,
                revision: 0,
                next_entry: bank.slots.len() as u64 + 1,
                entries,
                selected_tab: 0,
                insert: definition.initial_insert,
                placeholders: definition.initial_placeholders,
                amount: 1,
                amount_all: None,
                noted: false,
            },
        })
    }

    pub fn validate_shape(&self) -> GameResult<()> {
        self.bank.amount_selection()?;
        for selection in self
            .production
            .iter()
            .filter_map(|menu| menu.inventory_selection.as_ref())
            .chain(self.production_input.iter())
        {
            selection.validate_shape()?;
        }
        let id = |value: &str| {
            !value.is_empty() && value.len() <= 192 && !value.chars().any(char::is_control)
        };
        let presentations: Vec<_> = self
            .production
            .iter()
            .map(|menu| &menu.id)
            .chain(self.rewards.iter().map(|reward| &reward.id))
            .chain(
                self.confirmation
                    .iter()
                    .map(|confirmation| &confirmation.view.id),
            )
            .chain(self.document.iter().map(|document| &document.id))
            .collect();
        if presentations.iter().any(|value| {
            value
                .strip_prefix("ui.")
                .and_then(|number| {
                    number.parse::<u64>().ok().filter(|value| {
                        *value > 0 && *value < self.next_id && value.to_string() == number
                    })
                })
                .is_none()
        }) || presentations.iter().collect::<BTreeSet<_>>().len() != presentations.len()
        {
            return Err(ui_state_error());
        }
        if self.version != UI_STATE_VERSION
            || self.next_id == 0
            || self.next_id > i64::MAX as u64
            || self.rewards.len() > 64
            || self.chat_messages.len() > 100
            || self.chat_ticks.len() > 128
            || self.chat_ticks.iter().any(|tick| *tick > i64::MAX as u64)
            || self.chat_ticks.windows(2).any(|ticks| ticks[0] > ticks[1])
            || self.bank.entries.len() > 4096
            || self.bank.next_entry == 0
            || self.bank.next_entry > i64::MAX as u64
            || self.bank.revision > i64::MAX as u64
            || self
                .bank
                .entries
                .iter()
                .any(|entry| entry.id == 0 || entry.id >= self.bank.next_entry)
            || self
                .bank
                .entries
                .iter()
                .map(|entry| entry.id)
                .collect::<BTreeSet<_>>()
                .len()
                != self.bank.entries.len()
            || self
                .bank
                .entries
                .iter()
                .map(|entry| entry.slot)
                .collect::<BTreeSet<_>>()
                .len()
                != self.bank.entries.len()
            || self.production.as_ref().is_some_and(|menu| {
                !id(&menu.id)
                    || menu.target.is_none() != menu.inventory_selection.is_some()
                    || menu.recipes.is_empty()
                    || menu.recipes.len() > 256
                    || menu.recipes.iter().collect::<BTreeSet<_>>().len() != menu.recipes.len()
                    || !id(&menu.action)
            })
            || self.rewards.iter().any(|reward| {
                !id(&reward.id)
                    || reward.lines.len() > 64
                    || reward.items.len() > 128
                    || reward.title.len() > 512
                    || reward.lines.iter().any(|line| line.len() > 2048)
                    || reward.xp.len() > 64
            })
            || self.confirmation.as_ref().is_some_and(|confirmation| {
                !id(&confirmation.view.id)
                    || confirmation.view.title.len() > 512
                    || confirmation.view.lines.len() > 64
                    || confirmation.view.lines.iter().any(|line| line.len() > 2048)
                    || confirmation.view.items.len() > 256
            })
            || self.document.as_ref().is_some_and(|document| {
                !id(&document.id)
                    || (!document.native_map && document.pages.is_empty())
                    || document.pages.len() > 128
                    || document.title.len() > 512
                    || document.pages.iter().map(String::len).sum::<usize>() > 32768
                    || (!document.native_map && usize::from(document.page) >= document.pages.len())
            })
            || self.chat_messages.iter().any(|message| {
                !id(&message.id)
                    || message.text.len() > 512
                    || message.sender.len() > 80
                    || message.channel != "public"
                    || message.colour > 12
                    || message.effect > 5
            })
            || self
                .chat_messages
                .iter()
                .map(|message| &message.id)
                .collect::<BTreeSet<_>>()
                .len()
                != self.chat_messages.len()
        {
            return Err(ui_state_error());
        }
        Ok(())
    }
}

fn ui_state_error() -> GameError {
    GameError::new(
        GameErrorCode::InvalidInput,
        "Invalid versioned UI or bank metadata.",
    )
}

//! Versioned, authoritative M1 UI contract. Absence is unsupported, not an empty success.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{
    ActorId, AssetId, CombatBonuses, DeathId, GameErrorCode, InterfaceId, ItemId, ItemInstanceId,
    ProductionMode, QuestId, RecipeId, RecoveryItemId, RecoveryStorage, SkillId, SlotId,
    WorldTarget,
};

pub const GAMEPLAY_UI_VIEW_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiPermission {
    pub allowed: bool,
    pub code: Option<GameErrorCode>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiItem {
    pub item: ItemId,
    pub name: String,
    pub quantity: u32,
    pub source_id: Option<u32>,
    pub asset: Option<AssetId>,
    pub instance_id: Option<ItemInstanceId>,
    pub charges: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameplayUiView {
    pub version: u32,
    pub active_tab: Option<InterfaceId>,
    pub active_interface: Option<InterfaceId>,
    pub production: Option<ProductionUiView>,
    pub reward: Option<RewardUiView>,
    pub confirmation: Option<ConfirmationUiView>,
    pub document: Option<crate::DocumentUiView>,
    pub interfaces: Vec<InterfaceUiView>,
    pub combat_style: Option<String>,
    pub combat_styles: Vec<AbilityUiView>,
    pub prayers: Vec<AbilityUiView>,
    pub spells: Vec<AbilityUiView>,
    pub equipment: EquipmentUiView,
    pub inventory_actions: Vec<InventoryActionsUiView>,
    pub bank: Option<BankUiView>,
    pub kept_on_death: Option<DeathPreviewUiView>,
    pub recovery: Option<RecoveryUiControls>,
    pub appearance: AppearanceUiView,
    pub public_chat: PublicChatUiView,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionUiView {
    pub id: String,
    pub interface: InterfaceId,
    pub target: Option<WorldTarget>,
    pub recipes: Vec<ProductionChoiceUiView>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionChoiceUiView {
    pub recipe: RecipeId,
    pub name: String,
    pub outputs: Vec<UiItem>,
    pub single: UiPermission,
    pub make_x: UiPermission,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all: Option<UiPermission>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewardUiKind {
    Quest,
    LevelUp,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RewardUiView {
    pub id: String,
    pub kind: RewardUiKind,
    pub interface: InterfaceId,
    pub title: String,
    pub lines: Vec<String>,
    pub items: Vec<UiItem>,
    pub xp: Vec<UiXpAward>,
    pub quest_points: u32,
    pub quest: Option<QuestId>,
    pub skill: Option<SkillId>,
    pub level: Option<u16>,
    pub continuation: GameplayUiRequest,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiXpAward {
    pub skill: SkillId,
    pub amount_tenths: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiVisibility {
    Hidden,
    Locked,
    Enabled,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceUiView {
    pub interface: InterfaceId,
    pub visibility: UiVisibility,
    pub highlighted: bool,
    pub permission: UiPermission,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbilityUiView {
    pub id: String,
    pub name: String,
    pub selected: bool,
    pub visible: bool,
    pub permission: UiPermission,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EquipmentUiView {
    pub bonuses: CombatBonuses,
    pub weight_grams: String,
    pub slots: Vec<SlotId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryActionsUiView {
    pub slot: u8,
    pub item: ItemId,
    pub instance: Option<ItemInstanceId>,
    pub actions: Vec<ItemActionUiView>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemActionUiView {
    pub id: String,
    pub label: String,
    pub permission: UiPermission,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BankUiView {
    pub revision: String,
    pub capacity: u16,
    pub selected_tab: u8,
    pub insert_mode: bool,
    pub placeholders: bool,
    pub amount: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_selection: Option<crate::UiAmount>,
    pub noted: bool,
    pub tabs: Vec<BankTabUiView>,
    pub entries: Vec<BankEntryUiView>,
    pub deposit_equipment: UiPermission,
    pub unavailable_containers: Vec<ItemActionUiView>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BankTabUiView {
    pub tab: u8,
    pub first_entry: Option<String>,
    pub entries: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BankEntryUiView {
    pub id: String,
    pub slot: u16,
    pub tab: u8,
    pub item: ItemId,
    pub value: Option<UiItem>,
    /// A non-spendable reference; `value` is None, never a zero Quantity.
    pub placeholder: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeathPreviewUiView {
    pub scope: String,
    pub kept: Vec<UiItem>,
    pub lost: Vec<UiItem>,
    pub full_grave_fee: String,
    pub full_office_fee: String,
    pub value_revision: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryUiControls {
    pub coffer_balance: String,
    pub discard: UiPermission,
    pub coffer_offer: UiPermission,
    pub coffer_items: Vec<InventoryActionsUiView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub management: Option<crate::RecoveryManagementView>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmationUiView {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub lines: Vec<String>,
    pub items: Vec<UiItem>,
    pub credit: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppearanceUiView {
    pub choices: BTreeMap<String, Vec<AppearanceChoiceUiView>>,
    pub base: Option<PenguinBaseUiView>,
    pub confirmed: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppearanceChoiceUiView {
    pub value: u32,
    pub label: Option<String>,
    pub permission: UiPermission,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PenguinBaseUiView {
    pub asset: AssetId,
    pub source_npc: u32,
    pub adaptation: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicChatUiView {
    pub permission: UiPermission,
    pub maximum_bytes: u16,
    pub channel: String,
    pub messages: Vec<PublicChatLine>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicChatLine {
    pub id: String,
    pub actor: ActorId,
    pub sender: String,
    pub channel: String,
    pub text: String,
    pub colour: u8,
    pub effect: u8,
}

/// Requests only. The authoritative engine owns targets, consequences and all quantities.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GameplayUiRequest {
    ProductionSelectAll {
        menu_id: String,
        recipe: RecipeId,
    },
    BankSetAmount {
        amount: crate::UiAmount,
        noted: bool,
    },
    RecoveryTake {
        death: DeathId,
        storage: RecoveryStorage,
        items: Vec<crate::RecoveryItemAmount>,
    },
    RecoveryBankAll {
        records: Vec<crate::RecoveryRecordSelection>,
    },
    UiDocumentPage {
        document_id: String,
        page: u16,
    },
    BankPlaceholder {
        entry_id: String,
    },
    UiDismiss {
        presentation_id: String,
    },
    ProductionSelect {
        menu_id: String,
        recipe: RecipeId,
        quantity: u32,
        mode: ProductionMode,
    },
    ItemAction {
        inventory_slot: u8,
        expected_item: ItemId,
        expected_instance: Option<ItemInstanceId>,
        action: String,
    },
    BankSelectTab {
        tab: u8,
    },
    BankCreateTab {
        entry_id: String,
    },
    BankMove {
        entry_id: String,
        before_entry_id: Option<String>,
        tab: u8,
    },
    BankCollapseTab {
        tab: u8,
    },
    BankSetInsert {
        enabled: bool,
    },
    BankSetPlaceholders {
        enabled: bool,
    },
    BankReleasePlaceholder {
        entry_id: String,
    },
    BankDepositEquipment,
    BankWithdrawEntry {
        entry_id: String,
        quantity: u32,
        noted: bool,
    },
    BankSetOptions {
        amount: u32,
        noted: bool,
    },
    OpenDeathPreview,
    RequestRecoveryDiscard {
        death: DeathId,
        storage: RecoveryStorage,
        items: Vec<RecoveryItemId>,
    },
    CofferOffer {
        inventory_slot: u8,
        expected_item: ItemId,
        expected_instance: Option<ItemInstanceId>,
        quantity: u32,
    },
    UiConfirm {
        confirmation_id: String,
        accept: bool,
    },
    PublicChat {
        channel: String,
        text: String,
    },
}

impl GameplayUiRequest {
    pub fn requires_bank_revision(&self) -> bool {
        matches!(
            self,
            Self::BankSelectTab { .. }
                | Self::BankSetAmount { .. }
                | Self::RecoveryBankAll { .. }
                | Self::BankCreateTab { .. }
                | Self::BankMove { .. }
                | Self::BankCollapseTab { .. }
                | Self::BankSetInsert { .. }
                | Self::BankSetPlaceholders { .. }
                | Self::BankReleasePlaceholder { .. }
                | Self::BankPlaceholder { .. }
                | Self::BankDepositEquipment
                | Self::BankWithdrawEntry { .. }
                | Self::BankSetOptions { .. }
        )
    }
}

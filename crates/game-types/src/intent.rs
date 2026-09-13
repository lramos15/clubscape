use serde::{Deserialize, Serialize};

use crate::{
    InterfaceId, ItemStack, Quantity, QuestId, RecipeId, ShopId, SkillId, SlotId, SpawnId, StageId,
    Tile,
};

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
    CancelActivity,
    RequestLogout,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemTarget {
    Inventory { slot: u8 },
    World { spawn: SpawnId },
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
}

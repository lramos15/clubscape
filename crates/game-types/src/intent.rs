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
            Self::Moved { .. } | Self::Died | Self::Recovered | Self::Message { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tile;

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

use clubscape_game_types::{
    CharacterSetting, GameIntent, ItemTarget, ProductionMode, RecoveryStorage, WorldTarget,
};
use clubscape_protocol::game::{self, world_input::Action};

use crate::BridgeError;

fn target(value: WorldTarget) -> game::WorldTarget {
    game::WorldTarget {
        target: Some(match value {
            WorldTarget::Spawn { spawn } => game::world_target::Target::Spawn(spawn.to_string()),
            WorldTarget::TemporaryObject { object } => {
                game::world_target::Target::TemporaryObject(object.to_string())
            }
        }),
    }
}

pub(crate) fn action(input: &str) -> Result<Action, BridgeError> {
    if input.len() > clubscape_protocol::MAX_REQUEST_BYTES {
        return Err(BridgeError::input(
            "The input exceeds the protocol byte budget.",
        ));
    }
    let intent: GameIntent = serde_json::from_str(input)
        .map_err(|_| BridgeError::input("The input is not a valid typed GameIntent."))?;
    Ok(match intent {
        GameIntent::Walk {
            destination,
            running,
        } => Action::Walk(game::Walk {
            destination: Some(game::Tile {
                x: u32::from(destination.x()),
                y: u32::from(destination.y()),
                plane: u32::from(destination.plane()),
            }),
            running,
        }),
        GameIntent::Interact { target, action } => Action::Interact(game::Interact {
            target: target.to_string(),
            action,
        }),
        GameIntent::InteractWith { target: at, action } => {
            Action::InteractWith(game::InteractWith {
                target: Some(target(at)),
                action,
            })
        }
        GameIntent::SelectDialogue { speaker, choice } => {
            Action::DialogueChoice(game::DialogueChoice {
                speaker: speaker.to_string(),
                choice,
            })
        }
        GameIntent::OpenInterface { interface } => Action::OpenInterface(game::OpenInterface {
            interface: interface.to_string(),
        }),
        GameIntent::CloseInterface => Action::CloseInterface(game::Empty {}),
        GameIntent::Equip { inventory_slot } => Action::Equip(game::InventorySlot {
            slot: u32::from(inventory_slot),
        }),
        GameIntent::Unequip { slot } => Action::Unequip(game::Unequip {
            slot: slot.to_string(),
        }),
        GameIntent::Drop {
            inventory_slot,
            quantity,
        } => Action::Drop(game::Drop {
            inventory_slot: u32::from(inventory_slot),
            quantity: quantity.get(),
        }),
        GameIntent::TakeGroundItem { ground_item_id } => {
            Action::TakeGroundItem(game::TakeGroundItem { ground_item_id })
        }
        GameIntent::UseItem {
            inventory_slot,
            target,
        } => Action::UseItem(game::UseItem {
            inventory_slot: u32::from(inventory_slot),
            target: Some(match target {
                ItemTarget::Inventory { slot } => {
                    game::use_item::Target::OtherInventorySlot(u32::from(slot))
                }
                ItemTarget::World { spawn } => {
                    game::use_item::Target::WorldSpawn(spawn.to_string())
                }
                ItemTarget::TemporaryObject { object } => {
                    game::use_item::Target::TemporaryObject(object.to_string())
                }
                ItemTarget::Ground { ground_item_id } => {
                    game::use_item::Target::GroundItem(ground_item_id)
                }
            }),
        }),
        GameIntent::MoveInventory { from, to } => Action::MoveInventory(game::MoveInventory {
            from: u32::from(from),
            to: u32::from(to),
        }),
        GameIntent::Eat { inventory_slot } => Action::Eat(game::InventorySlot {
            slot: u32::from(inventory_slot),
        }),
        GameIntent::Produce {
            recipe,
            target,
            quantity,
        } => Action::Produce(game::Produce {
            recipe: recipe.to_string(),
            target: target.map(|id| id.to_string()),
            quantity: quantity.get(),
        }),
        GameIntent::ProduceAt {
            recipe,
            target: at,
            quantity,
        } => Action::ProduceAt(game::ProduceAt {
            recipe: recipe.to_string(),
            target: at.map(target),
            quantity: quantity.get(),
        }),
        GameIntent::BankDeposit {
            banker,
            inventory_slot,
            quantity,
        } => Action::BankDeposit(game::BankDeposit {
            banker: banker.to_string(),
            inventory_slot: u32::from(inventory_slot),
            quantity: quantity.get(),
        }),
        GameIntent::BankWithdraw {
            banker,
            bank_slot,
            quantity,
            noted,
        } => Action::BankWithdraw(game::BankWithdraw {
            banker: banker.to_string(),
            bank_slot: u32::from(bank_slot),
            quantity: quantity.get(),
            noted,
        }),
        GameIntent::ShopBuy {
            shop,
            item_index,
            quantity,
            expected_item,
        } => Action::ShopBuy(game::ShopBuy {
            shop: shop.to_string(),
            item_index: u32::from(item_index),
            quantity: quantity.get(),
            expected_item: expected_item.map(|item| item.to_string()),
        }),
        GameIntent::ShopSell {
            shop,
            inventory_slot,
            quantity,
        } => Action::ShopSell(game::ShopSell {
            shop: shop.to_string(),
            inventory_slot: u32::from(inventory_slot),
            quantity: quantity.get(),
        }),
        GameIntent::SetCombatStyle { style } => {
            Action::SetCombatStyle(game::SetCombatStyle { style })
        }
        GameIntent::Cast { spell, target } => Action::Cast(game::Cast {
            spell,
            target: target.map(|id| id.to_string()),
        }),
        GameIntent::SetPrayer { prayer, enabled } => {
            Action::SetPrayer(game::SetPrayer { prayer, enabled })
        }
        GameIntent::SetSetting { setting } => {
            let (setting, enabled) = match setting {
                CharacterSetting::Run(enabled) => (game::SettingKind::Run, enabled),
                CharacterSetting::AutoRetaliate(enabled) => {
                    (game::SettingKind::AutoRetaliate, enabled)
                }
                CharacterSetting::DeathAutoEquip(enabled) => {
                    (game::SettingKind::DeathAutoEquip, enabled)
                }
                CharacterSetting::DeathSupplyPiles(enabled) => {
                    (game::SettingKind::DeathSupplyPiles, enabled)
                }
            };
            Action::SetSetting(game::SetSetting {
                setting: setting as i32,
                enabled,
            })
        }
        GameIntent::ConfirmAppearance { appearance } => {
            Action::ConfirmAppearance(game::ConfirmAppearance {
                appearance: appearance.into_iter().collect(),
            })
        }
        GameIntent::SelectExperience { experience } => {
            Action::SelectExperience(game::SelectExperience {
                experience: experience.to_string(),
            })
        }
        GameIntent::Reclaim {
            death,
            storage,
            items,
        } => Action::Reclaim(game::Reclaim {
            death: death.to_string(),
            storage: match storage {
                RecoveryStorage::Grave => game::RecoveryStorage::Grave as i32,
                RecoveryStorage::DeathOffice => game::RecoveryStorage::DeathOffice as i32,
            },
            items: items.into_iter().map(|id| id.to_string()).collect(),
        }),
        GameIntent::CancelActivity => Action::CancelActivity(game::Empty {}),
        GameIntent::RequestLogout => Action::RequestLogout(game::Empty {}),
        GameIntent::ProduceSelected {
            recipe,
            target: at,
            quantity,
            mode,
        } => Action::ProduceSelected(game::ProduceSelected {
            recipe: recipe.to_string(),
            target: at.map(target),
            quantity: quantity.get(),
            mode: match mode {
                ProductionMode::Single => game::ProductionMode::Single as i32,
                ProductionMode::MakeX => game::ProductionMode::MakeX as i32,
            },
        }),
        GameIntent::OpenGrave { death } => Action::OpenGrave(game::OpenGrave {
            death: death.to_string(),
        }),
        GameIntent::OpenDeathOffice => Action::OpenDeathOffice(game::Empty {}),
    })
}

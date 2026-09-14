use clubscape_game_types::{
    GameIntent, INVENTORY_SLOTS, InterfaceId, ItemTarget, Quantity, RecipeId, ShopId, SlotId,
    SpawnId, Tile,
};

use crate::{ValidationError, game, invalid};

pub fn validate_world_session(value: &str) -> Result<(), ValidationError> {
    if value.len() != 36 || !uuid::Uuid::parse_str(value).is_ok_and(|id| !id.is_nil()) {
        return Err(invalid("A non-nil UUID world session ID is required."));
    }
    Ok(())
}

fn bounded_text(value: &str, maximum: usize) -> Result<String, ValidationError> {
    if value.is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(invalid(
            "A game identifier is empty, oversized or contains control characters.",
        ));
    }
    Ok(value.to_owned())
}

fn inventory_slot(value: u32) -> Result<u8, ValidationError> {
    if value >= INVENTORY_SLOTS as u32 {
        return Err(invalid("Inventory slot must be between 0 and 27."));
    }
    u8::try_from(value).map_err(|_| invalid("Inventory slot is out of range."))
}

fn bounded_index(value: u32) -> Result<u16, ValidationError> {
    if value >= 4096 {
        return Err(invalid(
            "Game collection index exceeds its supported bound.",
        ));
    }
    u16::try_from(value).map_err(|_| invalid("Game collection index is out of range."))
}

fn quantity(value: u32) -> Result<Quantity, ValidationError> {
    Quantity::new(value).map_err(|_| invalid("Item quantity is out of range."))
}

fn spawn(value: &str) -> Result<SpawnId, ValidationError> {
    SpawnId::new(value).map_err(|_| invalid("A valid spawn ID is required."))
}

fn optional_spawn(value: &Option<String>) -> Result<Option<SpawnId>, ValidationError> {
    value.as_deref().map(spawn).transpose()
}

pub fn validate_character_options(options: &game::CreateCharacter) -> Result<(), ValidationError> {
    bounded_text(&options.experience_choice, 64)?;
    if options.appearance.len() > 16 {
        return Err(invalid("Too many character appearance options."));
    }
    for (key, value) in &options.appearance {
        if key.is_empty()
            || key.len() > 32
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
            || *value > 0x00ff_ffff
        {
            return Err(invalid("Character appearance options are invalid."));
        }
    }
    Ok(())
}

pub fn game_intent(input: &game::WorldInput) -> Result<GameIntent, ValidationError> {
    validate_world_session(&input.world_session_id)?;
    if input.sequence == 0 {
        return Err(invalid("Game command sequences start at one."));
    }
    use game::world_input::Action;
    Ok(
        match input
            .action
            .as_ref()
            .ok_or_else(|| invalid("A supported game action is required."))?
        {
            Action::Walk(action) => {
                let tile = action
                    .destination
                    .as_ref()
                    .ok_or_else(|| invalid("A destination is required."))?;
                let x = u16::try_from(tile.x)
                    .map_err(|_| invalid("Destination coordinate is out of range."))?;
                let y = u16::try_from(tile.y)
                    .map_err(|_| invalid("Destination coordinate is out of range."))?;
                let plane = u8::try_from(tile.plane)
                    .map_err(|_| invalid("Destination plane is out of range."))?;
                GameIntent::Walk {
                    destination: Tile::new(x, y, plane)
                        .map_err(|_| invalid("Destination tile is invalid."))?,
                    running: action.running,
                }
            }
            Action::Interact(action) => GameIntent::Interact {
                target: spawn(&action.target)?,
                action: bounded_text(&action.action, 64)?,
            },
            Action::DialogueChoice(action) => GameIntent::SelectDialogue {
                speaker: spawn(&action.speaker)?,
                choice: bounded_text(&action.choice, 128)?,
            },
            Action::OpenInterface(action) => GameIntent::OpenInterface {
                interface: InterfaceId::new(&action.interface)
                    .map_err(|_| invalid("Invalid interface ID."))?,
            },
            Action::CloseInterface(_) => GameIntent::CloseInterface,
            Action::Equip(action) => GameIntent::Equip {
                inventory_slot: inventory_slot(action.slot)?,
            },
            Action::Unequip(action) => GameIntent::Unequip {
                slot: SlotId::new(&action.slot)
                    .map_err(|_| invalid("Invalid equipment slot ID."))?,
            },
            Action::Drop(action) => GameIntent::Drop {
                inventory_slot: inventory_slot(action.inventory_slot)?,
                quantity: quantity(action.quantity)?,
            },
            Action::TakeGroundItem(action) => GameIntent::TakeGroundItem {
                ground_item_id: bounded_text(&action.ground_item_id, 160)?,
            },
            Action::UseItem(action) => GameIntent::UseItem {
                inventory_slot: inventory_slot(action.inventory_slot)?,
                target: match action
                    .target
                    .as_ref()
                    .ok_or_else(|| invalid("An item-use target is required."))?
                {
                    game::use_item::Target::OtherInventorySlot(slot) => ItemTarget::Inventory {
                        slot: inventory_slot(*slot)?,
                    },
                    game::use_item::Target::WorldSpawn(target) => ItemTarget::World {
                        spawn: spawn(target)?,
                    },
                },
            },
            Action::MoveInventory(action) => GameIntent::MoveInventory {
                from: inventory_slot(action.from)?,
                to: inventory_slot(action.to)?,
            },
            Action::Eat(action) => GameIntent::Eat {
                inventory_slot: inventory_slot(action.slot)?,
            },
            Action::Produce(action) => GameIntent::Produce {
                recipe: RecipeId::new(&action.recipe).map_err(|_| invalid("Invalid recipe ID."))?,
                target: optional_spawn(&action.target)?,
                quantity: quantity(action.quantity)?,
            },
            Action::BankDeposit(action) => GameIntent::BankDeposit {
                banker: spawn(&action.banker)?,
                inventory_slot: inventory_slot(action.inventory_slot)?,
                quantity: quantity(action.quantity)?,
            },
            Action::BankWithdraw(action) => GameIntent::BankWithdraw {
                banker: spawn(&action.banker)?,
                bank_slot: bounded_index(action.bank_slot)?,
                quantity: quantity(action.quantity)?,
                noted: action.noted,
            },
            Action::ShopBuy(action) => GameIntent::ShopBuy {
                shop: ShopId::new(&action.shop).map_err(|_| invalid("Invalid shop ID."))?,
                item_index: bounded_index(action.item_index)?,
                quantity: quantity(action.quantity)?,
            },
            Action::ShopSell(action) => GameIntent::ShopSell {
                shop: ShopId::new(&action.shop).map_err(|_| invalid("Invalid shop ID."))?,
                inventory_slot: inventory_slot(action.inventory_slot)?,
                quantity: quantity(action.quantity)?,
            },
            Action::SetCombatStyle(action) => GameIntent::SetCombatStyle {
                style: bounded_text(&action.style, 64)?,
            },
            Action::Cast(action) => GameIntent::Cast {
                spell: bounded_text(&action.spell, 64)?,
                target: optional_spawn(&action.target)?,
            },
            Action::SetPrayer(action) => GameIntent::SetPrayer {
                prayer: bounded_text(&action.prayer, 64)?,
                enabled: action.enabled,
            },
            Action::CancelActivity(_) => GameIntent::CancelActivity,
            Action::RequestLogout(_) => GameIntent::RequestLogout,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    fn input(action: game::world_input::Action) -> game::WorldInput {
        game::WorldInput {
            world_session_id: "00000000-0000-4000-8000-000000000001".to_owned(),
            sequence: 1,
            expected_character_revision: Some(0),
            action: Some(action),
        }
    }

    #[test]
    fn game_wire_round_trip_maps_only_a_valid_intent() {
        let message = input(game::world_input::Action::Walk(game::Walk {
            destination: Some(game::Tile {
                x: 3200,
                y: 3201,
                plane: 0,
            }),
            running: true,
        }));
        let decoded = game::WorldInput::decode(message.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded, message);
        assert_eq!(
            game_intent(&decoded).unwrap(),
            GameIntent::Walk {
                destination: Tile::new(3200, 3201, 0).unwrap(),
                running: true
            }
        );
    }

    #[test]
    fn invalid_sessions_sequences_slots_and_quantities_fail() {
        let valid = input(game::world_input::Action::Drop(game::Drop {
            inventory_slot: 0,
            quantity: 1,
        }));
        let mut invalid = valid.clone();
        invalid.sequence = 0;
        assert!(game_intent(&invalid).is_err());
        invalid = valid.clone();
        invalid.world_session_id.clear();
        assert!(game_intent(&invalid).is_err());
        for (slot, amount) in [(28, 1), (u32::MAX, 1), (0, 0), (0, u32::MAX)] {
            invalid = input(game::world_input::Action::Drop(game::Drop {
                inventory_slot: slot,
                quantity: amount,
            }));
            assert!(game_intent(&invalid).is_err());
        }
    }

    #[test]
    fn malformed_targets_and_world_coordinates_fail() {
        for tile in [
            game::Tile {
                x: 16384,
                y: 1,
                plane: 0,
            },
            game::Tile {
                x: 1,
                y: 1,
                plane: 4,
            },
            game::Tile {
                x: u32::MAX,
                y: 1,
                plane: 0,
            },
        ] {
            assert!(
                game_intent(&input(game::world_input::Action::Walk(game::Walk {
                    destination: Some(tile),
                    running: false,
                })))
                .is_err()
            );
        }
        assert!(
            game_intent(&input(game::world_input::Action::UseItem(game::UseItem {
                inventory_slot: 0,
                target: None,
            })))
            .is_err()
        );
        assert!(
            game_intent(&input(game::world_input::Action::Interact(
                game::Interact {
                    target: "item.ore.copper".to_owned(),
                    action: "Mine".to_owned(),
                }
            )))
            .is_err()
        );
    }
}

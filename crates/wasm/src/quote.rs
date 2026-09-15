use std::collections::BTreeSet;

use clubscape_game_types::ItemId;
use clubscape_protocol::game;
use serde::Deserialize;

use crate::BridgeError;

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum Selection {
    BankDeposit {
        inventory_slot: u32,
        quantity: u32,
    },
    BankWithdraw {
        bank_slot: u32,
        quantity: u32,
        noted: bool,
    },
    ShopBuy {
        shop: String,
        item_index: u32,
        item_id: String,
        quantity: u32,
    },
    ShopSell {
        shop: String,
        inventory_slot: u32,
        item_id: String,
        quantity: u32,
    },
    Recovery {
        death: String,
        storage: String,
        items: Vec<String>,
    },
}

impl Selection {
    pub fn parse(input: &str) -> Result<Self, BridgeError> {
        serde_json::from_str(input)
            .map_err(|_| BridgeError::input("Invalid typed read-only quote selection."))
    }

    pub fn wire(&self) -> Result<game::QuoteRequest, BridgeError> {
        use game::quote_request::Request;
        let request = match self {
            Self::BankDeposit {
                inventory_slot,
                quantity,
            } => Request::BankDeposit(game::InventoryAmount {
                inventory_slot: *inventory_slot,
                quantity: *quantity,
            }),
            Self::BankWithdraw {
                bank_slot,
                quantity,
                noted,
            } => Request::BankWithdraw(game::BankWithdrawal {
                bank_slot: *bank_slot,
                quantity: *quantity,
                noted: *noted,
            }),
            Self::ShopBuy {
                shop,
                item_index,
                item_id,
                quantity,
            } => {
                ItemId::new(item_id)
                    .map_err(|_| BridgeError::input("A quote must retain the selected ItemId."))?;
                Request::ShopBuy(game::ShopBuy {
                    shop: shop.clone(),
                    item_index: *item_index,
                    quantity: *quantity,
                })
            }
            Self::ShopSell {
                shop,
                inventory_slot,
                item_id,
                quantity,
            } => {
                ItemId::new(item_id)
                    .map_err(|_| BridgeError::input("A quote must retain the selected ItemId."))?;
                Request::ShopSell(game::ShopSell {
                    shop: shop.clone(),
                    inventory_slot: *inventory_slot,
                    quantity: *quantity,
                })
            }
            Self::Recovery {
                death,
                storage,
                items,
            } => Request::Recovery(game::Reclaim {
                death: death.clone(),
                storage: match storage.as_str() {
                    "grave" => game::RecoveryStorage::Grave as i32,
                    "death_office" => game::RecoveryStorage::DeathOffice as i32,
                    _ => {
                        return Err(BridgeError::input(
                            "Select a supported source recovery storage.",
                        ));
                    }
                },
                items: items.clone(),
            }),
        };
        let result = game::QuoteRequest {
            request: Some(request),
        };
        clubscape_protocol::quote_request(&result)
            .map_err(|error| BridgeError::input(error.message))?;
        Ok(result)
    }

    /// A stale selection is a read-only mismatch, not an invented source denial.
    pub fn matches(&self, result: &game::Quote) -> bool {
        use game::quote::Result;
        match (self, result.result.as_ref()) {
            (
                Self::BankDeposit { quantity, .. } | Self::BankWithdraw { quantity, .. },
                Some(Result::Bank(value)),
            ) => {
                *quantity == value.requested
                    && value
                        .transferred
                        .as_ref()
                        .is_some_and(|stack| stack.quantity <= *quantity)
            }
            (
                Self::ShopBuy {
                    shop,
                    item_id,
                    quantity,
                    ..
                }
                | Self::ShopSell {
                    shop,
                    item_id,
                    quantity,
                    ..
                },
                Some(Result::Shop(value)),
            ) => {
                value.shop == *shop
                    && value.item == *item_id
                    && value.requested == *quantity
                    && value.quantity <= *quantity
            }
            (
                Self::Recovery {
                    death,
                    storage,
                    items,
                },
                Some(Result::Recovery(value)),
            ) => {
                value.death == *death
                    && super::context::storage(value.storage).ok() == Some(storage.as_str())
                    && items.iter().collect::<BTreeSet<_>>()
                        == value.selected.iter().collect::<BTreeSet<_>>()
                    && items.len() == value.selected.len()
            }
            _ => false,
        }
    }
}

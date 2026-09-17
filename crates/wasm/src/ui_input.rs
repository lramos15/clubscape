use clubscape_game_types::{GameplayUiRequest, ProductionMode, RecoveryStorage, UiAmount};
use clubscape_protocol::game;
use serde_json::{Value, json};

use crate::BridgeError;

fn storage(value: RecoveryStorage) -> i32 {
    (match value {
        RecoveryStorage::Grave => game::RecoveryStorage::Grave,
        RecoveryStorage::DeathOffice => game::RecoveryStorage::DeathOffice,
    }) as i32
}

fn amount(value: UiAmount) -> game::UiAmount {
    game::UiAmount {
        selection: Some(match value {
            UiAmount::Quantity { quantity } => game::ui_amount::Selection::Quantity(quantity.get()),
            UiAmount::All {} => game::ui_amount::Selection::All(game::Empty {}),
        }),
    }
}

pub(crate) fn capability(request: &GameplayUiRequest) -> Option<&'static str> {
    match request {
        GameplayUiRequest::ProductionSelectAll { .. } | GameplayUiRequest::BankSetAmount { .. } => {
            Some(crate::gameplay_ui::AMOUNTS_CAPABILITY)
        }
        GameplayUiRequest::RecoveryTake { .. }
        | GameplayUiRequest::RecoveryTakeAll { .. }
        | GameplayUiRequest::RecoveryBankAll { .. } => {
            Some(crate::gameplay_ui::RECOVERY_CAPABILITY)
        }
        _ => None,
    }
}

pub(crate) fn parse(
    input: &str,
) -> Result<Option<(GameplayUiRequest, Option<String>)>, BridgeError> {
    let mut value: Value = serde_json::from_str(input)
        .map_err(|_| BridgeError::input("The UI/game request is not valid JSON."))?;
    let Some(object) = value.as_object_mut() else {
        return Err(BridgeError::input("The UI/game request must be an object."));
    };
    let revision = object.remove("expected_bank_revision");
    let Ok(request) = serde_json::from_value::<GameplayUiRequest>(value) else {
        if revision.is_some() {
            return Err(BridgeError::input(
                "Bank revision metadata belongs only to a valid typed gameplay UI request.",
            ));
        }
        return Ok(None);
    };
    let revision = revision
        .map(|value| match value {
            Value::String(value) => Ok(value),
            _ => Err(BridgeError::input(
                "Expected bank revision must remain an exact decimal string.",
            )),
        })
        .transpose()?;
    Ok(Some((request, revision)))
}

pub(crate) fn wire(
    request: GameplayUiRequest,
    expected_bank_revision: Option<String>,
) -> Result<game::GameplayUiRequest, BridgeError> {
    use game::gameplay_ui_request::Request as R;
    let request = match request {
        GameplayUiRequest::ProductionSelectAll { menu_id, recipe } => {
            R::ProductionAll(game::UiProductionAll {
                menu_id,
                recipe: recipe.to_string(),
            })
        }
        GameplayUiRequest::BankSetAmount {
            amount: selected,
            noted,
        } => R::BankAmount(game::UiBankAmount {
            amount: Some(amount(selected)),
            noted,
        }),
        GameplayUiRequest::RecoveryTake {
            death,
            storage: location,
            items,
        } => R::RecoveryTake(game::UiRecoveryTake {
            death: death.to_string(),
            storage: storage(location),
            items: items
                .into_iter()
                .map(|entry| game::UiRecoveryItemAmount {
                    id: entry.id.to_string(),
                    amount: Some(amount(entry.amount)),
                })
                .collect(),
        }),
        GameplayUiRequest::RecoveryBankAll { records } => {
            R::RecoveryBankAll(game::UiRecoveryBankAll {
                records: records
                    .into_iter()
                    .map(|record| game::UiRecoveryRecordSelection {
                        death: record.death.to_string(),
                        items: record.items.into_iter().map(|id| id.to_string()).collect(),
                    })
                    .collect(),
            })
        }
        GameplayUiRequest::RecoveryTakeAll { selection } => {
            R::RecoveryTakeAll(game::UiRecoveryTakeAll {
                selection: Some(clubscape_protocol::recovery_context_selection_to_wire(
                    selection,
                )),
            })
        }
        GameplayUiRequest::UiDocumentPage { document_id, page } => {
            R::DocumentPage(game::UiDocumentPage {
                id: document_id,
                page: u32::from(page),
            })
        }
        GameplayUiRequest::BankPlaceholder { entry_id } => {
            R::Placeholder(game::UiIdentity { id: entry_id })
        }
        GameplayUiRequest::UiDismiss { presentation_id } => R::Dismiss(game::UiIdentity {
            id: presentation_id,
        }),
        GameplayUiRequest::ProductionSelect {
            menu_id,
            recipe,
            quantity,
            mode,
        } => R::Production(game::UiProductionSelection {
            menu_id,
            recipe: recipe.to_string(),
            quantity,
            mode: match mode {
                ProductionMode::Single => game::ProductionMode::Single,
                ProductionMode::MakeX => game::ProductionMode::MakeX,
            } as i32,
        }),
        GameplayUiRequest::ItemAction {
            inventory_slot,
            expected_item,
            expected_instance,
            action,
        } => R::ItemAction(game::UiItemAction {
            inventory_slot: u32::from(inventory_slot),
            expected_item: expected_item.to_string(),
            expected_instance: expected_instance.map(|id| id.to_string()),
            action,
        }),
        GameplayUiRequest::BankSelectTab { tab } => R::SelectTab(game::UiTab {
            tab: u32::from(tab),
        }),
        GameplayUiRequest::BankCreateTab { entry_id } => {
            R::CreateTab(game::UiIdentity { id: entry_id })
        }
        GameplayUiRequest::BankMove {
            entry_id,
            before_entry_id,
            tab,
        } => R::BankMove(game::UiBankMove {
            entry_id,
            before_entry_id,
            tab: u32::from(tab),
        }),
        GameplayUiRequest::BankCollapseTab { tab } => R::CollapseTab(game::UiTab {
            tab: u32::from(tab),
        }),
        GameplayUiRequest::BankSetInsert { enabled } => R::InsertMode(game::UiToggle { enabled }),
        GameplayUiRequest::BankSetPlaceholders { enabled } => {
            R::Placeholders(game::UiToggle { enabled })
        }
        GameplayUiRequest::BankReleasePlaceholder { entry_id } => {
            R::ReleasePlaceholder(game::UiIdentity { id: entry_id })
        }
        GameplayUiRequest::BankDepositEquipment => R::DepositEquipment(game::Empty {}),
        GameplayUiRequest::BankWithdrawEntry {
            entry_id,
            quantity,
            noted,
        } => R::BankWithdraw(game::UiBankWithdrawal {
            entry_id,
            quantity,
            noted,
        }),
        GameplayUiRequest::BankSetOptions { amount, noted } => {
            R::BankOptions(game::UiBankOptions { amount, noted })
        }
        GameplayUiRequest::OpenDeathPreview => R::DeathPreview(game::Empty {}),
        GameplayUiRequest::RequestRecoveryDiscard {
            death,
            storage,
            items,
        } => R::DiscardRecovery(game::Reclaim {
            death: death.to_string(),
            storage: match storage {
                RecoveryStorage::Grave => game::RecoveryStorage::Grave,
                RecoveryStorage::DeathOffice => game::RecoveryStorage::DeathOffice,
            } as i32,
            items: items.into_iter().map(|id| id.to_string()).collect(),
        }),
        GameplayUiRequest::CofferOffer {
            inventory_slot,
            expected_item,
            expected_instance,
            quantity,
        } => R::CofferOffer(game::UiCofferOffer {
            inventory_slot: u32::from(inventory_slot),
            expected_item: expected_item.to_string(),
            expected_instance: expected_instance.map(|id| id.to_string()),
            quantity,
        }),
        GameplayUiRequest::UiConfirm {
            confirmation_id,
            accept,
        } => R::Confirmation(game::UiConfirmation {
            id: confirmation_id,
            accept,
        }),
        GameplayUiRequest::PublicChat { channel, text } => {
            R::PublicChat(game::UiChat { channel, text })
        }
    };
    let result = game::GameplayUiRequest {
        expected_bank_revision,
        request: Some(request),
    };
    clubscape_protocol::ui_request(&result).map_err(|_| {
        BridgeError::input("The typed UI request or its bank-revision precondition is invalid.")
    })?;
    Ok(result)
}

pub(crate) fn json(value: &game::GameplayUiRequest) -> Result<Value, BridgeError> {
    let request = clubscape_protocol::ui_request(value)
        .map_err(|_| BridgeError::protocol("The UI continuation is not a valid typed request."))?;
    let mut result = json!(request);
    if let Some(revision) = &value.expected_bank_revision {
        result["expected_bank_revision"] = json!(revision);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    #[test]
    fn every_current_ui_request_round_trips_with_exact_generated_tags_and_bank_identity() {
        let requests = [
            json!({"kind":"production_select_all","menu_id":"menu.fixture","recipe":"recipe.fixture"}),
            json!({"kind":"bank_set_amount","amount":{"kind":"all"},"noted":false}),
            json!({"kind":"recovery_take","death":"death.fixture","storage":"grave","items":[{"id":"recovery_item.fixture","amount":{"kind":"quantity","quantity":2}}]}),
            json!({"kind":"recovery_bank_all","records":[{"death":"death.fixture","items":["recovery_item.fixture"]}]}),
            json!({"kind":"recovery_take_all","selection":{"context":{"kind":"death_office","interface":"interface.fixture","instance":"instance.fixture"},"records":[{"death":"death.fixture","entries":[{"id":"recovery_item.fixture","quantity":7,"current_storage":"grave"}]}]}}),
            json!({"kind":"ui_document_page","document_id":"document.fixture","page":1}),
            json!({"kind":"bank_placeholder","entry_id":"9007199254740993"}),
            json!({"kind":"ui_dismiss","presentation_id":"presentation.fixture"}),
            json!({"kind":"production_select","menu_id":"menu.fixture","recipe":"recipe.fixture","quantity":1,"mode":"single"}),
            json!({"kind":"item_action","inventory_slot":3,"expected_item":"item.fixture","expected_instance":"item_instance.fixture","action":"action.fixture"}),
            json!({"kind":"bank_select_tab","tab":2}),
            json!({"kind":"bank_create_tab","entry_id":"9007199254740993"}),
            json!({"kind":"bank_move","entry_id":"9007199254740993","before_entry_id":"9007199254740994","tab":2}),
            json!({"kind":"bank_collapse_tab","tab":2}),
            json!({"kind":"bank_set_insert","enabled":true}),
            json!({"kind":"bank_set_placeholders","enabled":true}),
            json!({"kind":"bank_release_placeholder","entry_id":"9007199254740993"}),
            json!({"kind":"bank_deposit_equipment"}),
            json!({"kind":"bank_withdraw_entry","entry_id":"9007199254740993","quantity":1,"noted":false}),
            json!({"kind":"bank_set_options","amount":5,"noted":true}),
            json!({"kind":"open_death_preview"}),
            json!({"kind":"request_recovery_discard","death":"death.fixture","storage":"grave","items":["recovery_item.fixture"]}),
            json!({"kind":"coffer_offer","inventory_slot":4,"expected_item":"item.fixture","expected_instance":null,"quantity":1}),
            json!({"kind":"ui_confirm","confirmation_id":"confirmation.fixture","accept":true}),
            json!({"kind":"public_chat","channel":"public","text":"Exact source public text"}),
        ];
        assert_eq!(requests.len(), 25);
        for mut input in requests {
            let request: GameplayUiRequest = serde_json::from_value(input.clone()).unwrap();
            if request.requires_bank_revision() {
                input["expected_bank_revision"] = json!("9007199254741993");
            }
            let (request, revision) = parse(&input.to_string()).unwrap().unwrap();
            let encoded = wire(request, revision).unwrap();
            let bytes = encoded.encode_to_vec();
            let decoded = game::GameplayUiRequest::decode(bytes.as_slice()).unwrap();
            assert_eq!(json(&decoded).unwrap(), input);
            if input.get("expected_bank_revision").is_some() {
                assert!(
                    bytes.windows(2).any(|tag| tag == [0xaa, 0x01]),
                    "field21 is retained"
                );
            }
            let outer = game::WorldInput {
                action: Some(game::world_input::Action::Ui(decoded)),
                ..Default::default()
            }
            .encode_to_vec();
            assert_eq!(&outer[..2], &[0xca, 0x02], "WorldInput.ui is tag41");
        }
    }

    #[test]
    fn ui_requests_reject_missing_or_numeric_bank_preconditions_without_client_price_or_tick_fallbacks()
     {
        let request = GameplayUiRequest::BankDepositEquipment;
        assert!(wire(request.clone(), None).is_err());
        for revision in ["01", "-1", "18446744073709551615"] {
            assert!(wire(request.clone(), Some(revision.into())).is_err());
        }
        assert!(
            parse(r#"{"kind":"bank_deposit_equipment","expected_bank_revision":9007199254740993}"#)
                .is_err()
        );
        assert!(wire(GameplayUiRequest::OpenDeathPreview, Some("2".into())).is_err());
    }
}

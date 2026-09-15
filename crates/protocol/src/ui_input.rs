use crate::game_input::{bounded_text, inventory_slot, quantity};
use crate::{ReadOnlyQuote, ValidationError, game, invalid, quote_request};
use clubscape_game_types::{
    DeathId, GameplayUiRequest, ItemId, ItemInstanceId, ProductionMode, RecoveryItemAmount,
    RecoveryItemId, RecoveryRecordSelection, RecoveryStorage, UiAmount,
};
use std::collections::BTreeSet;

fn amount(value: Option<&game::UiAmount>) -> Result<UiAmount, ValidationError> {
    match value.and_then(|value| value.selection.as_ref()) {
        Some(game::ui_amount::Selection::Quantity(value)) => Ok(UiAmount::Quantity {
            quantity: quantity(*value)?,
        }),
        Some(game::ui_amount::Selection::All(_)) => Ok(UiAmount::All {}),
        None => Err(invalid("An explicit quantity or All amount is required.")),
    }
}

fn death(value: &str) -> Result<DeathId, ValidationError> {
    DeathId::new(value).map_err(|_| invalid("An owned source death identity is required."))
}

fn recovery_id(value: &str) -> Result<RecoveryItemId, ValidationError> {
    RecoveryItemId::new(value).map_err(|_| invalid("An owned recovery entry identity is required."))
}

fn storage(value: i32) -> Result<RecoveryStorage, ValidationError> {
    match game::RecoveryStorage::try_from(value) {
        Ok(game::RecoveryStorage::Grave) => Ok(RecoveryStorage::Grave),
        Ok(game::RecoveryStorage::DeathOffice) => Ok(RecoveryStorage::DeathOffice),
        _ => Err(invalid("A source recovery storage is required.")),
    }
}

fn entry(value: &str) -> Result<String, ValidationError> {
    let number = value
        .parse::<u64>()
        .ok()
        .filter(|number| *number > 0 && *number <= i64::MAX as u64 && number.to_string() == value)
        .ok_or_else(|| invalid("A canonical bank entry identity is required."))?;
    Ok(number.to_string())
}
fn tab(value: u32) -> Result<u8, ValidationError> {
    u8::try_from(value)
        .ok()
        .filter(|value| *value <= 9)
        .ok_or_else(|| invalid("Source bank tab is out of range."))
}
fn item(value: &str) -> Result<ItemId, ValidationError> {
    ItemId::new(value).map_err(|_| invalid("A source item identity is required."))
}
fn instance(value: &Option<String>) -> Result<Option<ItemInstanceId>, ValidationError> {
    value
        .as_ref()
        .map(|value| {
            ItemInstanceId::new(value).map_err(|_| invalid("Invalid source item instance."))
        })
        .transpose()
}

pub fn ui_request(value: &game::GameplayUiRequest) -> Result<GameplayUiRequest, ValidationError> {
    use game::gameplay_ui_request::Request as R;
    let request = match value
        .request
        .as_ref()
        .ok_or_else(|| invalid("A typed UI request is required."))?
    {
        R::Dismiss(value) => GameplayUiRequest::UiDismiss {
            presentation_id: bounded_text(&value.id, 192)?,
        },
        R::DocumentPage(value) => GameplayUiRequest::UiDocumentPage {
            document_id: bounded_text(&value.id, 192)?,
            page: u16::try_from(value.page)
                .ok()
                .filter(|page| *page < 128)
                .ok_or_else(|| invalid("Document page is out of range."))?,
        },
        R::Production(value) => GameplayUiRequest::ProductionSelect {
            menu_id: bounded_text(&value.menu_id, 192)?,
            recipe: clubscape_game_types::RecipeId::new(&value.recipe)
                .map_err(|_| invalid("Invalid source recipe."))?,
            quantity: quantity(value.quantity)?.get(),
            mode: match game::ProductionMode::try_from(value.mode) {
                Ok(game::ProductionMode::Single) if value.quantity == 1 => ProductionMode::Single,
                Ok(game::ProductionMode::MakeX) => ProductionMode::MakeX,
                _ => {
                    return Err(invalid(
                        "Single requires quantity one; Make-X must be explicit.",
                    ));
                }
            },
        },
        R::ProductionAll(value) => GameplayUiRequest::ProductionSelectAll {
            menu_id: bounded_text(&value.menu_id, 192)?,
            recipe: clubscape_game_types::RecipeId::new(&value.recipe)
                .map_err(|_| invalid("Invalid source recipe."))?,
        },
        R::BankAmount(value) => GameplayUiRequest::BankSetAmount {
            amount: amount(value.amount.as_ref())?,
            noted: value.noted,
        },
        R::RecoveryTake(value) => {
            let items = value
                .items
                .iter()
                .map(|entry| {
                    Ok(RecoveryItemAmount {
                        id: recovery_id(&entry.id)?,
                        amount: amount(entry.amount.as_ref())?,
                    })
                })
                .collect::<Result<Vec<_>, ValidationError>>()?;
            if items.is_empty()
                || items.len() > 4096
                || items
                    .iter()
                    .map(|entry| &entry.id)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != items.len()
            {
                return Err(invalid("Select distinct bounded recovery entries."));
            }
            GameplayUiRequest::RecoveryTake {
                death: death(&value.death)?,
                storage: storage(value.storage)?,
                items,
            }
        }
        R::RecoveryBankAll(value) => {
            let records = value
                .records
                .iter()
                .map(|record| {
                    let items = record
                        .items
                        .iter()
                        .map(|id| recovery_id(id))
                        .collect::<Result<Vec<_>, _>>()?;
                    if items.is_empty()
                        || items.iter().collect::<BTreeSet<_>>().len() != items.len()
                    {
                        return Err(invalid("Select distinct nonempty recovery records."));
                    }
                    Ok(RecoveryRecordSelection {
                        death: death(&record.death)?,
                        items,
                    })
                })
                .collect::<Result<Vec<_>, ValidationError>>()?;
            if records.is_empty()
                || records.len() > 4096
                || records
                    .iter()
                    .map(|record| &record.death)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != records.len()
                || records
                    .iter()
                    .map(|record| record.items.len())
                    .sum::<usize>()
                    > 4096
            {
                return Err(invalid("Select distinct bounded recovery records."));
            }
            GameplayUiRequest::RecoveryBankAll { records }
        }
        R::ItemAction(value) => GameplayUiRequest::ItemAction {
            inventory_slot: inventory_slot(value.inventory_slot)?,
            expected_item: item(&value.expected_item)?,
            expected_instance: instance(&value.expected_instance)?,
            action: bounded_text(&value.action, 64)?,
        },
        R::SelectTab(value) => GameplayUiRequest::BankSelectTab {
            tab: tab(value.tab)?,
        },
        R::CreateTab(value) => GameplayUiRequest::BankCreateTab {
            entry_id: entry(&value.id)?,
        },
        R::BankMove(value) => GameplayUiRequest::BankMove {
            entry_id: entry(&value.entry_id)?,
            before_entry_id: value.before_entry_id.as_deref().map(entry).transpose()?,
            tab: tab(value.tab)?,
        },
        R::CollapseTab(value) => GameplayUiRequest::BankCollapseTab {
            tab: tab(value.tab)?,
        },
        R::InsertMode(value) => GameplayUiRequest::BankSetInsert {
            enabled: value.enabled,
        },
        R::Placeholders(value) => GameplayUiRequest::BankSetPlaceholders {
            enabled: value.enabled,
        },
        R::ReleasePlaceholder(value) => GameplayUiRequest::BankReleasePlaceholder {
            entry_id: entry(&value.id)?,
        },
        R::Placeholder(value) => GameplayUiRequest::BankPlaceholder {
            entry_id: entry(&value.id)?,
        },
        R::DepositEquipment(_) => GameplayUiRequest::BankDepositEquipment,
        R::BankWithdraw(value) => GameplayUiRequest::BankWithdrawEntry {
            entry_id: entry(&value.entry_id)?,
            quantity: quantity(value.quantity)?.get(),
            noted: value.noted,
        },
        R::BankOptions(value) => GameplayUiRequest::BankSetOptions {
            amount: quantity(value.amount)?.get(),
            noted: value.noted,
        },
        R::DeathPreview(_) => GameplayUiRequest::OpenDeathPreview,
        R::DiscardRecovery(value) => {
            let ReadOnlyQuote::Recovery {
                death,
                storage,
                items,
            } = quote_request(&game::QuoteRequest {
                request: Some(game::quote_request::Request::Recovery(value.clone())),
            })?
            else {
                unreachable!()
            };
            GameplayUiRequest::RequestRecoveryDiscard {
                death,
                storage,
                items,
            }
        }
        R::CofferOffer(value) => GameplayUiRequest::CofferOffer {
            inventory_slot: inventory_slot(value.inventory_slot)?,
            expected_item: item(&value.expected_item)?,
            expected_instance: instance(&value.expected_instance)?,
            quantity: quantity(value.quantity)?.get(),
        },
        R::Confirmation(value) => GameplayUiRequest::UiConfirm {
            confirmation_id: bounded_text(&value.id, 192)?,
            accept: value.accept,
        },
        R::PublicChat(value) => {
            if value.channel != "public" {
                return Err(invalid("Only public chat is supported."));
            }
            GameplayUiRequest::PublicChat {
                channel: value.channel.clone(),
                text: bounded_text(&value.text, 512)?,
            }
        }
    };
    if request.requires_bank_revision() != ui_bank_revision(value)?.is_some() {
        return Err(invalid(
            "Bank controls require their current bank revision; other UI requests must omit it.",
        ));
    }
    Ok(request)
}

pub fn ui_bank_revision(value: &game::GameplayUiRequest) -> Result<Option<u64>, ValidationError> {
    value
        .expected_bank_revision
        .as_ref()
        .map(|value| {
            value
                .parse::<u64>()
                .ok()
                .filter(|number| *number <= i64::MAX as u64 && number.to_string() == *value)
                .ok_or_else(|| invalid("A canonical decimal bank revision is required."))
        })
        .transpose()
}

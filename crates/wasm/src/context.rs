use std::collections::BTreeSet;

use clubscape_game_types::{INVENTORY_SLOTS, MAX_STACK_QUANTITY, SlotId};
use clubscape_protocol::game;
use serde_json::{Value, json};

use crate::{BridgeError, catalog::DisplayCatalog, view};

pub(crate) fn denial(value: &game::RuleDenial) -> Result<Value, BridgeError> {
    let code = game::RuleErrorCode::try_from(value.code)
        .map_err(|_| BridgeError::protocol("The server sent an unknown source denial code."))?;
    if code == game::RuleErrorCode::RuleErrorUnspecified {
        return Err(BridgeError::protocol(
            "The server sent an unspecified source denial.",
        ));
    }
    Ok(json!({"code":value.code,"name":code.as_str_name(),"message":value.message}))
}

pub(crate) fn permission(value: Option<&game::Permission>) -> Result<Value, BridgeError> {
    let Some(value) = value else {
        return Ok(json!({"allowed":null,"denial":null}));
    };
    if value.allowed == value.denial.is_some() {
        return Err(BridgeError::protocol(
            "The server sent an inconsistent source permission.",
        ));
    }
    Ok(json!({
        "allowed":value.allowed,
        "denial":value.denial.as_ref().map(denial).transpose()?,
    }))
}

pub(crate) fn presence(value: Option<&game::Presence>) -> Result<Value, BridgeError> {
    let Some(value) = value else {
        return Ok(Value::Null);
    };
    let kind = match game::PresenceKind::try_from(value.kind) {
        Ok(game::PresenceKind::Connected) => "connected",
        Ok(game::PresenceKind::Disconnecting) => "disconnecting",
        Ok(game::PresenceKind::Offline) => "offline",
        _ => {
            return Err(BridgeError::protocol(
                "The server sent an untracked/unknown presence state.",
            ));
        }
    };
    if (kind == "connected") != value.connected
        || (value.accepts_input && !value.connected)
        || (kind == "offline" && value.present_in_world)
    {
        return Err(BridgeError::protocol(
            "The server sent inconsistent presence facts.",
        ));
    }
    Ok(json!({
        "kind":kind,"connected":value.connected,
        "acceptsInput":value.accepts_input,"presentInWorld":value.present_in_world,
    }))
}

pub(crate) fn bank(
    snapshot: &game::WorldSnapshot,
    catalog: &DisplayCatalog,
) -> Result<Value, BridgeError> {
    let Some(context) = snapshot.bank_context.as_ref() else {
        return Ok(Value::Null);
    };
    let player = snapshot
        .player
        .as_ref()
        .ok_or_else(|| BridgeError::protocol("Bank view has no local player."))?;
    if !player.bank_open || player.bank_capacity == 0 || player.bank_capacity > 4096 {
        return Err(BridgeError::protocol(
            "The bank context has no valid authorized container.",
        ));
    }
    let mut slots: Vec<_> = (0..player.bank_capacity)
        .map(|index| json!({"index":index,"item":null}))
        .collect();
    let mut indices = BTreeSet::new();
    for slot in &player.bank {
        if !indices.insert(slot.index) {
            return Err(BridgeError::protocol(
                "The bank view contains duplicate slots.",
            ));
        }
        let item = view::stack(
            slot.stack
                .as_ref()
                .ok_or_else(|| BridgeError::protocol("Bank stack is missing."))?,
            catalog,
        )?;
        let destination = slots
            .get_mut(slot.index as usize)
            .ok_or_else(|| BridgeError::protocol("A bank slot exceeds its authorized capacity."))?;
        *destination = json!({"index":slot.index,"item":item});
    }
    Ok(json!({
        "banker":context.banker,"capacity":player.bank_capacity,"slots":slots,
        // This is wire request support, never per-item eligibility or a promised transfer.
        "allowNotes":true,
        "interfaceId":context.interface,
        "deposit":permission(context.deposit.as_ref())?,
        "withdraw":permission(context.withdraw.as_ref())?,
    }))
}

pub(crate) fn shop(
    value: Option<&game::ShopView>,
    catalog: &DisplayCatalog,
) -> Result<Value, BridgeError> {
    let Some(value) = value else {
        return Ok(Value::Null);
    };
    let definition = catalog
        .shops
        .get(&value.shop)
        .ok_or_else(|| BridgeError::protocol("Shop display metadata is missing."))?;
    if value.currency != definition.currency || !catalog.items.contains_key(&value.currency) {
        return Err(BridgeError::protocol(
            "The shop currency does not match its source identity.",
        ));
    }
    let mut indices = BTreeSet::new();
    let mut items = BTreeSet::new();
    let rows = value
        .lines
        .iter()
        .map(|line| {
            if line.index >= 4096
                || !indices.insert(line.index)
                || !items.insert(&line.item)
                || line.stock > MAX_STACK_QUANTITY
            {
                return Err(BridgeError::protocol(
                    "Shop row identities/stock are invalid or duplicated.",
                ));
            }
            let item = catalog
                .items
                .get(&line.item)
                .ok_or_else(|| BridgeError::protocol("Shop item metadata is missing."))?;
            Ok(json!({
                "index":line.index,"itemId":line.item,
                "item":{
                    "id":line.item,"name":item.name,"quantity":line.stock,
                    "sourceId":item.source_id,"iconAsset":catalog.icons.get(&line.item),
                    "instanceId":null,"charges":null,"actions":[],
                },
                "stock":line.stock,"buyPrice":line.buy_price,"sellPrice":line.sell_price,
            }))
        })
        .collect::<Result<Vec<_>, BridgeError>>()?;
    Ok(json!({
        "id":value.shop,"name":definition.name,"currency":value.currency,
        "interfaceId":value.interface,"rows":rows,
    }))
}

pub(crate) fn storage(value: i32) -> Result<&'static str, BridgeError> {
    match game::RecoveryStorage::try_from(value) {
        Ok(game::RecoveryStorage::Grave) => Ok("grave"),
        Ok(game::RecoveryStorage::DeathOffice) => Ok("death_office"),
        _ => Err(BridgeError::protocol(
            "The server sent an unknown recovery storage.",
        )),
    }
}

pub(crate) fn recovery(
    value: Option<&game::RecoveryContext>,
    catalog: &DisplayCatalog,
) -> Result<Value, BridgeError> {
    let Some(value) = value else {
        return Ok(Value::Null);
    };
    let mut panels = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let views = value.views.iter().map(|panel| {
        if !panels.insert((&panel.death, panel.storage)) {
            return Err(BridgeError::protocol("Recovery panels contain duplicate identities."));
        }
        let entries = panel.entries.iter().map(|entry| {
            if !ids.insert(&entry.id) {
                return Err(BridgeError::protocol("Recovery entries contain duplicate identities."));
            }
            let location = match entry.layout.as_ref().and_then(|layout| layout.location.as_ref()) {
                Some(game::item_layout::Location::InventorySlot(slot)) if (*slot as usize) < INVENTORY_SLOTS => {
                    json!({"kind":"inventory","slot":slot})
                }
                Some(game::item_layout::Location::EquipmentSlot(slot)) if SlotId::new(slot).is_ok() => {
                    json!({"kind":"equipment","slot":slot})
                }
                _ => return Err(BridgeError::protocol("A recovery item has no valid source layout.")),
            };
            Ok(json!({
                "id":entry.id,
                "item":view::stack(entry.stack.as_ref().ok_or_else(|| BridgeError::protocol("Recovery stack is missing."))?, catalog)?,
                "cost":null,"fullEntryFee":entry.full_entry_fee.to_string(),
                "storage":storage(entry.current_storage)?,"layout":location,
            }))
        }).collect::<Result<Vec<_>, BridgeError>>()?;
        Ok(json!({
            "death":panel.death,"storage":storage(panel.storage)?,"interfaceId":panel.interface,
            "items":entries,"remainingTicks":panel.active_ticks_remaining,
        }))
    }).collect::<Result<Vec<_>, BridgeError>>()?;
    Ok(json!({"views":views}))
}

pub(crate) fn quote(
    value: &game::Quote,
    catalog: &DisplayCatalog,
    revision: u64,
    tick: u64,
) -> Result<Value, BridgeError> {
    use game::quote::Result as Kind;
    let mut result = match value.result.as_ref() {
        Some(Kind::Bank(quote)) => json!({
            "kind":"bank","requested":quote.requested,
            "transferred":view::stack(quote.transferred.as_ref().ok_or_else(|| BridgeError::protocol("Bank quote has no transfer."))?, catalog)?,
        }),
        Some(Kind::Shop(quote)) => {
            if !catalog.items.contains_key(&quote.item)
                || !catalog.items.contains_key(&quote.currency)
            {
                return Err(BridgeError::protocol(
                    "Shop quote item/currency metadata is missing.",
                ));
            }
            json!({
                "kind":"shop","shop":quote.shop,"itemId":quote.item,"requested":quote.requested,
                "quantity":quote.quantity,"currency":quote.currency,"totalPrice":quote.total_price,
                "stockAfter":quote.stock_after,
                "partialReason":quote.partial_reason.as_ref().map(denial).transpose()?,
            })
        }
        Some(Kind::Recovery(quote)) => json!({
            "kind":"recovery","death":quote.death,"storage":storage(quote.storage)?,
            "selected":quote.selected,"fullSelectionFee":quote.full_selection_fee.to_string(),
        }),
        None => return Err(BridgeError::protocol("The quote has no typed result.")),
    };
    result["revision"] = json!(revision.to_string());
    result["tick"] = json!(tick.to_string());
    Ok(result)
}

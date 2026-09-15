use clubscape_game_types::{
    GAMEPLAY_UI_VIEW_VERSION, GameplayUiView, InventoryActionsUiView, UiItem,
};
use serde_json::{Value, json};

use crate::BridgeError;

pub const CAPABILITY: &str = "game.ui.v1";
pub const WIRE_SUPPORTED: bool = true;

pub(crate) fn decimal(value: &str, signed: bool) -> Result<&str, BridgeError> {
    let digits = if signed {
        value.strip_prefix('-').unwrap_or(value)
    } else {
        value
    };
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || digits.parse::<u64>().is_err()
    {
        return Err(BridgeError::protocol(
            "The authoritative UI contains an invalid decimal-string value.",
        ));
    }
    Ok(value)
}

fn item(value: &UiItem) -> Result<Value, BridgeError> {
    if value.quantity == 0 || value.quantity > clubscape_game_types::MAX_STACK_QUANTITY {
        return Err(BridgeError::protocol(
            "The authoritative UI item quantity is invalid; placeholders must have no value.",
        ));
    }
    Ok(json!({
        "id":value.item,"name":value.name,"quantity":value.quantity,
        "sourceId":value.source_id,"iconAsset":value.asset,"instanceId":value.instance_id,
        "charges":value.charges,"actions":[],
    }))
}

fn items(values: &[UiItem]) -> Result<Vec<Value>, BridgeError> {
    values.iter().map(item).collect()
}

fn inventory_actions(values: &[InventoryActionsUiView]) -> Vec<Value> {
    values.iter().map(|entry| json!({
        "slot":entry.slot,"item":entry.item,"instance":entry.instance,"actions":entry.actions,
    })).collect()
}

/// Pure shared-Rust-DTO to shared-TypeScript-DTO projection.
/// No JS setter or RPC accepts these values.
pub fn project(value: &GameplayUiView) -> Result<Value, BridgeError> {
    if value.version != GAMEPLAY_UI_VIEW_VERSION {
        return Err(BridgeError::protocol(
            "Unsupported authoritative gameplay UI view version.",
        ));
    }
    let production =
        value
            .production
            .as_ref()
            .map(|view| {
                let recipes = view.recipes.iter().map(|recipe| Ok(json!({
            "recipe":recipe.recipe,"name":recipe.name,"outputs":items(&recipe.outputs)?,
            "single":recipe.single,"makeX":recipe.make_x,
        }))).collect::<Result<Vec<_>, BridgeError>>()?;
                Ok::<_, BridgeError>(json!({
                    "id":view.id,"interface":view.interface,"target":view.target,"recipes":recipes,
                }))
            })
            .transpose()?;
    let reward = value.reward.as_ref().map(|view| {
        let xp = view.xp.iter().map(|award| Ok(json!({
            "skill":award.skill,"amountTenths":decimal(&award.amount_tenths, false)?,
        }))).collect::<Result<Vec<_>, BridgeError>>()?;
        Ok::<_, BridgeError>(json!({
            "id":view.id,"kind":view.kind,"interface":view.interface,"title":view.title,
            "lines":view.lines,"items":items(&view.items)?,"xp":xp,"questPoints":view.quest_points,
            "quest":view.quest,"skill":view.skill,"level":view.level,"continuation":view.continuation,
        }))
    }).transpose()?;
    let confirmation = value.confirmation.as_ref().map(|view| Ok::<_, BridgeError>(json!({
        "id":view.id,"kind":view.kind,"title":view.title,"lines":view.lines,
        "items":items(&view.items)?,"credit":view.credit.as_deref().map(|value| decimal(value, false)).transpose()?,
    }))).transpose()?;
    let bank = value.bank.as_ref().map(|view| {
        let mut entries = std::collections::BTreeSet::new();
        let values = view.entries.iter().map(|entry| {
            if !entries.insert(&entry.id) || entry.placeholder != entry.value.is_none()
                || entry.value.as_ref().is_some_and(|value| value.item != entry.item)
            {
                return Err(BridgeError::protocol("Invalid authoritative bank entry identity or placeholder shape."));
            }
            Ok(json!({
                "id":entry.id,"slot":entry.slot,"tab":entry.tab,"item":entry.item,
                "value":entry.value.as_ref().map(item).transpose()?,"placeholder":entry.placeholder,
            }))
        }).collect::<Result<Vec<_>, BridgeError>>()?;
        Ok::<_, BridgeError>(json!({
            "revision":decimal(&view.revision, false)?,"capacity":view.capacity,"selectedTab":view.selected_tab,
            "insertMode":view.insert_mode,"placeholders":view.placeholders,"amount":view.amount,"noted":view.noted,
            "tabs":view.tabs.iter().map(|tab| json!({"tab":tab.tab,"firstEntry":tab.first_entry,"entries":tab.entries})).collect::<Vec<_>>(),
            "entries":values,"depositEquipment":view.deposit_equipment,"unavailableContainers":view.unavailable_containers,
        }))
    }).transpose()?;
    let death = value
        .kept_on_death
        .as_ref()
        .map(|view| {
            if view.scope != "normal_unsafe_non_pvp" {
                return Err(BridgeError::protocol(
                    "The death preview has an unsupported source scope.",
                ));
            }
            Ok(json!({
                "scope":view.scope,"kept":items(&view.kept)?,"lost":items(&view.lost)?,
                "fullGraveFee":decimal(&view.full_grave_fee, false)?,
                "fullOfficeFee":decimal(&view.full_office_fee, false)?,
                "valueRevision":decimal(&view.value_revision, false)?,
            }))
        })
        .transpose()?;
    let recovery = value.recovery.as_ref().map(|view| Ok::<_, BridgeError>(json!({
        "cofferBalance":decimal(&view.coffer_balance, false)?,"discard":view.discard,"cofferOffer":view.coffer_offer,
        "cofferItems":inventory_actions(&view.coffer_items),
    }))).transpose()?;
    if value.public_chat.channel != "public"
        || value
            .public_chat
            .messages
            .iter()
            .any(|line| line.channel != "public")
    {
        return Err(BridgeError::protocol(
            "The public UI chat projection contains an unsupported channel.",
        ));
    }
    let bonuses = &value.equipment.bonuses;
    Ok(json!({
        "version":value.version,"activeTab":value.active_tab,"activeInterface":value.active_interface,
        "production":production,"reward":reward,"confirmation":confirmation,
        "document":value.document.as_ref().map(|document| json!({
            "id":document.id,"interface":document.interface,"title":document.title,"pages":document.pages,
            "page":document.page,"mapAsset":document.map_asset,"nativeMap":document.native_map,
        })),
        "interfaces":value.interfaces,"combatStyle":value.combat_style,"combatStyles":value.combat_styles,
        "prayers":value.prayers,"spells":value.spells,
        "equipment":{
            "bonuses":{"attack":bonuses.attack,"defence":bonuses.defence,"meleeStrength":bonuses.melee_strength,
                "rangedStrength":bonuses.ranged_strength,"magicDamagePercent":bonuses.magic_damage_percent,"prayer":bonuses.prayer},
            "weightGrams":decimal(&value.equipment.weight_grams, true)?,"slots":value.equipment.slots,
        },
        "inventoryActions":inventory_actions(&value.inventory_actions),
        "bank":bank,"keptOnDeath":death,"recovery":recovery,
        "appearance":{"choices":value.appearance.choices,"confirmed":value.appearance.confirmed,
            "base":value.appearance.base.as_ref().map(|base| json!({"asset":base.asset,"sourceNpc":base.source_npc,"adaptation":base.adaptation}))},
        "publicChat":{"permission":value.public_chat.permission,"maximumBytes":value.public_chat.maximum_bytes,
            "channel":value.public_chat.channel,"messages":value.public_chat.messages},
    }))
}

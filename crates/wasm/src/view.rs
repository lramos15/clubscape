use clubscape_protocol::game;
use serde_json::{Value, json};

use crate::{BridgeError, catalog::DisplayCatalog};

fn missing() -> BridgeError {
    BridgeError::protocol("The public state is missing required source display metadata.")
}

fn tile(value: Option<&game::Tile>) -> Result<Value, BridgeError> {
    let value = value.ok_or_else(missing)?;
    if value.x > u32::from(u16::MAX) || value.y > u32::from(u16::MAX) || value.plane > 3 {
        return Err(BridgeError::protocol(
            "A public tile is outside the protocol bounds.",
        ));
    }
    Ok(json!({"x":value.x, "y":value.y, "plane":value.plane}))
}

fn stack(value: &game::Stack, catalog: &DisplayCatalog) -> Result<Value, BridgeError> {
    let definition = catalog.items.get(&value.item).ok_or_else(missing)?;
    if value.quantity == 0 || value.quantity > clubscape_game_types::MAX_STACK_QUANTITY {
        return Err(BridgeError::protocol("A public item quantity is invalid."));
    }
    Ok(json!({
        "id":value.item, "name":definition.name, "quantity":value.quantity,
        "sourceId":definition.source_id, "iconAsset":catalog.icons.get(&value.item),
        "instanceId":value.instance_id, "charges":value.charges,
        // This protocol revision has no authorized inventory-menu projection.
        "actions":[],
    }))
}

fn setting(value: &game::SetSetting) -> Result<Value, BridgeError> {
    let name = match game::SettingKind::try_from(value.setting) {
        Ok(game::SettingKind::Run) => "run",
        Ok(game::SettingKind::AutoRetaliate) => "auto_retaliate",
        Ok(game::SettingKind::DeathAutoEquip) => "death_auto_equip",
        Ok(game::SettingKind::DeathSupplyPiles) => "death_supply_piles",
        _ => {
            return Err(BridgeError::protocol(
                "The server sent an unknown character setting.",
            ));
        }
    };
    Ok(json!({"setting":name,"enabled":value.enabled}))
}

pub(crate) fn world(
    value: &game::WorldSnapshot,
    catalog: &DisplayCatalog,
    messages: &[Value],
) -> Result<Value, BridgeError> {
    let player = value.player.as_ref().ok_or_else(missing)?;
    let mut inventory: Vec<Value> = (0..clubscape_game_types::INVENTORY_SLOTS)
        .map(|index| json!({"index":index,"item":null}))
        .collect();
    for slot in &player.inventory {
        let item = stack(slot.stack.as_ref().ok_or_else(missing)?, catalog)?;
        let cell = inventory.get_mut(slot.index as usize).ok_or_else(missing)?;
        *cell = json!({"index":slot.index,"item":item});
    }
    let equipment = catalog
        .equipment_slots
        .iter()
        .map(|id| {
            let item = player
                .equipment
                .iter()
                .find(|slot| &slot.slot == id)
                .and_then(|slot| slot.stack.as_ref())
                .map(|item| stack(item, catalog))
                .transpose()?;
            Ok(json!({"slot":id,"item":item}))
        })
        .collect::<Result<Vec<_>, BridgeError>>()?;
    if player
        .equipment
        .iter()
        .any(|slot| !catalog.equipment_slots.contains(&slot.slot))
    {
        return Err(missing());
    }
    let skills = player.skills.iter().map(|skill| {
        let definition = catalog.skills.get(&skill.id).ok_or_else(missing)?;
        Ok(json!({
            "id":skill.id,"name":definition.name,"xpTenths":skill.xp_tenths.to_string(),
            "baseLevel":skill.base_level,"currentLevel":skill.current_level,"iconAsset":catalog.icons.get(&skill.id),
        }))
    }).collect::<Result<Vec<_>, BridgeError>>()?;
    let quests = player
        .quests
        .iter()
        .map(|quest| {
            let definition = catalog.quests.get(&quest.id).ok_or_else(missing)?;
            Ok(json!({
                "id":quest.id,"name":definition.name,"stage":quest.stage,
                "journal":quest.journal,"completed":quest.stage == definition.completed_stage,
            }))
        })
        .collect::<Result<Vec<_>, BridgeError>>()?;
    let entities = value.entities.iter().map(|entity| {
        let kind = match game::EntityKind::try_from(entity.kind) {
            Ok(game::EntityKind::Npc) => "npc",
            Ok(game::EntityKind::Object) => "object",
            Ok(game::EntityKind::Player) => "player",
            _ => return Err(BridgeError::protocol("The server sent an unknown entity kind.")),
        };
        let definition = catalog.entities.get(&entity.definition_id);
        if kind != "player" && definition.is_none() { return Err(missing()); }
        let equipment = entity.equipment.iter().map(|slot| {
            let source_id = slot.stack.as_ref().map(|item| {
                catalog.items.get(&item.item).map(|definition| definition.source_id).ok_or_else(missing)
            }).transpose()?.flatten();
            Ok(json!({"slot":slot.slot,"sourceId":source_id}))
        }).collect::<Result<Vec<_>, BridgeError>>()?;
        Ok(json!({
            "id":entity.id,"definitionId":entity.definition_id,
            "sourceId":definition.and_then(|definition| definition.source_id),
            "name":entity.name,"kind":kind,"tile":tile(entity.tile.as_ref())?,"instance":entity.instance,
            "hitpoints":entity.hitpoints,"maxHitpoints":entity.max_hitpoints,"available":entity.available,
            "animation":entity.animation,"appearance":entity.appearance,"equipment":equipment,
            "actions":entity.actions.iter().map(|action| json!({
                "name":action,"allowed":entity.actions_evaluated,
                "reason":if entity.actions_evaluated {None} else {Some("The server has not evaluated this interaction's permissions.")},
            })).collect::<Vec<_>>(),
        }))
    }).collect::<Result<Vec<_>, BridgeError>>()?;
    let ground_items = value
        .ground_items
        .iter()
        .map(|item| {
            Ok(json!({
                "id":item.id,"tile":tile(item.tile.as_ref())?,
                "item":stack(item.stack.as_ref().ok_or_else(missing)?, catalog)?,
                "canTake":item.permissions_evaluated && item.can_take,
            }))
        })
        .collect::<Result<Vec<_>, BridgeError>>()?;
    let dialogue = value.dialogue.as_ref().map(|dialogue| json!({
        "id":dialogue.id,"speaker":dialogue.speaker,"speakerName":dialogue.speaker_name,
        "portraitAsset":null,"text":dialogue.text,
        "choices":dialogue.choices.iter().map(|choice| json!({"id":choice.id,"text":choice.text})).collect::<Vec<_>>(),
    }));
    let dynamic_objects = value
        .dynamic_objects
        .iter()
        .map(|object| {
            Ok(json!({
                "id":object.id,"definitionId":object.definition_id,"objectId":object.object_id,
                "tile":tile(object.tile.as_ref())?,"instance":object.instance,"state":object.state,
                "doorOpen":object.door_open,"quarterTurns":object.quarter_turns,
                "expiresAtTick":object.expires_at_tick.map(|tick| tick.to_string()),
            }))
        })
        .collect::<Result<Vec<_>, BridgeError>>()?;
    let mut unavailable_views: Vec<_> = value
        .unavailable_views
        .iter()
        .map(|view| json!({"view":view.view,"reason":view.reason}))
        .collect();
    if !player.inventory.is_empty() {
        unavailable_views.push(json!({
            "view":"inventory_actions",
            "reason":"This generated protocol revision has no source inventory-menu projection; item actions remain unavailable.",
        }));
    }
    Ok(json!({
        "revision":value.revision.to_string(),"tick":value.tick.to_string(),
        "player":{
            "id":player.actor_id,"displayName":player.display_name,"appearance":player.appearance,
            "region":player.region,"tile":tile(player.tile.as_ref())?,"instance":player.instance,
            "inventory":inventory,"equipment":equipment,"skills":skills,
            "hitpoints":player.hitpoints,"prayerPoints":player.prayer_points,"runEnergy":player.run_energy,
            "questPoints":player.quest_points,"tutorialStage":player.tutorial_stage,
            "tutorialInstruction":player.tutorial_instruction,"quests":quests,
            "unlockedInterfaces":player.unlocked_interfaces,"activePrayers":player.active_prayers,
            "activity":player.activity,"animation":player.animation,
            "settings":player.settings.iter().map(setting).collect::<Result<Vec<_>,_>>()?,
            "appearanceConfirmed":player.appearance_confirmed,"experience":player.experience,
            "combatStyle":player.combat_style,"activeDeath":player.active_death,
        },
        "entities":entities,"groundItems":ground_items,"dialogue":dialogue,
        "bank":null,"shop":null,"recovery":null,"messages":messages,
        "dynamicObjects":dynamic_objects,
        "unavailableViews":unavailable_views,
        "eventHistoryGap":value.event_history_gap,
        "eventHistoryFloorRevision":value.event_history_floor_revision.to_string(),
    }))
}

pub(crate) fn audio(event: &game::Event) -> Option<Value> {
    let (kind, asset) = match event.kind.as_str() {
        "sound" => ("sound", Some(event.sound_asset.as_str())),
        "animation" => ("animation", Some(event.animation_asset.as_str())),
        "interface_closed" => ("interface_closed", None),
        "music" | "jingle" | "quest_complete" | "level_up" => (
            event.kind.as_str(),
            (!event.sound_asset.is_empty()).then_some(event.sound_asset.as_str()),
        ),
        _ => return None,
    };
    Some(json!({
        "id":event.event_id,"kind":kind,"sourceId":null,"assetId":asset.filter(|id| !id.is_empty()),
        "actorId":(!event.actor_id.is_empty()).then_some(&event.actor_id),
        "tile":null,"sourceCycle":null,
        "payload":{"target":event.target,"text":event.text,"skill":event.skill,
            "xpTenths":event.xp_tenths.to_string(),"damage":event.damage},
    }))
}

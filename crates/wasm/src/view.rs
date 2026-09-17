use clubscape_protocol::game;
use serde_json::{Value, json};

use crate::{
    BridgeError, audio_authority, catalog::DisplayCatalog, context, observer, scene, ui_wire,
};

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

pub(crate) fn stack(value: &game::Stack, catalog: &DisplayCatalog) -> Result<Value, BridgeError> {
    let definition = catalog.items.get(&value.item).ok_or_else(missing)?;
    if value.quantity == 0 || value.quantity > clubscape_game_types::MAX_STACK_QUANTITY {
        return Err(BridgeError::protocol("A public item quantity is invalid."));
    }
    Ok(json!({
        "id":value.item, "name":definition.name, "quantity":value.quantity,
        "sourceId":definition.source_id, "iconAsset":catalog.icons.get(&value.item),
        "instanceId":value.instance_id, "charges":value.charges,
        "actions":catalog.inventory_actions.get(&value.item).cloned().unwrap_or_default(),
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
    if [
        value.dialogue.is_some(),
        value.bank_context.is_some(),
        value.shop.is_some(),
        value.recovery.is_some(),
    ]
    .into_iter()
    .filter(|present| *present)
    .count()
        > 1
    {
        return Err(BridgeError::protocol(
            "The server sent incompatible simultaneous interface contexts.",
        ));
    }
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
            Ok(game::EntityKind::Object) if clubscape_game_types::DynamicObjectId::new(&entity.id).is_ok() => "temporary_object",
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
        let actions = if entity.interaction_options.is_empty() {
            entity.actions.iter().map(|action| json!({
                "name":action,"allowed":entity.actions_evaluated,
                "reason":if entity.actions_evaluated {None} else {Some("The server has not evaluated this interaction's permissions.")},
                "denial":null,
            })).collect::<Vec<_>>()
        } else {
            entity.interaction_options.iter().map(|option| {
                let permission = context::permission(option.permission.as_ref())?;
                let allowed = entity.actions_evaluated && permission["allowed"] == true;
                let reason = if allowed {
                    Value::Null
                } else {
                    permission["denial"]["message"].as_str()
                        .map(|message| json!(message))
                        .unwrap_or_else(|| json!("The server has not supplied an evaluated permission for this option."))
                };
                Ok(json!({"name":option.name,"allowed":allowed,"reason":reason,"denial":permission["denial"]}))
            }).collect::<Result<Vec<_>, BridgeError>>()?
        };
        let mut result = json!({
            "id":entity.id,"definitionId":entity.definition_id,
            "sourceId":definition.and_then(|definition| definition.source_id),
            "name":entity.name,"kind":kind,"tile":tile(entity.tile.as_ref())?,"instance":entity.instance,
            "hitpoints":entity.hitpoints,"maxHitpoints":entity.max_hitpoints,"available":entity.available,
            "animation":entity.animation,"appearance":entity.appearance,"equipment":equipment,
            "actions":actions,"assetId":entity.asset,"width":entity.width,"height":entity.height,
            "presence":context::presence(entity.presence.as_ref())?,
        });
        result.as_object_mut().expect("entity object").extend(observer::fields(
            entity.running, &entity.movement_tick, entity.action.as_ref(),
        )?);
        Ok(result)
    }).collect::<Result<Vec<_>, BridgeError>>()?;
    let ground_items = value
        .ground_items
        .iter()
        .map(|item| {
            let permission = context::permission(item.permission.as_ref())?;
            Ok(json!({
                "id":item.id,"tile":tile(item.tile.as_ref())?,
                "item":stack(item.stack.as_ref().ok_or_else(missing)?, catalog)?,
                "canTake":item.permissions_evaluated && item.can_take && item.permission.as_ref().is_none_or(|permission| permission.allowed),
                "permission":permission,
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
            let source_id = object
                .object_id
                .as_ref()
                .map(|id| {
                    clubscape_game_types::ObjectId::new(id).map_err(|_| {
                        BridgeError::protocol("A dynamic object has no valid canonical object_id.")
                    })?;
                    catalog
                        .entities
                        .get(id)
                        .and_then(|definition| definition.source_id)
                        .ok_or_else(|| {
                            BridgeError::protocol(
                                "A dynamic object's canonical source metadata is unavailable.",
                            )
                        })
                })
                .transpose()?;
            if object.quarter_turns > 3 {
                return Err(BridgeError::protocol(
                    "A dynamic object rotation is outside its source quarter-turn range.",
                ));
            }
            let mut result = json!({
                "id":object.id,"definitionId":object.definition_id,
                "tile":tile(object.tile.as_ref())?,"instance":object.instance,"quarterTurns":object.quarter_turns,
                "expiresAtTick":object.expires_at_tick.map(|tick| tick.to_string()),
            });
            if let Some(id) = &object.object_id { result["objectId"] = json!(id); }
            if let Some(id) = source_id { result["sourceId"] = json!(id); }
            if let Some(state) = &object.state { result["state"] = json!(state); }
            if let Some(open) = object.door_open { result["doorOpen"] = json!(open); }
            Ok(result)
        })
        .collect::<Result<Vec<_>, BridgeError>>()?;
    let mut unavailable_views: Vec<_> = value
        .unavailable_views
        .iter()
        .map(|view| json!({"view":view.view,"reason":view.reason}))
        .collect();
    if player
        .inventory
        .iter()
        .filter_map(|slot| slot.stack.as_ref())
        .any(|stack| !catalog.inventory_actions.contains_key(&stack.item))
    {
        unavailable_views.push(json!({
            "view":"inventory_actions",
            "reason":"Original inventory action-label bindings are missing for a visible item; labels are unavailable, not inferred.",
        }));
    }
    let recovery_context = context::recovery(value.recovery.as_ref(), catalog)?;
    let recovery = recovery_context["views"]
        .as_array()
        .filter(|views| views.len() == 1)
        .and_then(|views| views.first())
        .cloned()
        .unwrap_or(Value::Null);
    let mut result = json!({
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
            "presence":context::presence(player.presence.as_ref())?,
        },
        "entities":entities,"groundItems":ground_items,"dialogue":dialogue,
        "bank":context::bank(value, catalog)?,"shop":context::shop(value.shop.as_ref(), catalog)?,
        "recovery":recovery,"recoveryContext":recovery_context,"messages":messages,
        "dynamicObjects":dynamic_objects,
        "unavailableViews":unavailable_views,
        "eventHistoryGap":value.event_history_gap,
        "eventHistoryFloorRevision":value.event_history_floor_revision.to_string(),
    });
    result["player"]
        .as_object_mut()
        .expect("player object")
        .extend(observer::fields(
            player.running,
            &player.movement_tick,
            player.action.as_ref(),
        )?);
    if let Some(ui) = &value.ui {
        result["ui"] = ui_wire::decode(ui)?;
    }
    if let Some(scene) = &value.scene {
        result["scene"] = scene::project(scene, player)?;
    }
    if let Some(authority) = &value.audio_authority {
        result["audioAuthority"] = audio_authority::project(authority)?;
    }
    Ok(result)
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
    let mut payload = json!({"committed":true});
    if kind == "level_up" && !event.skill.is_empty() {
        payload["skillId"] = json!(event.skill);
    }
    Some(json!({
        "id":event.event_id,"kind":kind,"sourceId":null,"assetId":asset.filter(|id| !id.is_empty()),
        "actorId":(!event.actor_id.is_empty()).then_some(&event.actor_id),
        "tile":null,"sourceCycle":null,
        // These events came from client-core's validated committed event stream.
        // Source-cycle/group/cue metadata absent from this wire must stay absent.
        "payload":payload,
    }))
}

use std::collections::BTreeMap;

use clubscape_content::CompiledContent;
use clubscape_game_types::*;
use clubscape_protocol::game;
use clubscape_simulation::skills::level_for_xp;
use prost::Message;

use crate::{
    error::ApiError,
    game_storage::{CommittedActorEvent, WorldSnapshot},
};

pub(super) const VIEW_RADIUS: u16 = 32;

pub(super) fn check_context(character: &CharacterState) -> GameResult<()> {
    let container = match &character.runtime.engine {
        EngineMetadata::Legacy => character
            .flags
            .keys()
            .any(|key| key.starts_with("__world_engine.access.")),
        EngineMetadata::Typed { schedule } => schedule.access.is_some(),
    };
    if container {
        return Err(gap(
            "Guarded bank/shop public-view helpers are not exposed by the engine.",
        ));
    }
    if character.dialogue.is_some() {
        return Err(gap(
            "A guarded dialogue public-view helper is not exposed by the engine.",
        ));
    }
    if character.runtime.active_death.is_some() {
        return Err(gap(
            "An authorized recovery/fee public-view helper is not exposed by the engine.",
        ));
    }
    Ok(())
}

pub(super) fn preflight(content: &GameContent, intent: &GameIntent) -> Result<(), ApiError> {
    match intent {
        GameIntent::RequestLogout => {
            return Err(unavailable(
                "The source presence/logout transition API is missing; no logout was performed.",
            ));
        }
        GameIntent::Reclaim { .. } => {
            return Err(unavailable(
                "The engine recovery/fee public-view API is missing.",
            ));
        }
        GameIntent::Interact { target, action } => {
            if let Some(interaction) = content.spawns.get(target).and_then(|spawn| {
                spawn
                    .interactions
                    .iter()
                    .find(|interaction| &interaction.name == action)
            }) {
                match interaction.action {
                    InteractionAction::Bank | InteractionAction::OpenBank { .. } => {
                        return Err(unavailable(
                            "The engine guarded bank public-view API is missing.",
                        ));
                    }
                    InteractionAction::Shop { .. } | InteractionAction::OpenShop { .. } => {
                        return Err(unavailable(
                            "The engine guarded shop/quote public-view API is missing.",
                        ));
                    }
                    InteractionAction::Dialogue { .. } => {
                        return Err(unavailable(
                            "The engine guarded dialogue public-view API is missing.",
                        ));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn snapshot(
    compiled: &CompiledContent,
    world: &WorldSnapshot,
    actor: &ActorId,
    character_revision: u64,
) -> Result<game::WorldSnapshot, ApiError> {
    let content = compiled.definition();
    world
        .state
        .validate_runtime(content)
        .map_err(|_| ApiError::internal("game_view_runtime_integrity"))?;
    let character = world
        .state
        .characters
        .get(actor)
        .ok_or_else(|| ApiError::internal("game_view_actor"))?;
    check_context(character).map_err(|_| unavailable("The authoritative context requires an engine public-view helper that is not available."))?;
    let stage = content
        .tutorial
        .get(&character.tutorial_stage)
        .ok_or_else(|| ApiError::internal("game_view_tutorial"))?;
    let skills = character
        .skills
        .iter()
        .map(|(id, state)| {
            let definition = content
                .skills
                .get(id)
                .ok_or_else(|| ApiError::internal("game_view_skill"))?;
            Ok(game::Skill {
                id: id.to_string(),
                xp_tenths: state.xp_tenths,
                base_level: u32::from(
                    level_for_xp(definition, state.xp_tenths)
                        .map_err(|_| ApiError::internal("game_view_level"))?,
                ),
                current_level: u32::from(state.current_level),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let quests = character
        .quests
        .iter()
        .map(|(id, state)| {
            let journal = content
                .quests
                .get(id)
                .and_then(|definition| definition.journal.get(&state.stage))
                .ok_or_else(|| ApiError::internal("game_view_quest"))?;
            Ok(game::Quest {
                id: id.to_string(),
                stage: state.stage.to_string(),
                journal: journal.clone(),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let settings = [
        (
            game::SettingKind::Run,
            character.runtime.settings.run_enabled,
        ),
        (
            game::SettingKind::AutoRetaliate,
            character.runtime.settings.auto_retaliate,
        ),
        (
            game::SettingKind::DeathAutoEquip,
            character.runtime.settings.death_auto_equip,
        ),
        (
            game::SettingKind::DeathSupplyPiles,
            character.runtime.settings.death_supply_piles,
        ),
    ]
    .into_iter()
    .filter_map(|(kind, enabled)| {
        enabled.map(|enabled| game::SetSetting {
            setting: kind as i32,
            enabled,
        })
    })
    .collect();
    let player = game::Player {
        actor_id: actor.to_string(),
        display_name: character.display_name.clone(),
        appearance: character
            .appearance
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect(),
        region: character.region.to_string(),
        tile: Some(tile(character.tile)),
        inventory: character
            .inventory
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                value.as_ref().map(|value| game::ItemSlot {
                    index: index as u32,
                    stack: Some(stack(value)),
                })
            })
            .collect(),
        equipment: character
            .equipment
            .iter()
            .map(|(slot, value)| game::EquipmentSlot {
                slot: slot.to_string(),
                stack: Some(stack(value)),
            })
            .collect(),
        bank: Vec::new(),
        bank_open: false,
        bank_capacity: 0,
        skills,
        hitpoints: u32::from(character.hitpoints),
        prayer_points: u32::from(character.prayer_points),
        run_energy: u32::from(character.run_energy),
        quest_points: character.quest_points,
        tutorial_stage: character.tutorial_stage.to_string(),
        tutorial_instruction: stage.instruction.clone(),
        quests,
        unlocked_interfaces: character
            .interfaces
            .iter()
            .map(ToString::to_string)
            .collect(),
        activity: activity(&character.activity).into(),
        animation: String::new(),
        settings,
        experience: character
            .runtime
            .settings
            .experience
            .as_ref()
            .map(ToString::to_string),
        instance: character.runtime.instance.as_ref().map(ToString::to_string),
        active_prayers: character
            .runtime
            .combat
            .active_prayers
            .iter()
            .map(ToString::to_string)
            .collect(),
        active_death: None,
        appearance_confirmed: character.runtime.settings.appearance_confirmed,
        combat_style: character
            .runtime
            .combat
            .style
            .as_ref()
            .map(ToString::to_string),
    };
    let instance = character
        .runtime
        .instance
        .as_ref()
        .map(|id| {
            let instance = world
                .state
                .runtime
                .instances
                .get(id)
                .ok_or_else(|| ApiError::internal("game_view_instance"))?;
            let definition = content
                .mechanics
                .instances
                .get(&instance.template)
                .ok_or_else(|| ApiError::internal("game_view_instance_template"))?;
            if definition.private_to_character && instance.owner.as_ref() != Some(actor) {
                return Err(ApiError::internal("game_view_instance_ownership"));
            }
            Ok(instance)
        })
        .transpose()?;
    let scoped = instance.map_or(&world.state.entities, |instance| &instance.entities);
    let transforms = instance.map_or(&world.state.runtime.object_states, |instance| {
        &instance.object_states
    });
    let transformed: BTreeMap<_, _> = transforms
        .keys()
        .map(|id| {
            let definition = content
                .mechanics
                .object_transforms
                .get(id)
                .ok_or_else(|| ApiError::internal("game_view_transform"))?;
            Ok((definition.spawn.clone(), id))
        })
        .collect::<Result<_, ApiError>>()?;
    let mut entities = Vec::new();
    for (id, state) in scoped {
        if transformed.contains_key(id) || !visible(character.tile, state.tile) {
            continue;
        }
        let definition = content
            .spawns
            .get(id)
            .ok_or_else(|| ApiError::internal("game_view_spawn"))?;
        let (definition_id, name, maximum, kind) = match &definition.kind {
            SpawnKind::Npc { npc } => {
                let npc = content
                    .npcs
                    .get(npc)
                    .ok_or_else(|| ApiError::internal("game_view_npc"))?;
                if npc.morph.is_some() {
                    return Err(unavailable(
                        "The engine NPC morph public-view helper is missing.",
                    ));
                }
                (
                    npc.id.to_string(),
                    npc.name.clone(),
                    npc.combat.as_ref().map_or(0, |combat| combat.hitpoints),
                    game::EntityKind::Npc,
                )
            }
            SpawnKind::Object { object } => {
                let object = content
                    .objects
                    .get(object)
                    .ok_or_else(|| ApiError::internal("game_view_object"))?;
                if object.morph.is_some() {
                    return Err(unavailable(
                        "The engine object morph public-view helper is missing.",
                    ));
                }
                (
                    object.id.to_string(),
                    object.name.clone(),
                    0,
                    game::EntityKind::Object,
                )
            }
            SpawnKind::Item { .. } => continue,
        };
        entities.push(game::Entity {
            id: id.to_string(),
            definition_id,
            name,
            kind: kind as i32,
            tile: Some(tile(state.tile)),
            hitpoints: u32::from(state.hitpoints),
            max_hitpoints: u32::from(maximum),
            available: state.available_at_tick <= world.state.tick
                && (maximum == 0 || state.hitpoints > 0),
            // These are source-declared menu names, not a permission/guard evaluation.
            actions: definition
                .interactions
                .iter()
                .map(|interaction| interaction.name.clone())
                .collect(),
            instance: character.runtime.instance.as_ref().map(ToString::to_string),
            ..Default::default()
        });
    }
    for (id, other) in &world.state.characters {
        if id == actor
            || other.runtime.instance != character.runtime.instance
            || !visible(character.tile, other.tile)
        {
            continue;
        }
        entities.push(game::Entity {
            id: id.to_string(),
            kind: game::EntityKind::Player as i32,
            name: other.display_name.clone(),
            tile: Some(tile(other.tile)),
            available: true,
            appearance: other
                .appearance
                .iter()
                .map(|(key, value)| (key.clone(), *value))
                .collect(),
            instance: other.runtime.instance.as_ref().map(ToString::to_string),
            ..Default::default()
        });
    }
    let ground_items = world
        .state
        .ground_items
        .iter()
        .filter(|item| {
            item.instance == character.runtime.instance
                && visible(character.tile, item.tile)
                && item.expires_at_tick > world.state.tick
                && (item.owner.as_ref() == Some(actor) || item.public_at_tick <= world.state.tick)
        })
        .map(|item| game::GroundItem {
            id: item.id.clone(),
            tile: Some(tile(item.tile)),
            stack: Some(stack(&item.stack)),
            can_take: false,
            permissions_evaluated: false,
            instance: item.instance.as_ref().map(ToString::to_string),
        })
        .collect::<Vec<_>>();
    let mut dynamic_objects = Vec::new();
    for (id, object) in &world.state.runtime.temporary_objects {
        if object.location.instance != character.runtime.instance
            || !visible(character.tile, object.location.tile)
        {
            continue;
        }
        let definition = content
            .mechanics
            .temporary_objects
            .get(&object.definition)
            .ok_or_else(|| ApiError::internal("game_view_temporary_object"))?;
        dynamic_objects.push(game::DynamicObject {
            id: id.to_string(),
            definition_id: object.definition.to_string(),
            object_id: Some(definition.object.to_string()),
            tile: Some(tile(object.location.tile)),
            instance: object.location.instance.as_ref().map(ToString::to_string),
            expires_at_tick: Some(object.expires_at_tick),
            ..Default::default()
        });
    }
    for (id, selected) in transforms {
        let definition = content
            .mechanics
            .object_transforms
            .get(id)
            .ok_or_else(|| ApiError::internal("game_view_transform"))?;
        let state = definition
            .states
            .get(selected)
            .ok_or_else(|| ApiError::internal("game_view_transform_state"))?;
        if !visible(character.tile, state.tile) {
            continue;
        }
        if let Some(object) = &state.object {
            let object = content
                .objects
                .get(object)
                .ok_or_else(|| ApiError::internal("game_view_transform_object"))?;
            let source = content
                .spawns
                .get(&definition.spawn)
                .ok_or_else(|| ApiError::internal("game_view_transform_spawn"))?;
            let live = scoped
                .get(&definition.spawn)
                .ok_or_else(|| ApiError::internal("game_view_transform_entity"))?;
            entities.push(game::Entity {
                id: definition.spawn.to_string(),
                definition_id: object.id.to_string(),
                kind: game::EntityKind::Object as i32,
                name: object.name.clone(),
                tile: Some(tile(state.tile)),
                available: live.available_at_tick <= world.state.tick,
                actions: source
                    .interactions
                    .iter()
                    .map(|interaction| interaction.name.clone())
                    .collect(),
                instance: character.runtime.instance.as_ref().map(ToString::to_string),
                ..Default::default()
            });
        }
        dynamic_objects.push(game::DynamicObject {
            id: id.to_string(),
            definition_id: id.to_string(),
            object_id: state.object.as_ref().map(ToString::to_string),
            tile: Some(tile(state.tile)),
            state: Some(selected.to_string()),
            door_open: state
                .door
                .as_ref()
                .map(|door| matches!(door, DoorPosition::Open)),
            quarter_turns: u32::from(state.placement.quarter_turns),
            instance: character.runtime.instance.as_ref().map(ToString::to_string),
            ..Default::default()
        });
    }
    if entities.len() > 2048 {
        return Err(unavailable(
            "The public entity view exceeds its response bound.",
        ));
    }
    entities.sort_by(|a, b| a.id.cmp(&b.id));
    let mut unavailable_views = vec![game::UnavailableView {
        view: "interaction_permissions".into(),
        reason: "The engine has no guarded interaction-menu public-view API; action names are declarations only.".into(),
    }];
    if !ground_items.is_empty() {
        unavailable_views.push(game::UnavailableView {
            view: "ground_item_permissions".into(),
            reason: "The engine has no ground-item permission public-view API; can_take is not evaluated.".into(),
        });
    }
    let result = game::WorldSnapshot {
        revision: world.state.revision,
        tick: world.state.tick,
        character_revision,
        player: Some(player),
        entities,
        ground_items,
        dialogue: None,
        events: Vec::new(),
        full_snapshot: true,
        removed_entities: Vec::new(),
        event_history_gap: false,
        event_history_floor_revision: world.state.revision,
        dynamic_objects,
        unavailable_views,
        next_sequence: character
            .last_command_sequence
            .checked_add(1)
            .ok_or_else(|| ApiError::internal("game_view_sequence"))?,
    };
    bounded(&result)?;
    Ok(result)
}

pub(super) fn bounded(snapshot: &game::WorldSnapshot) -> Result<(), ApiError> {
    if snapshot.encoded_len() > clubscape_protocol::MAX_GAME_RESPONSE_BYTES - 1024
        || snapshot.events.len() > 256
    {
        return Err(unavailable(
            "The public world view exceeds its response byte/event bound.",
        ));
    }
    Ok(())
}

pub(super) fn event(value: &CommittedActorEvent) -> game::Event {
    let mut result = game::Event {
        kind: value.event.kind().into(),
        target: value.event.primary_target().unwrap_or_default().into(),
        event_id: value.event_id.clone(),
        actor_id: value.actor_id.to_string(),
        ..Default::default()
    };
    match &value.event {
        GameEvent::Message { text }
        | GameEvent::Inspected {
            explanation: text, ..
        } => result.text.clone_from(text),
        GameEvent::Sound { asset } => result.sound_asset.clone_from(asset),
        GameEvent::Animation { animation, .. } => result.animation_asset.clone_from(animation),
        GameEvent::XpGained {
            skill,
            amount_tenths,
        } => {
            result.skill = skill.to_string();
            result.xp_tenths = *amount_tenths;
        }
        GameEvent::Hit { damage, .. }
        | GameEvent::CombatResolved { damage, .. }
        | GameEvent::SpellResolved { damage, .. } => result.damage = u32::from(*damage),
        GameEvent::Gathered { stack: item, .. } | GameEvent::Equipped { stack: item, .. } => {
            result.item = Some(stack(item))
        }
        GameEvent::Produced { outputs, .. } | GameEvent::ProductionResolved { outputs, .. } => {
            result.items = outputs.iter().map(stack).collect()
        }
        GameEvent::ItemTransferred { items, .. } => {
            result.items = items.iter().map(stack).collect()
        }
        _ => {}
    }
    result
}

pub(super) fn entity_delta(
    snapshot: &mut game::WorldSnapshot,
    previous: &BTreeMap<String, game::Entity>,
) {
    let current: BTreeMap<_, _> = snapshot
        .entities
        .iter()
        .map(|entity| (entity.id.clone(), entity))
        .collect();
    snapshot.removed_entities = previous
        .keys()
        .filter(|id| !current.contains_key(*id))
        .cloned()
        .collect();
    snapshot
        .entities
        .retain(|entity| previous.get(&entity.id) != Some(entity));
    snapshot.full_snapshot = false;
}

fn visible(from: Tile, to: Tile) -> bool {
    from.distance(to)
        .is_some_and(|distance| distance <= VIEW_RADIUS)
}

fn stack(value: &ItemStack) -> game::Stack {
    game::Stack {
        item: value.item.to_string(),
        quantity: value.quantity.get(),
        instance_id: value
            .instance
            .as_ref()
            .map(|instance| instance.id.to_string()),
        charges: value
            .instance
            .as_ref()
            .and_then(|instance| instance.charges.as_ref())
            .map(|charges| charges.remaining),
    }
}

fn tile(value: Tile) -> game::Tile {
    game::Tile {
        x: u32::from(value.x()),
        y: u32::from(value.y()),
        plane: u32::from(value.plane()),
    }
}

fn activity(value: &Activity) -> &'static str {
    match value {
        Activity::Idle => "idle",
        Activity::Walking { .. } => "walking",
        Activity::Gathering { .. } => "gathering",
        Activity::Producing { .. } | Activity::ProducingAt { .. } => "producing",
        Activity::Fighting { .. } => "fighting",
        Activity::Casting { .. } => "casting",
    }
}

fn gap(message: &'static str) -> GameError {
    GameError::new(GameErrorCode::Unavailable, message)
}

pub(super) fn unavailable(message: &'static str) -> ApiError {
    ApiError::new(
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        clubscape_protocol::ErrorCode::Unavailable,
        message,
    )
}

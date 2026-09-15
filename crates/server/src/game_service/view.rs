use std::collections::BTreeMap;

use clubscape_content::CompiledContent;
use clubscape_game_types::*;
use clubscape_protocol::{ReadOnlyQuote, game};
use clubscape_simulation::skills::level_for_xp;
use clubscape_world_engine::{ContextView, WorldEngine};
use prost::Message;

use crate::{
    error::ApiError,
    game_storage::{CommittedActorEvent, WorldSnapshot},
};

pub(super) const VIEW_RADIUS: u16 = 32;

pub(super) fn snapshot(
    compiled: &CompiledContent,
    engine: &WorldEngine,
    world: &WorldSnapshot,
    actor: &ActorId,
    character_revision: u64,
    requested_quote: Option<&ReadOnlyQuote>,
) -> Result<game::WorldSnapshot, ApiError> {
    let content = compiled.definition();
    world
        .state
        .validate_runtime(content)
        .map_err(engine_error)?;
    let character = world
        .state
        .characters
        .get(actor)
        .ok_or_else(|| ApiError::internal("game_view_actor"))?;
    let stage = content
        .tutorial
        .get(&character.tutorial_stage)
        .ok_or_else(|| ApiError::internal("game_view_tutorial"))?;
    let presence = engine
        .presence_view(&world.state, actor)
        .map_err(engine_error)?;
    let (animation, observation) = super::observer::project(
        content,
        engine
            .actor_observer(&world.state, actor)
            .map_err(engine_error)?,
    )?;
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
                    level_for_xp(definition, state.xp_tenths).map_err(engine_error)?,
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
    let mut player = game::Player {
        actor_id: actor.to_string(),
        display_name: character.display_name.clone(),
        appearance: character
            .appearance
            .iter()
            .map(|(key, value)| (key.clone(), *value))
            .collect(),
        region: character.region.to_string(),
        tile: Some(tile(character.tile)),
        inventory: item_slots(character.inventory.slots.iter()),
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
        animation,
        running: Some(observation.running),
        movement_tick: observation.movement_tick,
        action: observation.action.map(super::observer::action),
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
        active_death: character
            .runtime
            .active_death
            .as_ref()
            .map(ToString::to_string),
        appearance_confirmed: character.runtime.settings.appearance_confirmed,
        combat_style: character
            .runtime
            .combat
            .style
            .as_ref()
            .map(ToString::to_string),
        presence: Some(presence_view(&presence)),
    };
    let mut dialogue = None;
    let mut bank_context = None;
    let mut shop = None;
    let mut recovery = None;
    match engine
        .context_view(&world.state, actor)
        .map_err(engine_error)?
    {
        ContextView::None => {}
        ContextView::Dialogue { view } => {
            let speaker = engine
                .target_view(
                    &world.state,
                    actor,
                    &WorldTarget::Spawn {
                        spawn: view.speaker.clone(),
                    },
                )
                .map_err(engine_error)?
                .ok_or_else(|| ApiError::internal("game_view_dialogue_speaker"))?;
            dialogue = Some(game::Dialogue {
                id: view.dialogue.to_string(),
                speaker: view.speaker.to_string(),
                speaker_name: speaker.name,
                text: view.text,
                choices: view
                    .choices
                    .into_iter()
                    .map(|choice| game::Choice {
                        id: choice.id,
                        text: choice.text,
                    })
                    .collect(),
            });
        }
        ContextView::Bank { view } => {
            player.bank_open = true;
            player.bank = item_slots(view.bank.slots.iter());
            player.bank_capacity = u32::from(view.bank.capacity);
            bank_context = Some(game::BankContext {
                banker: view.banker.to_string(),
                interface: view.interface.map(|id| id.to_string()),
                deposit: Some(permission(&view.deposit)),
                withdraw: Some(permission(&view.withdraw)),
            });
        }
        ContextView::Shop { view } => {
            shop = Some(game::ShopView {
                shop: view.shop.to_string(),
                interface: view.interface.map(|id| id.to_string()),
                currency: view.currency.to_string(),
                lines: view
                    .lines
                    .into_iter()
                    .map(|line| game::ShopLine {
                        index: u32::from(line.index),
                        item: line.item.to_string(),
                        stock: line.stock,
                        buy_price: line.buy_price,
                        sell_price: line.sell_price,
                    })
                    .collect(),
            });
        }
        ContextView::Recovery { views } => {
            recovery = Some(game::RecoveryContext {
                views: views.iter().map(recovery_view).collect(),
            });
        }
    }
    let scoped = if let Some(instance) = &character.runtime.instance {
        let instance = world
            .state
            .runtime
            .instances
            .get(instance)
            .ok_or_else(|| ApiError::internal("game_view_instance"))?;
        let definition = content
            .mechanics
            .instances
            .get(&instance.template)
            .ok_or_else(|| ApiError::internal("game_view_instance_template"))?;
        if definition.private_to_character && instance.owner.as_ref() != Some(actor) {
            return Err(ApiError::internal("game_view_instance_ownership"));
        }
        &instance.entities
    } else {
        &world.state.entities
    };
    let transforms = if let Some(instance) = &character.runtime.instance {
        &world.state.runtime.instances[instance].object_states
    } else {
        &world.state.runtime.object_states
    };
    let transformed: std::collections::BTreeSet<_> = transforms
        .keys()
        .map(|id| {
            content
                .mechanics
                .object_transforms
                .get(id)
                .map(|definition| &definition.spawn)
                .ok_or_else(|| ApiError::internal("game_view_transform"))
        })
        .collect::<Result<_, _>>()?;
    let mut entities = Vec::new();
    for (id, state) in scoped {
        if !visible(character.tile, state.tile) && !transformed.contains(id) {
            continue;
        }
        let definition = content
            .spawns
            .get(id)
            .ok_or_else(|| ApiError::internal("game_view_spawn"))?;
        if matches!(definition.kind, SpawnKind::Item { .. }) {
            continue;
        }
        let Some(target) = engine
            .target_view(
                &world.state,
                actor,
                &WorldTarget::Spawn { spawn: id.clone() },
            )
            .map_err(engine_error)?
        else {
            continue;
        };
        if !visible(character.tile, target.tile) {
            continue;
        }
        let (definition_id, kind, maximum) = if let Some(npc) = &target.npc {
            let definition = content
                .npcs
                .get(npc)
                .ok_or_else(|| ApiError::internal("game_view_npc"))?;
            (
                npc.to_string(),
                game::EntityKind::Npc,
                definition
                    .combat
                    .as_ref()
                    .map_or(0, |combat| combat.hitpoints),
            )
        } else {
            (
                target
                    .object
                    .as_ref()
                    .ok_or_else(|| ApiError::internal("game_view_object"))?
                    .to_string(),
                game::EntityKind::Object,
                0,
            )
        };
        entities.push(game::Entity {
            id: id.to_string(),
            definition_id,
            kind: kind as i32,
            name: target.name,
            tile: Some(tile(target.tile)),
            hitpoints: u32::from(state.hitpoints),
            max_hitpoints: u32::from(maximum),
            available: target.available,
            actions: target
                .interactions
                .iter()
                .filter(|option| option.permission.allowed)
                .map(|option| option.name.clone())
                .collect(),
            interaction_options: target.interactions.iter().map(interaction).collect(),
            actions_evaluated: true,
            instance: player.instance.clone(),
            asset: target.asset.map(|id| id.to_string()),
            width: u32::from(target.width),
            height: u32::from(target.height),
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
        let presence = engine
            .presence_view(&world.state, id)
            .map_err(engine_error)?;
        if !presence.present_in_world {
            continue;
        }
        let (animation, observation) = super::observer::project(
            content,
            engine
                .actor_observer(&world.state, id)
                .map_err(engine_error)?,
        )?;
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
            instance: player.instance.clone(),
            presence: Some(presence_view(&presence)),
            animation,
            running: Some(observation.running),
            movement_tick: observation.movement_tick,
            action: observation.action.map(super::observer::action),
            width: 1,
            height: 1,
            ..Default::default()
        });
    }
    let ground_items = engine
        .ground_item_views(&world.state, actor)
        .map_err(engine_error)?
        .into_iter()
        .filter(|item| visible(character.tile, item.tile))
        .map(|item| game::GroundItem {
            id: item.id,
            tile: Some(tile(item.tile)),
            stack: Some(stack(&item.stack)),
            can_take: item.can_take.allowed,
            permissions_evaluated: true,
            permission: Some(permission(&item.can_take)),
            instance: player.instance.clone(),
        })
        .collect();
    let mut dynamic_objects = Vec::new();
    for (id, object) in &world.state.runtime.temporary_objects {
        if object.location.instance != character.runtime.instance
            || !visible(character.tile, object.location.tile)
        {
            continue;
        }
        let Some(target) = engine
            .target_view(
                &world.state,
                actor,
                &WorldTarget::TemporaryObject { object: id.clone() },
            )
            .map_err(engine_error)?
        else {
            continue;
        };
        if !visible(character.tile, target.tile) {
            continue;
        }
        dynamic_objects.push(game::DynamicObject {
            id: id.to_string(),
            definition_id: object.definition.to_string(),
            object_id: target.object.as_ref().map(ToString::to_string),
            tile: Some(tile(target.tile)),
            instance: player.instance.clone(),
            expires_at_tick: Some(object.expires_at_tick),
            ..Default::default()
        });
        entities.push(game::Entity {
            id: id.to_string(),
            definition_id: target.object.map(|id| id.to_string()).unwrap_or_default(),
            kind: game::EntityKind::Object as i32,
            name: target.name,
            tile: Some(tile(target.tile)),
            available: target.available,
            actions: target
                .interactions
                .iter()
                .filter(|option| option.permission.allowed)
                .map(|option| option.name.clone())
                .collect(),
            interaction_options: target.interactions.iter().map(interaction).collect(),
            actions_evaluated: true,
            instance: player.instance.clone(),
            asset: target.asset.map(|id| id.to_string()),
            width: u32::from(target.width),
            height: u32::from(target.height),
            ..Default::default()
        });
    }
    for (id, selected) in transforms {
        let definition = &content.mechanics.object_transforms[id];
        let selected_state = &definition.states[selected];
        let target = engine
            .target_view(
                &world.state,
                actor,
                &WorldTarget::Spawn {
                    spawn: definition.spawn.clone(),
                },
            )
            .map_err(engine_error)?;
        let location = target
            .as_ref()
            .map_or(selected_state.tile, |target| target.tile);
        if !visible(character.tile, location) {
            continue;
        }
        dynamic_objects.push(game::DynamicObject {
            id: id.to_string(),
            definition_id: id.to_string(),
            object_id: target
                .and_then(|target| target.object)
                .map(|id| id.to_string()),
            tile: Some(tile(location)),
            state: Some(selected.to_string()),
            door_open: selected_state
                .door
                .as_ref()
                .map(|door| matches!(door, DoorPosition::Open)),
            quarter_turns: u32::from(selected_state.placement.quarter_turns),
            instance: player.instance.clone(),
            ..Default::default()
        });
    }
    if entities.len() > 2048 {
        return Err(unavailable(
            "The public entity view exceeds its response bound.",
        ));
    }
    entities.sort_by(|a, b| a.id.cmp(&b.id));
    let result = game::WorldSnapshot {
        audio_authority: None,
        scene: Some({
            let scene = engine
                .scene_view(&world.state, actor)
                .map_err(engine_error)?;
            game::CurrentScene {
                region: scene.region.to_string(),
                instance: scene.instance.map(|id| id.to_string()),
                instance_template: scene.instance_template.map(|id| id.to_string()),
            }
        }),
        ui: if content.ui.is_some() {
            Some(super::ui_wire::view(
                engine.ui_view(&world.state, actor).map_err(engine_error)?,
            )?)
        } else {
            None
        },
        revision: world.state.revision,
        tick: world.state.tick,
        character_revision,
        player: Some(player),
        entities,
        ground_items,
        dialogue,
        bank_context,
        shop,
        recovery,
        quote: requested_quote
            .map(|quote| quote_view(engine, &world.state, actor, quote))
            .transpose()?,
        events: Vec::new(),
        full_snapshot: true,
        removed_entities: Vec::new(),
        event_history_gap: false,
        event_history_floor_revision: world.state.revision,
        dynamic_objects,
        unavailable_views: Vec::new(),
        next_sequence: character
            .last_command_sequence
            .checked_add(1)
            .ok_or_else(|| ApiError::internal("game_view_sequence"))?,
    };
    bounded(&result)?;
    Ok(result)
}

fn quote_view(
    engine: &WorldEngine,
    world: &WorldState,
    actor: &ActorId,
    request: &ReadOnlyQuote,
) -> Result<game::Quote, ApiError> {
    use game::quote::Result as Wire;
    let result = match request {
        ReadOnlyQuote::BankDeposit { slot, quantity } => {
            let view = engine
                .bank_deposit_quote(world, actor, *slot, *quantity)
                .map_err(engine_error)?;
            Wire::Bank(game::BankQuote {
                requested: view.requested.get(),
                transferred: Some(stack(&view.transferred)),
            })
        }
        ReadOnlyQuote::BankWithdraw {
            slot,
            quantity,
            noted,
        } => {
            let view = engine
                .bank_withdraw_quote(world, actor, *slot, *quantity, *noted)
                .map_err(engine_error)?;
            Wire::Bank(game::BankQuote {
                requested: view.requested.get(),
                transferred: Some(stack(&view.transferred)),
            })
        }
        ReadOnlyQuote::ShopBuy {
            shop,
            index,
            quantity,
            expected_item,
        } => Wire::Shop(shop_quote(
            engine
                .shop_buy_quote(
                    world,
                    actor,
                    shop,
                    *index,
                    *quantity,
                    expected_item.as_ref(),
                )
                .map_err(engine_error)?,
        )),
        ReadOnlyQuote::ShopSell {
            shop,
            slot,
            quantity,
        } => Wire::Shop(shop_quote(
            engine
                .shop_sell_quote(world, actor, shop, *slot, *quantity)
                .map_err(engine_error)?,
        )),
        ReadOnlyQuote::Recovery {
            death,
            storage,
            items,
        } => {
            let view = engine
                .recovery_quote(world, actor, death, *storage, items)
                .map_err(engine_error)?;
            Wire::Recovery(game::RecoveryQuote {
                death: view.death.to_string(),
                storage: recovery_storage(view.storage),
                selected: view.selected.iter().map(ToString::to_string).collect(),
                full_selection_fee: view.full_selection_fee,
            })
        }
    };
    Ok(game::Quote {
        result: Some(result),
    })
}

fn shop_quote(view: clubscape_world_engine::ShopQuote) -> game::ShopQuote {
    game::ShopQuote {
        shop: view.shop.to_string(),
        item: view.item.to_string(),
        requested: view.requested.get(),
        quantity: view.quantity,
        currency: view.currency.to_string(),
        total_price: view.total_price,
        stock_after: view.stock_after,
        partial_reason: view.partial_reason.as_ref().map(denial),
    }
}

fn recovery_storage(storage: RecoveryStorage) -> i32 {
    match storage {
        RecoveryStorage::Grave => game::RecoveryStorage::Grave as i32,
        RecoveryStorage::DeathOffice => game::RecoveryStorage::DeathOffice as i32,
    }
}

fn recovery_view(view: &clubscape_world_engine::RecoveryView) -> game::RecoveryView {
    game::RecoveryView {
        death: view.death.to_string(),
        storage: recovery_storage(view.storage),
        interface: view.interface.as_ref().map(ToString::to_string),
        active_ticks_remaining: view.active_ticks_remaining,
        entries: view
            .entries
            .iter()
            .map(|entry| game::RecoveryEntry {
                id: entry.id.to_string(),
                stack: Some(stack(&entry.stack)),
                full_entry_fee: entry.full_entry_fee,
                current_storage: recovery_storage(entry.current_storage),
                layout: Some(game::ItemLayout {
                    location: Some(match &entry.layout {
                        ItemLayout::Inventory { slot } => {
                            game::item_layout::Location::InventorySlot(u32::from(*slot))
                        }
                        ItemLayout::Equipment { slot } => {
                            game::item_layout::Location::EquipmentSlot(slot.to_string())
                        }
                    }),
                }),
            })
            .collect(),
    }
}

fn presence_view(view: &clubscape_world_engine::PresenceView) -> game::Presence {
    game::Presence {
        kind: match view.state {
            PresenceState::Connected { .. } => game::PresenceKind::Connected as i32,
            PresenceState::Disconnecting { .. } => game::PresenceKind::Disconnecting as i32,
            PresenceState::Offline { .. } => game::PresenceKind::Offline as i32,
            PresenceState::Untracked => 0,
        },
        connected: view.connected,
        accepts_input: view.accepts_input,
        present_in_world: view.present_in_world,
    }
}

fn interaction(view: &clubscape_world_engine::InteractionView) -> game::InteractionOption {
    game::InteractionOption {
        name: view.name.clone(),
        permission: Some(permission(&view.permission)),
    }
}

fn permission(view: &clubscape_world_engine::Permission) -> game::Permission {
    game::Permission {
        allowed: view.allowed,
        denial: view.denial.as_ref().map(denial),
    }
}

pub(super) fn denial(error: &GameError) -> game::RuleDenial {
    use game::RuleErrorCode as Code;
    let (code, message) = match error.code {
        GameErrorCode::InvalidInput => (Code::InvalidInput, "Invalid selection."),
        GameErrorCode::UnknownContent => (Code::UnknownContent, "Unknown source content."),
        GameErrorCode::InvalidContent => (Code::InvalidContent, "Invalid source content."),
        GameErrorCode::Unavailable => (
            Code::Unavailable,
            "A required source binding is unavailable.",
        ),
        GameErrorCode::NotOwned => (
            Code::NotOwned,
            "The requested state is not owned or accessible.",
        ),
        GameErrorCode::InsufficientItems => {
            (Code::InsufficientItems, "Insufficient items or currency.")
        }
        GameErrorCode::InventoryFull => (
            Code::InventoryFull,
            "The destination has insufficient capacity.",
        ),
        GameErrorCode::StackOverflow => (
            Code::StackOverflow,
            "The item or price bound would be exceeded.",
        ),
        GameErrorCode::RequirementNotMet => (
            Code::RequirementNotMet,
            "Source requirements are not satisfied.",
        ),
        GameErrorCode::OutOfReach => (Code::OutOfReach, "The target is out of reach."),
        GameErrorCode::Blocked => (Code::Blocked, "The source action is blocked."),
        GameErrorCode::Busy => (
            Code::Busy,
            "The source action cannot currently be performed.",
        ),
        GameErrorCode::StaleCommand => (Code::StaleCommand, "The command is stale."),
        GameErrorCode::SessionConflict => {
            (Code::SessionConflict, "Rejoin before submitting new input.")
        }
    };
    game::RuleDenial {
        code: code as i32,
        message: message.into(),
    }
}

pub(super) fn engine_error(error: GameError) -> ApiError {
    let message = match error.code {
        GameErrorCode::InvalidInput | GameErrorCode::UnknownContent => {
            return ApiError::invalid("The source query or selection is invalid.");
        }
        GameErrorCode::InvalidContent => return ApiError::internal("game_source_view"),
        GameErrorCode::Unavailable => {
            return unavailable("A required source binding is unavailable.");
        }
        _ => "The authoritative source query requirements are not satisfied.",
    };
    ApiError::new(
        axum::http::StatusCode::CONFLICT,
        clubscape_protocol::ErrorCode::Conflict,
        message,
    )
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
        GameEvent::PublicChat { line } => {
            result.public_chat = Some(super::ui_wire::chat(line.clone()));
            result.text = line.text.clone();
        }
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

fn item_slots<'a>(slots: impl Iterator<Item = &'a Option<ItemStack>>) -> Vec<game::ItemSlot> {
    slots
        .enumerate()
        .filter_map(|(index, value)| {
            value.as_ref().map(|value| game::ItemSlot {
                index: index as u32,
                stack: Some(stack(value)),
            })
        })
        .collect()
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
        Activity::InventoryAction { .. } => "item_action",
        Activity::Idle => "idle",
        Activity::Walking { .. } => "walking",
        Activity::Gathering { .. } => "gathering",
        Activity::Producing { .. }
        | Activity::ProducingAt { .. }
        | Activity::ProducingSelected { .. } => "producing",
        Activity::Fighting { .. } => "fighting",
        Activity::Casting { .. } => "casting",
    }
}

pub(super) fn unavailable(message: &'static str) -> ApiError {
    ApiError::new(
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        clubscape_protocol::ErrorCode::Unavailable,
        message,
    )
}

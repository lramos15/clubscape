use crate::error::ApiError;
use clubscape_game_types as types;
use clubscape_protocol::game;

fn permission(value: types::UiPermission) -> game::Permission {
    game::Permission {
        allowed: value.allowed,
        denial: value.code.map(|code| {
            super::view::denial(&types::GameError::new(
                code,
                value.reason.unwrap_or_default(),
            ))
        }),
    }
}
fn item(value: types::UiItem) -> game::UiItem {
    game::UiItem {
        stack: Some(game::Stack {
            item: value.item.to_string(),
            quantity: value.quantity,
            instance_id: value.instance_id.map(|id| id.to_string()),
            charges: value.charges,
        }),
        name: value.name,
        source_id: value.source_id,
        asset: value.asset.map(|id| id.to_string()),
    }
}
fn operation(value: types::ItemActionUiView) -> game::UiItemOperation {
    game::UiItemOperation {
        id: value.id,
        label: value.label,
        permission: Some(permission(value.permission)),
    }
}
fn inventory_actions(value: types::InventoryActionsUiView) -> game::UiInventoryActions {
    game::UiInventoryActions {
        slot: u32::from(value.slot),
        item: value.item.to_string(),
        instance: value.instance.map(|id| id.to_string()),
        actions: value.actions.into_iter().map(operation).collect(),
    }
}
fn ability(value: types::AbilityUiView) -> game::UiAbility {
    game::UiAbility {
        id: value.id,
        name: value.name,
        selected: value.selected,
        visible: value.visible,
        permission: Some(permission(value.permission)),
    }
}
pub(super) fn chat(value: types::PublicChatLine) -> game::PublicChatLine {
    game::PublicChatLine {
        id: value.id,
        actor: value.actor.to_string(),
        sender: value.sender,
        channel: value.channel,
        text: value.text,
        colour: u32::from(value.colour),
        effect: u32::from(value.effect),
    }
}
fn target(value: types::WorldTarget) -> game::WorldTarget {
    game::WorldTarget {
        target: Some(match value {
            types::WorldTarget::Spawn { spawn } => {
                game::world_target::Target::Spawn(spawn.to_string())
            }
            types::WorldTarget::TemporaryObject { object } => {
                game::world_target::Target::TemporaryObject(object.to_string())
            }
        }),
    }
}

fn amount(value: types::UiAmount) -> game::UiAmount {
    game::UiAmount {
        selection: Some(match value {
            types::UiAmount::Quantity { quantity } => {
                game::ui_amount::Selection::Quantity(quantity.get())
            }
            types::UiAmount::All {} => game::ui_amount::Selection::All(game::Empty {}),
        }),
    }
}

fn storage(value: types::RecoveryStorage) -> i32 {
    match value {
        types::RecoveryStorage::Grave => game::RecoveryStorage::Grave as i32,
        types::RecoveryStorage::DeathOffice => game::RecoveryStorage::DeathOffice as i32,
    }
}

fn recovery_management(value: types::RecoveryManagementView) -> game::UiRecoveryManagement {
    game::UiRecoveryManagement {
        bank_revision: value.bank_revision,
        panels: value
            .panels
            .into_iter()
            .map(|panel| game::UiRecoveryPanelControl {
                death: panel.death.to_string(),
                storage: storage(panel.storage),
                entries: panel
                    .entries
                    .into_iter()
                    .map(|entry| game::UiRecoveryEntryControl {
                        id: entry.id.to_string(),
                        item: Some(item(entry.item)),
                        unit_fee: entry.unit_fee,
                        full_stack_fee: entry.full_stack_fee,
                        inventory_capacity: entry.inventory_capacity,
                        bank_capacity: entry.bank_capacity,
                        take: Some(permission(entry.take)),
                        bank: Some(permission(entry.bank)),
                    })
                    .collect(),
                full_selection_fee: panel.full_selection_fee,
                take_all: Some(permission(panel.take_all)),
            })
            .collect(),
        bank_all: Some(permission(value.bank_all)),
        bank_all_records: value
            .bank_all_records
            .into_iter()
            .map(|record| game::UiRecoveryRecordSelection {
                death: record.death.to_string(),
                items: record.items.into_iter().map(|id| id.to_string()).collect(),
            })
            .collect(),
    }
}

pub(super) fn view(value: types::GameplayUiView) -> Result<game::GameplayUiView, ApiError> {
    if value.version != types::GAMEPLAY_UI_VIEW_VERSION {
        return Err(ApiError::internal("ui_view_version"));
    }
    let reward = value
        .reward
        .map(|reward| {
            let types::GameplayUiRequest::UiDismiss { presentation_id } = reward.continuation
            else {
                return Err(ApiError::internal("ui_reward_continuation"));
            };
            Ok(game::UiReward {
                id: reward.id,
                kind: match reward.kind {
                    types::RewardUiKind::Quest => "quest",
                    types::RewardUiKind::LevelUp => "level_up",
                }
                .into(),
                interface: reward.interface.to_string(),
                title: reward.title,
                lines: reward.lines,
                items: reward.items.into_iter().map(item).collect(),
                xp: reward
                    .xp
                    .into_iter()
                    .map(|xp| game::UiXp {
                        skill: xp.skill.to_string(),
                        amount_tenths: xp.amount_tenths,
                    })
                    .collect(),
                quest_points: reward.quest_points,
                quest: reward.quest.map(|id| id.to_string()),
                skill: reward.skill.map(|id| id.to_string()),
                level: reward.level.map(u32::from),
                continuation: Some(game::GameplayUiRequest {
                    expected_bank_revision: None,
                    request: Some(game::gameplay_ui_request::Request::Dismiss(
                        game::UiIdentity {
                            id: presentation_id,
                        },
                    )),
                }),
            })
        })
        .transpose()?;
    let bonus = |(kind, amount): (types::AttackType, i16)| game::UiBonus {
        kind: match kind {
            types::AttackType::Stab => "stab",
            types::AttackType::Slash => "slash",
            types::AttackType::Crush => "crush",
            types::AttackType::Ranged => "ranged",
            types::AttackType::Magic => "magic",
        }
        .into(),
        amount: i32::from(amount),
    };
    Ok(game::GameplayUiView {
        version: value.version,
        active_tab: value.active_tab.map(|id| id.to_string()),
        active_interface: value.active_interface.map(|id| id.to_string()),
        production: value.production.map(|menu| game::UiProduction {
            id: menu.id,
            interface: menu.interface.to_string(),
            target: menu.target.map(target),
            recipes: menu
                .recipes
                .into_iter()
                .map(|choice| game::UiProductionChoice {
                    recipe: choice.recipe.to_string(),
                    name: choice.name,
                    outputs: choice.outputs.into_iter().map(item).collect(),
                    single: Some(permission(choice.single)),
                    make_x: Some(permission(choice.make_x)),
                    all: choice.all.map(permission),
                })
                .collect(),
        }),
        reward,
        confirmation: value
            .confirmation
            .map(|confirmation| game::UiConfirmationView {
                id: confirmation.id,
                kind: confirmation.kind,
                title: confirmation.title,
                lines: confirmation.lines,
                items: confirmation.items.into_iter().map(item).collect(),
                credit: confirmation.credit,
            }),
        interfaces: value
            .interfaces
            .into_iter()
            .map(|state| game::UiInterfaceState {
                interface: state.interface.to_string(),
                visibility: match state.visibility {
                    types::UiVisibility::Hidden => "hidden",
                    types::UiVisibility::Locked => "locked",
                    types::UiVisibility::Enabled => "enabled",
                }
                .into(),
                highlighted: state.highlighted,
                permission: Some(permission(state.permission)),
            })
            .collect(),
        combat_style: value.combat_style,
        combat_styles: value.combat_styles.into_iter().map(ability).collect(),
        prayers: value.prayers.into_iter().map(ability).collect(),
        spells: value.spells.into_iter().map(ability).collect(),
        equipment: Some(game::UiEquipment {
            attack: value
                .equipment
                .bonuses
                .attack
                .into_iter()
                .map(bonus)
                .collect(),
            defence: value
                .equipment
                .bonuses
                .defence
                .into_iter()
                .map(bonus)
                .collect(),
            melee_strength: i32::from(value.equipment.bonuses.melee_strength),
            ranged_strength: i32::from(value.equipment.bonuses.ranged_strength),
            magic_damage_percent: i32::from(value.equipment.bonuses.magic_damage_percent),
            prayer: i32::from(value.equipment.bonuses.prayer),
            weight_grams: value.equipment.weight_grams,
            slots: value
                .equipment
                .slots
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
        }),
        inventory_actions: value
            .inventory_actions
            .into_iter()
            .map(inventory_actions)
            .collect(),
        bank: value.bank.map(|bank| game::UiBank {
            revision: bank.revision,
            capacity: u32::from(bank.capacity),
            selected_tab: u32::from(bank.selected_tab),
            insert_mode: bank.insert_mode,
            placeholders: bank.placeholders,
            amount: bank.amount,
            amount_selection: bank.amount_selection.map(amount),
            noted: bank.noted,
            tabs: bank
                .tabs
                .into_iter()
                .map(|tab| game::UiBankTab {
                    tab: u32::from(tab.tab),
                    first_entry: tab.first_entry,
                    entries: tab.entries,
                })
                .collect(),
            entries: bank
                .entries
                .into_iter()
                .map(|entry| game::UiBankEntry {
                    id: entry.id,
                    slot: u32::from(entry.slot),
                    tab: u32::from(entry.tab),
                    item: entry.item.to_string(),
                    value: entry.value.map(item),
                    placeholder: entry.placeholder,
                })
                .collect(),
            deposit_equipment: Some(permission(bank.deposit_equipment)),
            unavailable_containers: bank
                .unavailable_containers
                .into_iter()
                .map(operation)
                .collect(),
        }),
        kept_on_death: value.kept_on_death.map(|death| game::UiDeathPreview {
            scope: death.scope,
            kept: death.kept.into_iter().map(item).collect(),
            lost: death.lost.into_iter().map(item).collect(),
            full_grave_fee: death.full_grave_fee,
            full_office_fee: death.full_office_fee,
            value_revision: death.value_revision,
        }),
        recovery: value.recovery.map(|recovery| game::UiRecoveryControls {
            coffer_balance: recovery.coffer_balance,
            discard: Some(permission(recovery.discard)),
            coffer_offer: Some(permission(recovery.coffer_offer)),
            coffer_items: recovery
                .coffer_items
                .into_iter()
                .map(inventory_actions)
                .collect(),
            management: recovery.management.map(recovery_management),
        }),
        appearance: Some(game::UiAppearance {
            choices: value
                .appearance
                .choices
                .into_iter()
                .map(|(key, values)| game::UiAppearanceParameter {
                    key,
                    choices: values
                        .into_iter()
                        .map(|choice| game::UiAppearanceChoice {
                            value: choice.value,
                            label: choice.label,
                            permission: Some(permission(choice.permission)),
                        })
                        .collect(),
                })
                .collect(),
            base: value.appearance.base.map(|base| game::UiPenguinBase {
                asset: base.asset.to_string(),
                source_npc: base.source_npc,
                adaptation: base.adaptation,
            }),
            confirmed: value.appearance.confirmed,
        }),
        public_chat: Some(game::UiPublicChat {
            permission: Some(permission(value.public_chat.permission)),
            maximum_bytes: u32::from(value.public_chat.maximum_bytes),
            channel: value.public_chat.channel,
            messages: value.public_chat.messages.into_iter().map(chat).collect(),
        }),
        document: value.document.map(|document| game::UiDocument {
            id: document.id,
            interface: document.interface.to_string(),
            title: document.title,
            pages: document.pages,
            page: u32::from(document.page),
            map_asset: document.map_asset.map(|id| id.to_string()),
            native_map: document.native_map,
        }),
    })
}

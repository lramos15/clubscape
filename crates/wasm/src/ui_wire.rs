use std::collections::BTreeMap;

use clubscape_game_types as types;
use clubscape_protocol::game;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{BridgeError, gameplay_ui, ui_input};

#[cfg(test)]
#[path = "../../protocol/tests/support/recovery_context.rs"]
mod recovery_fixture;

pub(crate) fn validate_capabilities(
    value: &game::GameplayUiView,
    capabilities: &std::collections::BTreeSet<String>,
) -> Result<(), BridgeError> {
    let amounts = capabilities.contains(gameplay_ui::AMOUNTS_CAPABILITY);
    let recovery = capabilities.contains(gameplay_ui::RECOVERY_CAPABILITY);
    if value.production.as_ref().is_some_and(|menu| {
        menu.recipes
            .iter()
            .any(|recipe| recipe.all.is_some() != amounts)
    }) || value
        .bank
        .as_ref()
        .is_some_and(|bank| bank.amount_selection.is_some() != amounts)
        || value
            .recovery
            .as_ref()
            .is_some_and(|controls| controls.management.is_some() != recovery)
    {
        return Err(BridgeError::protocol(
            "Negotiated UI amount/recovery capabilities require their complete exact projections.",
        ));
    }
    Ok(())
}

fn invalid() -> BridgeError {
    BridgeError::protocol("The versioned gameplay UI wire view is incomplete or invalid.")
}

fn amount(value: &game::UiAmount) -> Result<types::UiAmount, BridgeError> {
    Ok(match required(value.selection.as_ref())? {
        game::ui_amount::Selection::Quantity(value) => types::UiAmount::Quantity {
            quantity: types::Quantity::new(*value).map_err(|_| invalid())?,
        },
        game::ui_amount::Selection::All(_) => types::UiAmount::All {},
    })
}

fn storage(value: i32) -> Result<types::RecoveryStorage, BridgeError> {
    match game::RecoveryStorage::try_from(value).map_err(|_| invalid())? {
        game::RecoveryStorage::Grave => Ok(types::RecoveryStorage::Grave),
        game::RecoveryStorage::DeathOffice => Ok(types::RecoveryStorage::DeathOffice),
        game::RecoveryStorage::RecoveryUnspecified => Err(invalid()),
    }
}

fn recovery_management(
    value: &game::UiRecoveryManagement,
) -> Result<types::RecoveryManagementView, BridgeError> {
    Ok(types::RecoveryManagementView {
        context: value
            .context
            .as_ref()
            .map(clubscape_protocol::recovery_context_from_wire)
            .transpose()
            .map_err(|_| invalid())?,
        bank_revision: value.bank_revision.clone(),
        panels: value
            .panels
            .iter()
            .map(|panel| {
                Ok(types::RecoveryPanelControlView {
                    death: identity(&panel.death)?,
                    storage: storage(panel.storage)?,
                    entries: panel
                        .entries
                        .iter()
                        .map(|entry| {
                            Ok(types::RecoveryEntryControlView {
                                id: identity(&entry.id)?,
                                item: item(required(entry.item.as_ref())?)?,
                                unit_fee: entry.unit_fee.clone(),
                                full_stack_fee: entry.full_stack_fee.clone(),
                                inventory_capacity: entry.inventory_capacity,
                                bank_capacity: entry.bank_capacity,
                                take: permission(entry.take.as_ref())?,
                                bank: permission(entry.bank.as_ref())?,
                            })
                        })
                        .collect::<Result<_, BridgeError>>()?,
                    full_selection_fee: panel.full_selection_fee.clone(),
                    take_all: permission(panel.take_all.as_ref())?,
                })
            })
            .collect::<Result<_, BridgeError>>()?,
        bank_all: permission(value.bank_all.as_ref())?,
        bank_all_records: value
            .bank_all_records
            .iter()
            .map(|record| {
                Ok(types::RecoveryRecordSelection {
                    death: identity(&record.death)?,
                    items: record
                        .items
                        .iter()
                        .map(|id| identity(id))
                        .collect::<Result<_, _>>()?,
                })
            })
            .collect::<Result<_, BridgeError>>()?,
    })
}

fn required<T>(value: Option<&T>) -> Result<&T, BridgeError> {
    value.ok_or_else(invalid)
}

fn identity<T: DeserializeOwned>(value: &str) -> Result<T, BridgeError> {
    serde_json::from_value(Value::String(value.to_owned())).map_err(|_| invalid())
}

fn optional_id<T: DeserializeOwned>(value: &Option<String>) -> Result<Option<T>, BridgeError> {
    value.as_deref().map(identity).transpose()
}

fn bounded<T: TryFrom<u32>>(value: u32) -> Result<T, BridgeError> {
    value.try_into().map_err(|_| invalid())
}

fn permission(value: Option<&game::Permission>) -> Result<types::UiPermission, BridgeError> {
    let value = required(value)?;
    if value.allowed == value.denial.is_some() {
        return Err(invalid());
    }
    let code = value
        .denial
        .as_ref()
        .map(|denial| {
            use game::RuleErrorCode as W;
            use types::GameErrorCode as T;
            Ok(match W::try_from(denial.code).map_err(|_| invalid())? {
                W::InvalidInput => T::InvalidInput,
                W::UnknownContent => T::UnknownContent,
                W::InvalidContent => T::InvalidContent,
                W::Unavailable => T::Unavailable,
                W::NotOwned => T::NotOwned,
                W::InsufficientItems => T::InsufficientItems,
                W::InventoryFull => T::InventoryFull,
                W::StackOverflow => T::StackOverflow,
                W::RequirementNotMet => T::RequirementNotMet,
                W::OutOfReach => T::OutOfReach,
                W::Blocked => T::Blocked,
                W::Busy => T::Busy,
                W::StaleCommand => T::StaleCommand,
                W::SessionConflict => T::SessionConflict,
                W::RuleErrorUnspecified => return Err(invalid()),
            })
        })
        .transpose()?;
    Ok(types::UiPermission {
        allowed: value.allowed,
        code,
        reason: value.denial.as_ref().map(|denial| denial.message.clone()),
    })
}

pub(crate) fn target(value: &game::WorldTarget) -> Result<types::WorldTarget, BridgeError> {
    Ok(match required(value.target.as_ref())? {
        game::world_target::Target::Spawn(id) => types::WorldTarget::Spawn {
            spawn: identity(id)?,
        },
        game::world_target::Target::TemporaryObject(id) => types::WorldTarget::TemporaryObject {
            object: identity(id)?,
        },
    })
}

fn item(value: &game::UiItem) -> Result<types::UiItem, BridgeError> {
    let stack = required(value.stack.as_ref())?;
    Ok(types::UiItem {
        item: identity(&stack.item)?,
        name: value.name.clone(),
        quantity: stack.quantity,
        source_id: value.source_id,
        asset: optional_id(&value.asset)?,
        instance_id: optional_id(&stack.instance_id)?,
        charges: stack.charges,
    })
}

fn items(values: &[game::UiItem]) -> Result<Vec<types::UiItem>, BridgeError> {
    values.iter().map(item).collect()
}

fn operation(value: &game::UiItemOperation) -> Result<types::ItemActionUiView, BridgeError> {
    Ok(types::ItemActionUiView {
        id: value.id.clone(),
        label: value.label.clone(),
        permission: permission(value.permission.as_ref())?,
    })
}

fn inventory(
    value: &game::UiInventoryActions,
) -> Result<types::InventoryActionsUiView, BridgeError> {
    Ok(types::InventoryActionsUiView {
        slot: bounded(value.slot)?,
        item: identity(&value.item)?,
        instance: optional_id(&value.instance)?,
        actions: value
            .actions
            .iter()
            .map(operation)
            .collect::<Result<_, _>>()?,
    })
}

fn ability(value: &game::UiAbility) -> Result<types::AbilityUiView, BridgeError> {
    Ok(types::AbilityUiView {
        id: value.id.clone(),
        name: value.name.clone(),
        selected: value.selected,
        visible: value.visible,
        permission: permission(value.permission.as_ref())?,
    })
}

fn bonuses(values: &[game::UiBonus]) -> Result<BTreeMap<types::AttackType, i16>, BridgeError> {
    let mut result = BTreeMap::new();
    for value in values {
        let kind = match value.kind.as_str() {
            "stab" => types::AttackType::Stab,
            "slash" => types::AttackType::Slash,
            "crush" => types::AttackType::Crush,
            "ranged" => types::AttackType::Ranged,
            "magic" => types::AttackType::Magic,
            _ => return Err(invalid()),
        };
        if result
            .insert(kind, value.amount.try_into().map_err(|_| invalid())?)
            .is_some()
        {
            return Err(invalid());
        }
    }
    Ok(result)
}

fn chat(value: &game::PublicChatLine) -> Result<types::PublicChatLine, BridgeError> {
    Ok(types::PublicChatLine {
        id: value.id.clone(),
        actor: identity(&value.actor)?,
        sender: value.sender.clone(),
        channel: value.channel.clone(),
        text: value.text.clone(),
        colour: bounded(value.colour)?,
        effect: bounded(value.effect)?,
    })
}

pub(crate) fn decode(value: &game::GameplayUiView) -> Result<Value, BridgeError> {
    let equipment = required(value.equipment.as_ref())?;
    let appearance = required(value.appearance.as_ref())?;
    let public_chat = required(value.public_chat.as_ref())?;
    let mut choices = BTreeMap::new();
    for entry in &appearance.choices {
        let rows = entry
            .choices
            .iter()
            .map(|choice| {
                Ok(types::AppearanceChoiceUiView {
                    value: choice.value,
                    label: choice.label.clone(),
                    permission: permission(choice.permission.as_ref())?,
                })
            })
            .collect::<Result<_, BridgeError>>()?;
        if choices.insert(entry.key.clone(), rows).is_some() {
            return Err(invalid());
        }
    }
    let source = types::GameplayUiView {
        version: value.version,
        active_tab: optional_id(&value.active_tab)?,
        active_interface: optional_id(&value.active_interface)?,
        production: value
            .production
            .as_ref()
            .map(|menu| {
                Ok::<_, BridgeError>(types::ProductionUiView {
                    id: menu.id.clone(),
                    interface: identity(&menu.interface)?,
                    target: menu.target.as_ref().map(target).transpose()?,
                    recipes: menu
                        .recipes
                        .iter()
                        .map(|recipe| {
                            Ok(types::ProductionChoiceUiView {
                                recipe: identity(&recipe.recipe)?,
                                name: recipe.name.clone(),
                                outputs: items(&recipe.outputs)?,
                                single: permission(recipe.single.as_ref())?,
                                make_x: permission(recipe.make_x.as_ref())?,
                                all: recipe
                                    .all
                                    .as_ref()
                                    .map(|value| permission(Some(value)))
                                    .transpose()?,
                            })
                        })
                        .collect::<Result<_, BridgeError>>()?,
                })
            })
            .transpose()?,
        reward: value
            .reward
            .as_ref()
            .map(|reward| {
                Ok(types::RewardUiView {
                    id: reward.id.clone(),
                    kind: match reward.kind.as_str() {
                        "quest" => types::RewardUiKind::Quest,
                        "level_up" => types::RewardUiKind::LevelUp,
                        _ => return Err(invalid()),
                    },
                    interface: identity(&reward.interface)?,
                    title: reward.title.clone(),
                    lines: reward.lines.clone(),
                    items: items(&reward.items)?,
                    xp: reward
                        .xp
                        .iter()
                        .map(|xp| {
                            Ok(types::UiXpAward {
                                skill: identity(&xp.skill)?,
                                amount_tenths: xp.amount_tenths.clone(),
                            })
                        })
                        .collect::<Result<_, BridgeError>>()?,
                    quest_points: reward.quest_points,
                    quest: optional_id(&reward.quest)?,
                    skill: optional_id(&reward.skill)?,
                    level: reward.level.map(bounded).transpose()?,
                    continuation: clubscape_protocol::ui_request(required(
                        reward.continuation.as_ref(),
                    )?)
                    .map_err(|_| invalid())?,
                })
            })
            .transpose()?,
        confirmation: value
            .confirmation
            .as_ref()
            .map(|confirmation| {
                Ok::<_, BridgeError>(types::ConfirmationUiView {
                    id: confirmation.id.clone(),
                    kind: confirmation.kind.clone(),
                    title: confirmation.title.clone(),
                    lines: confirmation.lines.clone(),
                    items: items(&confirmation.items)?,
                    credit: confirmation.credit.clone(),
                })
            })
            .transpose()?,
        document: value
            .document
            .as_ref()
            .map(|document| {
                Ok::<_, BridgeError>(types::DocumentUiView {
                    id: document.id.clone(),
                    interface: identity(&document.interface)?,
                    title: document.title.clone(),
                    pages: document.pages.clone(),
                    page: bounded(document.page)?,
                    map_asset: optional_id(&document.map_asset)?,
                    native_map: document.native_map,
                })
            })
            .transpose()?,
        interfaces: value
            .interfaces
            .iter()
            .map(|entry| {
                Ok(types::InterfaceUiView {
                    interface: identity(&entry.interface)?,
                    visibility: match entry.visibility.as_str() {
                        "hidden" => types::UiVisibility::Hidden,
                        "locked" => types::UiVisibility::Locked,
                        "enabled" => types::UiVisibility::Enabled,
                        _ => return Err(invalid()),
                    },
                    highlighted: entry.highlighted,
                    permission: permission(entry.permission.as_ref())?,
                })
            })
            .collect::<Result<_, BridgeError>>()?,
        combat_style: value.combat_style.clone(),
        combat_styles: value
            .combat_styles
            .iter()
            .map(ability)
            .collect::<Result<_, _>>()?,
        prayers: value
            .prayers
            .iter()
            .map(ability)
            .collect::<Result<_, _>>()?,
        spells: value.spells.iter().map(ability).collect::<Result<_, _>>()?,
        equipment: types::EquipmentUiView {
            bonuses: types::CombatBonuses {
                attack: bonuses(&equipment.attack)?,
                defence: bonuses(&equipment.defence)?,
                melee_strength: equipment.melee_strength.try_into().map_err(|_| invalid())?,
                ranged_strength: equipment
                    .ranged_strength
                    .try_into()
                    .map_err(|_| invalid())?,
                magic_damage_percent: equipment
                    .magic_damage_percent
                    .try_into()
                    .map_err(|_| invalid())?,
                prayer: equipment.prayer.try_into().map_err(|_| invalid())?,
            },
            weight_grams: equipment.weight_grams.clone(),
            slots: equipment
                .slots
                .iter()
                .map(|slot| identity(slot))
                .collect::<Result<_, _>>()?,
        },
        inventory_actions: value
            .inventory_actions
            .iter()
            .map(inventory)
            .collect::<Result<_, _>>()?,
        bank: value
            .bank
            .as_ref()
            .map(|bank| {
                Ok::<_, BridgeError>(types::BankUiView {
                    revision: bank.revision.clone(),
                    capacity: bounded(bank.capacity)?,
                    selected_tab: bounded(bank.selected_tab)?,
                    insert_mode: bank.insert_mode,
                    placeholders: bank.placeholders,
                    amount: bank.amount,
                    amount_selection: bank.amount_selection.as_ref().map(amount).transpose()?,
                    noted: bank.noted,
                    tabs: bank
                        .tabs
                        .iter()
                        .map(|tab| {
                            Ok(types::BankTabUiView {
                                tab: bounded(tab.tab)?,
                                first_entry: tab.first_entry.clone(),
                                entries: tab.entries,
                            })
                        })
                        .collect::<Result<_, BridgeError>>()?,
                    entries: bank
                        .entries
                        .iter()
                        .map(|entry| {
                            Ok(types::BankEntryUiView {
                                id: entry.id.clone(),
                                slot: bounded(entry.slot)?,
                                tab: bounded(entry.tab)?,
                                item: identity(&entry.item)?,
                                value: entry.value.as_ref().map(item).transpose()?,
                                placeholder: entry.placeholder,
                            })
                        })
                        .collect::<Result<_, BridgeError>>()?,
                    deposit_equipment: permission(bank.deposit_equipment.as_ref())?,
                    unavailable_containers: bank
                        .unavailable_containers
                        .iter()
                        .map(operation)
                        .collect::<Result<_, _>>()?,
                })
            })
            .transpose()?,
        kept_on_death: value
            .kept_on_death
            .as_ref()
            .map(|death| {
                Ok::<_, BridgeError>(types::DeathPreviewUiView {
                    scope: death.scope.clone(),
                    kept: items(&death.kept)?,
                    lost: items(&death.lost)?,
                    full_grave_fee: death.full_grave_fee.clone(),
                    full_office_fee: death.full_office_fee.clone(),
                    value_revision: death.value_revision.clone(),
                })
            })
            .transpose()?,
        recovery: value
            .recovery
            .as_ref()
            .map(|recovery| {
                Ok::<_, BridgeError>(types::RecoveryUiControls {
                    coffer_balance: recovery.coffer_balance.clone(),
                    discard: permission(recovery.discard.as_ref())?,
                    coffer_offer: permission(recovery.coffer_offer.as_ref())?,
                    coffer_items: recovery
                        .coffer_items
                        .iter()
                        .map(inventory)
                        .collect::<Result<_, _>>()?,
                    management: recovery
                        .management
                        .as_ref()
                        .map(recovery_management)
                        .transpose()?,
                })
            })
            .transpose()?,
        appearance: types::AppearanceUiView {
            choices,
            base: appearance
                .base
                .as_ref()
                .map(|base| {
                    Ok::<_, BridgeError>(types::PenguinBaseUiView {
                        asset: identity(&base.asset)?,
                        source_npc: base.source_npc,
                        adaptation: base.adaptation.clone(),
                    })
                })
                .transpose()?,
            confirmed: appearance.confirmed,
        },
        public_chat: types::PublicChatUiView {
            permission: permission(public_chat.permission.as_ref())?,
            maximum_bytes: bounded(public_chat.maximum_bytes)?,
            channel: public_chat.channel.clone(),
            messages: public_chat
                .messages
                .iter()
                .map(chat)
                .collect::<Result<_, _>>()?,
        },
    };
    let mut result = gameplay_ui::project(&source)?;
    if let Some(reward) = &value.reward {
        result["reward"]["continuation"] = ui_input::json(required(reward.continuation.as_ref())?)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    use serde_json::json;

    fn allowed() -> Option<game::Permission> {
        Some(game::Permission {
            allowed: true,
            denial: None,
        })
    }
    fn item() -> game::UiItem {
        game::UiItem {
            stack: Some(game::Stack {
                item: "item.fixture".into(),
                quantity: 1,
                instance_id: Some("item_instance.fixture".into()),
                charges: Some(4),
            }),
            name: "Source item".into(),
            source_id: Some(17),
            asset: Some("asset.fixture.icon".into()),
        }
    }
    fn inventory() -> game::UiInventoryActions {
        game::UiInventoryActions {
            slot: 3,
            item: "item.fixture".into(),
            instance: Some("item_instance.fixture".into()),
            actions: vec![game::UiItemOperation {
                id: "action.fixture".into(),
                label: "Read".into(),
                permission: allowed(),
            }],
        }
    }
    fn fixture() -> game::GameplayUiView {
        game::GameplayUiView {
            version: 1,
            active_tab: Some("interface.inventory".into()),
            active_interface: Some("interface.production".into()),
            production: Some(game::UiProduction {
                id: "menu.fixture".into(),
                interface: "interface.production".into(),
                target: None,
                recipes: vec![game::UiProductionChoice {
                    recipe: "recipe.fixture".into(),
                    name: "Source recipe".into(),
                    outputs: vec![item()],
                    single: allowed(),
                    make_x: allowed(),
                    all: None,
                }],
            }),
            reward: Some(game::UiReward {
                id: "presentation.fixture".into(),
                kind: "quest".into(),
                interface: "interface.reward".into(),
                title: "Source reward".into(),
                lines: vec!["Source line".into()],
                items: vec![item()],
                xp: vec![game::UiXp {
                    skill: "skill.fixture".into(),
                    amount_tenths: u64::MAX.to_string(),
                }],
                quest_points: 1,
                quest: Some("quest.fixture".into()),
                skill: None,
                level: None,
                continuation: Some(game::GameplayUiRequest {
                    expected_bank_revision: None,
                    request: Some(game::gameplay_ui_request::Request::Dismiss(
                        game::UiIdentity {
                            id: "presentation.fixture".into(),
                        },
                    )),
                }),
            }),
            confirmation: Some(game::UiConfirmationView {
                id: "confirmation.fixture".into(),
                kind: "coffer_offer".into(),
                title: "Source confirmation".into(),
                lines: vec![],
                items: vec![item()],
                credit: Some("9007199254740993".into()),
            }),
            document: Some(game::UiDocument {
                id: "document.fixture".into(),
                interface: "interface.document".into(),
                title: "Source map".into(),
                pages: vec!["First".into(), "Second".into()],
                page: 1,
                map_asset: Some("asset.fixture.map".into()),
                native_map: true,
            }),
            interfaces: vec![game::UiInterfaceState {
                interface: "interface.production".into(),
                visibility: "enabled".into(),
                highlighted: true,
                permission: allowed(),
            }],
            combat_style: Some("style.fixture".into()),
            combat_styles: vec![game::UiAbility {
                id: "style.fixture".into(),
                name: "Source style".into(),
                selected: true,
                visible: true,
                permission: allowed(),
            }],
            prayers: vec![],
            spells: vec![],
            equipment: Some(game::UiEquipment {
                attack: vec![game::UiBonus {
                    kind: "stab".into(),
                    amount: -1,
                }],
                defence: vec![],
                melee_strength: 2,
                ranged_strength: 3,
                magic_damage_percent: 4,
                prayer: 5,
                weight_grams: "-9007199254740993".into(),
                slots: vec!["slot.weapon".into()],
            }),
            inventory_actions: vec![inventory()],
            bank: Some(game::UiBank {
                revision: "9007199254741993".into(),
                capacity: 2,
                selected_tab: 1,
                insert_mode: true,
                placeholders: true,
                amount: 5,
                amount_selection: None,
                noted: false,
                tabs: vec![game::UiBankTab {
                    tab: 1,
                    first_entry: Some("9007199254740993".into()),
                    entries: 2,
                }],
                entries: vec![
                    game::UiBankEntry {
                        id: "9007199254740993".into(),
                        slot: 0,
                        tab: 1,
                        item: "item.fixture".into(),
                        value: None,
                        placeholder: true,
                    },
                    game::UiBankEntry {
                        id: "9007199254740994".into(),
                        slot: 1,
                        tab: 1,
                        item: "item.fixture".into(),
                        value: Some(item()),
                        placeholder: false,
                    },
                ],
                deposit_equipment: allowed(),
                unavailable_containers: vec![],
            }),
            kept_on_death: Some(game::UiDeathPreview {
                scope: "normal_unsafe_non_pvp".into(),
                kept: vec![item()],
                lost: vec![],
                full_grave_fee: u64::MAX.to_string(),
                full_office_fee: "9007199254740993".into(),
                value_revision: "9007199254742993".into(),
            }),
            recovery: Some(game::UiRecoveryControls {
                coffer_balance: u64::MAX.to_string(),
                discard: allowed(),
                coffer_offer: allowed(),
                coffer_items: vec![inventory()],
                management: None,
            }),
            appearance: Some(game::UiAppearance {
                choices: vec![game::UiAppearanceParameter {
                    key: "body_type".into(),
                    choices: vec![game::UiAppearanceChoice {
                        value: 0,
                        label: None,
                        permission: allowed(),
                    }],
                }],
                base: Some(game::UiPenguinBase {
                    asset: "asset.fixture.penguin".into(),
                    source_npc: 2063,
                    adaptation: "adaptation.fixture".into(),
                }),
                confirmed: false,
            }),
            public_chat: Some(game::UiPublicChat {
                permission: allowed(),
                maximum_bytes: 120,
                channel: "public".into(),
                messages: vec![game::PublicChatLine {
                    id: "chat.fixture".into(),
                    actor: "actor.fixture".into(),
                    sender: "Source sender".into(),
                    channel: "public".into(),
                    text: "Source text".into(),
                    colour: 2,
                    effect: 1,
                }],
            }),
        }
    }

    #[test]
    fn complete_ui4_wire_preserves_all_fields_ids_nulls_and_decimal_values() {
        let original = fixture();
        let bytes = game::WorldSnapshot {
            ui: Some(original.clone()),
            ..Default::default()
        }
        .encode_to_vec();
        assert_eq!(&bytes[..2], &[0xa2, 0x01], "WorldSnapshot.ui is tag20");
        let snapshot = game::WorldSnapshot::decode(bytes.as_slice()).unwrap();
        let value = decode(snapshot.ui.as_ref().unwrap()).unwrap();
        assert_eq!(value["activeTab"], "interface.inventory");
        assert_eq!(
            value["document"],
            json!({"id":"document.fixture","interface":"interface.document",
            "title":"Source map","pages":["First","Second"],"page":1,"mapAsset":"asset.fixture.map","nativeMap":true})
        );
        assert!(value["production"]["target"].is_null());
        assert_eq!(
            value["production"]["recipes"][0]["outputs"][0]["instanceId"],
            "item_instance.fixture"
        );
        assert_eq!(
            value["reward"]["xp"][0]["amountTenths"],
            u64::MAX.to_string()
        );
        assert_eq!(
            value["reward"]["continuation"],
            json!({"kind":"ui_dismiss","presentation_id":"presentation.fixture"})
        );
        assert_eq!(value["confirmation"]["credit"], "9007199254740993");
        assert_eq!(value["bank"]["revision"], "9007199254741993");
        assert_eq!(value["bank"]["entries"][0]["id"], "9007199254740993");
        assert!(value["bank"]["entries"][0]["value"].is_null());
        assert_eq!(value["equipment"]["weightGrams"], "-9007199254740993");
        assert_eq!(value["recovery"]["cofferBalance"], u64::MAX.to_string());
        assert_eq!(value["appearance"]["base"]["sourceNpc"], 2063);
        assert_eq!(value["publicChat"]["messages"][0]["id"], "chat.fixture");
        assert_eq!(original, fixture());
    }

    #[test]
    fn malformed_required_ui_messages_do_not_become_empty_success() {
        let mut value = fixture();
        value.equipment = None;
        assert!(decode(&value).is_err());
        value = fixture();
        value.bank.as_mut().unwrap().deposit_equipment = None;
        assert!(decode(&value).is_err());
        value = fixture();
        value.version = 2;
        assert!(decode(&value).is_err());
        value = fixture();
        value
            .equipment
            .as_mut()
            .unwrap()
            .attack
            .push(game::UiBonus {
                kind: "stab".into(),
                amount: 4,
            });
        assert!(decode(&value).is_err());
    }

    #[test]
    fn full_recovery_context_is_lossless_from_wire_to_camel_case_without_legacy_selection() {
        let context = recovery_fixture::context(9_007_199_254_740_993);
        let mut original = fixture();
        original.recovery.as_mut().unwrap().management = Some(game::UiRecoveryManagement {
            context: Some(clubscape_protocol::recovery_context_to_wire(
                context.clone(),
            )),
            bank_revision: "9007199254742993".into(),
            panels: Vec::new(),
            bank_all: allowed(),
            bank_all_records: Vec::new(),
        });
        let bytes = original.encode_to_vec();
        let projected = decode(&game::GameplayUiView::decode(bytes.as_slice()).unwrap()).unwrap();
        let view = &projected["recovery"]["management"]["context"];
        assert_eq!(view["identity"], serde_json::json!(context.identity));
        assert_eq!(view["counts"]["entries"], 2);
        assert_eq!(view["counts"]["nativeItemTypes"], 1);
        assert_eq!(view["counts"]["capacity"], 120);
        assert_eq!(view["slots"][0]["entry"]["unitFee"], "9007199254740993");
        assert_eq!(view["slots"][1]["slot"], 1);
        assert_eq!(view["slots"][1]["entry"]["item"]["sourceId"], 882);
        assert_eq!(view["slots"][0]["selectedTypeCaption"]["quantity"], "14");
        assert_eq!(
            view["takeAll"]["selection"],
            serde_json::json!(context.take_all.selection)
        );
        assert_eq!(
            view["takeAll"]["plan"]["totalFee"],
            context.take_all.plan.unwrap().total_fee
        );
        assert!(view.get("take_all").is_none());

        let mut malformed = original.clone();
        malformed
            .recovery
            .as_mut()
            .unwrap()
            .management
            .as_mut()
            .unwrap()
            .context
            .as_mut()
            .unwrap()
            .slots[1]
            .slot = 0;
        assert!(decode(&malformed).is_err());
        original
            .recovery
            .as_mut()
            .unwrap()
            .management
            .as_mut()
            .unwrap()
            .context = None;
        assert!(
            decode(&original).unwrap()["recovery"]["management"]
                .get("context")
                .is_none()
        );
    }

    #[test]
    fn empty_office_identity_survives_wire_projection_without_a_fake_death_record() {
        let mut context = recovery_fixture::context(42);
        context.slots.clear();
        context.counts.entries = 0;
        context.counts.native_item_types = Some(0);
        context.counts.stored = 0;
        context.counts.offered = 0;
        context.take_all.selection.records.clear();
        context.take_all.permission = types::UiPermission {
            allowed: false,
            code: Some(types::GameErrorCode::NotOwned),
            reason: Some("There are no recovery items to take.".into()),
        };
        context.take_all.plan = None;
        let mut original = fixture();
        original.recovery.as_mut().unwrap().management = Some(game::UiRecoveryManagement {
            context: Some(clubscape_protocol::recovery_context_to_wire(
                context.clone(),
            )),
            bank_revision: "0".into(),
            panels: Vec::new(),
            bank_all: allowed(),
            bank_all_records: Vec::new(),
        });
        let projected = decode(&original).unwrap();
        let result = &projected["recovery"]["management"]["context"];
        assert_eq!(result["identity"], serde_json::json!(context.identity));
        assert_eq!(result["slots"], serde_json::json!([]));
        assert_eq!(result["counts"]["capacity"], 120);
        assert_eq!(result["takeAll"]["permission"]["allowed"], false);
        assert!(result["takeAll"]["plan"].is_null());
    }

    #[test]
    fn semantic_amount_and_recovery_controls_keep_distinct_fees_capacities_and_opaque_selections() {
        let mut original = fixture();
        original.production.as_mut().unwrap().recipes[0].all = allowed();
        original.bank.as_mut().unwrap().amount_selection = Some(game::UiAmount {
            selection: Some(game::ui_amount::Selection::All(game::Empty {})),
        });
        let denied = Some(game::Permission {
            allowed: false,
            denial: Some(game::RuleDenial {
                code: game::RuleErrorCode::Unavailable as i32,
                message: "Source normal-grave Bank-All permission is not verified.".into(),
            }),
        });
        let mut stack = item();
        stack.stack.as_mut().unwrap().quantity = 7;
        original.recovery.as_mut().unwrap().management = Some(game::UiRecoveryManagement {
            context: None,
            bank_revision: "9007199254742993".into(),
            panels: vec![game::UiRecoveryPanelControl {
                death: "death.fixture".into(),
                storage: game::RecoveryStorage::Grave as i32,
                entries: vec![game::UiRecoveryEntryControl {
                    id: "recovery_item.fixture".into(),
                    item: Some(stack),
                    unit_fee: "3".into(),
                    full_stack_fee: "17".into(),
                    inventory_capacity: 3,
                    bank_capacity: 2,
                    take: allowed(),
                    bank: denied.clone(),
                }],
                full_selection_fee: u64::MAX.to_string(),
                take_all: allowed(),
            }],
            bank_all: denied,
            bank_all_records: vec![game::UiRecoveryRecordSelection {
                death: "death.fixture".into(),
                items: vec!["recovery_item.fixture".into()],
            }],
        });
        let capabilities = [
            gameplay_ui::CAPABILITY,
            gameplay_ui::AMOUNTS_CAPABILITY,
            gameplay_ui::RECOVERY_CAPABILITY,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        validate_capabilities(&original, &capabilities).unwrap();
        let bytes = original.encode_to_vec();
        let value = decode(&game::GameplayUiView::decode(bytes.as_slice()).unwrap()).unwrap();
        assert_eq!(value["bank"]["amountSelection"], json!({"kind":"all"}));
        assert_eq!(
            value["bank"]["amount"], 5,
            "The legacy quantity is not an All sentinel."
        );
        assert_eq!(value["production"]["recipes"][0]["all"]["allowed"], true);
        let management = &value["recovery"]["management"];
        assert_eq!(management["bankRevision"], "9007199254742993");
        assert_eq!(management["panels"][0]["entries"][0]["unitFee"], "3");
        assert_eq!(management["panels"][0]["entries"][0]["fullStackFee"], "17");
        assert_eq!(
            management["panels"][0]["entries"][0]["inventoryCapacity"],
            3
        );
        assert_eq!(management["panels"][0]["entries"][0]["bankCapacity"], 2);
        assert_eq!(
            management["panels"][0]["fullSelectionFee"],
            u64::MAX.to_string()
        );
        assert_eq!(management["bankAll"]["allowed"], false);
        assert_eq!(
            management["bankAll"]["reason"],
            "Source normal-grave Bank-All permission is not verified."
        );
        assert_eq!(
            management["bankAllRecords"],
            json!([{"death":"death.fixture","items":["recovery_item.fixture"]}])
        );
        original.bank.as_mut().unwrap().amount_selection = None;
        assert!(validate_capabilities(&original, &capabilities).is_err());
    }
}

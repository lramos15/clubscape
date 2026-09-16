use clubscape_game_types as types;

use crate::{ValidationError, game, invalid};

fn malformed() -> ValidationError {
    invalid("The recovery context is incomplete, inconsistent or out of bounds.")
}

fn required<T>(value: Option<T>) -> Result<T, ValidationError> {
    value.ok_or_else(malformed)
}

fn storage_to_wire(value: types::RecoveryStorage) -> i32 {
    match value {
        types::RecoveryStorage::Grave => game::RecoveryStorage::Grave as i32,
        types::RecoveryStorage::DeathOffice => game::RecoveryStorage::DeathOffice as i32,
    }
}

fn storage_from_wire(value: i32) -> Result<types::RecoveryStorage, ValidationError> {
    match game::RecoveryStorage::try_from(value).map_err(|_| malformed())? {
        game::RecoveryStorage::Grave => Ok(types::RecoveryStorage::Grave),
        game::RecoveryStorage::DeathOffice => Ok(types::RecoveryStorage::DeathOffice),
        game::RecoveryStorage::RecoveryUnspecified => Err(malformed()),
    }
}

fn identity_to_wire(value: types::RecoveryContextIdentity) -> game::UiRecoveryContextIdentity {
    use game::ui_recovery_context_identity::Context;
    game::UiRecoveryContextIdentity {
        context: Some(match value {
            types::RecoveryContextIdentity::Grave { interface, death } => {
                Context::Grave(game::UiRecoveryGraveContext {
                    interface: interface.to_string(),
                    death: death.to_string(),
                })
            }
            types::RecoveryContextIdentity::DeathOffice {
                interface,
                instance,
            } => Context::DeathOffice(game::UiRecoveryOfficeContext {
                interface: interface.to_string(),
                instance: instance.map(|id| id.to_string()),
            }),
        }),
    }
}

fn identity_from_wire(
    value: &game::UiRecoveryContextIdentity,
) -> Result<types::RecoveryContextIdentity, ValidationError> {
    use game::ui_recovery_context_identity::Context;
    Ok(match required(value.context.as_ref())? {
        Context::Grave(value) => types::RecoveryContextIdentity::Grave {
            interface: types::InterfaceId::new(&value.interface).map_err(|_| malformed())?,
            death: types::DeathId::new(&value.death).map_err(|_| malformed())?,
        },
        Context::DeathOffice(value) => types::RecoveryContextIdentity::DeathOffice {
            interface: types::InterfaceId::new(&value.interface).map_err(|_| malformed())?,
            instance: value
                .instance
                .as_ref()
                .map(types::InstanceId::new)
                .transpose()
                .map_err(|_| malformed())?,
        },
    })
}

pub fn recovery_context_selection_to_wire(
    value: types::RecoveryContextSelection,
) -> game::UiRecoveryContextSelection {
    game::UiRecoveryContextSelection {
        context: Some(identity_to_wire(value.context)),
        records: value
            .records
            .into_iter()
            .map(|record| game::UiRecoveryContextRecordSelection {
                death: record.death.to_string(),
                entries: record
                    .entries
                    .into_iter()
                    .map(|entry| game::UiRecoveryContextEntrySelection {
                        id: entry.id.to_string(),
                        quantity: entry.quantity.get(),
                        current_storage: storage_to_wire(entry.current_storage),
                    })
                    .collect(),
            })
            .collect(),
    }
}

pub fn recovery_context_selection_from_wire(
    value: &game::UiRecoveryContextSelection,
) -> Result<types::RecoveryContextSelection, ValidationError> {
    if value.records.len() > types::MAX_RECOVERY_SELECTION_ENTRIES
        || value
            .records
            .iter()
            .map(|record| record.entries.len())
            .sum::<usize>()
            > types::MAX_RECOVERY_SELECTION_ENTRIES
    {
        return Err(malformed());
    }
    let result = types::RecoveryContextSelection {
        context: identity_from_wire(required(value.context.as_ref())?)?,
        records: value
            .records
            .iter()
            .map(|record| {
                Ok(types::RecoveryContextRecordSelection {
                    death: types::DeathId::new(&record.death).map_err(|_| malformed())?,
                    entries: record
                        .entries
                        .iter()
                        .map(|entry| {
                            Ok(types::RecoveryContextEntrySelection {
                                id: types::RecoveryItemId::new(&entry.id)
                                    .map_err(|_| malformed())?,
                                quantity: types::Quantity::new(entry.quantity)
                                    .map_err(|_| malformed())?,
                                current_storage: storage_from_wire(entry.current_storage)?,
                            })
                        })
                        .collect::<Result<_, ValidationError>>()?,
                })
            })
            .collect::<Result<_, ValidationError>>()?,
    };
    result.validate_shape().map_err(|_| malformed())?;
    Ok(result)
}

fn code_to_wire(value: types::GameErrorCode) -> game::RuleErrorCode {
    use types::GameErrorCode as C;
    match value {
        C::InvalidInput => game::RuleErrorCode::InvalidInput,
        C::UnknownContent => game::RuleErrorCode::UnknownContent,
        C::InvalidContent => game::RuleErrorCode::InvalidContent,
        C::Unavailable => game::RuleErrorCode::Unavailable,
        C::NotOwned => game::RuleErrorCode::NotOwned,
        C::InsufficientItems => game::RuleErrorCode::InsufficientItems,
        C::InventoryFull => game::RuleErrorCode::InventoryFull,
        C::StackOverflow => game::RuleErrorCode::StackOverflow,
        C::RequirementNotMet => game::RuleErrorCode::RequirementNotMet,
        C::OutOfReach => game::RuleErrorCode::OutOfReach,
        C::Blocked => game::RuleErrorCode::Blocked,
        C::Busy => game::RuleErrorCode::Busy,
        C::StaleCommand => game::RuleErrorCode::StaleCommand,
        C::SessionConflict => game::RuleErrorCode::SessionConflict,
    }
}

fn code_from_wire(value: i32) -> Result<types::GameErrorCode, ValidationError> {
    use game::RuleErrorCode as C;
    Ok(match C::try_from(value).map_err(|_| malformed())? {
        C::InvalidInput => types::GameErrorCode::InvalidInput,
        C::UnknownContent => types::GameErrorCode::UnknownContent,
        C::InvalidContent => types::GameErrorCode::InvalidContent,
        C::Unavailable => types::GameErrorCode::Unavailable,
        C::NotOwned => types::GameErrorCode::NotOwned,
        C::InsufficientItems => types::GameErrorCode::InsufficientItems,
        C::InventoryFull => types::GameErrorCode::InventoryFull,
        C::StackOverflow => types::GameErrorCode::StackOverflow,
        C::RequirementNotMet => types::GameErrorCode::RequirementNotMet,
        C::OutOfReach => types::GameErrorCode::OutOfReach,
        C::Blocked => types::GameErrorCode::Blocked,
        C::Busy => types::GameErrorCode::Busy,
        C::StaleCommand => types::GameErrorCode::StaleCommand,
        C::SessionConflict => types::GameErrorCode::SessionConflict,
        C::RuleErrorUnspecified => return Err(malformed()),
    })
}

fn permission_to_wire(value: types::UiPermission) -> game::Permission {
    game::Permission {
        allowed: value.allowed,
        denial: value.code.map(|code| game::RuleDenial {
            code: code_to_wire(code) as i32,
            message: value.reason.unwrap_or_default(),
        }),
    }
}

fn permission_from_wire(value: &game::Permission) -> Result<types::UiPermission, ValidationError> {
    if value.allowed == value.denial.is_some() {
        return Err(malformed());
    }
    Ok(types::UiPermission {
        allowed: value.allowed,
        code: value
            .denial
            .as_ref()
            .map(|denial| code_from_wire(denial.code))
            .transpose()?,
        reason: value.denial.as_ref().map(|denial| denial.message.clone()),
    })
}

fn entry_to_wire(value: types::RecoveryEntryControlView) -> game::UiRecoveryEntryControl {
    game::UiRecoveryEntryControl {
        id: value.id.to_string(),
        item: Some(game::UiItem {
            stack: Some(game::Stack {
                item: value.item.item.to_string(),
                quantity: value.item.quantity,
                instance_id: value.item.instance_id.map(|id| id.to_string()),
                charges: value.item.charges,
            }),
            name: value.item.name,
            source_id: value.item.source_id,
            asset: value.item.asset.map(|id| id.to_string()),
        }),
        unit_fee: value.unit_fee,
        full_stack_fee: value.full_stack_fee,
        inventory_capacity: value.inventory_capacity,
        bank_capacity: value.bank_capacity,
        take: Some(permission_to_wire(value.take)),
        bank: Some(permission_to_wire(value.bank)),
    }
}

fn entry_from_wire(
    value: &game::UiRecoveryEntryControl,
) -> Result<types::RecoveryEntryControlView, ValidationError> {
    let item = required(value.item.as_ref())?;
    let stack = required(item.stack.as_ref())?;
    types::Quantity::new(stack.quantity).map_err(|_| malformed())?;
    Ok(types::RecoveryEntryControlView {
        id: types::RecoveryItemId::new(&value.id).map_err(|_| malformed())?,
        item: types::UiItem {
            item: types::ItemId::new(&stack.item).map_err(|_| malformed())?,
            quantity: stack.quantity,
            instance_id: stack
                .instance_id
                .as_ref()
                .map(types::ItemInstanceId::new)
                .transpose()
                .map_err(|_| malformed())?,
            charges: stack.charges,
            name: item.name.clone(),
            source_id: item.source_id,
            asset: item
                .asset
                .as_ref()
                .map(types::AssetId::new)
                .transpose()
                .map_err(|_| malformed())?,
        },
        unit_fee: value.unit_fee.clone(),
        full_stack_fee: value.full_stack_fee.clone(),
        inventory_capacity: value.inventory_capacity,
        bank_capacity: value.bank_capacity,
        take: permission_from_wire(required(value.take.as_ref())?)?,
        bank: permission_from_wire(required(value.bank.as_ref())?)?,
    })
}

pub fn recovery_context_to_wire(value: types::RecoveryContextView) -> game::UiRecoveryContext {
    use game::ui_recovery_type_caption::Caption;
    game::UiRecoveryContext {
        version: value.version,
        identity: Some(identity_to_wire(value.identity)),
        counts: Some(game::UiRecoveryContextCounts {
            entries: value.counts.entries,
            native_item_types: value.counts.native_item_types,
            capacity: u32::from(value.counts.capacity),
            capacity_unit: match value.counts.capacity_unit {
                types::RecoveryCapacityUnit::Entries => {
                    game::UiRecoveryCapacityUnit::RecoveryCapacityEntries as i32
                }
                types::RecoveryCapacityUnit::ItemTypesOrInstances => {
                    game::UiRecoveryCapacityUnit::RecoveryCapacityItemTypesOrInstances as i32
                }
            },
            stored: value.counts.stored,
            offered: value.counts.offered,
        }),
        slots: value
            .slots
            .into_iter()
            .map(|slot| game::UiRecoveryContextSlot {
                slot: slot.slot,
                death: slot.death.to_string(),
                current_storage: storage_to_wire(slot.current_storage),
                entry: Some(entry_to_wire(slot.entry)),
                selected_type_caption: Some(game::UiRecoveryTypeCaption {
                    caption: Some(match slot.selected_type_caption {
                        types::RecoveryTypeCaption::Source {
                            source_id,
                            quantity,
                            unit_fee,
                            total_fee,
                        } => Caption::Source(game::UiRecoverySourceTypeCaption {
                            source_id,
                            quantity,
                            unit_fee,
                            total_fee,
                        }),
                        types::RecoveryTypeCaption::Unavailable { reason } => {
                            Caption::UnavailableReason(reason)
                        }
                    }),
                }),
            })
            .collect(),
        take_all: Some(game::UiRecoveryTakeAllControl {
            permission: Some(permission_to_wire(value.take_all.permission)),
            selection: Some(recovery_context_selection_to_wire(value.take_all.selection)),
            plan: value.take_all.plan.map(|plan| game::UiRecoveryExecution {
                total_fee: plan.total_fee,
                transfers: plan
                    .transfers
                    .into_iter()
                    .map(|item| game::UiRecoveryPlannedItem {
                        death: item.death.to_string(),
                        id: item.id.to_string(),
                        quantity: item.quantity.get(),
                        fee: item.fee,
                    })
                    .collect(),
                partial: plan.partial,
            }),
        }),
    }
}

pub fn recovery_context_from_wire(
    value: &game::UiRecoveryContext,
) -> Result<types::RecoveryContextView, ValidationError> {
    use game::ui_recovery_type_caption::Caption;
    if value.slots.len() > types::MAX_RECOVERY_SELECTION_ENTRIES {
        return Err(malformed());
    }
    let counts = required(value.counts.as_ref())?;
    let take = required(value.take_all.as_ref())?;
    let result = types::RecoveryContextView {
        version: value.version,
        identity: identity_from_wire(required(value.identity.as_ref())?)?,
        counts: types::RecoveryContextCounts {
            entries: counts.entries,
            native_item_types: counts.native_item_types,
            capacity: u16::try_from(counts.capacity).map_err(|_| malformed())?,
            capacity_unit: match game::UiRecoveryCapacityUnit::try_from(counts.capacity_unit)
                .map_err(|_| malformed())?
            {
                game::UiRecoveryCapacityUnit::RecoveryCapacityEntries => {
                    types::RecoveryCapacityUnit::Entries
                }
                game::UiRecoveryCapacityUnit::RecoveryCapacityItemTypesOrInstances => {
                    types::RecoveryCapacityUnit::ItemTypesOrInstances
                }
                game::UiRecoveryCapacityUnit::RecoveryCapacityUnspecified => {
                    return Err(malformed());
                }
            },
            stored: counts.stored,
            offered: counts.offered,
        },
        slots: value
            .slots
            .iter()
            .map(|slot| {
                let caption = required(
                    required(slot.selected_type_caption.as_ref())?
                        .caption
                        .as_ref(),
                )?;
                Ok(types::RecoveryContextSlotView {
                    slot: slot.slot,
                    death: types::DeathId::new(&slot.death).map_err(|_| malformed())?,
                    current_storage: storage_from_wire(slot.current_storage)?,
                    entry: entry_from_wire(required(slot.entry.as_ref())?)?,
                    selected_type_caption: match caption {
                        Caption::Source(value) => types::RecoveryTypeCaption::Source {
                            source_id: value.source_id,
                            quantity: value.quantity.clone(),
                            unit_fee: value.unit_fee.clone(),
                            total_fee: value.total_fee.clone(),
                        },
                        Caption::UnavailableReason(reason) => {
                            types::RecoveryTypeCaption::Unavailable {
                                reason: reason.clone(),
                            }
                        }
                    },
                })
            })
            .collect::<Result<_, ValidationError>>()?,
        take_all: types::RecoveryTakeAllControlView {
            permission: permission_from_wire(required(take.permission.as_ref())?)?,
            selection: recovery_context_selection_from_wire(required(take.selection.as_ref())?)?,
            plan: take
                .plan
                .as_ref()
                .map(|plan| {
                    if plan.transfers.len() > types::MAX_RECOVERY_SELECTION_ENTRIES {
                        return Err(malformed());
                    }
                    Ok(types::RecoveryExecutionView {
                        total_fee: plan.total_fee.clone(),
                        transfers: plan
                            .transfers
                            .iter()
                            .map(|item| {
                                Ok(types::RecoveryPlannedItemView {
                                    death: types::DeathId::new(&item.death)
                                        .map_err(|_| malformed())?,
                                    id: types::RecoveryItemId::new(&item.id)
                                        .map_err(|_| malformed())?,
                                    quantity: types::Quantity::new(item.quantity)
                                        .map_err(|_| malformed())?,
                                    fee: item.fee.clone(),
                                })
                            })
                            .collect::<Result<_, ValidationError>>()?,
                        partial: plan.partial,
                    })
                })
                .transpose()?,
        },
    };
    result.validate_shape().map_err(|_| malformed())?;
    Ok(result)
}

//! Typed transport fixtures, not source gameplay or balances.
use clubscape_game_types::*;

pub fn allowed() -> UiPermission {
    UiPermission {
        allowed: true,
        code: None,
        reason: None,
    }
}

pub fn denied() -> UiPermission {
    UiPermission {
        allowed: false,
        code: Some(GameErrorCode::Unavailable),
        reason: Some("This source Office has no Bank-All control.".into()),
    }
}

pub fn context(unit_fee: u64) -> RecoveryContextView {
    let identity = RecoveryContextIdentity::DeathOffice {
        interface: InterfaceId::new("interface.fixture.office").unwrap(),
        instance: Some(InstanceId::new("instance.fixture.office").unwrap()),
    };
    let slots: Vec<_> = ["one", "two"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let fee = unit_fee + 2 * index as u64;
            RecoveryContextSlotView {
                slot: index as u32,
                death: DeathId::new(format!("death.fixture.{name}")).unwrap(),
                current_storage: RecoveryStorage::DeathOffice,
                entry: RecoveryEntryControlView {
                    id: RecoveryItemId::new(format!("recovery_item.fixture.{name}")).unwrap(),
                    item: UiItem {
                        item: ItemId::new("item.fixture.arrow").unwrap(),
                        name: "Bronze arrow".into(),
                        quantity: 7,
                        source_id: Some(882),
                        asset: Some(AssetId::new("asset.fixture.arrow").unwrap()),
                        instance_id: None,
                        charges: None,
                    },
                    unit_fee: fee.to_string(),
                    full_stack_fee: (7 * fee).to_string(),
                    inventory_capacity: 7,
                    bank_capacity: 0,
                    take: allowed(),
                    bank: denied(),
                },
                selected_type_caption: RecoveryTypeCaption::Source {
                    source_id: 882,
                    quantity: "14".into(),
                    unit_fee: fee.to_string(),
                    total_fee: (14 * fee).to_string(),
                },
            }
        })
        .collect();
    let records = slots
        .iter()
        .map(|slot| RecoveryContextRecordSelection {
            death: slot.death.clone(),
            entries: vec![RecoveryContextEntrySelection {
                id: slot.entry.id.clone(),
                quantity: Quantity::new(7).unwrap(),
                current_storage: slot.current_storage,
            }],
        })
        .collect();
    let transfers = slots
        .iter()
        .map(|slot| RecoveryPlannedItemView {
            death: slot.death.clone(),
            id: slot.entry.id.clone(),
            quantity: Quantity::new(7).unwrap(),
            fee: slot.entry.full_stack_fee.clone(),
        })
        .collect();
    RecoveryContextView {
        version: RECOVERY_CONTEXT_VERSION,
        identity: identity.clone(),
        counts: RecoveryContextCounts {
            entries: 2,
            native_item_types: Some(1),
            capacity: 120,
            capacity_unit: RecoveryCapacityUnit::ItemTypesOrInstances,
            stored: 1,
            offered: 1,
        },
        slots,
        take_all: RecoveryTakeAllControlView {
            permission: allowed(),
            selection: RecoveryContextSelection {
                context: identity,
                records,
            },
            plan: Some(RecoveryExecutionView {
                total_fee: (14 * unit_fee + 14).to_string(),
                transfers,
                partial: false,
            }),
        },
    }
}

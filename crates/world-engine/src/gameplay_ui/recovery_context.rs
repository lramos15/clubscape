use super::projections::permission;
use super::recovery_controls::all_items;
use super::*;
use crate::RecoveryView;
use crate::death::{RecoveryBatch, RecoveryDestination};

fn context_identity(character: &CharacterState) -> GameResult<RecoveryContextIdentity> {
    match &runtime::schedule(character)?.access {
        Some(ContainerSession::Grave { death, interface }) => Ok(RecoveryContextIdentity::Grave {
            interface: interface.clone(),
            death: death.clone(),
        }),
        Some(ContainerSession::DeathOffice { interface }) => {
            Ok(RecoveryContextIdentity::DeathOffice {
                interface: interface.clone(),
                instance: character.runtime.instance.clone(),
            })
        }
        _ => Err(GameError::new(
            GameErrorCode::NotOwned,
            "An actual recovery interface is required.",
        )),
    }
}

fn selection(
    context: RecoveryContextIdentity,
    views: &[RecoveryView],
) -> GameResult<RecoveryContextSelection> {
    let selection = RecoveryContextSelection {
        context,
        records: views
            .iter()
            .filter(|view| !view.entries.is_empty())
            .map(|view| RecoveryContextRecordSelection {
                death: view.death.clone(),
                entries: view
                    .entries
                    .iter()
                    .map(|entry| RecoveryContextEntrySelection {
                        id: entry.id.clone(),
                        quantity: entry.stack.quantity,
                        current_storage: entry.current_storage,
                    })
                    .collect(),
            })
            .collect(),
    };
    selection.validate_shape()?;
    Ok(selection)
}

impl WorldEngine {
    fn take_all_batches(
        &self,
        world: &WorldState,
        character: &CharacterState,
        selected: &RecoveryContextSelection,
    ) -> GameResult<Vec<RecoveryBatch>> {
        selected.validate_shape()?;
        self.input_permission(character)?;
        self.require_ui_free(character)?;
        self.authorize(character, &["reclaim".into()])?;
        let identity = context_identity(character)?;
        if identity != selected.context {
            return Err(GameError::new(
                GameErrorCode::StaleCommand,
                "Take-All must echo the current recovery context.",
            ));
        }
        for selected in &selected.records {
            if world
                .runtime
                .deaths
                .get(&selected.death)
                .is_some_and(|record| record.owner != character.actor_id)
            {
                return Err(GameError::new(
                    GameErrorCode::NotOwned,
                    "Recovery belongs to another actor.",
                ));
            }
        }
        let views = self.recovery_context_panels(world, character)?;
        if *selected != selection(identity, &views)? {
            return Err(GameError::new(
                GameErrorCode::StaleCommand,
                "Take-All must echo every current owned entry, quantity and storage in order.",
            ));
        }
        if selected.records.is_empty() {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "There are no recovery items to take.",
            ));
        }
        views
            .iter()
            .filter(|view| !view.entries.is_empty())
            .map(|view| {
                self.recovery_ui_permission(world, character, &view.death, view.storage)?;
                Ok(RecoveryBatch {
                    death: view.death.clone(),
                    storage: view.storage,
                    items: all_items(view),
                })
            })
            .collect()
    }

    pub(super) fn take_all_recovery_ui(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        selected: &RecoveryContextSelection,
    ) -> GameResult<Vec<GameEvent>> {
        let batches = self.take_all_batches(world, character, selected)?;
        let plan =
            self.plan_recovery(world, character, &batches, RecoveryDestination::Inventory)?;
        Ok(plan.install(world, character))
    }

    pub(super) fn current_recovery_context(
        &self,
        world: &WorldState,
        character: &CharacterState,
        views: &[RecoveryView],
        panels: &[RecoveryPanelControlView],
    ) -> GameResult<RecoveryContextView> {
        let identity = context_identity(character)?;
        let selected = selection(identity.clone(), views)?;
        let storage = match identity {
            RecoveryContextIdentity::Grave { .. } => RecoveryStorage::Grave,
            RecoveryContextIdentity::DeathOffice { .. } => RecoveryStorage::DeathOffice,
        };
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unavailable("Recovery policy is not bound."))?;
        let mut slots = Vec::new();
        let mut stored_keys = BTreeSet::new();
        let mut offered_keys = BTreeSet::new();
        let mut stored_entries = 0_u32;
        let mut type_quantities = BTreeMap::<u32, u64>::new();
        let mut source_complete = true;
        if views.len() != panels.len() {
            return Err(invalid_state("Recovery context lost its panel mapping."));
        }
        for (view, panel) in views.iter().zip(panels) {
            if view.death != panel.death
                || view.storage != storage
                || panel.storage != storage
                || view.entries.len() != panel.entries.len()
            {
                return Err(invalid_state(
                    "Recovery context does not match its owned panels.",
                ));
            }
            for (item, entry) in view.entries.iter().zip(&panel.entries) {
                if item.id != entry.id {
                    return Err(invalid_state("Recovery slot lost its owned entry."));
                }
                offered_keys.insert(recovery_slot_key(&item.stack));
                if item.current_storage == storage {
                    stored_entries = stored_entries
                        .checked_add(1)
                        .ok_or_else(|| invalid_state("Recovery storage count overflow."))?;
                    stored_keys.insert(recovery_slot_key(&item.stack));
                }
                if let Some(id) = entry.item.source_id {
                    let total = type_quantities.entry(id).or_default();
                    *total = total
                        .checked_add(u64::from(item.stack.quantity.get()))
                        .ok_or_else(|| invalid_state("Recovery source-type quantity overflow."))?;
                } else {
                    source_complete = false;
                }
                slots.push(RecoveryContextSlotView {
                    slot: u32::try_from(slots.len())
                        .map_err(|_| invalid_state("Recovery context slot count overflow."))?,
                    death: view.death.clone(),
                    current_storage: item.current_storage,
                    entry: entry.clone(),
                    selected_type_caption: RecoveryTypeCaption::Unavailable {
                        reason: "Complete original recovery item identities are not bound.".into(),
                    },
                });
            }
        }
        if source_complete {
            for slot in &mut slots {
                let source_id =
                    slot.entry.item.source_id.ok_or_else(|| {
                        invalid_state("Recovery source item identity disappeared.")
                    })?;
                let quantity = type_quantities[&source_id];
                let fee =
                    slot.entry.unit_fee.parse::<u64>().map_err(|_| {
                        invalid_state("Recovery source unit fee is not an integer.")
                    })?;
                let total = quantity
                    .checked_mul(fee)
                    .ok_or_else(|| invalid_state("Recovery source display fee overflow."))?;
                slot.selected_type_caption = RecoveryTypeCaption::Source {
                    source_id,
                    quantity: quantity.to_string(),
                    unit_fee: fee.to_string(),
                    total_fee: total.to_string(),
                };
            }
        }
        let count = |value: usize| {
            u32::try_from(value).map_err(|_| invalid_state("Recovery context count overflow."))
        };
        let counts = RecoveryContextCounts {
            entries: count(slots.len())?,
            native_item_types: source_complete
                .then(|| count(type_quantities.len()))
                .transpose()?,
            capacity: match storage {
                RecoveryStorage::Grave => policy.grave_capacity,
                RecoveryStorage::DeathOffice => policy.office_capacity,
            },
            capacity_unit: match storage {
                RecoveryStorage::Grave => RecoveryCapacityUnit::Entries,
                RecoveryStorage::DeathOffice => RecoveryCapacityUnit::ItemTypesOrInstances,
            },
            stored: match storage {
                RecoveryStorage::Grave => stored_entries,
                RecoveryStorage::DeathOffice => count(stored_keys.len())?,
            },
            offered: match storage {
                RecoveryStorage::Grave => count(slots.len())?,
                RecoveryStorage::DeathOffice => count(offered_keys.len())?,
            },
        };
        let planned = self
            .take_all_batches(world, character, &selected)
            .and_then(|batches| {
                self.plan_recovery(world, character, &batches, RecoveryDestination::Inventory)
            });
        let (take_permission, plan) = match planned {
            Ok(plan) => (permission(Ok(()))?, Some(plan.preview()?)),
            Err(error) => (permission(Err(error))?, None),
        };
        Ok(RecoveryContextView {
            version: RECOVERY_CONTEXT_VERSION,
            identity,
            counts,
            slots,
            take_all: RecoveryTakeAllControlView {
                permission: take_permission,
                selection: selected,
                plan,
            },
        })
    }
}

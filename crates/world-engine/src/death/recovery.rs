use super::*;
use crate::commerce::{TransferPlan, capacity_error, plan_transfer_up_to};

pub(crate) struct RecoveryBatch {
    pub death: DeathId,
    pub storage: RecoveryStorage,
    pub items: Vec<RecoveryItemAmount>,
}

#[derive(Clone, Copy)]
pub(crate) enum RecoveryDestination {
    Inventory,
    Bank,
}

pub(crate) struct RecoveryTransfer<'a> {
    pub item: &'a RecoveryItem,
    pub storage: RecoveryStorage,
    pub amount: u32,
    pub fee_limit: Option<u64>,
    pub destination: RecoveryDestination,
}

pub(crate) struct RecoveryTransferValue {
    pub fee: u64,
    pub stack: ItemStack,
}

pub(crate) struct RecoveryPlan {
    pub character: CharacterState,
    pub records: BTreeMap<DeathId, DeathRecord>,
    pub events: Vec<GameEvent>,
}

impl RecoveryPlan {
    pub fn install(self, world: &mut WorldState, character: &mut CharacterState) -> Vec<GameEvent> {
        *character = self.character;
        world.runtime.deaths.extend(self.records);
        self.events
    }
}

impl WorldEngine {
    /// Only actor-owned records and bounded container candidates are copied, never a speculative world.
    pub(crate) fn prepare_recovery_records(
        &self,
        world: &WorldState,
        character: &CharacterState,
        batches: &[RecoveryBatch],
    ) -> GameResult<BTreeMap<DeathId, DeathRecord>> {
        if batches.is_empty()
            || batches.len() > 4096
            || batches
                .iter()
                .map(|batch| &batch.death)
                .collect::<BTreeSet<_>>()
                .len()
                != batches.len()
            || batches.iter().map(|batch| batch.items.len()).sum::<usize>() > 4096
        {
            return Err(invalid_state("Select distinct bounded recovery records."));
        }
        for batch in batches {
            if batch.items.is_empty()
                || batch
                    .items
                    .iter()
                    .map(|item| &item.id)
                    .collect::<BTreeSet<_>>()
                    .len()
                    != batch.items.len()
            {
                return Err(invalid_state("Select distinct recovery entries."));
            }
            let record =
                world.runtime.deaths.get(&batch.death).ok_or_else(|| {
                    GameError::new(GameErrorCode::NotOwned, "Unknown death record.")
                })?;
            if record.owner != character.actor_id {
                return Err(GameError::new(
                    GameErrorCode::NotOwned,
                    "Recovery belongs to another actor.",
                ));
            }
            match batch.storage {
                RecoveryStorage::Grave => self.grave_access(world, character, &batch.death)?,
                RecoveryStorage::DeathOffice => self.office_access(world, character)?,
            }
            for selected in &batch.items {
                if record.reclaimed.contains(&selected.id)
                    || !record
                        .grave
                        .iter()
                        .flat_map(|grave| &grave.items)
                        .chain(if batch.storage == RecoveryStorage::DeathOffice {
                            record.office.iter()
                        } else {
                            [].iter()
                        })
                        .any(|item| item.id == selected.id)
                {
                    return Err(GameError::new(
                        GameErrorCode::NotOwned,
                        "Entry is not in this recovery storage.",
                    ));
                }
            }
        }
        let mut records: BTreeMap<_, _> = world
            .runtime
            .deaths
            .iter()
            .filter(|(_, record)| record.owner == character.actor_id)
            .map(|(id, record)| (id.clone(), record.clone()))
            .collect();
        if records.len() > 4096 {
            return Err(invalid_state("Owned recovery history exceeds its bound."));
        }
        for batch in batches {
            if batch.storage == RecoveryStorage::DeathOffice {
                let record = records
                    .get_mut(&batch.death)
                    .ok_or_else(|| invalid_state("Missing recovery record."))?;
                if let Some(grave) = record.grave.take() {
                    record.office.extend(grave.items);
                    self.enforce_office_records(&mut records, &character.actor_id)?;
                }
            }
        }
        Ok(records)
    }

    pub(crate) fn recovery_transfer_plan(
        &self,
        character: &CharacterState,
        transfer: RecoveryTransfer<'_>,
    ) -> GameResult<TransferPlan<RecoveryTransferValue>> {
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unavailable("Recovery policy is not bound."))?;
        let rule = match transfer.storage {
            RecoveryStorage::Grave => policy.grave_fee.require()?,
            RecoveryStorage::DeathOffice => policy.office_fee.require()?,
        };
        let requested = transfer.amount.min(transfer.item.stack.quantity.get());
        plan_transfer_up_to(
            character,
            Quantity::new(requested)?,
            |candidate, quantity| {
                let item = RecoveryItem {
                    stack: ItemStack {
                        quantity,
                        ..transfer.item.stack.clone()
                    },
                    ..transfer.item.clone()
                };
                let mut fee = recovery_fee(rule, &item, quantity.get())?;
                if let Some(limit) = transfer.fee_limit {
                    fee = fee.min(limit);
                }
                let stack = match transfer.destination {
                    RecoveryDestination::Inventory => {
                        let auto_equip =
                            candidate.runtime.settings.death_auto_equip.ok_or_else(|| {
                                unavailable("Source death auto-equip setting must be bound.")
                            })?;
                        self.restore_recovery_item(candidate, &item, auto_equip)?;
                        item.stack
                    }
                    RecoveryDestination::Bank => {
                        bank::receive_owned_stack(candidate, &self.content, &item.stack)?
                    }
                };
                self.pay_fee(candidate, policy, fee)?;
                Ok(RecoveryTransferValue { fee, stack })
            },
        )
    }

    pub(crate) fn plan_recovery(
        &self,
        world: &WorldState,
        character: &CharacterState,
        batches: &[RecoveryBatch],
        destination: RecoveryDestination,
    ) -> GameResult<RecoveryPlan> {
        let mut records = self.prepare_recovery_records(world, character, batches)?;
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unavailable("Recovery policy is not bound."))?;
        let mut candidate = character.clone();
        let mut events = Vec::new();
        let mut prior_error = None;
        for batch in batches {
            let rule = match batch.storage {
                RecoveryStorage::Grave => policy.grave_fee.require()?,
                RecoveryStorage::DeathOffice => policy.office_fee.require()?,
            };
            let mut fee = 0_u64;
            let mut recovered = Vec::new();
            let mut stacks = Vec::new();
            for selected in &batch.items {
                let record = records
                    .get(&batch.death)
                    .ok_or_else(|| invalid_state("Missing recovery record."))?;
                let item = recovery_entries(record, batch.storage)?
                    .iter()
                    .find(|item| item.id == selected.id)
                    .cloned()
                    .ok_or_else(|| {
                        GameError::new(
                            GameErrorCode::NotOwned,
                            "Recovery entry no longer survives in this storage.",
                        )
                    })?;
                let owned = item.stack.quantity.get();
                let requested = match selected.amount {
                    UiAmount::Quantity { quantity } => quantity.get().min(owned),
                    UiAmount::All {} => owned,
                };
                let plan = match self.recovery_transfer_plan(
                    &candidate,
                    RecoveryTransfer {
                        item: &item,
                        storage: batch.storage,
                        amount: requested,
                        destination,
                        fee_limit: match rule {
                            RecoveryFee::Bands { maximum_total, .. } => {
                                Some(maximum_total.saturating_sub(fee))
                            }
                            _ => None,
                        },
                    },
                ) {
                    Ok(plan) => plan,
                    Err(error) if capacity_error(&error.code) => {
                        prior_error.get_or_insert(error);
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                candidate = plan.character;
                if let Some(error) = plan.partial_reason {
                    prior_error.get_or_insert(error);
                }
                fee = fee
                    .checked_add(plan.value.fee)
                    .ok_or_else(|| invalid_state("Recovery fee overflow."))?;
                let record = records
                    .get_mut(&batch.death)
                    .ok_or_else(|| invalid_state("Missing recovery record."))?;
                let entries = recovery_entries_mut(record, batch.storage)?;
                if plan.quantity == owned {
                    entries.retain(|entry| entry.id != selected.id);
                    record.reclaimed.insert(selected.id.clone());
                } else {
                    let remaining = entries
                        .iter_mut()
                        .find(|entry| entry.id == selected.id)
                        .ok_or_else(|| invalid_state("Recovery remainder disappeared."))?;
                    remaining.stack.quantity = Quantity::new(owned - plan.quantity)?;
                    let applied_credit =
                        recovery_fee_value(rule, item.effective_unit_value, 0, plan.quantity)?;
                    remaining.fee_paid = remaining.fee_paid.saturating_sub(applied_credit);
                }
                recovered.push(selected.id.clone());
                stacks.push(plan.value.stack);
            }
            if !recovered.is_empty() {
                events.extend([
                    GameEvent::Recovered,
                    GameEvent::RecoveryCompleted {
                        death: batch.death.clone(),
                        storage: batch.storage,
                        items: recovered,
                        fee,
                    },
                    GameEvent::ItemTransferred {
                        from: match batch.storage {
                            RecoveryStorage::Grave => ContainerKind::Grave,
                            RecoveryStorage::DeathOffice => ContainerKind::DeathOffice,
                        },
                        to: match destination {
                            RecoveryDestination::Inventory => ContainerKind::Inventory,
                            RecoveryDestination::Bank => ContainerKind::Bank,
                        },
                        items: stacks,
                    },
                ]);
            }
        }
        if events.is_empty() {
            return Err(prior_error.unwrap_or_else(|| {
                GameError::new(GameErrorCode::NotOwned, "Nothing can be reclaimed.")
            }));
        }
        if let Some(error) = prior_error {
            events.push(GameEvent::Message {
                text: format!("Some recovery items remain: {}", error.message),
            });
        }
        Ok(RecoveryPlan {
            character: candidate,
            records,
            events,
        })
    }
}

fn recovery_entries(record: &DeathRecord, storage: RecoveryStorage) -> GameResult<&[RecoveryItem]> {
    match storage {
        RecoveryStorage::Grave => record
            .grave
            .as_ref()
            .map(|grave| grave.items.as_slice())
            .ok_or_else(|| invalid_state("Missing grave.")),
        RecoveryStorage::DeathOffice => Ok(&record.office),
    }
}

fn recovery_entries_mut(
    record: &mut DeathRecord,
    storage: RecoveryStorage,
) -> GameResult<&mut Vec<RecoveryItem>> {
    match storage {
        RecoveryStorage::Grave => record
            .grave
            .as_mut()
            .map(|grave| &mut grave.items)
            .ok_or_else(|| invalid_state("Missing grave.")),
        RecoveryStorage::DeathOffice => Ok(&mut record.office),
    }
}

use super::projections::permission;
use super::*;
use crate::RecoveryView;
use crate::death::{RecoveryBatch, RecoveryDestination, RecoveryTransfer, recovery_fee};

fn all_items(view: &RecoveryView) -> Vec<RecoveryItemAmount> {
    view.entries
        .iter()
        .map(|entry| RecoveryItemAmount {
            id: entry.id.clone(),
            amount: UiAmount::All {},
        })
        .collect()
}

impl WorldEngine {
    fn recovery_ui_permission(
        &self,
        world: &WorldState,
        character: &CharacterState,
        death: &DeathId,
        storage: RecoveryStorage,
    ) -> GameResult<()> {
        self.input_permission(character)?;
        self.require_ui_free(character)?;
        self.authorize(character, &["reclaim".into()])?;
        self.require_recovery_context(character, death, storage)?;
        match storage {
            RecoveryStorage::Grave => self.grave_access(world, character, death),
            RecoveryStorage::DeathOffice => self.office_access(world, character),
        }
    }

    fn recovery_bank_permission(
        &self,
        world: &WorldState,
        character: &CharacterState,
        storage: RecoveryStorage,
    ) -> GameResult<()> {
        let definition = self
            .content
            .ui
            .as_ref()
            .and_then(|ui| ui.recovery.as_ref())
            .ok_or_else(|| {
                unavailable("This source profile has no declared recovery banking permission.")
            })?;
        let rule = match storage {
            RecoveryStorage::Grave => &definition.grave_bank,
            RecoveryStorage::DeathOffice => &definition.office_bank,
        };
        match rule {
            RecoveryBankRule::Allowed { guard, .. } => self.require_guard(world, character, guard),
            RecoveryBankRule::Unavailable { reason, .. } => Err(unavailable(reason.clone())),
        }
    }

    pub(super) fn take_recovery_ui(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        death: &DeathId,
        storage: RecoveryStorage,
        items: &[RecoveryItemAmount],
    ) -> GameResult<Vec<GameEvent>> {
        self.recovery_ui_permission(world, character, death, storage)?;
        let batches = [RecoveryBatch {
            death: death.clone(),
            storage,
            items: items.to_vec(),
        }];
        let plan =
            self.plan_recovery(world, character, &batches, RecoveryDestination::Inventory)?;
        Ok(plan.install(world, character))
    }

    fn recovery_context_panels(
        &self,
        world: &WorldState,
        character: &CharacterState,
    ) -> GameResult<Vec<RecoveryView>> {
        match &runtime::schedule(character)?.access {
            Some(ContainerSession::Grave { death, .. }) => Ok(vec![self.recovery_view_for(
                world,
                character,
                death,
                RecoveryStorage::Grave,
            )?]),
            Some(ContainerSession::DeathOffice { .. }) => world
                .runtime
                .deaths
                .iter()
                .filter(|(_, record)| record.owner == character.actor_id)
                .map(|(death, _)| {
                    self.recovery_view_for(world, character, death, RecoveryStorage::DeathOffice)
                })
                .collect(),
            _ => Err(GameError::new(
                GameErrorCode::NotOwned,
                "An actual recovery interface is required.",
            )),
        }
    }

    fn bank_all_batches(
        &self,
        world: &WorldState,
        character: &CharacterState,
        records: &[RecoveryRecordSelection],
    ) -> GameResult<Vec<RecoveryBatch>> {
        let panels = self.recovery_context_panels(world, character)?;
        let expected: Vec<_> = panels
            .iter()
            .filter(|panel| !panel.entries.is_empty())
            .map(|panel| RecoveryRecordSelection {
                death: panel.death.clone(),
                items: panel.entries.iter().map(|entry| entry.id.clone()).collect(),
            })
            .collect();
        if records != expected {
            return Err(GameError::new(
                GameErrorCode::StaleCommand,
                "Bank-All must echo the exact current owned recovery record and entry identities.",
            ));
        }
        if records.is_empty() {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "There are no recovery items to bank.",
            ));
        }
        panels
            .iter()
            .filter(|panel| !panel.entries.is_empty())
            .map(|panel| {
                self.recovery_ui_permission(world, character, &panel.death, panel.storage)?;
                self.recovery_bank_permission(world, character, panel.storage)?;
                Ok(RecoveryBatch {
                    death: panel.death.clone(),
                    storage: panel.storage,
                    items: all_items(panel),
                })
            })
            .collect()
    }

    pub(super) fn bank_all_recovery_ui(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        records: &[RecoveryRecordSelection],
    ) -> GameResult<Vec<GameEvent>> {
        let batches = self.bank_all_batches(world, character, records)?;
        let plan = self.plan_recovery(world, character, &batches, RecoveryDestination::Bank)?;
        Ok(plan.install(world, character))
    }

    pub(super) fn recovery_management_view(
        &self,
        world: &WorldState,
        character: &CharacterState,
        views: &[RecoveryView],
    ) -> GameResult<RecoveryManagementView> {
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unavailable("Recovery policy is not bound."))?;
        let mut panels = Vec::new();
        for view in views {
            if view.entries.is_empty() {
                panels.push(RecoveryPanelControlView {
                    death: view.death.clone(),
                    storage: view.storage,
                    entries: Vec::new(),
                    full_selection_fee: "0".into(),
                    take_all: permission(Err(GameError::new(
                        GameErrorCode::NotOwned,
                        "There are no recovery items to take.",
                    )))?,
                });
                continue;
            }
            let rule = match view.storage {
                RecoveryStorage::Grave => policy.grave_fee.require()?,
                RecoveryStorage::DeathOffice => policy.office_fee.require()?,
            };
            let batches = [RecoveryBatch {
                death: view.death.clone(),
                storage: view.storage,
                items: all_items(view),
            }];
            let prepared = self
                .recovery_ui_permission(world, character, &view.death, view.storage)
                .and_then(|()| self.prepare_recovery_records(world, character, &batches));
            let mut entries = Vec::new();
            let mut full_fee = 0_u64;
            for entry in &view.entries {
                let record = world
                    .runtime
                    .deaths
                    .get(&view.death)
                    .ok_or_else(|| invalid_state("Recovery projection lost its record."))?;
                let item = record
                    .grave
                    .iter()
                    .flat_map(|grave| &grave.items)
                    .chain(&record.office)
                    .find(|item| item.id == entry.id)
                    .ok_or_else(|| invalid_state("Recovery projection lost its entry."))?;
                let full_stack_fee = recovery_fee(rule, item, item.stack.quantity.get())?;
                full_fee = full_fee
                    .checked_add(full_stack_fee)
                    .ok_or_else(|| invalid_state("Recovery quote overflow."))?;
                let transfer = |destination| {
                    let records = prepared.as_ref().map_err(Clone::clone)?;
                    let record = records
                        .get(&view.death)
                        .ok_or_else(|| invalid_state("Prepared recovery record disappeared."))?;
                    let survives = record
                        .grave
                        .iter()
                        .flat_map(|grave| &grave.items)
                        .chain(&record.office)
                        .any(|candidate| candidate.id == entry.id);
                    if !survives {
                        return Err(GameError::new(
                            GameErrorCode::NotOwned,
                            "This entry does not survive the declared source Office overflow policy.",
                        ));
                    }
                    self.recovery_transfer_plan(
                        character,
                        RecoveryTransfer {
                            item,
                            storage: view.storage,
                            amount: item.stack.quantity.get(),
                            fee_limit: None,
                            destination,
                        },
                    )
                };
                let take = transfer(RecoveryDestination::Inventory);
                let bank = self
                    .recovery_bank_permission(world, character, view.storage)
                    .and_then(|()| transfer(RecoveryDestination::Bank));
                let inventory_capacity = take.as_ref().map_or(0, |plan| plan.quantity);
                let bank_capacity = bank.as_ref().map_or(0, |plan| plan.quantity);
                entries.push(RecoveryEntryControlView {
                    id: item.id.clone(),
                    item: self.ui_item(&item.stack)?,
                    unit_fee: recovery_fee(rule, item, 1)?.to_string(),
                    full_stack_fee: full_stack_fee.to_string(),
                    inventory_capacity,
                    bank_capacity,
                    take: permission(take.map(|_| ()))?,
                    bank: permission(bank.map(|_| ()))?,
                });
            }
            if let RecoveryFee::Bands { maximum_total, .. } = rule {
                full_fee = full_fee.min(*maximum_total);
            }
            let take_all = self
                .recovery_ui_permission(world, character, &view.death, view.storage)
                .and_then(|()| {
                    self.plan_recovery(world, character, &batches, RecoveryDestination::Inventory)
                        .map(|_| ())
                });
            panels.push(RecoveryPanelControlView {
                death: view.death.clone(),
                storage: view.storage,
                entries,
                full_selection_fee: full_fee.to_string(),
                take_all: permission(take_all)?,
            });
        }
        let bank_all_records: Vec<_> = views
            .iter()
            .filter(|view| !view.entries.is_empty())
            .map(|view| RecoveryRecordSelection {
                death: view.death.clone(),
                items: view.entries.iter().map(|entry| entry.id.clone()).collect(),
            })
            .collect();
        let bank_all = self
            .bank_all_batches(world, character, &bank_all_records)
            .and_then(|batches| {
                self.plan_recovery(world, character, &batches, RecoveryDestination::Bank)
                    .map(|_| ())
            });
        Ok(RecoveryManagementView {
            bank_revision: character
                .runtime
                .ui
                .as_ref()
                .ok_or_else(|| invalid_state("Recovery banking lost its persisted bank metadata."))?
                .bank
                .revision
                .to_string(),
            panels,
            bank_all: permission(bank_all)?,
            bank_all_records,
        })
    }
}

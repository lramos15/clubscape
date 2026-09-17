use std::collections::{BTreeMap, BTreeSet};

use clubscape_game_types::*;
use clubscape_simulation::{bank, equipment, inventory};

use crate::{WorldEngine, invalid_content, invalid_state, unknown};

impl WorldEngine {
    pub(crate) fn grant(
        &self,
        character: &mut CharacterState,
        id: &GrantId,
    ) -> GameResult<Vec<GameEvent>> {
        let definition = self
            .content
            .mechanics
            .grants
            .get(id)
            .ok_or_else(|| unknown("Unknown grant."))?;
        let mut delivered = BTreeMap::new();
        let mut satisfied = BTreeSet::new();
        if let Some(id) = &definition.entitlement {
            let purpose = &self
                .content
                .mechanics
                .entitlements
                .get(id)
                .ok_or_else(|| unknown("Unknown grant entitlement."))?
                .purpose;
            if !matches!(purpose, EntitlementPurpose::Grant { grant } if grant == &definition.id) {
                return Err(invalid_content("Grant entitlement purpose mismatch."));
            }
            match character.runtime.entitlements.get(id) {
                Some(EntitlementState::Grant { complete: true, .. }) => return Ok(vec![]),
                Some(EntitlementState::Grant {
                    delivered: prior,
                    satisfied: prior_satisfied,
                    ..
                }) => {
                    delivered = prior.clone();
                    satisfied = prior_satisfied.clone();
                }
                Some(_) => return Err(invalid_state("Grant ledger has the wrong kind.")),
                None => {}
            }
        }
        let mut events = Vec::new();
        for line in &definition.lines {
            if satisfied.contains(&line.item) {
                continue;
            }
            let already_delivered = delivered.get(&line.item).copied().unwrap_or(0);
            let owned = self.owned_count(character, &line.item, &line.ownership)?;
            let unclaimed = line
                .quantity
                .get()
                .checked_sub(already_delivered)
                .ok_or_else(|| invalid_state("Grant ledger exceeds source entitlement."))?;
            let requested = match line.mode {
                GrantMode::Add => unclaimed,
                GrantMode::MissingOnly if owned > 0 => 0,
                GrantMode::MissingOnly => unclaimed,
                GrantMode::TopUp => line.quantity.get().saturating_sub(owned).min(unclaimed),
            };
            if requested == 0 {
                satisfied.insert(line.item.clone());
                continue;
            }
            let mut trial = character.clone();
            let result = self.grant_amount(&mut trial, &definition.target, &line.item, requested);
            let (amount, accepted) = match result {
                Ok(()) => (requested, trial),
                Err(error)
                    if definition.capacity == CapacityPolicy::OrderedPartial
                        && matches!(
                            error.code,
                            GameErrorCode::InventoryFull | GameErrorCode::StackOverflow
                        ) =>
                {
                    let mut best = (0, character.clone());
                    let (mut low, mut high) = (1, requested - 1);
                    while low <= high {
                        let mid = low + (high - low) / 2;
                        let mut trial = character.clone();
                        match self.grant_amount(&mut trial, &definition.target, &line.item, mid) {
                            Ok(()) => {
                                best = (mid, trial);
                                low = mid + 1;
                            }
                            Err(error)
                                if matches!(
                                    error.code,
                                    GameErrorCode::InventoryFull | GameErrorCode::StackOverflow
                                ) =>
                            {
                                high = mid - 1
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    best
                }
                Err(error) => return Err(error),
            };
            *character = accepted;
            if amount > 0 {
                delivered.insert(line.item.clone(), already_delivered + amount);
                events.push(GameEvent::Message {
                    text: format!("Received {amount} {}.", line.item),
                });
            }
            if amount == requested {
                satisfied.insert(line.item.clone());
            } else {
                events.push(GameEvent::Message {
                    text: "Make room to receive the rest of this supply.".into(),
                });
                break;
            }
        }
        if let Some(entitlement) = &definition.entitlement {
            character.runtime.entitlements.insert(
                entitlement.clone(),
                EntitlementState::Grant {
                    complete: satisfied.len() == definition.lines.len(),
                    delivered,
                    satisfied,
                },
            );
        }
        Ok(events)
    }

    fn grant_amount(
        &self,
        character: &mut CharacterState,
        container: &ContainerKind,
        item: &ItemId,
        amount: u32,
    ) -> GameResult<()> {
        match container {
            ContainerKind::Inventory => inventory::add(
                &mut character.inventory,
                &self.content.items,
                &ItemStack {
                    item: item.clone(),
                    quantity: Quantity::new(amount)?,
                    instance: None,
                },
            ),
            ContainerKind::Bank => {
                let stackable = self
                    .content
                    .items
                    .get(item)
                    .ok_or_else(|| unknown("Unknown bank-grant item."))?
                    .stackable
                    .fixed()?;
                if !stackable && amount > 28 * 2048 {
                    return Err(crate::unavailable(
                        "Bank nonstackable grant exceeds bounded primitive-transfer work.",
                    ));
                }
                let mut staging = character.clone();
                staging.inventory = Inventory::default();
                let mut remaining = amount;
                while remaining > 0 {
                    let chunk = if stackable {
                        remaining
                    } else {
                        remaining.min(28)
                    };
                    inventory::add(
                        &mut staging.inventory,
                        &self.content.items,
                        &ItemStack {
                            item: item.clone(),
                            quantity: Quantity::new(chunk)?,
                            instance: None,
                        },
                    )?;
                    bank::deposit(&mut staging, &self.content, 0, Quantity::new(chunk)?)?;
                    remaining -= chunk;
                }
                character.bank = staging.bank;
                bank::reconcile_ui(character, true)?;
                Ok(())
            }
            _ => Err(invalid_content(
                "Source grants require an inventory or bank target.",
            )),
        }
    }

    pub(crate) fn reconcile(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        id: &ReconciliationId,
    ) -> GameResult<()> {
        let definition = self
            .content
            .mechanics
            .reconciliations
            .get(id)
            .ok_or_else(|| unknown("Unknown reconciliation."))?;
        if character
            .runtime
            .entitlements
            .contains_key(&definition.entitlement)
        {
            return Ok(());
        }
        if character.runtime.pending_travel.is_none() {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "Reconciliation is only permitted during completed source transport.",
            ));
        }
        for policy in definition.policies.require()? {
            match policy {
                ContainerReconciliation::Preserve => {}
                ContainerReconciliation::ReplaceInventory { inventory } => {
                    character.inventory = *inventory.clone()
                }
                ContainerReconciliation::ReplaceEquipment { equipment } => {
                    character.equipment = equipment.clone()
                }
                ContainerReconciliation::ReplaceBank { slots } => {
                    character.bank.slots = slots.clone();
                    bank::reconcile_ui(character, true)?;
                }
                ContainerReconciliation::RemoveItems { container, items } => match container {
                    ContainerKind::Inventory => {
                        for item in items {
                            let count =
                                inventory::count(&character.inventory, &self.content.items, item)?;
                            if count > 0 {
                                inventory::remove(
                                    &mut character.inventory,
                                    &self.content.items,
                                    &ItemStack {
                                        item: item.clone(),
                                        quantity: Quantity::new(count)?,
                                        instance: None,
                                    },
                                )?;
                            }
                        }
                    }
                    ContainerKind::Equipment => character
                        .equipment
                        .retain(|_, stack| !items.contains(&stack.item)),
                    ContainerKind::Bank => {
                        for slot in &mut character.bank.slots {
                            if slot
                                .as_ref()
                                .is_some_and(|stack| items.contains(&stack.item))
                            {
                                *slot = None;
                            }
                        }
                    }
                    _ => {
                        return Err(invalid_content(
                            "Reconciliation cannot remove world/recovery items.",
                        ));
                    }
                },
            }
        }
        inventory::validate(&character.inventory, &self.content.items)?;
        equipment::validate(&character.equipment, &self.content)?;
        self.refresh_combat_style(character)?;
        bank::validate(&character.bank, &self.content.items)?;
        character.runtime.entitlements.insert(
            definition.entitlement.clone(),
            EntitlementState::Claimed {
                at_tick: world.tick,
            },
        );
        Ok(())
    }
}

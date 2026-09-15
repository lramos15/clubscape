use std::collections::{BTreeMap, BTreeSet};

use clubscape_game_types::*;
use clubscape_simulation::{bank, equipment, inventory};

use crate::{
    ActorEvent, TickContext, WorldEngine, invalid_content, invalid_state, runtime, source_math,
    tag, unavailable, unknown,
};

mod recovery;
pub(crate) use recovery::{RecoveryBatch, RecoveryDestination, RecoveryTransfer};

impl WorldEngine {
    pub(crate) fn advance_death(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
    ) -> GameResult<Vec<GameEvent>> {
        if let LifeState::Dying { death, at_tick } = character.runtime.life.clone() {
            if world.tick < at_tick {
                return Ok(vec![]);
            }
            let record = world
                .runtime
                .deaths
                .get(&death)
                .ok_or_else(|| invalid_state("Dying state lacks its death receipt."))?;
            let arrival = record.arrival.as_ref().ok_or_else(|| {
                unavailable("Legacy dying state needs explicit source phase migration.")
            })?;
            if record.owner != character.actor_id
                || arrival.dying_until_tick != at_tick
                || arrival.completed_at_tick.is_some()
            {
                return Err(invalid_state(
                    "Dying state disagrees with its persisted source phases.",
                ));
            }
            character.runtime.life = LifeState::Respawning {
                death,
                destination: arrival.destination.clone(),
                at_tick: arrival.arrives_at_tick,
            };
        }
        self.advance_respawn(world, character)
    }

    pub(crate) fn advance_respawn(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
    ) -> GameResult<Vec<GameEvent>> {
        let LifeState::Respawning {
            death,
            destination,
            at_tick,
        } = character.runtime.life.clone()
        else {
            return Err(invalid_state("No pending respawn."));
        };
        if world.tick < at_tick {
            return Ok(vec![]);
        }
        let record = world
            .runtime
            .deaths
            .get(&death)
            .ok_or_else(|| invalid_state("Pending respawn has no death receipt."))?;
        if record.owner != character.actor_id {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Respawn receipt belongs to another actor.",
            ));
        }
        if let Some(arrival) = &record.arrival
            && (arrival.destination != destination
                || arrival.arrives_at_tick != at_tick
                || arrival.completed_at_tick.is_some())
        {
            return Err(invalid_state(
                "Respawn phase was changed or already completed.",
            ));
        }
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unknown("Missing respawn policy."))?;
        let office = if destination != record.respawn {
            let source_office = policy.first_office.require()?;
            source_office.region == destination.region
                && source_office.tile == destination.tile
                && destination
                    .instance
                    .as_ref()
                    .and_then(|id| world.runtime.instances.get(id))
                    .is_some_and(|instance| {
                        source_office.instance.as_ref() == Some(&instance.template)
                            && instance.owner.as_ref() == Some(&character.actor_id)
                    })
        } else {
            false
        };
        if destination != record.respawn && !office {
            return Err(invalid_state(
                "Pending arrival does not match its source death destinations.",
            ));
        }
        self.move_to(world, character, destination.clone())?;
        for (vital, restoration) in &policy.restoration.require()?.on_arrival {
            self.restore_vital(character, *vital, restoration)?;
        }
        if character.hitpoints == 0 {
            return Err(invalid_content(
                "Respawn restoration must restore live hitpoints.",
            ));
        }
        character.runtime.life = if office {
            character.runtime.first_item_loss_seen = true;
            LifeState::FirstDeathOffice {
                death: death.clone(),
                instance: destination
                    .instance
                    .ok_or_else(|| invalid_state("Missing Office instance."))?,
            }
        } else {
            LifeState::Alive
        };
        if !office
            && let Some(grave) = world
                .runtime
                .deaths
                .get_mut(&death)
                .and_then(|record| record.grave.as_mut())
        {
            grave.clock_started = true;
            grave.started_at_tick = Some(world.tick);
        }
        if let Some(arrival) = world
            .runtime
            .deaths
            .get_mut(&death)
            .and_then(|record| record.arrival.as_mut())
        {
            arrival.completed_at_tick = Some(world.tick);
        }
        Ok(vec![GameEvent::Moved {
            tile: character.tile,
        }])
    }

    pub(crate) fn die(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
    ) -> GameResult<Vec<GameEvent>> {
        if character.hitpoints != 0 {
            return Err(invalid_state(
                "Death requires an actually depleted HP pool.",
            ));
        }
        if !matches!(character.runtime.life, LifeState::Alive) {
            return Err(unavailable(
                "Legacy or intermediate life state requires explicit migration/arrival scheduling; death retention cannot be replayed.",
            ));
        }
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unavailable("Source death policy is not bound."))?;
        let timing = policy.timing.require()?;
        let dying_until_tick = runtime::deadline(world.tick, u64::from(timing.dying_ticks))?;
        let arrives_at_tick = runtime::deadline(dying_until_tick, u64::from(timing.respawn_ticks))?;
        if character.runtime.instance.is_some() {
            return Err(unavailable(
                "Normal unsafe non-PvP death does not define arbitrary combat-instance retention.",
            ));
        }
        let provider = self
            .content
            .mechanics
            .value_providers
            .get(&policy.value_provider)
            .ok_or_else(|| unknown("Unknown death value provider."))?;
        let origin = runtime::location(character);
        let id = DeathId::new(format!(
            "death.engine.{}.{}",
            world.tick,
            world.runtime.deaths.len()
        ))?;
        if world.runtime.deaths.contains_key(&id) {
            return Err(invalid_state("Death identity collision."));
        }
        let valued = self.retention_plan(character)?;
        let mut retained = Vec::new();
        let mut lost = Vec::new();
        for RetentionPart {
            order,
            stack,
            layout,
            value,
            kept,
        } in valued
        {
            let lose = stack.quantity.get() - kept;
            if kept > 0 {
                retained.push(RecoveryItem {
                    id: RecoveryItemId::new(format!(
                        "recovery_item.{}.kept.{order}",
                        &id.as_str()[6..]
                    ))?,
                    stack: ItemStack {
                        quantity: Quantity::new(kept)?,
                        ..stack.clone()
                    },
                    layout: layout.clone(),
                    effective_unit_value: value,
                    fee_paid: 0,
                });
            }
            if lose > 0 {
                let losing = ItemStack {
                    quantity: Quantity::new(lose)?,
                    ..stack.clone()
                };
                match &layout {
                    ItemLayout::Inventory { slot } => {
                        inventory::remove_from_slot(
                            &mut character.inventory,
                            &self.content.items,
                            usize::from(*slot),
                            losing.quantity,
                        )?;
                    }
                    ItemLayout::Equipment { slot } => {
                        let equipped = character
                            .equipment
                            .get_mut(slot)
                            .ok_or_else(|| invalid_state("Death equipment disappeared."))?;
                        if kept == 0 {
                            character.equipment.remove(slot);
                        } else {
                            equipped.quantity = Quantity::new(kept)?;
                        }
                    }
                }
                lost.push(RecoveryItem {
                    id: RecoveryItemId::new(format!(
                        "recovery_item.{}.lost.{order}",
                        &id.as_str()[6..]
                    ))?,
                    stack: losing,
                    layout,
                    effective_unit_value: value,
                    fee_paid: 0,
                });
            }
        }
        self.refresh_combat_style(character)?;
        let items_lost = !lost.is_empty();
        let respawn =
            self.resolve_location(world, &character.actor_id, policy.respawn.require()?)?;
        let first_office = items_lost && !character.runtime.first_item_loss_seen;
        let mut grave = if items_lost {
            Some(GraveState {
                location: origin.clone(),
                active_ticks_remaining: policy.grave_active_ticks,
                clock_started: false,
                started_at_tick: None,
                paused: BTreeSet::new(),
                items: lost,
            })
        } else {
            None
        };
        let mut office = Vec::new();
        if let Some(previous_id) = &character.runtime.active_death
            && let Some(previous_grave) = world
                .runtime
                .deaths
                .get_mut(previous_id)
                .and_then(|record| record.grave.take())
        {
            if let Some(new_grave) = &mut grave {
                let repeat = policy.repeat.require()?;
                new_grave.location = if repeat.keep_old_grave_location {
                    previous_grave.location.clone()
                } else {
                    origin.clone()
                };
                if !repeat.refresh_timer_if_contents_change {
                    new_grave.active_ticks_remaining = previous_grave.active_ticks_remaining;
                }
                let mut counts = BTreeMap::<ItemId, u32>::new();
                for mut item in previous_grave.items {
                    if repeat.supply_items.contains(&item.stack.item) {
                        self.put_ground_from(
                            world,
                            item.stack,
                            &character.actor_id,
                            &previous_grave.location,
                            &repeat.supply_ground_policy,
                            GroundProducer::DeathSupply {
                                actor: character.actor_id.clone(),
                                at_tick: world.tick,
                            },
                        )?;
                    } else if repeat.old_items_to_office.contains(&item.stack.item) {
                        office.push(item);
                    } else {
                        let stackable = self
                            .content
                            .items
                            .get(&item.stack.item)
                            .ok_or_else(|| unknown("Unknown old grave item."))?
                            .stackable
                            .fixed()?;
                        let count = counts.entry(item.stack.item.clone()).or_default();
                        if !stackable && *count >= u32::from(repeat.old_unstackable_per_item_limit)
                        {
                            office.push(item);
                        } else if !stackable
                            && item.stack.quantity.get()
                                > u32::from(repeat.old_unstackable_per_item_limit) - *count
                        {
                            let kept = u32::from(repeat.old_unstackable_per_item_limit) - *count;
                            let mut overflow = item.clone();
                            overflow.id = RecoveryItemId::new(format!(
                                "recovery_item.{}.overflow.{}",
                                &id.as_str()[6..],
                                office.len()
                            ))?;
                            overflow.stack.quantity =
                                Quantity::new(item.stack.quantity.get() - kept)?;
                            overflow.fee_paid = 0;
                            item.stack.quantity = Quantity::new(kept)?;
                            new_grave.items.push(item);
                            office.push(overflow);
                            *count += kept;
                        } else {
                            *count += item.stack.quantity.get();
                            new_grave.items.push(item);
                        }
                    }
                }
            } else {
                world
                    .runtime
                    .deaths
                    .get_mut(previous_id)
                    .ok_or_else(|| invalid_state("Previous death disappeared."))?
                    .grave = Some(previous_grave);
            }
        }
        if let Some(grave) = &mut grave {
            let supply = character
                .runtime
                .settings
                .death_supply_piles
                .ok_or_else(|| {
                    unavailable("Death supply-pile setting must be source-bound, not defaulted.")
                })?;
            if supply {
                let repeat = policy.repeat.require()?;
                let mut keep = Vec::new();
                for item in std::mem::take(&mut grave.items) {
                    if repeat.supply_items.contains(&item.stack.item) {
                        self.put_ground_from(
                            world,
                            item.stack,
                            &character.actor_id,
                            &origin,
                            &repeat.supply_ground_policy,
                            GroundProducer::DeathSupply {
                                actor: character.actor_id.clone(),
                                at_tick: world.tick,
                            },
                        )?;
                    } else {
                        keep.push(item);
                    }
                }
                grave.items = keep;
            }
            if grave.items.len() > usize::from(policy.grave_capacity) {
                return Err(GameError::new(
                    GameErrorCode::InventoryFull,
                    "Source grave capacity would be exceeded.",
                ));
            }
            if first_office && policy.grave_pauses.contains(&ClockPause::FirstDeathOffice) {
                grave.paused.insert(ClockPause::FirstDeathOffice);
            }
        }
        world.runtime.deaths.insert(
            id.clone(),
            DeathRecord {
                owner: character.actor_id.clone(),
                occurred_at_tick: world.tick,
                origin,
                respawn: respawn.clone(),
                value_provider: provider.id.clone(),
                value_revision: provider.revision.clone(),
                retained,
                grave,
                office,
                reclaimed: BTreeSet::new(),
                discarded: BTreeSet::new(),
                arrival: None,
            },
        );
        self.enforce_office_capacity(world, &character.actor_id)?;
        character.runtime.previous_respawn = Some(respawn.clone());
        if items_lost {
            character.runtime.active_death = Some(id.clone());
        }
        runtime::interrupt(character)?;
        character.runtime.pending_travel = None;
        character.runtime.combat.active_prayers.clear();
        character.runtime.combat.prayer_drain = None;
        character.runtime.combat.last_attacker = None;
        character.runtime.combat.attack_ready = world.tick;
        character.runtime.combat.spell_ready = world.tick;
        character.runtime.combat.last_combat_tick = None;
        for entity in world.entities.values_mut() {
            if entity.runtime.retaliation_target.as_ref() == Some(&character.actor_id) {
                entity.runtime.retaliation_target = None;
            }
        }
        let destination = if first_office {
            self.resolve_location(world, &character.actor_id, policy.first_office.require()?)?
        } else {
            respawn
        };
        if first_office {
            if destination.instance.is_none() {
                return Err(invalid_content("First Office requires a private instance."));
            }
            character.runtime.first_item_loss_seen = true;
            character.runtime.death_topics.clear();
        }
        world
            .runtime
            .deaths
            .get_mut(&id)
            .ok_or_else(|| invalid_state("Death receipt disappeared."))?
            .arrival = Some(DeathArrival {
            destination,
            first_office,
            dying_until_tick,
            arrives_at_tick,
            completed_at_tick: None,
        });
        character.runtime.life = LifeState::Dying {
            death: id.clone(),
            at_tick: dying_until_tick,
        };
        let mut events = vec![
            GameEvent::Died,
            GameEvent::DeathOccurred {
                death: id.clone(),
                items_lost,
            },
        ];
        if world.tick >= dying_until_tick {
            events.extend(self.advance_death(world, character)?);
        }
        Ok(events)
    }

    pub(crate) fn finish_office_exit(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
    ) -> GameResult<()> {
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unknown("Unknown death policy."))?;
        if !policy
            .required_topics
            .is_subset(&character.runtime.death_topics)
        {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "All Death topics are required.",
            ));
        }
        if let Some(id) = &character.runtime.active_death
            && let Some(grave) = world
                .runtime
                .deaths
                .get_mut(id)
                .and_then(|record| record.grave.as_mut())
        {
            grave.clock_started = true;
            grave.started_at_tick = Some(world.tick);
            grave.paused.remove(&ClockPause::FirstDeathOffice);
        }
        for (vital, restoration) in &policy.restoration.require()?.on_first_office_exit {
            self.restore_vital(character, *vital, restoration)?;
        }
        character.runtime.life = LifeState::Alive;
        Ok(())
    }

    pub(crate) fn advance_graves(
        &self,
        world: &mut WorldState,
        context: &TickContext,
    ) -> GameResult<Vec<ActorEvent>> {
        let Some(policy) = &self.content.mechanics.death else {
            return Ok(vec![]);
        };
        let mut result = Vec::new();
        let ids: Vec<_> = world.runtime.deaths.keys().cloned().collect();
        for id in ids {
            let record = &world.runtime.deaths[&id];
            let Some(grave) = &record.grave else { continue };
            if record.occurred_at_tick == world.tick
                || grave.started_at_tick == Some(world.tick)
                || grave.items.is_empty()
            {
                continue;
            }
            let owner = record.owner.clone();
            let character = world.characters.get(&owner).ok_or_else(|| {
                unavailable("Grave clock requires loaded owner state and authority presence.")
            })?;
            let mut pauses = BTreeSet::new();
            for pause in &policy.grave_pauses {
                if *pause == ClockPause::GraveInterface {
                    if matches!(&runtime::schedule(character)?.access, Some(ContainerSession::Grave { death, .. }) if death == &id)
                    {
                        pauses.insert(*pause);
                    }
                    continue;
                }
                if context.paused(
                    character,
                    &BTreeSet::from([*pause]),
                    Some(policy.idle_after_milliseconds),
                    false,
                    self.in_combat(world, character)?,
                )? {
                    pauses.insert(*pause);
                }
            }
            let expires =
                grave.clock_started && pauses.is_empty() && grave.active_ticks_remaining <= 1;
            let record = world
                .runtime
                .deaths
                .get_mut(&id)
                .ok_or_else(|| invalid_state("Death record disappeared."))?;
            let grave = record
                .grave
                .as_mut()
                .ok_or_else(|| invalid_state("Grave disappeared."))?;
            grave.paused = pauses;
            if !grave.clock_started || !grave.paused.is_empty() {
                continue;
            }
            if expires {
                let grave = record
                    .grave
                    .take()
                    .ok_or_else(|| invalid_state("Grave disappeared."))?;
                record.office.extend(grave.items);
                self.enforce_office_capacity(world, &owner)?;
                result.extend(tag(&owner, vec![GameEvent::GraveExpired { death: id }]));
            } else {
                grave.active_ticks_remaining -= 1;
            }
        }
        Ok(result)
    }

    fn enforce_office_capacity(&self, world: &mut WorldState, actor: &ActorId) -> GameResult<()> {
        self.enforce_office_records(&mut world.runtime.deaths, actor)
    }

    fn enforce_office_records(
        &self,
        records: &mut BTreeMap<DeathId, DeathRecord>,
        actor: &ActorId,
    ) -> GameResult<()> {
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unknown("Missing Office policy."))?;
        let count = records
            .values()
            .filter(|record| &record.owner == actor)
            .flat_map(|record| &record.office)
            .map(|item| recovery_slot_key(&item.stack))
            .collect::<BTreeSet<_>>()
            .len();
        let excess = count.saturating_sub(usize::from(policy.office_capacity));
        if excess == 0 {
            return Ok(());
        }
        let overflow = policy.office_overflow.require()?;
        if matches!(overflow, RecoveryOverflow::RejectTransfer) {
            return Err(GameError::new(
                GameErrorCode::InventoryFull,
                "Death Office is full.",
            ));
        }
        let mut entries: Vec<_> = records
            .iter()
            .filter(|(_, record)| &record.owner == actor)
            .flat_map(|(id, record)| {
                record.office.iter().enumerate().map(move |(order, item)| {
                    (
                        id.clone(),
                        recovery_slot_key(&item.stack),
                        item.effective_unit_value,
                        record.occurred_at_tick,
                        order,
                    )
                })
            })
            .collect();
        entries.sort_by_key(|entry| match overflow {
            RecoveryOverflow::DeleteLowestValue => (entry.2, entry.3, entry.4),
            _ => (0, entry.3, entry.4),
        });
        let mut removed = BTreeSet::new();
        for (_, key, ..) in entries {
            removed.insert(key);
            if removed.len() == excess {
                break;
            }
        }
        for record in records.values_mut().filter(|record| &record.owner == actor) {
            record
                .office
                .retain(|entry| !removed.contains(&recovery_slot_key(&entry.stack)));
        }
        Ok(())
    }

    pub(crate) fn reclaim(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        id: &DeathId,
        storage: RecoveryStorage,
        items: &[RecoveryItemId],
    ) -> GameResult<Vec<GameEvent>> {
        let batches = [RecoveryBatch {
            death: id.clone(),
            storage,
            items: items
                .iter()
                .map(|id| RecoveryItemAmount {
                    id: id.clone(),
                    amount: UiAmount::All {},
                })
                .collect(),
        }];
        let plan =
            self.plan_recovery(world, character, &batches, RecoveryDestination::Inventory)?;
        Ok(plan.install(world, character))
    }

    fn restore_recovery_item(
        &self,
        character: &mut CharacterState,
        item: &RecoveryItem,
        auto_equip: bool,
    ) -> GameResult<()> {
        if let ItemLayout::Equipment { slot } = &item.layout
            && auto_equip
            && equipment::occupant(&character.equipment, &self.content, slot)?.is_none()
        {
            let definition = self
                .content
                .items
                .get(&item.stack.item)
                .and_then(|item| item.equipment.as_ref())
                .ok_or_else(|| invalid_content("Recovery equipment lacks a source definition."))?;
            let mut free = true;
            for occupied in &definition.occupied_slots {
                free &=
                    equipment::occupant(&character.equipment, &self.content, occupied)?.is_none();
            }
            if free {
                let mut staging = character.clone();
                staging.inventory = Inventory::default();
                inventory::add(&mut staging.inventory, &self.content.items, &item.stack)?;
                match equipment::equip(&mut staging, &self.content, 0) {
                    Ok(_) => {
                        character.equipment = staging.equipment;
                        return Ok(());
                    }
                    Err(error) if error.code == GameErrorCode::RequirementNotMet => {}
                    Err(error) => return Err(error),
                }
            }
        }
        let before = character.inventory.clone();
        inventory::add(&mut character.inventory, &self.content.items, &item.stack)?;
        let added_slot = character
            .inventory
            .slots
            .iter()
            .enumerate()
            .find(|(slot, stack)| {
                *stack != &before.slots[*slot]
                    && stack
                        .as_ref()
                        .is_some_and(|stack| stack.item == item.stack.item)
            })
            .map(|(slot, _)| slot)
            .ok_or_else(|| invalid_state("Recovery transfer did not add the selected item."))?;
        match &item.layout {
            ItemLayout::Inventory { slot }
                if before.slots[usize::from(*slot)].is_none()
                    && added_slot != usize::from(*slot) =>
            {
                inventory::swap(
                    &mut character.inventory,
                    &self.content.items,
                    added_slot,
                    usize::from(*slot),
                )?
            }
            ItemLayout::Equipment { slot }
                if auto_equip
                    && equipment::occupant(&character.equipment, &self.content, slot)?
                        .is_none() =>
            {
                let mut trial = character.clone();
                match equipment::equip(&mut trial, &self.content, added_slot) {
                    Ok(_) => *character = trial,
                    Err(error)
                        if matches!(
                            error.code,
                            GameErrorCode::InventoryFull | GameErrorCode::RequirementNotMet
                        ) => {}
                    Err(error) => return Err(error),
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn pay_fee(
        &self,
        character: &mut CharacterState,
        policy: &DeathPolicy,
        mut fee: u64,
    ) -> GameResult<()> {
        for source in &policy.payment_order {
            if fee == 0 {
                break;
            }
            let available = match source {
                FeeSource::Coffer => character.runtime.death_coffer,
                FeeSource::Bank => u64::from(bank::count(
                    &character.bank,
                    &self.content.items,
                    &policy.currency,
                )?),
                FeeSource::Inventory => u64::from(inventory::count(
                    &character.inventory,
                    &self.content.items,
                    &policy.currency,
                )?),
            };
            let amount = fee.min(available);
            if amount == 0 {
                continue;
            }
            match source {
                FeeSource::Coffer => character.runtime.death_coffer -= amount,
                FeeSource::Bank => {
                    let slot = character
                        .bank
                        .slots
                        .iter()
                        .position(|stack| {
                            stack
                                .as_ref()
                                .is_some_and(|stack| stack.item == policy.currency)
                        })
                        .ok_or_else(|| invalid_state("Bank fee currency disappeared."))?;
                    let mut staging = character.clone();
                    staging.inventory = Inventory::default();
                    bank::withdraw(
                        &mut staging,
                        &self.content,
                        slot,
                        Quantity::new(amount as u32)?,
                        false,
                    )?;
                    character.bank = staging.bank;
                    bank::reconcile_ui(character, true)?;
                }
                FeeSource::Inventory => inventory::remove(
                    &mut character.inventory,
                    &self.content.items,
                    &ItemStack {
                        item: policy.currency.clone(),
                        quantity: Quantity::new(amount as u32)?,
                        instance: None,
                    },
                )?,
            }
            fee -= amount;
        }
        if fee > 0 {
            return Err(GameError::new(
                GameErrorCode::InsufficientItems,
                "Insufficient source recovery-fee funds.",
            ));
        }
        Ok(())
    }
}

pub(crate) struct RetentionPart {
    pub order: usize,
    pub stack: ItemStack,
    pub layout: ItemLayout,
    pub value: u64,
    pub kept: u32,
}

impl WorldEngine {
    /// The actual normal-unsafe retention planner, shared by death and the immutable UI preview.
    pub(crate) fn retention_plan(
        &self,
        character: &CharacterState,
    ) -> GameResult<Vec<RetentionPart>> {
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unavailable("Death policy is not bound."))?;
        let provider = self
            .content
            .mechanics
            .value_providers
            .get(&policy.value_provider)
            .ok_or_else(|| unknown("Unknown death value provider."))?;
        let values = provider.values.require()?;
        let ties = policy.ties.require()?;
        let mut carried = Vec::new();
        for (slot, stack) in character
            .inventory
            .slots
            .iter()
            .enumerate()
            .filter_map(|(slot, stack)| stack.as_ref().map(|stack| (slot, stack)))
        {
            carried.push((stack.clone(), ItemLayout::Inventory { slot: slot as u8 }));
        }
        for slot in &self.content.equipment_slots {
            if let Some(stack) = character.equipment.get(slot) {
                carried.push((stack.clone(), ItemLayout::Equipment { slot: slot.clone() }));
            }
        }
        let mut valued = carried
            .into_iter()
            .enumerate()
            .map(|(order, (stack, layout))| {
                let value = *values.get(&stack.item).ok_or_else(|| {
                    unavailable(format!("Death value table lacks {}.", stack.item))
                })?;
                Ok(RetentionPart {
                    order,
                    stack,
                    layout,
                    value,
                    kept: 0,
                })
            })
            .collect::<GameResult<Vec<_>>>()?;
        valued.sort_by(|a, b| {
            b.value
                .cmp(&a.value)
                .then_with(|| match ties {
                    RetentionTiePolicy::InventoryThenEquipment => {
                        layout_key(&a.layout).cmp(&layout_key(&b.layout))
                    }
                    RetentionTiePolicy::EquipmentThenInventory => {
                        (!matches!(a.layout, ItemLayout::Equipment { .. }), a.order)
                            .cmp(&(!matches!(b.layout, ItemLayout::Equipment { .. }), b.order))
                    }
                    RetentionTiePolicy::StableItemIdThenOriginalSlot => {
                        (&a.stack.item, a.order).cmp(&(&b.stack.item, b.order))
                    }
                })
                .then_with(|| a.order.cmp(&b.order))
        });
        let mut remaining = u32::from(policy.retained_unskulled);
        for part in &mut valued {
            part.kept = remaining.min(part.stack.quantity.get());
            remaining -= part.kept;
        }
        Ok(valued)
    }
}

fn layout_key(layout: &ItemLayout) -> (u8, String) {
    match layout {
        ItemLayout::Inventory { slot } => (0, format!("{slot:02}")),
        ItemLayout::Equipment { slot } => (1, slot.to_string()),
    }
}

pub(crate) fn recovery_fee(
    rule: &RecoveryFee,
    item: &RecoveryItem,
    quantity: u32,
) -> GameResult<u64> {
    recovery_fee_value(rule, item.effective_unit_value, item.fee_paid, quantity)
}

pub(crate) fn recovery_fee_value(
    rule: &RecoveryFee,
    value: u64,
    paid: u64,
    quantity: u32,
) -> GameResult<u64> {
    let fee = match rule {
        RecoveryFee::Bands {
            bands,
            maximum_total,
        } => {
            let fee = bands
                .iter()
                .filter(|band| value >= band.minimum_value)
                .max_by_key(|band| band.minimum_value)
                .map_or(0, |band| band.fee);
            u64::from(fee)
                .checked_mul(u64::from(quantity))
                .ok_or_else(|| invalid_content("Recovery fee overflow."))?
                .min(*maximum_total)
        }

        RecoveryFee::Percentage {
            free_below,
            rate,
            rounding,
        } => {
            if value < *free_below {
                0
            } else {
                source_math::rounded(
                    u128::from(value) * u128::from(rate.numerator),
                    u128::from(rate.denominator),
                    rounding,
                )?
                .checked_mul(u64::from(quantity))
                .ok_or_else(|| invalid_content("Recovery fee overflow."))?
            }
        }
    };
    Ok(fee.saturating_sub(paid))
}

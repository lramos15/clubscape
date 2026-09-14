use std::collections::{BTreeMap, BTreeSet};

use crate::*;

fn invalid(message: &str) -> GameError {
    GameError::new(GameErrorCode::InvalidInput, message)
}

fn counters(
    values: &BTreeMap<CounterId, CounterValue>,
    content: &GameContent,
    scope: CounterScope,
) -> GameResult<()> {
    let definitions = &content.mechanics.counters;
    if values.len()
        != definitions
            .values()
            .filter(|definition| definition.scope == scope)
            .count()
    {
        return Err(invalid(
            "Counter state needs an explicit migration for this content revision.",
        ));
    }
    for (id, value) in values {
        let definition = definitions
            .get(id)
            .ok_or_else(|| invalid("Unknown persisted counter."))?;
        if definition.scope != scope {
            return Err(invalid("Counter persisted in the wrong ownership scope."));
        }
        definition.validate_value(*value)?;
    }
    Ok(())
}

pub(crate) fn stack_instance(stack: &ItemStack, content: &GameContent) -> GameResult<()> {
    let definition = content
        .items
        .get(&stack.item)
        .ok_or_else(|| invalid("Unknown persisted item."))?;
    match (&stack.instance, &definition.charges) {
        (None, Some(_)) => return Err(invalid("Charged item has no instance state.")),
        (Some(instance), charges) => {
            if stack.quantity.get() != 1 {
                return Err(invalid(
                    "A unique item instance cannot represent multiple items.",
                ));
            }
            match (&instance.charges, charges) {
                (Some(value), Some(definition)) => {
                    if value.kind != definition.kind
                        || value.remaining > definition.maximum
                        || (value.remaining == 0 && stack.item != definition.empty_variant)
                        || (value.remaining > 0 && stack.item != definition.charged_variant)
                    {
                        return Err(invalid("Persisted charges violate their item definition."));
                    }
                }
                (None, None) => {}
                _ => {
                    return Err(invalid(
                        "Persisted charge state does not match the item kind.",
                    ));
                }
            }
            if let Some(origin) = &instance.origin
                && (origin.acquired_at_tick > i64::MAX as u64
                    || origin.npc.is_some() != origin.life.is_some())
            {
                return Err(invalid("Invalid item-instance origin."));
            }
        }
        (None, None) => {}
    }
    Ok(())
}

impl CharacterState {
    /// Validates extension references and invariants, not mechanics or source fidelity.
    /// Container/equipment/XP validation remains the caller's existing responsibility.
    pub fn validate_runtime(&self, content: &GameContent) -> GameResult<()> {
        self.runtime.validate_shape()?;
        counters(&self.runtime.counters, content, CounterScope::Character)?;
        let definitions = &content.mechanics;
        let runtime = &self.runtime;
        if matches!(runtime.presence, PresenceState::Offline { .. })
            && (!matches!(self.activity, Activity::Idle)
                || self.dialogue.is_some()
                || runtime.pending_travel.is_some()
                || runtime.pending_fire.is_some()
                || matches!(&runtime.engine, EngineMetadata::Typed { schedule } if schedule.access.is_some()))
        {
            return Err(invalid(
                "Offline state cannot discard an active gameplay operation or interface.",
            ));
        }
        if runtime
            .settings
            .experience
            .as_ref()
            .is_some_and(|id| !definitions.experiences.contains_key(id))
            || runtime
                .combat
                .style
                .as_ref()
                .is_some_and(|id| !definitions.combat_styles.contains_key(id))
            || runtime
                .combat
                .target
                .as_ref()
                .is_some_and(|id| !content.spawns.contains_key(id))
            || runtime
                .combat
                .last_attacker
                .as_ref()
                .is_some_and(|id| !content.spawns.contains_key(id))
            || runtime
                .combat
                .active_prayers
                .iter()
                .any(|id| !definitions.prayers.contains_key(id))
            || runtime
                .travel_cooldowns
                .keys()
                .any(|id| !definitions.travels.contains_key(id))
        {
            return Err(invalid(
                "Runtime state references unknown source mechanics.",
            ));
        }

        if let Some(travel) = &runtime.pending_travel
            && !definitions.travels.contains_key(&travel.travel)
        {
            return Err(invalid(
                "Pending travel definition is missing; do not clear the operation.",
            ));
        }
        if let Some(fire) = &runtime.pending_fire
            && !content
                .recipes
                .get(&fire.recipe)
                .and_then(|recipe| recipe.mechanics.as_ref())
                .is_some_and(|mechanics| {
                    matches!(mechanics.lifecycle, RecipeLifecycle::Firemaking { .. })
                })
        {
            return Err(invalid(
                "Pending firemaking definition is missing; do not clear the operation.",
            ));
        }
        if let Activity::ProducingAt {
            recipe,
            remaining,
            next_tick,
            ..
        }
        | Activity::ProducingSelected {
            recipe,
            remaining,
            next_tick,
            ..
        } = &self.activity
            && (!content.recipes.contains_key(recipe)
                || *remaining == 0
                || *next_tick > i64::MAX as u64)
        {
            return Err(invalid("Invalid pending dynamic-facility production."));
        }
        if matches!(&self.activity, Activity::ProducingSelected { mode: ProductionMode::Single, remaining, .. } if *remaining != 1)
        {
            return Err(invalid(
                "A single production cannot contain multiple operations.",
            ));
        }
        for prayer in &runtime.combat.active_prayers {
            if definitions.prayers[prayer]
                .exclusive_with
                .iter()
                .any(|id| runtime.combat.active_prayers.contains(id))
            {
                return Err(invalid("Mutually exclusive prayers are active together."));
            }
        }
        for (id, claim) in &runtime.entitlements {
            let definition = definitions
                .entitlements
                .get(id)
                .ok_or_else(|| invalid("Unknown reward entitlement in persisted ledger."))?;
            match (&definition.purpose, claim) {
                (
                    EntitlementPurpose::AtomicReward | EntitlementPurpose::Reconciliation { .. },
                    EntitlementState::Claimed { .. },
                ) => {}
                (
                    EntitlementPurpose::Grant { grant },
                    EntitlementState::Grant {
                        delivered,
                        satisfied,
                        complete,
                    },
                ) => {
                    let grant = definitions
                        .grants
                        .get(grant)
                        .ok_or_else(|| invalid("Ledger grant definition is missing."))?;
                    let lines: BTreeMap<_, _> =
                        grant.lines.iter().map(|line| (&line.item, line)).collect();
                    if delivered.iter().any(|(id, amount)| {
                        lines
                            .get(id)
                            .is_none_or(|line| *amount > line.quantity.get())
                    }) || satisfied.iter().any(|id| !lines.contains_key(id))
                        || *complete != (satisfied.len() == lines.len())
                    {
                        return Err(invalid(
                            "Partial grant ledger exceeds its entitlement or has inconsistent completion.",
                        ));
                    }
                }
                _ => {
                    return Err(invalid(
                        "Persisted entitlement ledger has the wrong purpose.",
                    ));
                }
            }
        }
        let mut instances = BTreeSet::new();
        for stack in self
            .inventory
            .slots
            .iter()
            .flatten()
            .chain(self.equipment.values())
            .chain(self.bank.slots.iter().flatten())
        {
            stack_instance(stack, content)?;
            if let Some(instance) = &stack.instance
                && !instances.insert(&instance.id)
            {
                return Err(invalid(
                    "One item instance appears in multiple owned slots.",
                ));
            }
        }
        if let EngineMetadata::Typed { schedule } = &runtime.engine {
            if self
                .flags
                .keys()
                .any(|key| key.starts_with("__world_engine."))
            {
                return Err(invalid(
                    "Typed and legacy engine scheduling metadata conflict.",
                ));
            }
            if matches!(self.activity, Activity::Gathering { .. })
                != schedule.gather_interaction.is_some()
                || self.dialogue.is_some() != schedule.dialogue_interaction.is_some()
            {
                return Err(invalid(
                    "Typed schedule and pending activity/dialogue disagree.",
                ));
            }
            match &schedule.access {
                Some(ContainerSession::Grave { interface, .. })
                    if content
                        .mechanics
                        .death
                        .as_ref()
                        .and_then(|policy| policy.interfaces.as_ref())
                        .is_none_or(|ids| &ids.grave != interface) =>
                {
                    return Err(invalid("Grave session has no matching source interface."));
                }
                Some(ContainerSession::DeathOffice { interface })
                    if content
                        .mechanics
                        .death
                        .as_ref()
                        .and_then(|policy| policy.interfaces.as_ref())
                        .is_none_or(|ids| &ids.office != interface) =>
                {
                    return Err(invalid("Office session has no matching source interface."));
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl CharacterRuntime {
    /// Append-only reward accounting survives replay, close/reopen, death and restart.
    pub fn validate_ledger_successor(&self, successor: &Self) -> GameResult<()> {
        for (id, previous) in &self.entitlements {
            let next = successor
                .entitlements
                .get(id)
                .ok_or_else(|| invalid("Acknowledged reward entitlement was removed."))?;
            let valid = match (previous, next) {
                (
                    EntitlementState::Claimed { at_tick: before },
                    EntitlementState::Claimed { at_tick: after },
                ) => before == after,
                (
                    EntitlementState::Grant {
                        delivered: before,
                        satisfied: satisfied_before,
                        complete: complete_before,
                    },
                    EntitlementState::Grant {
                        delivered: after,
                        satisfied: satisfied_after,
                        complete: complete_after,
                    },
                ) => {
                    (!complete_before || *complete_after)
                        && satisfied_before.is_subset(satisfied_after)
                        && before.iter().all(|(item, quantity)| {
                            after.get(item).is_some_and(|after| after >= quantity)
                        })
                        && (!complete_before || previous == next)
                }
                _ => false,
            };
            if !valid {
                return Err(invalid(
                    "Acknowledged reward entitlement regressed or changed purpose.",
                ));
            }
        }
        Ok(())
    }
}

impl EntityRuntime {
    pub fn validate_shape(&self) -> GameResult<()> {
        if self.life > i64::MAX as u64
            || self.attack_ready > i64::MAX as u64
            || self
                .last_combat_tick
                .is_some_and(|tick| tick > i64::MAX as u64)
            || self.aggression_ready > i64::MAX as u64
            || self.kill.as_ref().is_some_and(|kill| {
                kill.life != self.life || kill.at_tick > i64::MAX as u64 || !self.loot_resolved
            })
            || self
                .next_movement_tick
                .is_some_and(|tick| tick > i64::MAX as u64)
            || self.contributions.len() > 256
            || self.contributions.values().any(|contribution| {
                contribution.damage == 0
                    || contribution.damage > i64::MAX as u64
                    || contribution.first_hit_tick > contribution.last_hit_tick
                    || contribution.last_hit_tick > i64::MAX as u64
                    || (contribution.first_hit_tick == contribution.last_hit_tick
                        && contribution.first_hit_order > contribution.last_hit_order)
            })
        {
            return Err(invalid("Invalid persisted NPC combat/movement state."));
        }
        for contribution in self.contributions.values() {
            let mut total = 0_u64;
            for method in contribution.methods.values() {
                if method.damage == 0
                    || method.first_hit_tick > method.last_hit_tick
                    || method.last_hit_tick > i64::MAX as u64
                    || method.first_hit_tick < contribution.first_hit_tick
                    || method.last_hit_tick > contribution.last_hit_tick
                    || method.first_hit_tick == method.last_hit_tick
                        && method.first_hit_order > method.last_hit_order
                {
                    return Err(invalid("Invalid method-specific damage history."));
                }
                total = total
                    .checked_add(method.damage)
                    .ok_or_else(|| invalid("Method damage overflow."))?;
            }
            if total > contribution.damage
                || contribution.methods_complete && total != contribution.damage
            {
                return Err(invalid(
                    "Contribution totals disagree with complete method history.",
                ));
            }
        }
        Ok(())
    }
}

impl WorldRuntime {
    pub fn validate_shape(&self) -> GameResult<()> {
        if self.schema_version != RUNTIME_SCHEMA_VERSION
            || self.next_ground_id > i64::MAX as u64
            || self.ground_provenance.len() > 32_768
            || self.counters.len() > 2048
            || self.object_states.len() > 32_768
            || self.temporary_objects.len() > 32_768
            || self.instances.len() > 4096
            || self.projectiles.len() > 32_768
            || self.deaths.len() > 32_768
            || self.stock_deadlines.len() > 4096
            || self
                .stock_deadlines
                .values()
                .any(|rows| rows.len() > 4096 || rows.values().any(|tick| *tick > i64::MAX as u64))
            || self.instances.values().any(|instance| {
                instance.counters.len() > 2048
                    || instance.entities.len() > 32_768
                    || instance.object_states.len() > 32_768
            })
            || self.temporary_objects.values().any(|object| {
                object.created_at_tick >= object.expires_at_tick
                    || object.expires_at_tick > i64::MAX as u64
            })
        {
            return Err(invalid(
                "World runtime collections or deadlines exceed their bounds.",
            ));
        }
        for instance in self.instances.values() {
            for entity in instance.entities.values() {
                entity.runtime.validate_shape()?;
            }
        }
        let mut projectiles = BTreeSet::new();
        for projectile in &self.projectiles {
            if projectile.id == 0
                || !projectiles.insert(projectile.id)
                || projectile.launched_at_tick > projectile.impacts_at_tick
                || projectile.impacts_at_tick > i64::MAX as u64
                || projectile.resources_spent.len() > 28
                || (projectile.outcome != CombatOutcome::Hit && projectile.damage != 0)
            {
                return Err(invalid(
                    "Invalid pending projectile identity/timing/outcome.",
                ));
            }
        }
        let mut recovery_ids = BTreeSet::new();
        for record in self.deaths.values() {
            if record.occurred_at_tick > i64::MAX as u64
                || record.value_revision.is_empty()
                || record.value_revision.len() > 256
                || record.retained.len() > 64
                || record.office.len() > 4096
                || record.reclaimed.len() > 4096
                || record.arrival.as_ref().is_some_and(|arrival| {
                    arrival.dying_until_tick < record.occurred_at_tick
                        || arrival.arrives_at_tick < arrival.dying_until_tick
                        || arrival.arrives_at_tick > i64::MAX as u64
                        || arrival.completed_at_tick.is_some_and(|tick| {
                            tick < arrival.arrives_at_tick || tick > i64::MAX as u64
                        })
                })
                || record
                    .grave
                    .as_ref()
                    .is_some_and(|grave| grave.items.len() > 4096)
            {
                return Err(invalid("Invalid death/recovery storage bounds."));
            }
            let mut local = BTreeSet::new();
            for item in record
                .retained
                .iter()
                .chain(&record.office)
                .chain(record.grave.iter().flat_map(|grave| &grave.items))
            {
                if !local.insert(&item.id)
                    || item.effective_unit_value > i64::MAX as u64
                    || item.fee_paid > i64::MAX as u64
                    || item
                        .effective_unit_value
                        .checked_mul(u64::from(item.stack.quantity.get()))
                        .is_none()
                    || matches!(item.layout, ItemLayout::Inventory { slot } if usize::from(slot) >= INVENTORY_SLOTS)
                {
                    return Err(invalid("Invalid retained/lost item identity/layout/value."));
                }
            }
            for item in record
                .office
                .iter()
                .chain(record.grave.iter().flat_map(|grave| &grave.items))
            {
                if record.reclaimed.contains(&item.id) || !recovery_ids.insert(&item.id) {
                    return Err(invalid(
                        "Recovery ownership was reclaimed or appears twice.",
                    ));
                }
            }
        }
        Ok(())
    }
}

impl WorldState {
    fn validate_runtime_location(
        &self,
        location: &RuntimeLocation,
        content: &GameContent,
    ) -> GameResult<()> {
        if content
            .regions
            .get(&location.region)
            .is_none_or(|region| !region.cells.iter().any(|cell| cell.tile == location.tile))
        {
            return Err(invalid(
                "Runtime location has no explicit source region/cell.",
            ));
        }
        if let Some(instance) = &location.instance {
            let instance =
                self.runtime.instances.get(instance).ok_or_else(|| {
                    invalid("Runtime location references an unknown live instance.")
                })?;
            let definition = content
                .mechanics
                .instances
                .get(&instance.template)
                .ok_or_else(|| invalid("Runtime instance template is missing."))?;
            if !definition.chunks.iter().any(|chunk| {
                chunk.destination_region == location.region
                    && chunk.destination_origin.plane() == location.tile.plane()
                    && location.tile.x() >= chunk.destination_origin.x()
                    && location.tile.y() >= chunk.destination_origin.y()
                    && location.tile.x() - chunk.destination_origin.x()
                        < u16::from(definition.chunk_size)
                    && location.tile.y() - chunk.destination_origin.y()
                        < u16::from(definition.chunk_size)
            }) {
                return Err(invalid(
                    "Runtime location is outside its live instance mapping.",
                ));
            }
        }
        Ok(())
    }

    pub fn validate_runtime(&self, content: &GameContent) -> GameResult<()> {
        self.runtime.validate_shape()?;
        if self.schema_version != GAME_SCHEMA_VERSION || self.content_revision != content.revision {
            return Err(invalid(
                "World state needs an explicit content/schema migration.",
            ));
        }
        counters(&self.runtime.counters, content, CounterScope::World)?;
        let definitions = &content.mechanics;
        let mut instances = BTreeSet::new();
        let mut check_item = |stack: &ItemStack| {
            stack_instance(stack, content)?;
            if let Some(instance) = &stack.instance
                && !instances.insert(instance.id.clone())
            {
                return Err(invalid("One item instance has multiple spendable owners."));
            }
            Ok(())
        };
        for (id, character) in &self.characters {
            if id != &character.actor_id {
                return Err(invalid("Character map key and actor identity disagree."));
            }
            character.validate_runtime(content)?;
            match &character.runtime.life {
                LifeState::Dying { death, at_tick } => {
                    let record = self
                        .runtime
                        .deaths
                        .get(death)
                        .ok_or_else(|| invalid("Dying actor has no death receipt."))?;
                    if record.owner != *id
                        || character.hitpoints != 0
                        || record.arrival.as_ref().is_some_and(|arrival| {
                            arrival.dying_until_tick != *at_tick
                                || arrival.completed_at_tick.is_some()
                        })
                    {
                        return Err(invalid(
                            "Dying state conflicts with its owned phase receipt.",
                        ));
                    }
                }
                LifeState::Respawning {
                    death,
                    destination,
                    at_tick,
                } => {
                    let record = self
                        .runtime
                        .deaths
                        .get(death)
                        .ok_or_else(|| invalid("Respawning actor has no death receipt."))?;
                    self.validate_runtime_location(destination, content)?;
                    if record.owner != *id
                        || character.hitpoints != 0
                        || record.arrival.as_ref().is_some_and(|arrival| {
                            arrival.destination != *destination
                                || arrival.arrives_at_tick != *at_tick
                                || arrival.completed_at_tick.is_some()
                        })
                    {
                        return Err(invalid(
                            "Respawn state conflicts with its owned phase receipt.",
                        ));
                    }
                }
                LifeState::FirstDeathOffice { death, instance }
                    if character.runtime.instance.as_ref() != Some(instance)
                        || self
                            .runtime
                            .deaths
                            .get(death)
                            .is_none_or(|record| record.owner != *id) =>
                {
                    return Err(invalid("First Office life state has mismatched ownership."));
                }
                _ => {}
            }
            self.validate_runtime_location(
                &RuntimeLocation {
                    region: character.region.clone(),
                    tile: character.tile,
                    instance: character.runtime.instance.clone(),
                },
                content,
            )?;
            if character
                .runtime
                .instance
                .as_ref()
                .is_some_and(|id| !self.runtime.instances.contains_key(id))
                || character.runtime.active_death.as_ref().is_some_and(|id| {
                    self.runtime
                        .deaths
                        .get(id)
                        .is_none_or(|death| death.owner != character.actor_id)
                })
            {
                return Err(invalid(
                    "Character instance/death ownership reference is invalid.",
                ));
            }
            if character
                .runtime
                .instance
                .as_ref()
                .and_then(|id| self.runtime.instances.get(id))
                .is_some_and(|instance| {
                    content.mechanics.instances[&instance.template].private_to_character
                        && instance.owner.as_ref() != Some(&character.actor_id)
                })
            {
                return Err(invalid("Character does not own its private instance."));
            }
            if let Some(travel) = &character.runtime.pending_travel {
                self.validate_runtime_location(&travel.origin, content)?;
                self.validate_runtime_location(&travel.destination, content)?;
            }
            if let EngineMetadata::Typed { schedule } = &character.runtime.engine
                && let Some(ContainerSession::Grave { death, .. }) = &schedule.access
                && self.runtime.deaths.get(death).is_none_or(|record| {
                    record.owner != character.actor_id || record.grave.is_none()
                })
            {
                return Err(invalid("Grave session has no owned live grave."));
            }
            for item in character
                .inventory
                .slots
                .iter()
                .flatten()
                .chain(character.equipment.values())
                .chain(character.bank.slots.iter().flatten())
            {
                check_item(item)?;
            }
        }
        if self.ground_items.len() > 32_768 {
            return Err(invalid("Ground item collection exceeds its runtime bound."));
        }
        let mut ground_ids = BTreeSet::new();
        let mut life_drops = BTreeSet::new();
        for ground in &self.ground_items {
            if !ground_ids.insert(&ground.id) {
                return Err(invalid("Duplicate ground item identity."));
            }
            if ground
                .instance
                .as_ref()
                .is_some_and(|id| !self.runtime.instances.contains_key(id))
            {
                return Err(invalid("Ground item references an unknown live instance."));
            }
            check_item(&ground.stack)?;
            if let Some(provenance) = self.runtime.ground_provenance.get(&ground.id) {
                if !definitions.ground_policies.contains_key(&provenance.policy) {
                    return Err(invalid("Unknown live ground policy."));
                }
                let ordinal = ground
                    .id
                    .strip_prefix("ground.engine.")
                    .and_then(|value| value.parse::<u64>().ok())
                    .ok_or_else(|| invalid("Invalid generated ground identity."))?;
                if ordinal == 0 || ordinal > self.runtime.next_ground_id {
                    return Err(invalid("Ground identity counter needs migration."));
                }
                match &provenance.producer {
                    GroundProducer::PlayerDrop { actor, at_tick }
                    | GroundProducer::Activity { actor, at_tick }
                    | GroundProducer::DeathSupply { actor, at_tick } => {
                        if ground.owner.as_ref() != Some(actor) || *at_tick > self.tick {
                            return Err(invalid("Ground producer ownership/time mismatch."));
                        }
                    }
                    GroundProducer::NpcLoot {
                        spawn,
                        life,
                        instance,
                        ordinal,
                    } => {
                        if !content.spawns.contains_key(spawn)
                            || *life > i64::MAX as u64
                            || ground.instance != *instance
                            || !life_drops.insert((spawn, life, instance, ordinal))
                        {
                            return Err(invalid("Duplicate or invalid NPC-life loot origin."));
                        }
                    }
                }
            }
        }
        if self
            .runtime
            .ground_provenance
            .keys()
            .any(|id| !ground_ids.contains(id))
        {
            return Err(invalid("Orphaned ground provenance."));
        }
        for (id, state) in &self.runtime.object_states {
            if definitions
                .object_transforms
                .get(id)
                .is_none_or(|definition| {
                    definition.scope != CounterScope::World
                        || !definition.states.contains_key(state)
                })
            {
                return Err(invalid("Undefined live object transform/state."));
            }
        }
        for object in self.runtime.temporary_objects.values() {
            self.validate_runtime_location(&object.location, content)?;
            let definition = definitions
                .temporary_objects
                .get(&object.definition)
                .ok_or_else(|| invalid("Undefined live temporary object."))?;
            let elapsed = object.expires_at_tick - object.created_at_tick;
            if let SourceBinding::Bound { value, .. } = &definition.lifetime {
                let valid = match value {
                    TickDuration::Fixed { ticks } => elapsed == u64::from(*ticks),
                    TickDuration::UniformInclusive { minimum, maximum } => {
                        (u64::from(*minimum)..=u64::from(*maximum)).contains(&elapsed)
                    }
                };
                if !valid {
                    return Err(invalid(
                        "Temporary object lifetime violates its source bounds.",
                    ));
                }
            } else {
                return Err(invalid("A live object cannot have an unresolved lifetime."));
            }
        }
        for instance in self.runtime.instances.values() {
            let definition = definitions
                .instances
                .get(&instance.template)
                .ok_or_else(|| invalid("Unknown live instance template."))?;
            if definition.private_to_character && instance.owner.is_none() {
                return Err(invalid("Private instance has no owner."));
            }
            counters(&instance.counters, content, CounterScope::Instance)?;
            for (id, state) in &instance.object_states {
                if definitions
                    .object_transforms
                    .get(id)
                    .is_none_or(|definition| {
                        definition.scope != CounterScope::Instance
                            || !definition.states.contains_key(state)
                    })
                {
                    return Err(invalid("Invalid instance-scoped object state."));
                }
            }
            for (spawn, entity) in &instance.entities {
                if !content.spawns.contains_key(spawn) {
                    return Err(invalid("Unknown instance NPC/object placement."));
                }
                entity.runtime.validate_shape()?;
            }
        }
        for (id, entity) in &self.entities {
            entity.runtime.validate_shape()?;
            if !content.spawns.contains_key(id) {
                return Err(invalid("Unknown live entity."));
            }
        }
        for projectile in &self.runtime.projectiles {
            if !definitions.projectiles.contains_key(&projectile.definition)
                || !definitions.combat_styles.contains_key(&projectile.style)
                || projectile
                    .spell
                    .as_ref()
                    .is_some_and(|id| !definitions.spells.contains_key(id))
            {
                return Err(invalid(
                    "Pending projectile references missing content; preserve it for migration.",
                ));
            }
            if let Some(snapshot) = &projectile.target_snapshot
                && (!content.npcs.contains_key(&snapshot.npc)
                    || content
                        .regions
                        .get(&snapshot.location.region)
                        .is_none_or(|region| {
                            !region
                                .cells
                                .iter()
                                .any(|cell| cell.tile == snapshot.location.tile)
                        })
                    || !matches!(&projectile.target, Combatant::Npc { instance, .. } if instance == &snapshot.location.instance))
            {
                return Err(invalid(
                    "Projectile target identity/location snapshot is invalid.",
                ));
            }
        }
        let mut office_counts = BTreeMap::<&ActorId, BTreeSet<RecoverySlotKey>>::new();
        for death in self.runtime.deaths.values() {
            self.validate_runtime_location(&death.origin, content)?;
            self.validate_runtime_location(&death.respawn, content)?;
            if let Some(arrival) = &death.arrival {
                self.validate_runtime_location(&arrival.destination, content)?;
            }
            let policy = definitions
                .death
                .as_ref()
                .ok_or_else(|| invalid("Death state has no source policy."))?;
            let provider = definitions
                .value_providers
                .get(&death.value_provider)
                .ok_or_else(|| invalid("Death value provider is missing."))?;
            if death.value_revision != provider.revision {
                return Err(invalid(
                    "Death valuation snapshot changed without a migration.",
                ));
            }
            if let Some(grave) = &death.grave
                && (grave.active_ticks_remaining > policy.grave_active_ticks
                    || grave.items.len() > usize::from(policy.grave_capacity)
                    || !grave.paused.is_subset(&policy.grave_pauses))
            {
                return Err(invalid(
                    "Grave timer/storage/pause state violates its source policy.",
                ));
            }
            if let Some(grave) = &death.grave {
                self.validate_runtime_location(&grave.location, content)?;
            }
            let count = office_counts.entry(&death.owner).or_default();
            count.extend(
                death
                    .office
                    .iter()
                    .map(|item| recovery_slot_key(&item.stack)),
            );
            if count.len() > usize::from(policy.office_capacity) {
                return Err(invalid(
                    "Owner's combined Death Office storage exceeds capacity.",
                ));
            }
            for item in &death.retained {
                stack_instance(&item.stack, content)?;
            }
            for item in death
                .office
                .iter()
                .chain(death.grave.iter().flat_map(|grave| &grave.items))
            {
                check_item(&item.stack)?;
            }
        }
        for (shop, rows) in &self.runtime.stock_deadlines {
            let definition = content
                .shops
                .get(shop)
                .ok_or_else(|| invalid("Unknown persisted shop clock."))?;
            if rows.keys().any(|item| {
                !definition.stock.iter().any(|row| &row.item == item)
                    && !(matches!(
                        definition.unstocked,
                        Some(UnstockedShopPolicy::Accept { .. })
                    ) && self
                        .shops
                        .get(shop)
                        .is_some_and(|state| state.stock.contains_key(item))
                        && content.items.get(item).is_some_and(|item| {
                            item.tradable
                                && item.unnoted_variant.is_none()
                                && item.id != definition.currency
                        }))
            }) {
                return Err(invalid(
                    "Stock deadline references an undefined source stock line.",
                ));
            }
        }
        for (shop, state) in &self.shops {
            let definition = content
                .shops
                .get(shop)
                .ok_or_else(|| invalid("Unknown shop state."))?;
            if definition
                .stock
                .iter()
                .any(|row| !state.stock.contains_key(&row.item))
                || state
                    .stock
                    .values()
                    .any(|quantity| *quantity > MAX_STACK_QUANTITY)
            {
                return Err(invalid(
                    "Shop stock has missing rows or invalid quantities.",
                ));
            }
            let extras: Vec<_> = state
                .stock
                .keys()
                .filter(|item| !definition.stock.iter().any(|row| &row.item == *item))
                .collect();
            if !extras.is_empty() {
                let Some(UnstockedShopPolicy::Accept {
                    maximum_lines,
                    rule,
                    ..
                }) = &definition.unstocked
                else {
                    return Err(invalid("Shop has unapproved extra stock rows."));
                };
                if extras.len() > usize::from(*maximum_lines) {
                    return Err(invalid("Shop exceeds its unstocked line limit."));
                }
                for item in extras {
                    if content.items.get(item).is_none_or(|item| {
                        !item.tradable
                            || item.unnoted_variant.is_some()
                            || item.id == definition.currency
                    }) {
                        return Err(invalid(
                            "Unstocked row has an untradeable or invalid item form.",
                        ));
                    }
                    if matches!(
                        &rule.restock.phase,
                        SourceBinding::Bound {
                            value: RestockPhase::SinceLastStockChange,
                            ..
                        }
                    ) && self
                        .runtime
                        .stock_deadlines
                        .get(shop)
                        .is_none_or(|rows| !rows.contains_key(item))
                    {
                        return Err(invalid(
                            "Since-change unstocked row lacks its persisted phase.",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

use std::collections::BTreeMap;

use clubscape_game_types::*;

use crate::{
    RandomSource, WorldEngine, invalid_content, invalid_state, navigation::map_chunk,
    progression::EffectFrame, runtime, unavailable, unknown,
};

fn ground_clock(policy: &GroundItemPolicy, producer: &GroundProducer) -> GameResult<GroundClock> {
    if let Some(clock) = &policy.clock {
        return clock.require().copied();
    }
    // Legacy content had one origin-based exception. New policies declare their
    // clock explicitly; the selected value is frozen with each persisted drop.
    Ok(if matches!(producer, GroundProducer::DeathSupply { .. }) {
        GroundClock::OwnerOnlineTicks
    } else {
        GroundClock::WorldTicks
    })
}

impl WorldEngine {
    pub(crate) fn player_drop_policy(
        &self,
        character: &CharacterState,
        item: &ItemId,
    ) -> GameResult<&GroundPolicyId> {
        let selector = self
            .content
            .mechanics
            .player_drop
            .as_ref()
            .ok_or_else(|| unavailable("Player drop policy is not configured."))?;
        if let Some(stage) = selector.stages.get(&character.tutorial_stage) {
            return stage.require();
        }
        if !self
            .content
            .items
            .get(item)
            .ok_or_else(|| unknown("Unknown dropped item."))?
            .tradable
            && let Some(untradeable) = &selector.untradeable
        {
            return untradeable.require();
        }
        if let Some(before) = &selector.before_playtime {
            let before = before.require()?;
            let played = character.runtime.played_time.as_ref().ok_or_else(|| {
                unavailable(
                    "Player-drop selection requires explicitly migrated authoritative playtime.",
                )
            })?;
            if played.ticks < before.played_ticks_below {
                return Ok(&before.ground_policy);
            }
        }
        selector.ordinary.require()
    }

    pub(crate) fn advance_playtime(
        &self,
        world: &mut WorldState,
        context: &crate::TickContext,
    ) -> GameResult<()> {
        for (actor, character) in &mut world.characters {
            let Some(played) = &mut character.runtime.played_time else {
                continue;
            };
            let presence = context.actors.get(actor).ok_or_else(|| {
                unavailable("Owner playtime requires authoritative actor presence.")
            })?;
            if played
                .through_world_tick
                .is_some_and(|tick| tick > world.tick)
            {
                return Err(invalid_state("Owner playtime clock is future-dated."));
            }
            if played.through_world_tick != Some(world.tick) {
                if presence.online {
                    played.ticks = runtime::deadline(played.ticks, 1)?;
                }
                played.through_world_tick = Some(world.tick);
            }
        }
        Ok(())
    }

    pub(crate) fn move_to(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        location: RuntimeLocation,
    ) -> GameResult<()> {
        if matches!(character.runtime.life, LifeState::FirstDeathOffice { .. }) {
            let policy = self
                .content
                .mechanics
                .death
                .as_ref()
                .ok_or_else(|| unknown("Missing first-Office policy."))?;
            if !policy
                .required_topics
                .is_subset(&character.runtime.death_topics)
                || character
                    .runtime
                    .pending_travel
                    .as_ref()
                    .is_none_or(|travel| travel.destination != location)
            {
                return Err(GameError::new(
                    GameErrorCode::RequirementNotMet,
                    "First Office exit requires the guarded, completed source transport.",
                ));
            }
        }
        let map = self.collision_for(world, location.instance.as_ref())?;
        if self.regions_by_tile.get(&location.tile) != Some(&location.region)
            || !map.cell(location.tile).is_some_and(|cell| cell.walkable)
        {
            return Err(GameError::new(
                GameErrorCode::Blocked,
                "Travel destination has no walkable source cell.",
            ));
        }
        if let Some(id) = &location.instance {
            let instance = world
                .runtime
                .instances
                .get(id)
                .ok_or_else(|| invalid_state("Unknown destination instance."))?;
            if instance
                .owner
                .as_ref()
                .is_some_and(|owner| owner != &character.actor_id)
            {
                return Err(GameError::new(
                    GameErrorCode::NotOwned,
                    "Private instance belongs to another actor.",
                ));
            }
        }
        runtime::interrupt(character)?;
        character.tile = location.tile;
        character.region = location.region;
        character.runtime.instance = location.instance;
        Ok(())
    }

    pub(crate) fn resolve_location(
        &self,
        world: &mut WorldState,
        actor: &ActorId,
        location: &WorldLocation,
    ) -> GameResult<RuntimeLocation> {
        let instance = match &location.instance {
            None => None,
            Some(template_id) => {
                let template = self
                    .content
                    .mechanics
                    .instances
                    .get(template_id)
                    .ok_or_else(|| unknown("Unknown instance template."))?;
                let prior = world.runtime.instances.iter().find(|(_, state)| {
                    &state.template == template_id
                        && state.owner.as_ref()
                            == if template.private_to_character {
                                Some(actor)
                            } else {
                                None
                            }
                });
                if let Some((id, _)) = prior {
                    Some(id.clone())
                } else {
                    let id = InstanceId::new(format!(
                        "instance.engine.{}.{}",
                        world.tick,
                        world.runtime.instances.len()
                    ))?;
                    if world.runtime.instances.contains_key(&id) {
                        return Err(invalid_state("Instance identity collision."));
                    }
                    let mut entities = BTreeMap::new();
                    for (spawn, definition) in &self.content.spawns {
                        if template.chunks.iter().any(|chunk| {
                            (chunk.source_region == definition.region)
                                && map_chunk(definition.tile, chunk, template.chunk_size).is_some()
                        }) {
                            let (width, height) = self.spawn_dimensions(definition)?;
                            let tile = crate::navigation::map_footprint(
                                template,
                                definition.tile,
                                width,
                                height,
                            )?
                            .0;
                            let mut state = self.initial_entity(definition)?;
                            state.tile = tile;
                            entities.insert(spawn.clone(), state);
                            if let SpawnKind::Item { stack, .. } = &definition.kind {
                                let mut item =
                                    self.spawn_ground_item(spawn, tile, stack, world.tick);
                                item.instance = Some(id.clone());
                                item.id = format!("source:{spawn}:{}:{id}", world.tick);
                                world.ground_items.push(item);
                            }
                        }
                    }
                    let counters = self
                        .content
                        .mechanics
                        .counters
                        .iter()
                        .filter(|(_, definition)| definition.scope == CounterScope::Instance)
                        .map(|(id, definition)| (id.clone(), definition.initial))
                        .collect();
                    let object_states = self
                        .content
                        .mechanics
                        .object_transforms
                        .iter()
                        .filter(|(_, definition)| {
                            definition.scope == CounterScope::Instance
                                && entities.contains_key(&definition.spawn)
                        })
                        .map(|(id, definition)| (id.clone(), definition.initial.clone()))
                        .collect();
                    world.runtime.instances.insert(
                        id.clone(),
                        InstanceState {
                            template: template_id.clone(),
                            owner: template.private_to_character.then(|| actor.clone()),
                            counters,
                            entities,
                            object_states,
                        },
                    );
                    Some(id)
                }
            }
        };
        Ok(RuntimeLocation {
            region: location.region.clone(),
            tile: location.tile,
            instance,
        })
    }

    pub(crate) fn travel_permission(
        &self,
        world: &WorldState,
        character: &CharacterState,
        id: &TravelId,
    ) -> GameResult<()> {
        let definition = self
            .content
            .mechanics
            .travels
            .get(id)
            .ok_or_else(|| unknown("Unknown source travel."))?;
        self.require_guard(world, character, &definition.guard)?;
        if character.runtime.pending_travel.is_some() {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "A source transport is already pending.",
            ));
        }
        if character
            .runtime
            .travel_cooldowns
            .get(id)
            .is_some_and(|ready| *ready > world.tick)
            || definition
                .interruptions
                .contains(&InterruptionCause::Combat)
                && self.combat_locked(world, character, crate::combat::CombatLock::Travel)?
        {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "Source transport is blocked by its cooldown or combat lock.",
            ));
        }
        definition.channel_ticks.require()?;
        definition.cooldown_ticks.require()?;
        definition.cooldown_start.require()?;
        match definition.destination.require()? {
            TravelDestination::Fixed { .. } => {}
            TravelDestination::Experience { branches } => {
                if character
                    .runtime
                    .settings
                    .experience
                    .as_ref()
                    .is_none_or(|experience| !branches.contains_key(experience))
                {
                    return Err(GameError::new(
                        GameErrorCode::RequirementNotMet,
                        "Source travel has no selected experience destination.",
                    ));
                }
            }
            TravelDestination::PreviousRespawn if character.runtime.previous_respawn.is_none() => {
                return Err(invalid_state("Previous respawn is not recorded."));
            }
            TravelDestination::PreviousRespawn => {}
        }
        Ok(())
    }

    pub(crate) fn start_travel(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        id: &TravelId,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        self.travel_permission(world, character, id)?;
        let definition = self
            .content
            .mechanics
            .travels
            .get(id)
            .ok_or_else(|| unknown("Unknown source travel."))?;
        self.require_guard(world, character, &definition.guard)?;
        if character.runtime.pending_travel.is_some() {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "A transport is already pending.",
            ));
        }
        if definition
            .interruptions
            .contains(&InterruptionCause::Combat)
            && self.combat_locked(world, character, crate::combat::CombatLock::Travel)?
        {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "Source transport cannot begin during combat.",
            ));
        }
        if character
            .runtime
            .travel_cooldowns
            .get(id)
            .is_some_and(|ready| *ready > world.tick)
        {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "Travel cooldown is not ready.",
            ));
        }
        let duration = *definition.channel_ticks.require()?;
        let cooldown = *definition.cooldown_ticks.require()?;
        let cooldown_start = definition.cooldown_start.require()?;
        let destination = match definition.destination.require()? {
            TravelDestination::Fixed { location } => {
                self.resolve_location(world, &character.actor_id, location)?
            }
            TravelDestination::Experience { branches } => {
                let experience =
                    character
                        .runtime
                        .settings
                        .experience
                        .as_ref()
                        .ok_or_else(|| {
                            GameError::new(
                                GameErrorCode::RequirementNotMet,
                                "Select a source experience branch first.",
                            )
                        })?;
                self.resolve_location(
                    world,
                    &character.actor_id,
                    branches.get(experience).ok_or_else(|| {
                        unavailable("No travel destination for this source experience.")
                    })?,
                )?
            }
            TravelDestination::PreviousRespawn => character
                .runtime
                .previous_respawn
                .clone()
                .ok_or_else(|| invalid_state("Previous source respawn is not recorded."))?,
        };
        let map = self.collision_for(world, destination.instance.as_ref())?;
        if !map.cell(destination.tile).is_some_and(|cell| cell.walkable) {
            return Err(GameError::new(
                GameErrorCode::Blocked,
                "Transport destination is blocked.",
            ));
        }
        runtime::interrupt(character)?;
        character.runtime.pending_travel = Some(PendingTravel {
            travel: id.clone(),
            origin: runtime::location(character),
            destination,
            started_at_tick: world.tick,
            completes_at_tick: runtime::deadline(world.tick, u64::from(duration))?,
        });
        if matches!(
            cooldown_start,
            CooldownStart::Accepted | CooldownStart::Launched
        ) {
            character.runtime.travel_cooldowns.insert(
                id.clone(),
                runtime::deadline(world.tick, u64::from(cooldown))?,
            );
        }
        let mut events = vec![GameEvent::Teleport {
            travel: id.clone(),
            phase: TeleportPhase::Started,
        }];
        if duration == 0 {
            events.extend(self.advance_travel(world, character, rng)?);
        }
        Ok(events)
    }

    pub(crate) fn interrupt_travel(
        &self,
        character: &mut CharacterState,
        cause: InterruptionCause,
    ) -> GameResult<Vec<GameEvent>> {
        let Some(pending) = &character.runtime.pending_travel else {
            return Ok(vec![]);
        };
        let definition = self
            .content
            .mechanics
            .travels
            .get(&pending.travel)
            .ok_or_else(|| unknown("Unknown pending travel."))?;
        if !definition.interruptions.contains(&cause) {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "This action cannot interrupt the current source transport.",
            ));
        }
        let id = pending.travel.clone();
        character.runtime.pending_travel = None;
        Ok(vec![GameEvent::Teleport {
            travel: id,
            phase: TeleportPhase::Interrupted { reason: cause },
        }])
    }

    pub(crate) fn advance_travel(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        let Some(pending) = character.runtime.pending_travel.clone() else {
            return Ok(vec![]);
        };
        if pending.completes_at_tick > world.tick {
            return Ok(vec![]);
        }
        let definition = self
            .content
            .mechanics
            .travels
            .get(&pending.travel)
            .ok_or_else(|| unknown("Unknown pending travel."))?;
        self.require_guard(world, character, &definition.guard)?;
        if runtime::location(character) != pending.origin {
            return Err(invalid_state(
                "Pending transport origin changed without interruption.",
            ));
        }
        let leaving_office = matches!(character.runtime.life, LifeState::FirstDeathOffice { .. });
        if leaving_office {
            let policy = self
                .content
                .mechanics
                .death
                .as_ref()
                .ok_or_else(|| unknown("Missing first-Office policy."))?;
            if !policy
                .required_topics
                .is_subset(&character.runtime.death_topics)
            {
                return Err(GameError::new(
                    GameErrorCode::RequirementNotMet,
                    "Complete every required Death topic before leaving.",
                ));
            }
        }
        self.move_to(world, character, pending.destination.clone())?;
        let completed = GameEvent::Teleport {
            travel: pending.travel.clone(),
            phase: TeleportPhase::Completed {
                origin: pending.origin,
                destination: pending.destination,
            },
        };
        let mut frame = EffectFrame {
            trigger: Some(completed.clone()),
            ..EffectFrame::default()
        };
        self.effects(
            world,
            character,
            &definition.completion_effects,
            rng,
            &mut frame,
        )?;
        if matches!(
            definition.cooldown_start.require()?,
            CooldownStart::Completed
        ) {
            character.runtime.travel_cooldowns.insert(
                pending.travel,
                runtime::deadline(world.tick, u64::from(*definition.cooldown_ticks.require()?))?,
            );
        }
        character.runtime.pending_travel = None;
        if leaving_office {
            self.finish_office_exit(world, character)?;
        }
        frame.events.push(completed);
        Ok(frame.events)
    }

    pub(crate) fn create_temporary(
        &self,
        world: &mut WorldState,
        character: &CharacterState,
        id: &TemporaryObjectId,
        rng: &mut impl RandomSource,
    ) -> GameResult<GameEvent> {
        let definition = self
            .content
            .mechanics
            .temporary_objects
            .get(id)
            .ok_or_else(|| unknown("Unknown temporary object definition."))?;
        self.require_guard(world, character, &definition.placement_guard)?;
        self.validate_temporary_placement(world, character, id)?;
        let lifetime = runtime::duration(definition.lifetime.require()?, rng)?;
        let policy = self
            .content
            .mechanics
            .ground_policies
            .get(&definition.ground_policy)
            .ok_or_else(|| unknown("Unknown expiry ground policy."))?;
        policy.public_after.require()?;
        policy.expires_after.require()?;
        let object = DynamicObjectId::new(format!(
            "dynamic_object.engine.{}.{}",
            world.tick,
            world.runtime.temporary_objects.len()
        ))?;
        if world.runtime.temporary_objects.contains_key(&object) {
            return Err(invalid_state("Dynamic object ID collision."));
        }
        world.runtime.temporary_objects.insert(
            object.clone(),
            DynamicObject {
                definition: id.clone(),
                owner: character.actor_id.clone(),
                location: runtime::location(character),
                created_at_tick: world.tick,
                expires_at_tick: runtime::deadline(world.tick, lifetime)?,
            },
        );
        Ok(GameEvent::TemporaryObjectCreated {
            object,
            definition: id.clone(),
            tile: character.tile,
        })
    }

    pub(crate) fn validate_temporary_placement(
        &self,
        world: &WorldState,
        character: &CharacterState,
        id: &TemporaryObjectId,
    ) -> GameResult<()> {
        let definition = self
            .content
            .mechanics
            .temporary_objects
            .get(id)
            .ok_or_else(|| unknown("Unknown temporary object."))?;
        self.require_guard(world, character, &definition.placement_guard)?;
        let object = self
            .content
            .objects
            .get(&definition.object)
            .ok_or_else(|| unknown("Unknown temporary footprint."))?;
        let map = self.collision_for(world, character.runtime.instance.as_ref())?;
        for existing in world
            .runtime
            .temporary_objects
            .values()
            .filter(|existing| existing.location.instance == character.runtime.instance)
        {
            let kind = self
                .content
                .mechanics
                .temporary_objects
                .get(&existing.definition)
                .ok_or_else(|| unknown("Unknown occupied temporary definition."))?;
            let geometry = self
                .content
                .objects
                .get(&kind.object)
                .ok_or_else(|| unknown("Unknown occupied temporary footprint."))?;
            let a = character.tile;
            let b = existing.location.tile;
            if a.plane() == b.plane()
                && u32::from(a.x()) < u32::from(b.x()) + u32::from(geometry.size_x)
                && u32::from(b.x()) < u32::from(a.x()) + u32::from(object.size_x)
                && u32::from(a.y()) < u32::from(b.y()) + u32::from(geometry.size_y)
                && u32::from(b.y()) < u32::from(a.y()) + u32::from(object.size_y)
            {
                return Err(GameError::new(
                    GameErrorCode::Blocked,
                    "Temporary-object footprints overlap.",
                ));
            }
        }
        for dx in 0..object.size_x {
            for dy in 0..object.size_y {
                let tile = character
                    .tile
                    .offset(i16::from(dx), i16::from(dy))
                    .ok_or_else(|| invalid_content("Temporary footprint overflow."))?;
                if !map.cell(tile).is_some_and(|cell| cell.walkable) {
                    return Err(GameError::new(
                        GameErrorCode::Blocked,
                        "Temporary-object placement is blocked or already occupied.",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn put_ground(
        &self,
        world: &mut WorldState,
        stack: ItemStack,
        owner: &ActorId,
        location: &RuntimeLocation,
        policy_id: &GroundPolicyId,
    ) -> GameResult<String> {
        self.put_ground_from(
            world,
            stack,
            owner,
            location,
            policy_id,
            GroundProducer::Activity {
                actor: owner.clone(),
                at_tick: world.tick,
            },
        )
    }

    pub(crate) fn put_ground_from(
        &self,
        world: &mut WorldState,
        stack: ItemStack,
        owner: &ActorId,
        location: &RuntimeLocation,
        policy_id: &GroundPolicyId,
        producer: GroundProducer,
    ) -> GameResult<String> {
        let policy = self
            .content
            .mechanics
            .ground_policies
            .get(policy_id)
            .ok_or_else(|| unknown("Unknown ground policy."))?;
        let public_at_tick = match policy.public_after.require()? {
            Some(ticks) => runtime::deadline(world.tick, u64::from(*ticks))?,
            None => u64::MAX,
        };
        let expires_at_tick = match policy.expires_after.require()? {
            Some(ticks) => runtime::deadline(world.tick, u64::from(*ticks))?,
            None => u64::MAX,
        };
        let clock = ground_clock(policy, &producer)?;
        if world.ground_items.len() >= 32_768 {
            return Err(GameError::new(
                GameErrorCode::InventoryFull,
                "Ground capacity is full.",
            ));
        }
        world.runtime.next_ground_id = runtime::deadline(world.runtime.next_ground_id, 1)?;
        let id = format!("ground.engine.{}", world.runtime.next_ground_id);
        if world.ground_items.iter().any(|item| item.id == id) {
            return Err(invalid_state("Ground identity counter requires migration."));
        }
        world.runtime.ground_provenance.insert(
            id.clone(),
            GroundProvenance {
                policy: policy_id.clone(),
                producer,
                clock: Some(clock),
            },
        );
        world.ground_items.push(GroundItem {
            id: id.clone(),
            tile: location.tile,
            stack,
            owner: Some(owner.clone()),
            public_at_tick,
            expires_at_tick,
            instance: location.instance.clone(),
        });
        Ok(id)
    }

    pub(crate) fn advance_ground_clocks(
        &self,
        world: &mut WorldState,
        context: &crate::TickContext,
    ) -> GameResult<()> {
        for item in &mut world.ground_items {
            let Some(provenance) = world.runtime.ground_provenance.get_mut(&item.id) else {
                continue;
            };
            let clock = match provenance.clock {
                Some(clock) => clock,
                None => {
                    let policy = self
                        .content
                        .mechanics
                        .ground_policies
                        .get(&provenance.policy)
                        .ok_or_else(|| unknown("Unknown persisted ground policy."))?;
                    let clock = ground_clock(policy, &provenance.producer)?;
                    provenance.clock = Some(clock);
                    clock
                }
            };
            if clock != GroundClock::OwnerOnlineTicks {
                continue;
            }
            let owner = item
                .owner
                .as_ref()
                .ok_or_else(|| invalid_state("Owner clock has no owner."))?;
            let presence = context.actors.get(owner).ok_or_else(|| {
                unavailable("Ground-item active clocks require authoritative owner presence.")
            })?;
            if !presence.online {
                if item.public_at_tick != u64::MAX {
                    item.public_at_tick = runtime::deadline(item.public_at_tick, 1)?;
                }
                if item.expires_at_tick != u64::MAX {
                    item.expires_at_tick = runtime::deadline(item.expires_at_tick, 1)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn expire_objects(&self, world: &mut WorldState) -> GameResult<()> {
        let expired: Vec<_> = world
            .runtime
            .temporary_objects
            .iter()
            .filter(|(_, object)| object.expires_at_tick <= world.tick)
            .map(|(id, object)| (id.clone(), object.clone()))
            .collect();
        for (id, object) in expired {
            let definition = self
                .content
                .mechanics
                .temporary_objects
                .get(&object.definition)
                .ok_or_else(|| unknown("Unknown expiring object."))?;
            for item in &definition.expired_items {
                self.put_ground(
                    world,
                    item.clone(),
                    &object.owner,
                    &object.location,
                    &definition.ground_policy,
                )?;
            }
            world.runtime.temporary_objects.remove(&id);
        }
        Ok(())
    }
}

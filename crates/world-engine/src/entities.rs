use std::collections::BTreeMap;

use clubscape_game_types::*;

use crate::{
    ActorEvent, RandomSource, WorldEngine, combat::npc_stat, invalid_content, invalid_state,
    navigation::ROUTE_ORDER, permissions::touch_edge, random, runtime, source_math, tag,
    unavailable, unknown,
};

impl WorldEngine {
    pub(crate) fn initial_entity(&self, spawn: &SpawnDefinition) -> GameResult<EntityState> {
        let hitpoints = match &spawn.kind {
            SpawnKind::Npc { npc } => self
                .content
                .npcs
                .get(npc)
                .ok_or_else(|| unknown("Unknown initial NPC."))?
                .combat
                .as_ref()
                .map_or(0, |combat| combat.hitpoints),
            _ => 0,
        };
        Ok(EntityState {
            tile: spawn.tile,
            hitpoints,
            available_at_tick: 0,
            flags: BTreeMap::new(),
            runtime: EntityRuntime::default(),
        })
    }

    pub(crate) fn advance_entities(
        &self,
        world: &mut WorldState,
        rng: &mut impl RandomSource,
    ) -> GameResult<()> {
        let mut locations = vec![None];
        locations.extend(world.runtime.instances.keys().cloned().map(Some));
        for instance in locations {
            let ids: Vec<_> = match &instance {
                Some(id) => world.runtime.instances[id]
                    .entities
                    .keys()
                    .cloned()
                    .collect(),
                None => world.entities.keys().cloned().collect(),
            };
            for id in ids {
                let definition = self
                    .content
                    .spawns
                    .get(&id)
                    .ok_or_else(|| unknown("Unknown entity definition."))?;
                let entity = runtime::entity(world, instance.as_ref(), &id)?.clone();
                if entity.available_at_tick != 0 && entity.available_at_tick <= world.tick {
                    let mut state = self.initial_entity(definition)?;
                    state.tile =
                        self.instance_spawn_origin(world, instance.as_ref(), definition)?;
                    if let SpawnKind::Npc { .. } = definition.kind {
                        state.runtime.life = entity
                            .runtime
                            .life
                            .checked_add(1)
                            .ok_or_else(|| invalid_state("NPC life overflow."))?;
                    }
                    if let SpawnKind::Item { stack, .. } = &definition.kind {
                        let mut item = self.spawn_ground_item(&id, state.tile, stack, world.tick);
                        item.instance = instance.clone();
                        if let Some(instance) = &instance {
                            item.id = format!("source:{id}:{}:{instance}", world.tick);
                        }
                        world.ground_items.push(item);
                    }
                    *runtime::entity_mut(world, instance.as_ref(), &id)? = state;
                }
                if runtime::entity(world, instance.as_ref(), &id)?.available_at_tick > world.tick {
                    continue;
                }
                for interaction in &definition.interactions {
                    if let InteractionAction::Gather { rule } = &interaction.action
                        && let Some(relocation) =
                            rule.mechanics.as_ref().and_then(|m| m.relocation.as_ref())
                    {
                        let policy = relocation.require()?;
                        let state = runtime::entity(world, instance.as_ref(), &id)?;
                        if state.runtime.next_movement_tick.is_none() {
                            let deadline = runtime::deadline(
                                world.tick,
                                runtime::duration(&policy.interval, rng)?,
                            )?;
                            runtime::entity_mut(world, instance.as_ref(), &id)?
                                .runtime
                                .next_movement_tick = Some(deadline);
                        } else if state
                            .runtime
                            .next_movement_tick
                            .is_some_and(|tick| tick <= world.tick)
                        {
                            let current = state.tile;
                            let candidates: Vec<_> = policy
                                .locations
                                .iter()
                                .map(|id| {
                                    let spawn = self
                                        .content
                                        .spawns
                                        .get(id)
                                        .ok_or_else(|| unknown("Unknown relocation anchor."))?;
                                    self.map_instance_tile(world, instance.as_ref(), spawn.tile)
                                })
                                .collect::<GameResult<Vec<_>>>()?
                                .into_iter()
                                .filter(|tile| !policy.exclude_current || *tile != current)
                                .collect();
                            if candidates.is_empty() {
                                return Err(invalid_content(
                                    "Relocation has no eligible source location.",
                                ));
                            }
                            let tile =
                                candidates[random::draw(rng, candidates.len() as u32)? as usize];
                            let deadline = runtime::deadline(
                                world.tick,
                                runtime::duration(&policy.interval, rng)?,
                            )?;
                            let state = runtime::entity_mut(world, instance.as_ref(), &id)?;
                            state.tile = tile;
                            state.runtime.next_movement_tick = Some(deadline);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn advance_npcs(
        &self,
        world: &mut WorldState,
        rng: &mut impl RandomSource,
        context: &crate::TickContext,
    ) -> GameResult<Vec<ActorEvent>> {
        let mut events = Vec::new();
        let mut locations = vec![None];
        locations.extend(world.runtime.instances.keys().cloned().map(Some));
        for instance in locations {
            let ids: Vec<_> = match &instance {
                Some(id) => world.runtime.instances[id]
                    .entities
                    .keys()
                    .cloned()
                    .collect(),
                None => world.entities.keys().cloned().collect(),
            };
            for id in ids {
                let spawn = self
                    .content
                    .spawns
                    .get(&id)
                    .ok_or_else(|| unknown("Unknown NPC spawn."))?;
                let SpawnKind::Npc { npc: npc_id } = &spawn.kind else {
                    continue;
                };
                let definition = self
                    .content
                    .npcs
                    .get(npc_id)
                    .ok_or_else(|| unknown("Unknown NPC."))?;
                self.update_engagement(world, instance.as_ref(), &id, definition, context)?;
                let entity = runtime::entity(world, instance.as_ref(), &id)?.clone();
                if entity.available_at_tick > world.tick
                    || definition.combat.is_some() && entity.hitpoints == 0
                {
                    continue;
                }
                if entity.runtime.returning_to_spawn {
                    continue;
                }
                if let Some(actor) = &entity.runtime.retaliation_target {
                    if context.actors.get(actor).is_some_and(|facts| !facts.online) {
                        runtime::entity_mut(world, instance.as_ref(), &id)?
                            .runtime
                            .retaliation_target = None;
                        continue;
                    }
                    let Some(mut character) = world.characters.remove(actor) else {
                        return Err(unavailable(
                            "Retaliation target must remain loaded or be explicitly disengaged by authority.",
                        ));
                    };
                    if character.runtime.instance != instance
                        || character.hitpoints == 0
                        || !matches!(character.runtime.life, LifeState::Alive | LifeState::Legacy)
                    {
                        runtime::entity_mut(world, instance.as_ref(), &id)?
                            .runtime
                            .retaliation_target = None;
                        world.characters.insert(actor.clone(), character);
                        continue;
                    }
                    let before = character.clone();
                    let combat = definition
                        .combat
                        .as_ref()
                        .ok_or_else(|| invalid_state("Noncombat NPC has a retaliation target."))?;
                    let mechanics = combat
                        .mechanics
                        .as_ref()
                        .ok_or_else(|| unavailable("Retaliation needs typed NPC combat."))?;
                    let map = self.collision_for(world, instance.as_ref())?;
                    let in_range = npc_reach(
                        &map,
                        entity.tile,
                        definition.size,
                        character.tile,
                        mechanics,
                    )?;
                    let mut own = Vec::new();
                    if !in_range
                        && let NpcNavigation::Mobile { step_ticks, .. } = &definition.navigation
                        && entity
                            .runtime
                            .next_movement_tick
                            .is_none_or(|tick| tick <= world.tick)
                    {
                        let path = match crate::navigation::route(&map, entity.tile, character.tile)
                        {
                            Ok(path) => Some(path),
                            Err(error)
                                if matches!(
                                    error.code,
                                    GameErrorCode::OutOfReach | GameErrorCode::Blocked
                                ) =>
                            {
                                None
                            }
                            Err(error) => return Err(error),
                        };
                        let mut next_step = None;
                        if let Some(path) = path
                            && let Some(next) = path.first().copied()
                            && !footprint_tiles(next, definition.size)?.contains(&character.tile)
                            && self.npc_can_step(
                                world,
                                &instance,
                                &id,
                                definition,
                                (entity.tile, next),
                                (context, Some(&character)),
                            )?
                        {
                            next_step = Some(next);
                        }
                        if next_step.is_none() {
                            for direction in ROUTE_ORDER {
                                let (dx, dy) = direction.offset();
                                if let Some(next) = entity.tile.offset(dx, dy)
                                    && crate::combat::melee_adjacent(
                                        character.tile,
                                        next,
                                        definition.size,
                                        definition.size,
                                    )
                                    && self.npc_can_step(
                                        world,
                                        &instance,
                                        &id,
                                        definition,
                                        (entity.tile, next),
                                        (context, Some(&character)),
                                    )?
                                {
                                    next_step = Some(next);
                                    break;
                                }
                            }
                        }
                        if let Some(next) = next_step {
                            runtime::entity_mut(world, instance.as_ref(), &id)?.tile = next;
                        }
                        let period = *step_ticks.require()?;
                        if period == 0 {
                            return Err(invalid_content("NPC movement period is zero."));
                        }
                        let deadline = runtime::deadline(world.tick, u64::from(period))?;
                        runtime::entity_mut(world, instance.as_ref(), &id)?
                            .runtime
                            .next_movement_tick = Some(deadline);
                    }
                    let current_tile = runtime::entity(world, instance.as_ref(), &id)?.tile;
                    if entity.runtime.attack_ready <= world.tick
                        && npc_reach(
                            &map,
                            current_tile,
                            definition.size,
                            character.tile,
                            mechanics,
                        )?
                    {
                        mechanics.accuracy.require()?;
                        mechanics.negative_rolls.require()?;
                        let effective = (i64::from(npc_stat(combat, mechanics.attack_stat))
                            + i64::from(*mechanics.effective_level_bonus.require()?))
                        .max(0);
                        let attack = source_math::maximum_accuracy_roll(
                            effective as u32,
                            i32::from(
                                *combat
                                    .bonuses
                                    .attack
                                    .get(&mechanics.attack_type)
                                    .unwrap_or(&0),
                            ),
                        )?;
                        let style = character.runtime.combat.style.as_ref().and_then(|id| self.content.mechanics.combat_styles.get(id))
                            .ok_or_else(|| unavailable("Player defence requires a selected source style; no unarmed default style is declared."))?;
                        let defence = self.maximum_player_roll(
                            &character,
                            &style.defence,
                            mechanics.attack_type,
                            true,
                        )?;
                        let hit = source_math::opposed_accuracy(attack, defence, rng)?;
                        let rolled = if hit {
                            random::draw(rng, u32::from(combat.max_hit) + 1)? as u16
                        } else {
                            0
                        };
                        let policy = mechanics.damage.require()?;
                        let raw = if hit {
                            rolled.max(policy.successful_minimum)
                        } else {
                            0
                        };
                        let stage = self
                            .content
                            .tutorial
                            .get(&character.tutorial_stage)
                            .ok_or_else(|| unknown("Unknown current tutorial stage."))?;
                        let damage = source_math::incoming_damage(
                            character.hitpoints,
                            raw,
                            stage.nonfatal_combat,
                        );
                        let deadline =
                            runtime::deadline(world.tick, u64::from(combat.attack_speed_ticks))?;
                        runtime::entity_mut(world, instance.as_ref(), &id)?
                            .runtime
                            .attack_ready = deadline;
                        character.runtime.combat.last_attacker = Some(id.clone());
                        character.runtime.combat.last_combat_tick = Some(world.tick);
                        runtime::entity_mut(world, instance.as_ref(), &id)?
                            .runtime
                            .last_combat_tick = Some(world.tick);
                        if character.runtime.pending_travel.is_some() {
                            own.extend(
                                self.interrupt_travel(&mut character, InterruptionCause::Combat)?,
                            );
                        }
                        character.hitpoints -= damage;
                        if character.hitpoints == 0 {
                            own.extend(self.die(world, &mut character)?);
                        } else {
                            own.push(GameEvent::Message {
                                text: format!("{} dealt {damage} damage.", definition.name),
                            });
                            if !matches!(character.activity, Activity::Fighting { .. }) {
                                runtime::interrupt(&mut character)?;
                                if character.runtime.settings.auto_retaliate == Some(true) {
                                    let style = character.runtime.combat.style.clone().ok_or_else(
                                        || unavailable("Auto-retaliation has no source style."),
                                    )?;
                                    character.runtime.combat.target = Some(id.clone());
                                    character.activity = Activity::Fighting {
                                        target: id.clone(),
                                        style: style.to_string(),
                                        next_tick: character
                                            .runtime
                                            .combat
                                            .attack_ready
                                            .max(runtime::deadline(world.tick, 1)?),
                                    };
                                }
                            }
                        }
                    }
                    self.progress(world, &mut character, &before, &mut own, rng)?;
                    self.check_reward_atomicity(&before, &character)?;
                    self.session_close_event(&before, &character, &mut own)?;
                    world.characters.insert(actor.clone(), character);
                    events.extend(tag(actor, own));
                } else if let NpcNavigation::Mobile {
                    wander_radius,
                    step_ticks,
                    ..
                } = &definition.navigation
                    && *wander_radius > 0
                    && entity
                        .runtime
                        .next_movement_tick
                        .is_none_or(|tick| tick <= world.tick)
                {
                    let origin = self.instance_spawn_origin(world, instance.as_ref(), spawn)?;
                    let mut candidates = Vec::new();
                    for direction in ROUTE_ORDER {
                        let (dx, dy) = direction.offset();
                        if let Some(tile) = entity.tile.offset(dx, dy)
                            && origin
                                .distance(tile)
                                .is_some_and(|distance| distance <= *wander_radius)
                            && self.npc_can_step(
                                world,
                                &instance,
                                &id,
                                definition,
                                (entity.tile, tile),
                                (context, None),
                            )?
                        {
                            candidates.push(tile);
                        }
                    }
                    let interval = *step_ticks.require()?;
                    if interval == 0 {
                        return Err(invalid_content("NPC movement period is zero."));
                    }
                    let deadline = runtime::deadline(world.tick, u64::from(interval))?;
                    let tile = if candidates.is_empty() {
                        entity.tile
                    } else {
                        candidates[random::draw(rng, candidates.len() as u32)? as usize]
                    };
                    let entity = runtime::entity_mut(world, instance.as_ref(), &id)?;
                    entity.tile = tile;
                    entity.runtime.next_movement_tick = Some(deadline);
                }
            }
        }
        Ok(events)
    }

    pub(crate) fn npc_can_step(
        &self,
        world: &WorldState,
        instance: &Option<InstanceId>,
        id: &SpawnId,
        definition: &NpcDefinition,
        edge: (Tile, Tile),
        actors: (&crate::TickContext, Option<&CharacterState>),
    ) -> GameResult<bool> {
        let (from, to) = edge;
        let (context, actor) = actors;
        let map = self.collision_for(world, instance.as_ref())?;
        for dx in 0..definition.size {
            for dy in 0..definition.size {
                let from = from
                    .offset(i16::from(dx), i16::from(dy))
                    .ok_or_else(|| invalid_content("NPC footprint overflow."))?;
                let to = to
                    .offset(i16::from(dx), i16::from(dy))
                    .ok_or_else(|| invalid_content("NPC footprint overflow."))?;
                if !map.can_step(from, to) {
                    return Ok(false);
                }
            }
        }
        if matches!(
            definition.navigation,
            NpcNavigation::Mobile {
                clip: NpcClipPolicy::MovementAndActors,
                ..
            }
        ) {
            let cells = footprint_tiles(to, definition.size)?;
            if world.characters.values().chain(actor).any(|actor| {
                &actor.runtime.instance == instance
                    && cells.contains(&actor.tile)
                    && context
                        .actors
                        .get(&actor.actor_id)
                        .is_some_and(|presence| presence.online)
            }) {
                return Ok(false);
            }
            let entities = match instance {
                Some(id) => &world.runtime.instances[id].entities,
                None => &world.entities,
            };
            for (other_id, entity) in entities {
                if other_id == id || entity.available_at_tick > world.tick {
                    continue;
                }
                if let Some(SpawnDefinition {
                    kind: SpawnKind::Npc { npc },
                    ..
                }) = self.content.spawns.get(other_id)
                {
                    let other = self
                        .content
                        .npcs
                        .get(npc)
                        .ok_or_else(|| unknown("Unknown collision NPC."))?;
                    if footprint_tiles(entity.tile, other.size)?
                        .iter()
                        .any(|tile| cells.contains(tile))
                    {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }

    pub(crate) fn dispatch_kill_credit(
        &self,
        world: &mut WorldState,
        instance: Option<&InstanceId>,
        events: &[GameEvent],
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<ActorEvent>> {
        let mut result = Vec::new();
        for event in events {
            let GameEvent::NpcKilled {
                target,
                npc,
                life,
                method: _,
                credited: false,
                tile,
            } = event
            else {
                continue;
            };
            let entity = runtime::entity(world, instance, target)?;
            if entity.runtime.life != *life {
                return Err(invalid_state("Kill-credit life changed."));
            }
            let resolution = entity
                .runtime
                .kill
                .as_ref()
                .ok_or_else(|| invalid_state("Kill has no persisted resolution."))?;
            let actor = resolution.credited.clone();
            let credited_method = resolution.method;
            let mut character = world.characters.remove(&actor).ok_or_else(|| {
                unavailable("Credited actor state must be loaded for progression.")
            })?;
            let before = character.clone();
            let mut own = vec![GameEvent::NpcKilled {
                target: target.clone(),
                npc: npc.clone(),
                life: *life,
                method: credited_method,
                credited: true,
                tile: *tile,
            }];
            self.progress(world, &mut character, &before, &mut own, rng)?;
            self.check_reward_atomicity(&before, &character)?;
            world.characters.insert(actor.clone(), character);
            result.extend(tag(&actor, own));
        }
        Ok(result)
    }
}

fn footprint_tiles(origin: Tile, size: u8) -> GameResult<Vec<Tile>> {
    let mut cells = Vec::new();
    for x in 0..size {
        for y in 0..size {
            cells.push(
                origin
                    .offset(i16::from(x), i16::from(y))
                    .ok_or_else(|| invalid_content("NPC footprint overflow."))?,
            );
        }
    }
    Ok(cells)
}

fn npc_reach(
    map: &clubscape_simulation::navigation::CollisionMap,
    origin: Tile,
    size: u8,
    target: Tile,
    mechanics: &NpcCombatMechanics,
) -> GameResult<bool> {
    let cardinal = !matches!(
        mechanics.attack_type,
        AttackType::Stab | AttackType::Slash | AttackType::Crush
    ) || mechanics.reach != 1
        || crate::combat::melee_adjacent(target, origin, size, size);
    Ok(cardinal
        && footprint_tiles(origin, size)?.into_iter().any(|tile| {
            tile.distance(target)
                .is_some_and(|distance| distance <= mechanics.reach)
                && map.line_of_sight(tile, target)
                && (mechanics.reach > 1 || touch_edge(map, tile, target))
        }))
}

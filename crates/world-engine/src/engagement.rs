use clubscape_game_types::*;

use crate::{TickContext, WorldEngine, invalid_content, runtime, unavailable, unknown};

impl WorldEngine {
    pub(crate) fn update_engagement(
        &self,
        world: &mut WorldState,
        instance: Option<&InstanceId>,
        id: &SpawnId,
        definition: &NpcDefinition,
        context: &TickContext,
    ) -> GameResult<()> {
        let Some(combat) = &definition.combat else {
            return Ok(());
        };
        let Some(mechanics) = &combat.mechanics else {
            return Ok(());
        };
        let current = runtime::entity(world, instance, id)?.clone();
        if current.hitpoints == 0 || current.available_at_tick > world.tick {
            return Ok(());
        }
        if current.runtime.retaliation_target.is_none()
            && !current.runtime.returning_to_spawn
            && !combat.aggressive
        {
            return Ok(());
        }
        let policy = mechanics.engagement.require()?;
        if policy.inactivity_ticks == 0 || policy.leash_range == 0 {
            return Err(invalid_content(
                "Engagement policy requires positive timeout/leash.",
            ));
        }
        let spawn = self
            .content
            .spawns
            .get(id)
            .ok_or_else(|| unknown("Unknown engagement spawn."))?;
        let origin = self.instance_spawn_origin(world, instance, spawn)?;
        if let Some(actor) = &current.runtime.retaliation_target {
            let last = current.runtime.last_combat_tick.ok_or_else(|| {
                unavailable("Legacy active engagement needs a migrated last-contact tick.")
            })?;
            let lost = world.characters.get(actor).is_none_or(|character| {
                character.hitpoints == 0
                    || character.runtime.instance.as_ref() != instance
                    || !matches!(character.runtime.life, LifeState::Alive)
                    || origin
                        .distance(character.tile)
                        .is_none_or(|distance| distance > policy.leash_range)
                    || context
                        .actors
                        .get(actor)
                        .is_some_and(|presence| !presence.online)
            });
            let timed_out =
                world.tick >= runtime::deadline(last, u64::from(policy.inactivity_ticks))?;
            if lost || timed_out {
                let ready = runtime::deadline(world.tick, u64::from(policy.reacquire_delay_ticks))?;
                let entity = runtime::entity_mut(world, instance, id)?;
                entity.runtime.retaliation_target = None;
                entity.runtime.aggression_ready = ready;
                entity.runtime.returning_to_spawn = policy.return_to_spawn;
                if let Some(character) = world.characters.get_mut(actor)
                    && character.runtime.combat.target.as_ref() == Some(id)
                    && !matches!(
                        character.activity,
                        Activity::Fighting { .. } | Activity::Casting { .. }
                    )
                {
                    character.runtime.combat.target = None;
                }
            }
        }
        let current = runtime::entity(world, instance, id)?.clone();
        if current.runtime.returning_to_spawn {
            if current.tile != origin {
                let NpcNavigation::Mobile { step_ticks, .. } = &definition.navigation else {
                    return Err(invalid_content(
                        "A displaced stationary NPC cannot walk to its origin.",
                    ));
                };
                if current
                    .runtime
                    .next_movement_tick
                    .is_none_or(|tick| tick <= world.tick)
                {
                    let map = self.collision_for(world, instance)?;
                    let route = match crate::navigation::route(&map, current.tile, origin) {
                        Ok(route) => route,
                        Err(error)
                            if matches!(
                                error.code,
                                GameErrorCode::Blocked | GameErrorCode::OutOfReach
                            ) =>
                        {
                            vec![]
                        }
                        Err(error) => return Err(error),
                    };
                    if let Some(next) = route.first().copied()
                        && self.npc_can_step(
                            world,
                            &instance.cloned(),
                            id,
                            definition,
                            (current.tile, next),
                            (context, None),
                        )?
                    {
                        runtime::entity_mut(world, instance, id)?.tile = next;
                    }
                    let period = *step_ticks.require()?;
                    if period == 0 {
                        return Err(invalid_content("NPC return cadence is zero."));
                    }
                    let ready = runtime::deadline(world.tick, u64::from(period))?;
                    runtime::entity_mut(world, instance, id)?
                        .runtime
                        .next_movement_tick = Some(ready);
                }
            }
            if runtime::entity(world, instance, id)?.tile == origin {
                let entity = runtime::entity_mut(world, instance, id)?;
                entity.runtime.returning_to_spawn = false;
                if policy.reset_life_on_return {
                    entity.hitpoints = combat.hitpoints;
                    entity.runtime.life = runtime::deadline(entity.runtime.life, 1)?;
                    entity.runtime.contributions.clear();
                    entity.runtime.kill = None;
                    entity.runtime.loot_resolved = false;
                }
            }
            return Ok(());
        }
        let current = runtime::entity(world, instance, id)?;
        if current.runtime.retaliation_target.is_some()
            || !combat.aggressive
            || current.runtime.aggression_ready > world.tick
        {
            return Ok(());
        }
        let aggression = policy
            .aggression
            .as_ref()
            .ok_or_else(|| invalid_content("Aggressive NPC lacks an acquisition policy."))?;
        let map = self.collision_for(world, instance)?;
        let mut eligible = Vec::new();
        for (actor, character) in &world.characters {
            if character.runtime.instance.as_ref() != instance
                || character.hitpoints == 0
                || !matches!(character.runtime.life, LifeState::Alive)
            {
                continue;
            }
            let distance = current.tile.distance(character.tile);
            if distance.is_none_or(|distance| distance > aggression.acquisition_range)
                || origin
                    .distance(character.tile)
                    .is_none_or(|distance| distance > policy.leash_range)
            {
                continue;
            }
            let presence = context.actors.get(actor).ok_or_else(|| {
                unavailable("Aggression requires authority-owned online presence.")
            })?;
            if !presence.online
                || aggression.require_line_of_sight
                    && !map.line_of_sight(current.tile, character.tile)
                || !self.guard(world, character, &aggression.guard, None)?
            {
                continue;
            }
            eligible.push((distance.unwrap_or(0), actor.clone()));
        }
        eligible.sort();
        if let Some((_, actor)) = eligible.first() {
            let now = world.tick;
            let ready = runtime::deadline(now, u64::from(policy.acquire_delay_ticks))?;
            let entity = runtime::entity_mut(world, instance, id)?;
            entity.runtime.retaliation_target = Some(actor.clone());
            entity.runtime.last_combat_tick = Some(now);
            entity.runtime.attack_ready = entity.runtime.attack_ready.max(ready);
        }
        Ok(())
    }
}

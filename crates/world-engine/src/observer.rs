use clubscape_game_types::*;

use crate::{ActorEvent, WorldEngine, invalid_state, runtime, unknown};

fn same_action(left: &ObservedAction, right: &ObservedAction) -> bool {
    left.activity == right.activity
        && left.action_id == right.action_id
        && left.target == right.target
        && left.recipe_id == right.recipe_id
        && left.style_id == right.style_id
        && left.spell_id == right.spell_id
}

impl WorldEngine {
    pub fn actor_observer(
        &self,
        world: &WorldState,
        actor: &ActorId,
    ) -> GameResult<ActorObserverView> {
        let character = world
            .characters
            .get(actor)
            .ok_or_else(|| GameError::new(GameErrorCode::NotOwned, "Unknown observed actor."))?;
        let Some(observation) = &character.runtime.observation else {
            return Ok(ActorObserverView {
                running: false,
                movement_tick: None,
                action: None,
            });
        };
        observation.validate(world.tick)?;
        let movement = observation.movement.as_ref().filter(|motion| {
            motion.tick == world.tick
                && motion.to == character.tile
                && motion.instance == character.runtime.instance
        });
        let current = self.current_actor_action(character)?;
        let action = observation
            .action
            .as_ref()
            .filter(|action| {
                action.completed_at_tick == Some(world.tick)
                    || (action.completed_at_tick.is_none()
                        && current
                            .as_ref()
                            .is_some_and(|(identity, _)| same_action(identity, &action.identity)))
            })
            .map(|action| ActorActionView {
                version: ACTOR_OBSERVER_VERSION,
                id: format!("actor_action.{}.{}", actor, action.ordinal),
                activity: action.identity.activity.clone(),
                action_id: action.identity.action_id.clone(),
                target: action.identity.target.clone(),
                recipe_id: action.identity.recipe_id.clone(),
                style_id: action.identity.style_id.clone(),
                spell_id: action.identity.spell_id.clone(),
                animation: action.identity.animation.clone(),
                started_at_tick: action.started_at_tick.to_string(),
                cycle_started_at_tick: action.cycle_started_at_tick.to_string(),
                next_action_tick: action.next_action_tick.map(|tick| tick.to_string()),
                observed_at_tick: world.tick.to_string(),
            });
        Ok(ActorObserverView {
            running: movement.is_some_and(|motion| motion.running),
            movement_tick: movement.map(|motion| motion.tick.to_string()),
            action,
        })
    }

    pub(crate) fn observe_world_actions(
        &self,
        before: &WorldState,
        after: &mut WorldState,
        events: &[ActorEvent],
    ) -> GameResult<()> {
        for (actor, character) in &mut after.characters {
            let previous = before
                .characters
                .get(actor)
                .ok_or_else(|| invalid_state("Observed actor identity changed."))?;
            let own: Vec<_> = events
                .iter()
                .filter(|event| &event.actor_id == actor)
                .map(|event| event.event.clone())
                .collect();
            self.observe_actor_action(after.tick, previous, character, &own)?;
        }
        Ok(())
    }

    pub(crate) fn observe_actor_action(
        &self,
        tick: u64,
        before: &CharacterState,
        after: &mut CharacterState,
        events: &[GameEvent],
    ) -> GameResult<()> {
        let prior = self.current_actor_action(before)?;
        let current = self.current_actor_action(after)?;
        let completed = current.is_none()
            && prior.is_some()
            && events.iter().any(|event| {
                matches!(
                    event,
                    GameEvent::ProductionResolved { .. }
                        | GameEvent::Produced { .. }
                        | GameEvent::Gathered { .. }
                )
            });
        let descriptor = current
            .clone()
            .or_else(|| completed.then(|| prior.clone()).flatten());
        let explicit_animation = events.iter().rev().find_map(|event| match event {
            GameEvent::Animation { target, animation } if target == after.actor_id.as_str() => {
                Some(animation.clone())
            }
            _ => None,
        });
        if let Some((mut identity, next)) = descriptor {
            if let Some(animation) = explicit_animation {
                identity.animation = Some(animation);
            }
            let recorded_by_execution = after
                .runtime
                .observation
                .as_ref()
                .and_then(|value| value.action.as_ref())
                != before
                    .runtime
                    .observation
                    .as_ref()
                    .and_then(|value| value.action.as_ref())
                && after
                    .runtime
                    .observation
                    .as_ref()
                    .and_then(|value| value.action.as_ref())
                    .is_some_and(|action| action.cycle_started_at_tick == tick);
            let new_instance = !completed
                && !recorded_by_execution
                && match (&prior, &current) {
                    (Some((old, _)), Some((new, _))) => !same_action(old, new),
                    (None, Some(_)) => true,
                    _ => false,
                };
            self.record_actor_action(tick, after, identity, next, new_instance, completed)?;
        } else if let Some(animation) = explicit_animation {
            self.record_actor_action(
                tick,
                after,
                ObservedAction {
                    activity: "animation".into(),
                    action_id: None,
                    target: None,
                    recipe_id: None,
                    style_id: None,
                    spell_id: None,
                    animation: Some(animation),
                },
                None,
                true,
                true,
            )?;
        } else if !after
            .runtime
            .observation
            .as_ref()
            .and_then(|observation| observation.action.as_ref())
            .is_some_and(|action| action.completed_at_tick == Some(tick))
            && let Some(observation) = &mut after.runtime.observation
        {
            observation.action = None;
        }
        Ok(())
    }

    pub(crate) fn record_actor_action(
        &self,
        tick: u64,
        character: &mut CharacterState,
        identity: ObservedAction,
        next_action_tick: Option<u64>,
        new_instance: bool,
        completed: bool,
    ) -> GameResult<()> {
        let observation = character
            .runtime
            .observation
            .get_or_insert_with(ActorObservation::default);
        if !new_instance
            && let Some(action) = &mut observation.action
            && same_action(&action.identity, &identity)
            && action.completed_at_tick.is_none()
        {
            if action.next_action_tick != next_action_tick {
                action.cycle_started_at_tick = tick;
            }
            action.next_action_tick = next_action_tick;
            if identity.animation.is_some() {
                action.identity.animation = identity.animation;
            }
            action.completed_at_tick = completed.then_some(tick);
        } else {
            let ordinal = observation.next_id;
            observation.next_id = ordinal
                .checked_add(1)
                .filter(|value| *value <= i64::MAX as u64)
                .ok_or_else(|| invalid_state("Actor action observation identities exhausted."))?;
            observation.action = Some(ActionObservation {
                ordinal,
                identity,
                started_at_tick: tick,
                cycle_started_at_tick: tick,
                next_action_tick,
                completed_at_tick: completed.then_some(tick),
            });
        }
        observation.validate(tick)
    }

    fn current_actor_action(
        &self,
        character: &CharacterState,
    ) -> GameResult<Option<(ObservedAction, Option<u64>)>> {
        if matches!(character.runtime.engine, EngineMetadata::Legacy) {
            let mut migrated = character.clone();
            migrated.migrate_engine_metadata(&self.content)?;
            return self.current_actor_action(&migrated);
        }
        let mut identity = ObservedAction {
            activity: String::new(),
            action_id: None,
            target: None,
            recipe_id: None,
            style_id: None,
            spell_id: None,
            animation: None,
        };
        if let Some(fire) = &character.runtime.pending_fire {
            identity.activity = "producing".into();
            self.observed_recipe(&mut identity, &fire.recipe)?;
            return Ok(Some((identity, Some(fire.next_attempt_tick))));
        }

        if let Some(travel) = &character.runtime.pending_travel {
            if let Some(action) = character.runtime.observation.as_ref().and_then(|observation| observation.action.as_ref())
                && let Some(spell) = &action.identity.spell_id
                && self.content.mechanics.spells.get(spell).is_some_and(|definition|
                    matches!(&definition.action, SpellAction::Teleport { travel: id } if id == &travel.travel))
            {
                return Ok(Some((action.identity.clone(), Some(travel.completes_at_tick))));
            }
            identity.activity = "travelling".into();
            return Ok(Some((identity, Some(travel.completes_at_tick))));
        }
        let next = match &character.activity {
            Activity::Idle | Activity::Walking { .. } => return Ok(None),
            Activity::Gathering { target, next_tick } => {
                let index = runtime::schedule(character)?
                    .gather_interaction
                    .ok_or_else(|| {
                        invalid_state("Observed gathering lost its actual interaction.")
                    })?;
                let interaction = runtime::interaction(&self.content, target, index as usize - 1)?;
                let InteractionAction::Gather { rule } = &interaction.action else {
                    return Err(invalid_state("Observed gather interaction changed kind."));
                };
                identity.activity = "gathering".into();
                identity.action_id = rule
                    .mechanics
                    .as_ref()
                    .map(|mechanics| mechanics.method.clone());
                identity.animation = rule.animation.as_ref().map(ToString::to_string);
                identity.target = Some(WorldTarget::Spawn {
                    spawn: target.clone(),
                });
                *next_tick
            }
            Activity::Producing {
                recipe,
                target,
                next_tick,
                ..
            } => {
                identity.activity = "producing".into();
                self.observed_recipe(&mut identity, recipe)?;
                identity.target = target.clone().map(|spawn| WorldTarget::Spawn { spawn });
                *next_tick
            }
            Activity::ProducingAt {
                recipe,
                target,
                next_tick,
                ..
            }
            | Activity::ProducingSelected {
                recipe,
                target,
                next_tick,
                ..
            } => {
                identity.activity = "producing".into();
                self.observed_recipe(&mut identity, recipe)?;
                identity.target = target.clone();
                *next_tick
            }
            Activity::InventoryAction {
                item,
                action,
                completes_at,
                ..
            } => {
                identity.activity = "inventory_action".into();
                let definition = self
                    .content
                    .ui
                    .as_ref()
                    .and_then(|ui| ui.item_actions.get(item))
                    .and_then(|actions| actions.iter().find(|definition| &definition.id == action))
                    .ok_or_else(|| unknown("Unknown observed source inventory action."))?;
                if let ItemUiAction::ConsumeRecipe { recipe } = &definition.action {
                    self.observed_recipe(&mut identity, recipe)?;
                }
                *completes_at
            }
            Activity::Fighting {
                target,
                style,
                next_tick,
            } => {
                identity.activity = "fighting".into();
                identity.target = Some(WorldTarget::Spawn {
                    spawn: target.clone(),
                });
                identity.style_id = self
                    .content
                    .mechanics
                    .combat_styles
                    .keys()
                    .find(|id| id.as_str() == style)
                    .cloned();
                *next_tick
            }
            Activity::Casting {
                spell,
                target,
                completes_at,
            } => {
                identity.activity = "casting".into();
                let id = SpellId::new(spell)?;
                if let Some(definition) = self.content.mechanics.spells.get(&id)
                    && let SpellAction::Combat { style, .. } = &definition.action
                {
                    identity.style_id = Some(style.clone());
                }
                identity.spell_id = Some(id);
                identity.target = target.clone().map(|spawn| WorldTarget::Spawn { spawn });
                *completes_at
            }
        };
        Ok(Some((identity, Some(next))))
    }

    fn observed_recipe(&self, identity: &mut ObservedAction, recipe: &RecipeId) -> GameResult<()> {
        let definition = self
            .content
            .recipes
            .get(recipe)
            .ok_or_else(|| unknown("Unknown observed source recipe."))?;
        identity.recipe_id = Some(recipe.clone());
        identity.action_id = definition
            .mechanics
            .as_ref()
            .map(|mechanics| mechanics.method.clone());
        Ok(())
    }
}

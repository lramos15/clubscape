use clubscape_game_types::*;
use clubscape_simulation::navigation::CollisionMap;

use crate::{WorldEngine, invalid_state, unknown};

impl WorldEngine {
    pub(crate) fn actor_can_step(
        &self,
        world: &WorldState,
        character: &CharacterState,
        map: &CollisionMap,
        from: Tile,
        to: Tile,
    ) -> GameResult<bool> {
        if !map.can_step(from, to) {
            return Ok(false);
        }
        let dx = i16::try_from(i32::from(to.x()) - i32::from(from.x()))
            .map_err(|_| invalid_state("Step overflow."))?;
        let dy = i16::try_from(i32::from(to.y()) - i32::from(from.y()))
            .map_err(|_| invalid_state("Step overflow."))?;
        let edges = if dx != 0 && dy != 0 {
            let x = from
                .offset(dx, 0)
                .ok_or_else(|| invalid_state("Diagonal step overflow."))?;
            let y = from
                .offset(0, dy)
                .ok_or_else(|| invalid_state("Diagonal step overflow."))?;
            vec![(from, x), (x, to), (from, y), (y, to)]
        } else {
            vec![(from, to)]
        };
        for definition in self.content.mechanics.traversal.values() {
            if (definition.scope == CounterScope::Instance) != character.runtime.instance.is_some()
            {
                continue;
            }
            for edge in &definition.edges {
                let mapped = self
                    .map_instance_tile(world, character.runtime.instance.as_ref(), edge.from)
                    .and_then(|a| {
                        self.map_instance_tile(world, character.runtime.instance.as_ref(), edge.to)
                            .map(|b| (a, b))
                    });
                let (a, b) = match mapped {
                    Ok(edge) => edge,
                    Err(error) if error.code == GameErrorCode::OutOfReach => continue,
                    Err(error) => return Err(error),
                };
                if edges.iter().any(|(start, end)| {
                    (*start == a && *end == b) || edge.bidirectional && *start == b && *end == a
                }) {
                    let mut entering = character.clone();
                    entering.tile = from;
                    entering.region = self
                        .regions_by_tile
                        .get(&from)
                        .ok_or_else(|| unknown("Traversal entry has no region."))?
                        .clone();
                    if !self.guard(world, &entering, &definition.guard, None)? {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }

    pub(crate) fn sync_morph_collision(
        &self,
        world: &mut WorldState,
        character: &CharacterState,
        counter: &CounterId,
    ) -> GameResult<Vec<GameEvent>> {
        let value = self.counter_value(world, character, counter)?;
        let value = match value {
            CounterValue::Integer(value) => value,
            CounterValue::Boolean(value) => i64::from(value),
        };
        let mut events = Vec::new();
        for object in self.content.objects.values() {
            let Some(morph) = &object.morph else { continue };
            if &morph.counter != counter {
                continue;
            }
            let Some(collision) = &morph.collision else {
                continue;
            };
            let link = collision.require()?;
            let selected = link
                .variants
                .iter()
                .find(|case| case.value == value)
                .map_or(&link.fallback, |case| &case.state);
            for id in link.placements.values() {
                let definition = self
                    .content
                    .mechanics
                    .object_transforms
                    .get(id)
                    .ok_or_else(|| unknown("Unknown morph transform."))?;
                let states = match definition.scope {
                    CounterScope::World => &mut world.runtime.object_states,
                    CounterScope::Instance => {
                        &mut world
                            .runtime
                            .instances
                            .get_mut(character.runtime.instance.as_ref().ok_or_else(|| {
                                invalid_state("Morph requires its live instance.")
                            })?)
                            .ok_or_else(|| invalid_state("Unknown morph instance."))?
                            .object_states
                    }
                    CounterScope::Character => {
                        return Err(invalid_state("Character-scoped clipping is not supported."));
                    }
                };
                if definition.scope == CounterScope::Instance && !states.contains_key(id) {
                    continue;
                }
                if !definition.states.contains_key(selected) {
                    return Err(unknown("Unknown morph collision state."));
                }
                states.insert(id.clone(), selected.clone());
                events.push(GameEvent::ObjectTransformed {
                    transform: id.clone(),
                    state: selected.clone(),
                });
            }
        }
        if !events.is_empty() {
            let scope = self
                .content
                .mechanics
                .counters
                .get(counter)
                .ok_or_else(|| unknown("Unknown morph counter."))?
                .scope;
            self.collision_for(
                world,
                if scope == CounterScope::World {
                    None
                } else {
                    character.runtime.instance.as_ref()
                },
            )?;
        }
        Ok(events)
    }
}

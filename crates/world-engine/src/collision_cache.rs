use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

use clubscape_game_types::{
    GameResult, InstanceId, InstanceTemplateId, ObjectStateId, ObjectTransformId,
    TemporaryObjectId, Tile, WorldState,
};
use clubscape_simulation::navigation::CollisionMap;

use crate::invalid_state;

const MAX_ENTRIES: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CollisionKey {
    template: Option<InstanceTemplateId>,
    states: BTreeMap<ObjectTransformId, ObjectStateId>,
    temporary: Vec<(TemporaryObjectId, Tile)>,
}

impl CollisionKey {
    pub(crate) fn from_world(
        world: &WorldState,
        instance: Option<&InstanceId>,
    ) -> GameResult<Self> {
        let (template, states) = match instance {
            Some(id) => {
                let state = world
                    .runtime
                    .instances
                    .get(id)
                    .ok_or_else(|| invalid_state("Unknown live instance."))?;
                (Some(state.template.clone()), state.object_states.clone())
            }
            None => (None, world.runtime.object_states.clone()),
        };
        let temporary = world
            .runtime
            .temporary_objects
            .values()
            .filter(|object| object.location.instance.as_ref() == instance)
            .map(|object| (object.definition.clone(), object.location.tile))
            .collect();
        Ok(Self {
            template,
            states,
            temporary,
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct CollisionCache {
    entries: VecDeque<(CollisionKey, Arc<CollisionMap>)>,
}

impl CollisionCache {
    pub(crate) fn get(&self, key: &CollisionKey) -> Option<Arc<CollisionMap>> {
        self.entries
            .iter()
            .rev()
            .find(|(stored, _)| stored == key)
            .map(|(_, map)| Arc::clone(map))
    }

    pub(crate) fn insert(&mut self, key: CollisionKey, map: Arc<CollisionMap>) {
        if self.entries.len() == MAX_ENTRIES {
            self.entries.pop_front();
        }
        self.entries.push_back((key, map));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        WorldEngine,
        test_support::{self as f, v2 as v},
    };
    use clubscape_game_types::*;

    fn setup() -> (WorldEngine, WorldState) {
        let mut content = v::content();
        v::with_fire(&mut content);
        v::with_death(&mut content);
        let blocker = content
            .mechanics
            .temporary_objects
            .get_mut(&v::fire())
            .unwrap();
        blocker.blocks_movement = true;
        blocker.blocks_projectiles = true;
        let mut other = blocker.clone();
        other.id = TemporaryObjectId::new("temporary_object.synthetic.transparent").unwrap();
        other.blocks_movement = false;
        other.blocks_projectiles = false;
        content
            .mechanics
            .temporary_objects
            .insert(other.id.clone(), other);
        f::cell(&mut content, f::tile(10, 10, 0)).height = 123;
        let mut rotated = content.mechanics.instances[&v::template()].clone();
        rotated.id = InstanceTemplateId::new("instance_template.synthetic.rotated").unwrap();
        rotated.chunks[0].quarter_turns = 1;
        content
            .mechanics
            .instances
            .insert(rotated.id.clone(), rotated);
        f::setup(content)
    }

    fn add_block(
        world: &mut WorldState,
        tile: Tile,
        instance: Option<InstanceId>,
    ) -> DynamicObjectId {
        let id = DynamicObjectId::new("dynamic_object.synthetic.cache").unwrap();
        let region = RegionId::new(format!("region.synthetic.floor_{}", tile.plane())).unwrap();
        world.runtime.temporary_objects.insert(
            id.clone(),
            DynamicObject {
                definition: v::fire(),
                owner: f::actor(),
                location: RuntimeLocation {
                    region,
                    tile,
                    instance,
                },
                created_at_tick: 0,
                expires_at_tick: 7,
            },
        );
        id
    }

    fn instance(engine: &WorldEngine, world: &mut WorldState, actor: ActorId) -> InstanceId {
        engine
            .resolve_location(
                world,
                &actor,
                &WorldLocation {
                    region: RegionId::new("region.synthetic.floor_1").unwrap(),
                    tile: f::tile(10, 10, 1),
                    instance: Some(v::template()),
                },
            )
            .unwrap()
            .instance
            .unwrap()
    }

    #[test]
    fn same_tick_temporary_moves_definitions_and_removal_never_reuse_stale_collision() {
        let (engine, mut world) = setup();
        let before = world.clone();
        let base = engine.collision_for(&world, None).unwrap();
        let first = f::tile(11, 11, 0);
        let second = f::tile(12, 11, 0);
        let id = add_block(&mut world, first, None);
        let blocked = engine.collision_for(&world, None).unwrap();
        assert!(!blocked.cell(first).unwrap().walkable);
        assert_eq!(blocked.cell(first).unwrap().blocked_sight, u8::MAX);
        assert_eq!(*blocked, engine.build_collision_map(&world, None).unwrap());
        assert!(Arc::ptr_eq(
            &blocked,
            &engine.collision_for(&world, None).unwrap()
        ));
        world
            .runtime
            .temporary_objects
            .get_mut(&id)
            .unwrap()
            .location
            .tile = second;
        let moved = engine.collision_for(&world, None).unwrap();
        assert!(!Arc::ptr_eq(&blocked, &moved));
        assert!(moved.cell(first).unwrap().walkable);
        assert!(!moved.cell(second).unwrap().walkable);
        assert!(!blocked.cell(first).unwrap().walkable);
        assert_eq!(*moved, engine.build_collision_map(&world, None).unwrap());
        world
            .runtime
            .temporary_objects
            .get_mut(&id)
            .unwrap()
            .definition = TemporaryObjectId::new("temporary_object.synthetic.transparent").unwrap();
        let transparent = engine.collision_for(&world, None).unwrap();
        assert!(transparent.cell(second).unwrap().walkable);
        assert_eq!(*transparent, *base);
        world.runtime.temporary_objects.remove(&id);
        assert!(Arc::ptr_eq(
            &base,
            &engine.collision_for(&world, None).unwrap()
        ));
        assert_eq!(world, before);
    }

    #[test]
    fn nonphysical_ticks_and_revisions_do_not_flush_an_exact_map() {
        let (engine, mut world) = setup();
        add_block(&mut world, f::tile(11, 11, 0), None);
        let map = engine.collision_for(&world, None).unwrap();
        world.tick = 2;
        world.revision = 99;
        assert!(Arc::ptr_eq(
            &map,
            &engine.collision_for(&world, None).unwrap()
        ));
        assert_eq!(*map, engine.build_collision_map(&world, None).unwrap());
    }

    #[test]
    fn instance_template_and_scoped_temporary_objects_are_part_of_physical_identity() {
        let (engine, mut world) = setup();
        let first = instance(&engine, &mut world, f::actor());
        let second = instance(&engine, &mut world, f::actor_two());
        let original = engine.collision_for(&world, Some(&first)).unwrap();
        assert!(Arc::ptr_eq(
            &original,
            &engine.collision_for(&world, Some(&second)).unwrap()
        ));
        let id = add_block(&mut world, f::tile(11, 11, 1), Some(first.clone()));
        let blocked = engine.collision_for(&world, Some(&first)).unwrap();
        assert!(!blocked.cell(f::tile(11, 11, 1)).unwrap().walkable);
        assert!(
            engine
                .collision_for(&world, Some(&second))
                .unwrap()
                .cell(f::tile(11, 11, 1))
                .unwrap()
                .walkable
        );
        assert!(
            engine
                .collision_for(&world, None)
                .unwrap()
                .cell(f::tile(11, 11, 1))
                .unwrap()
                .walkable
        );
        assert_eq!(
            *blocked,
            engine.build_collision_map(&world, Some(&first)).unwrap()
        );
        world.runtime.temporary_objects.remove(&id);
        world.runtime.instances.get_mut(&first).unwrap().template =
            InstanceTemplateId::new("instance_template.synthetic.rotated").unwrap();
        let rotated = engine.collision_for(&world, Some(&first)).unwrap();
        assert_ne!(*rotated, *original);
        assert_eq!(
            *rotated,
            engine.build_collision_map(&world, Some(&first)).unwrap()
        );
        world.runtime.instances.get_mut(&first).unwrap().template =
            InstanceTemplateId::new("instance_template.synthetic.missing").unwrap();
        assert!(engine.collision_for(&world, Some(&first)).is_err());
    }

    #[test]
    fn source_transform_changes_invalidate_without_advancing_tick_or_revision() {
        let (mut content, transform, open) = {
            let mut content = v::content();
            let transform = ObjectTransformId::new("transform.synthetic.cache").unwrap();
            let closed = ObjectStateId::new("object_state.closed").unwrap();
            let open = ObjectStateId::new("object_state.open").unwrap();
            f::add_object(
                &mut content,
                "door",
                InteractionAction::Effects { effects: vec![] },
            );
            let placement = SourceObjectPlacement {
                shape: 0,
                quarter_turns: 0,
                layer: ObjectLayer::Wall,
            };
            content.spawns.get_mut(&f::spawn("door")).unwrap().placement = Some(placement.clone());
            f::cell(&mut content, f::tile(10, 10, 0)).blocked_movement = Direction::East.mask();
            f::cell(&mut content, f::tile(11, 10, 0)).blocked_movement = Direction::West.mask();
            let cells = vec![
                *f::cell(&mut content, f::tile(10, 10, 0)),
                *f::cell(&mut content, f::tile(11, 10, 0)),
            ];
            let opened = cells
                .iter()
                .map(|cell| CollisionCell {
                    blocked_movement: 0,
                    ..*cell
                })
                .collect();
            let state = |door, collision| ObjectTransformState {
                object: Some(f::object("door")),
                tile: f::tile(11, 10, 0),
                door: Some(door),
                placement: placement.clone(),
                collision,
            };
            content.mechanics.object_transforms.insert(
                transform.clone(),
                ObjectTransformDefinition {
                    id: transform.clone(),
                    scope: CounterScope::World,
                    spawn: f::spawn("door"),
                    initial: closed.clone(),
                    states: BTreeMap::from([
                        (closed, state(DoorPosition::Closed, cells)),
                        (open.clone(), state(DoorPosition::Open, opened)),
                    ]),
                    source: f::source(),
                },
            );
            (content, transform, open)
        };
        // Keep the transformation request executable through the same native intent path.
        f::interaction(&mut content, "door").action = InteractionAction::Effects {
            effects: vec![Effect::TransformObject {
                transform: transform.clone(),
                state: open.clone(),
            }],
        };
        let (engine, mut world) = f::setup(content);
        let closed = world.runtime.object_states[&transform].clone();
        let first = engine.collision_for(&world, None).unwrap();
        assert!(!first.can_step(f::tile(10, 10, 0), f::tile(11, 10, 0)));
        f::apply(&engine, &mut world, f::interact("door"));
        let opened = engine.collision_for(&world, None).unwrap();
        assert!(opened.can_step(f::tile(10, 10, 0), f::tile(11, 10, 0)));
        assert_eq!(*opened, engine.build_collision_map(&world, None).unwrap());
        assert_eq!((world.tick, world.revision), (0, 0));
        world
            .runtime
            .object_states
            .insert(transform.clone(), closed);
        assert!(Arc::ptr_eq(
            &first,
            &engine.collision_for(&world, None).unwrap()
        ));
        world.runtime.object_states.insert(transform, open);
        assert!(Arc::ptr_eq(
            &opened,
            &engine.collision_for(&world, None).unwrap()
        ));
    }

    #[test]
    fn cache_retention_is_bounded_and_eviction_does_not_mutate_borrowed_maps() {
        let (engine, mut world) = setup();
        let id = add_block(&mut world, f::tile(11, 12, 0), None);
        let first = engine.collision_for(&world, None).unwrap();
        for step in 1..=MAX_ENTRIES {
            world
                .runtime
                .temporary_objects
                .get_mut(&id)
                .unwrap()
                .location
                .tile = f::tile(11 + step as u16, 12, 0);
            let map = engine.collision_for(&world, None).unwrap();
            assert_eq!(*map, engine.build_collision_map(&world, None).unwrap());
        }
        assert_eq!(
            engine.collision_cache.lock().unwrap().entries.len(),
            MAX_ENTRIES
        );
        world
            .runtime
            .temporary_objects
            .get_mut(&id)
            .unwrap()
            .location
            .tile = f::tile(11, 12, 0);
        let rebuilt = engine.collision_for(&world, None).unwrap();
        assert!(!Arc::ptr_eq(&first, &rebuilt));
        assert_eq!(*first, *rebuilt);
        assert!(!first.cell(f::tile(11, 12, 0)).unwrap().walkable);
    }

    #[test]
    fn concurrent_engine_clones_share_one_immutable_result() {
        let (engine, mut world) = setup();
        add_block(&mut world, f::tile(11, 11, 0), None);
        let barrier = std::sync::Barrier::new(4);
        let maps = std::thread::scope(|scope| {
            let jobs: Vec<_> = (0..4)
                .map(|_| {
                    let engine = engine.clone();
                    let world = &world;
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        engine.collision_for(world, None).unwrap()
                    })
                })
                .collect();
            jobs.into_iter()
                .map(|job| job.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert!(maps.iter().all(|map| Arc::ptr_eq(map, &maps[0])));
        assert_eq!(engine.collision_cache.lock().unwrap().entries.len(), 1);
    }

    #[test]
    fn invalid_builds_are_not_cached_and_do_not_poison_valid_results() {
        let (engine, mut world) = setup();
        let id = add_block(&mut world, f::tile(11, 11, 0), None);
        let good = engine.collision_for(&world, None).unwrap();
        world
            .runtime
            .temporary_objects
            .get_mut(&id)
            .unwrap()
            .location
            .tile = f::tile(1000, 1000, 0);
        assert!(engine.collision_for(&world, None).is_err());
        assert_eq!(engine.collision_cache.lock().unwrap().entries.len(), 1);
        world
            .runtime
            .temporary_objects
            .get_mut(&id)
            .unwrap()
            .location
            .tile = f::tile(11, 11, 0);
        assert!(Arc::ptr_eq(
            &good,
            &engine.collision_for(&world, None).unwrap()
        ));
    }

    #[test]
    fn actual_temporary_expiration_restores_source_collision() {
        let (engine, mut world) = setup();
        let tile = f::tile(11, 11, 0);
        add_block(&mut world, tile, None);
        let blocked = engine.collision_for(&world, None).unwrap();
        assert!(!blocked.cell(tile).unwrap().walkable);
        world.tick = 7;
        engine.expire_objects(&mut world).unwrap();
        assert!(world.runtime.temporary_objects.is_empty());
        let expired = engine.collision_for(&world, None).unwrap();
        assert!(expired.cell(tile).unwrap().walkable);
        assert_eq!(*expired, engine.build_collision_map(&world, None).unwrap());
        assert!(!blocked.cell(tile).unwrap().walkable);
    }
}

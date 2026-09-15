use std::{
    borrow::Cow,
    collections::{BTreeMap, VecDeque},
};

use clubscape_game_types::*;
use clubscape_simulation::navigation::CollisionMap;

use crate::{WorldEngine, invalid_state, runtime, unknown};

pub(crate) const ROUTE_ORDER: [Direction; 8] = [
    Direction::West,
    Direction::East,
    Direction::South,
    Direction::North,
    Direction::SouthWest,
    Direction::SouthEast,
    Direction::NorthWest,
    Direction::NorthEast,
];

#[derive(Clone)]
pub(crate) struct TargetShape {
    pub tile: Tile,
    pub width: u8,
    pub height: u8,
    pub object: Option<ObjectId>,
    pub npc: Option<NpcId>,
    pub access: Option<Vec<Tile>>,
    pub blocked_access: u8,
    pub solid_footprint: bool,
    pub opaque_footprint: bool,
}

impl TargetShape {
    pub(crate) fn distance_from(&self, actor: Tile) -> Option<u16> {
        if self.tile.plane() != actor.plane() || self.width == 0 || self.height == 0 {
            return None;
        }
        let right = self.tile.x().checked_add(u16::from(self.width) - 1)?;
        let top = self.tile.y().checked_add(u16::from(self.height) - 1)?;
        Some(
            actor
                .x()
                .abs_diff(actor.x().clamp(self.tile.x(), right))
                .max(actor.y().abs_diff(actor.y().clamp(self.tile.y(), top))),
        )
    }
}

impl WorldEngine {
    pub(crate) fn spawn_dimensions(&self, spawn: &SpawnDefinition) -> GameResult<(u8, u8)> {
        match &spawn.kind {
            SpawnKind::Npc { npc } => {
                let size = self
                    .content
                    .npcs
                    .get(npc)
                    .ok_or_else(|| unknown("Unknown footprint NPC."))?
                    .size;
                Ok((size, size))
            }
            SpawnKind::Object { object } => {
                let initial = self
                    .content
                    .mechanics
                    .object_transforms
                    .values()
                    .find(|transform| transform.spawn == spawn.id)
                    .and_then(|transform| transform.states.get(&transform.initial));
                let object = self
                    .content
                    .objects
                    .get(
                        initial
                            .and_then(|state| state.object.as_ref())
                            .unwrap_or(object),
                    )
                    .ok_or_else(|| unknown("Unknown footprint object."))?;
                Ok(
                    if initial
                        .map(|state| &state.placement)
                        .or(spawn.placement.as_ref())
                        .is_some_and(|placement| placement.quarter_turns % 2 == 1)
                    {
                        (object.size_y, object.size_x)
                    } else {
                        (object.size_x, object.size_y)
                    },
                )
            }
            SpawnKind::Item { .. } => Ok((1, 1)),
        }
    }

    pub(crate) fn instance_spawn_origin(
        &self,
        world: &WorldState,
        instance: Option<&InstanceId>,
        spawn: &SpawnDefinition,
    ) -> GameResult<Tile> {
        let Some(id) = instance else {
            return Ok(spawn.tile);
        };
        let state = world
            .runtime
            .instances
            .get(id)
            .ok_or_else(|| invalid_state("Unknown live instance."))?;
        let template = self
            .content
            .mechanics
            .instances
            .get(&state.template)
            .ok_or_else(|| unknown("Unknown instance template."))?;
        let (width, height) = self.spawn_dimensions(spawn)?;
        Ok(map_footprint(template, spawn.tile, width, height)?.0)
    }

    pub(crate) fn collision_for(
        &self,
        world: &WorldState,
        instance: Option<&InstanceId>,
    ) -> GameResult<Cow<'_, CollisionMap>> {
        if instance.is_none()
            && world.runtime.object_states.iter().all(|(id, state)| {
                self.content
                    .mechanics
                    .object_transforms
                    .get(id)
                    .is_some_and(|definition| &definition.initial == state)
            })
            && world.runtime.temporary_objects.is_empty()
        {
            return Ok(Cow::Borrowed(&self.collision));
        }
        let mut cells = BTreeMap::new();
        if let Some(id) = instance {
            let state = world
                .runtime
                .instances
                .get(id)
                .ok_or_else(|| invalid_state("Unknown live instance."))?;
            let template = self
                .content
                .mechanics
                .instances
                .get(&state.template)
                .ok_or_else(|| unknown("Missing instance template."))?;
            for chunk in &template.chunks {
                let source = self
                    .content
                    .regions
                    .get(&chunk.source_region)
                    .ok_or_else(|| unknown("Unknown source chunk region."))?;
                for cell in &source.cells {
                    if let Some(tile) = map_chunk(cell.tile, chunk, template.chunk_size) {
                        cells.insert(
                            tile,
                            CollisionCell {
                                tile,
                                blocked_movement: rotate_mask(
                                    cell.blocked_movement,
                                    chunk.quarter_turns,
                                ),
                                blocked_sight: rotate_mask(cell.blocked_sight, chunk.quarter_turns),
                                ..*cell
                            },
                        );
                    }
                }
            }
            self.apply_collision_states(&mut cells, &state.object_states, Some(template))?;
        } else {
            cells.extend(
                self.content
                    .regions
                    .values()
                    .flat_map(|r| &r.cells)
                    .map(|cell| (cell.tile, *cell)),
            );
            self.apply_collision_states(&mut cells, &world.runtime.object_states, None)?;
        }
        for dynamic in world
            .runtime
            .temporary_objects
            .values()
            .filter(|object| object.location.instance.as_ref() == instance)
        {
            let definition = self
                .content
                .mechanics
                .temporary_objects
                .get(&dynamic.definition)
                .ok_or_else(|| unknown("Missing temporary-object definition."))?;
            let object = self
                .content
                .objects
                .get(&definition.object)
                .ok_or_else(|| unknown("Missing temporary object geometry."))?;
            for dx in 0..object.size_x {
                for dy in 0..object.size_y {
                    let tile = dynamic
                        .location
                        .tile
                        .offset(i16::from(dx), i16::from(dy))
                        .ok_or_else(|| invalid_state("Temporary footprint overflow."))?;
                    let cell = cells.get_mut(&tile).ok_or_else(|| {
                        invalid_state("Temporary object occupies an unlisted cell.")
                    })?;
                    if definition.blocks_movement {
                        cell.walkable = false;
                    }
                    if definition.blocks_projectiles {
                        cell.blocked_sight = u8::MAX;
                    }
                }
            }
        }
        let mut regions = self.content.regions.clone();
        for region in regions.values_mut() {
            region.cells = region
                .cells
                .iter()
                .filter_map(|cell| cells.get(&cell.tile).copied())
                .collect();
        }
        CollisionMap::from_regions(regions.values()).map(Cow::Owned)
    }

    fn apply_collision_states(
        &self,
        cells: &mut BTreeMap<Tile, CollisionCell>,
        states: &BTreeMap<ObjectTransformId, ObjectStateId>,
        template: Option<&InstanceTemplateDefinition>,
    ) -> GameResult<()> {
        let mut overrides = BTreeMap::new();
        let mut grouped = std::collections::BTreeSet::new();
        for group in self.content.mechanics.collision_groups.values() {
            if !group.transforms.iter().any(|id| states.contains_key(id)) {
                continue;
            }
            let selection = group
                .transforms
                .iter()
                .map(|id| {
                    states
                        .get(id)
                        .cloned()
                        .map(|state| (id.clone(), state))
                        .ok_or_else(|| {
                            invalid_state(
                                "A live combined collision group is only partially mapped.",
                            )
                        })
                })
                .collect::<GameResult<BTreeMap<_, _>>>()?;
            grouped.extend(group.transforms.iter().cloned());
            if selection.iter().all(|(id, state)| {
                self.content
                    .mechanics
                    .object_transforms
                    .get(id)
                    .is_some_and(|definition| &definition.initial == state)
            }) {
                continue;
            }
            let selected = group
                .states
                .require()?
                .iter()
                .find(|state| state.selection == selection)
                .ok_or_else(|| {
                    invalid_state(
                        "No declared combined collision state matches the current selection.",
                    )
                })?;
            apply_replacements(cells, &mut overrides, &selected.collision, template)?;
        }
        for (id, state) in states {
            if grouped.contains(id) {
                continue;
            }
            let definition = self
                .content
                .mechanics
                .object_transforms
                .get(id)
                .ok_or_else(|| unknown("Unknown collision transform."))?;
            if state == &definition.initial {
                continue;
            }
            let state = definition
                .states
                .get(state)
                .ok_or_else(|| unknown("Undefined object state."))?;
            apply_replacements(cells, &mut overrides, &state.collision, template)?;
        }
        Ok(())
    }

    pub(crate) fn map_instance_tile(
        &self,
        world: &WorldState,
        instance: Option<&InstanceId>,
        source: Tile,
    ) -> GameResult<Tile> {
        let Some(id) = instance else {
            return Ok(source);
        };
        let state = world
            .runtime
            .instances
            .get(id)
            .ok_or_else(|| invalid_state("Unknown instance."))?;
        let template = self
            .content
            .mechanics
            .instances
            .get(&state.template)
            .ok_or_else(|| unknown("Unknown instance template."))?;
        template
            .chunks
            .iter()
            .find_map(|chunk| map_chunk(source, chunk, template.chunk_size))
            .ok_or_else(|| {
                GameError::new(
                    GameErrorCode::OutOfReach,
                    "Source target is not in this instance.",
                )
            })
    }

    pub(crate) fn target_shape(
        &self,
        world: &WorldState,
        character: &CharacterState,
        target: &WorldTarget,
    ) -> GameResult<TargetShape> {
        self.resolve_shape(world, character, target, true)
    }

    pub(crate) fn resolve_shape(
        &self,
        world: &WorldState,
        character: &CharacterState,
        target: &WorldTarget,
        require_available: bool,
    ) -> GameResult<TargetShape> {
        if let WorldTarget::TemporaryObject { object } = target {
            let dynamic = world.runtime.temporary_objects.get(object).ok_or_else(|| {
                GameError::new(
                    GameErrorCode::NotOwned,
                    "Temporary object no longer exists.",
                )
            })?;
            if dynamic.expires_at_tick <= world.tick
                || dynamic.location.instance != character.runtime.instance
            {
                return Err(GameError::new(
                    GameErrorCode::OutOfReach,
                    "Temporary object is expired or in another instance.",
                ));
            }
            let definition = self
                .content
                .mechanics
                .temporary_objects
                .get(&dynamic.definition)
                .ok_or_else(|| unknown("Undefined temporary object."))?;
            if require_available && definition.owner_only_use && dynamic.owner != character.actor_id
            {
                return Err(GameError::new(
                    GameErrorCode::NotOwned,
                    "Temporary object is private.",
                ));
            }
            let object = self
                .content
                .objects
                .get(&definition.object)
                .ok_or_else(|| unknown("Unknown temporary geometry."))?;
            return Ok(TargetShape {
                tile: dynamic.location.tile,
                width: object.size_x,
                height: object.size_y,
                object: Some(object.id.clone()),
                npc: None,
                access: None,
                blocked_access: object
                    .clip
                    .as_ref()
                    .map_or(0, |clip| clip.access_blocked_sides),
                solid_footprint: definition.blocks_movement,
                opaque_footprint: definition.blocks_projectiles,
            });
        }
        let WorldTarget::Spawn { spawn: id } = target else {
            unreachable!()
        };
        let spawn = self
            .content
            .spawns
            .get(id)
            .ok_or_else(|| unknown(format!("Unknown spawn {id}.")))?;
        let entity = runtime::entity(world, character.runtime.instance.as_ref(), id)?;
        if require_available && entity.available_at_tick > world.tick {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "Target is depleted or respawning.",
            ));
        }
        match &spawn.kind {
            SpawnKind::Object { object } => {
                let mut object = object.clone();
                let mut source_tile = if character.runtime.instance.is_none() {
                    entity.tile
                } else {
                    spawn.tile
                };
                let mut transformed = false;
                let mut rotation = spawn
                    .placement
                    .as_ref()
                    .map_or(0, |placement| placement.quarter_turns);
                let mut layer = spawn
                    .placement
                    .as_ref()
                    .map(|placement| placement.layer.clone());
                let states = match character.runtime.instance.as_ref() {
                    Some(id) => {
                        &world
                            .runtime
                            .instances
                            .get(id)
                            .ok_or_else(|| invalid_state("Unknown instance."))?
                            .object_states
                    }
                    None => &world.runtime.object_states,
                };
                for (id, state) in states {
                    let definition = self
                        .content
                        .mechanics
                        .object_transforms
                        .get(id)
                        .ok_or_else(|| unknown("Unknown transform."))?;
                    if definition.spawn == spawn.id {
                        let state = definition
                            .states
                            .get(state)
                            .ok_or_else(|| unknown("Undefined transform state."))?;
                        object = state.object.clone().ok_or_else(|| {
                            GameError::new(
                                GameErrorCode::NotOwned,
                                "The object is absent in its current state.",
                            )
                        })?;
                        source_tile = state.tile;
                        transformed = true;
                        rotation = state.placement.quarter_turns;
                        layer = Some(state.placement.layer.clone());
                    }
                }
                let base = self
                    .content
                    .objects
                    .get(&object)
                    .ok_or_else(|| unknown("Unknown object."))?;
                if let Some(morph) = &base.morph {
                    object = self
                        .morph_counter(world, character, &morph.counter)?
                        .and_then(|value| morph.variants.get(&value))
                        .unwrap_or(&morph.fallback)
                        .clone()
                        .ok_or_else(|| {
                            GameError::new(GameErrorCode::NotOwned, "Object morph is absent.")
                        })?;
                }
                let definition = self
                    .content
                    .objects
                    .get(&object)
                    .ok_or_else(|| unknown("Unknown object morph."))?;
                if base.clip != definition.clip
                    || base.size_x != definition.size_x
                    || base.size_y != definition.size_y
                {
                    return Err(crate::unavailable(
                        "Clipping-changing morphs require explicit object-transform collision replacements.",
                    ));
                }
                let (mut width, mut height) = if rotation % 2 == 1 {
                    (definition.size_y, definition.size_x)
                } else {
                    (definition.size_x, definition.size_y)
                };
                let tile = if let Some(id) = &character.runtime.instance {
                    let instance = world
                        .runtime
                        .instances
                        .get(id)
                        .ok_or_else(|| invalid_state("Unknown footprint instance."))?;
                    let template = self
                        .content
                        .mechanics
                        .instances
                        .get(&instance.template)
                        .ok_or_else(|| unknown("Unknown footprint template."))?;
                    let mapped = map_footprint(template, source_tile, width, height)?;
                    width = mapped.1;
                    height = mapped.2;
                    let chunk = template
                        .chunks
                        .iter()
                        .find(|chunk| map_chunk(source_tile, chunk, template.chunk_size).is_some())
                        .ok_or_else(|| invalid_state("Object has no instance mapping."))?;
                    rotation = (rotation + chunk.quarter_turns) % 4;
                    if transformed { mapped.0 } else { entity.tile }
                } else {
                    source_tile
                };
                Ok(TargetShape {
                    tile,
                    width,
                    height,
                    object: Some(object),
                    npc: None,
                    access: None,
                    blocked_access: rotate_mask(
                        definition
                            .clip
                            .as_ref()
                            .map_or(0, |clip| clip.access_blocked_sides),
                        rotation,
                    ),
                    solid_footprint: matches!(
                        layer,
                        Some(ObjectLayer::GameObject | ObjectLayer::FloorDecoration)
                    ) && definition
                        .clip
                        .as_ref()
                        .is_some_and(|clip| clip.blocks_movement),
                    opaque_footprint: definition
                        .clip
                        .as_ref()
                        .is_some_and(|clip| clip.blocks_projectiles),
                })
            }
            SpawnKind::Npc { npc } => {
                let mut definition = self
                    .content
                    .npcs
                    .get(npc)
                    .ok_or_else(|| unknown("Unknown NPC."))?;
                if let Some(morph) = &definition.morph {
                    let id = self
                        .morph_counter(world, character, &morph.counter)?
                        .and_then(|value| morph.variants.get(&value))
                        .unwrap_or(&morph.fallback)
                        .as_ref()
                        .ok_or_else(|| {
                            GameError::new(GameErrorCode::NotOwned, "NPC morph is absent.")
                        })?;
                    definition = self
                        .content
                        .npcs
                        .get(id)
                        .ok_or_else(|| unknown("Unknown NPC morph."))?;
                }
                if require_available && definition.combat.is_some() && entity.hitpoints == 0 {
                    return Err(GameError::new(GameErrorCode::Busy, "NPC is defeated."));
                }
                let access = match &definition.navigation {
                    NpcNavigation::Stationary {
                        anchor:
                            StationaryAnchor::NonWalkingResource { access_tiles }
                            | StationaryAnchor::SceneryBound { access_tiles, .. }
                            | StationaryAnchor::ScriptedActor { access_tiles, .. },
                    } => Some(
                        access_tiles
                            .iter()
                            .map(|tile| {
                                self.map_instance_tile(
                                    world,
                                    character.runtime.instance.as_ref(),
                                    *tile,
                                )
                            })
                            .collect::<GameResult<Vec<_>>>()?,
                    ),
                    _ => None,
                };
                Ok(TargetShape {
                    tile: entity.tile,
                    width: definition.size,
                    height: definition.size,
                    object: None,
                    npc: Some(definition.id.clone()),
                    access,
                    blocked_access: 0,
                    solid_footprint: false,
                    opaque_footprint: false,
                })
            }
            SpawnKind::Item { .. } => Ok(TargetShape {
                tile: entity.tile,
                width: 1,
                height: 1,
                object: None,
                npc: None,
                access: None,
                blocked_access: 0,
                solid_footprint: false,
                opaque_footprint: false,
            }),
        }
    }

    fn morph_counter(
        &self,
        world: &WorldState,
        character: &CharacterState,
        id: &CounterId,
    ) -> GameResult<Option<i64>> {
        Ok(match self.counter_value(world, character, id)? {
            CounterValue::Integer(value) => Some(value),
            CounterValue::Boolean(value) => Some(i64::from(value)),
        })
    }

    pub(crate) fn target_interactions<'a>(
        &'a self,
        world: &WorldState,
        target: &WorldTarget,
    ) -> GameResult<&'a [InteractionDefinition]> {
        match target {
            WorldTarget::Spawn { spawn } => Ok(&self
                .content
                .spawns
                .get(spawn)
                .ok_or_else(|| unknown("Unknown spawn."))?
                .interactions),
            WorldTarget::TemporaryObject { object } => {
                let definition = &world
                    .runtime
                    .temporary_objects
                    .get(object)
                    .ok_or_else(|| {
                        GameError::new(
                            GameErrorCode::NotOwned,
                            "Temporary object no longer exists.",
                        )
                    })?
                    .definition;
                Ok(&self
                    .content
                    .mechanics
                    .temporary_objects
                    .get(definition)
                    .ok_or_else(|| unknown("Unknown temporary definition."))?
                    .interactions)
            }
        }
    }

    pub(crate) fn route(
        &self,
        world: &WorldState,
        character: &CharacterState,
        goal: Tile,
    ) -> GameResult<Vec<Tile>> {
        let map = self.collision_for(world, character.runtime.instance.as_ref())?;
        route_with(&map, character.tile, goal, |from, to| {
            self.actor_can_step(world, character, &map, from, to)
        })
    }
}

pub(crate) fn route(map: &CollisionMap, start: Tile, goal: Tile) -> GameResult<Vec<Tile>> {
    route_with(map, start, goal, |from, to| Ok(map.can_step(from, to)))
}

fn route_with(
    map: &CollisionMap,
    start: Tile,
    goal: Tile,
    mut can_step: impl FnMut(Tile, Tile) -> GameResult<bool>,
) -> GameResult<Vec<Tile>> {
    if start.plane() != goal.plane() || map.cell(goal).is_none() || map.cell(start).is_none() {
        return Err(GameError::new(
            GameErrorCode::OutOfReach,
            "Route endpoints must be on one listed plane.",
        ));
    }
    if start.distance(goal).is_none_or(|distance| distance > 50) {
        return Err(GameError::new(
            GameErrorCode::OutOfReach,
            "Destination is outside the source 101-tile candidate window.",
        ));
    }
    let mut found = None;
    let mut predecessors = BTreeMap::from([(start, (start, 0_u16))]);
    let mut queue = VecDeque::from([start]);
    while let Some(current) = queue.pop_front() {
        if current == goal {
            found = Some(goal);
            break;
        }
        let distance = predecessors[&current].1;
        for direction in ROUTE_ORDER {
            let (dx, dy) = direction.offset();
            let Some(next) = current.offset(dx, dy) else {
                continue;
            };
            let offset_x = i32::from(next.x()) - i32::from(start.x());
            let offset_y = i32::from(next.y()) - i32::from(start.y());
            if !(-64..64).contains(&offset_x)
                || !(-64..64).contains(&offset_y)
                || predecessors.contains_key(&next)
                || !can_step(current, next)?
            {
                continue;
            }
            predecessors.insert(next, (current, distance + 1));
            queue.push_back(next);
        }
    }
    let destination = found
        .or_else(|| {
            predecessors
                .iter()
                .filter(|(tile, (_, distance))| {
                    **tile != start
                        && *distance < 100
                        && tile.x().abs_diff(goal.x()) <= 10
                        && tile.y().abs_diff(goal.y()) <= 10
                })
                .min_by_key(|(tile, (_, distance))| {
                    (
                        u32::from(tile.x().abs_diff(goal.x())).pow(2)
                            + u32::from(tile.y().abs_diff(goal.y())).pow(2),
                        *distance,
                        tile.x(),
                        tile.y(),
                    )
                })
                .map(|(tile, _)| *tile)
        })
        .ok_or_else(|| {
            GameError::new(GameErrorCode::Blocked, "No source route or legal fallback.")
        })?;
    let mut path = Vec::new();
    let mut tile = destination;
    while tile != start {
        path.push(tile);
        tile = predecessors[&tile].0;
    }
    path.reverse();
    let mut corners = 0;
    let mut previous = start;
    let mut direction = None;
    let mut limit = path.len();
    for (index, tile) in path.iter().enumerate() {
        let delta = (
            i32::from(tile.x()) - i32::from(previous.x()),
            i32::from(tile.y()) - i32::from(previous.y()),
        );
        if direction != Some(delta) {
            corners += 1;
            if corners > 25 {
                limit = index;
                break;
            }
            direction = Some(delta);
        }
        previous = *tile;
    }
    path.truncate(limit);
    Ok(path)
}

pub(crate) fn map_chunk(tile: Tile, chunk: &InstanceChunkMapping, size: u8) -> Option<Tile> {
    if tile.plane() != chunk.source_origin.plane() {
        return None;
    }
    let mut x = tile.x().checked_sub(chunk.source_origin.x())?;
    let mut y = tile.y().checked_sub(chunk.source_origin.y())?;
    let n = u16::from(size);
    if x >= n || y >= n {
        return None;
    }
    for _ in 0..chunk.quarter_turns {
        (x, y) = (y, n - 1 - x);
    }
    chunk.destination_origin.offset(x as i16, y as i16)
}

fn rotate_mask(mask: u8, rotations: u8) -> u8 {
    let mut result = 0;
    for direction in Direction::ALL {
        if mask & direction.mask() == 0 {
            continue;
        }
        let (mut dx, mut dy) = direction.offset();
        for _ in 0..rotations {
            (dx, dy) = (dy, -dx);
        }
        if let Some(rotated) = Direction::ALL
            .iter()
            .find(|direction| direction.offset() == (dx, dy))
        {
            result |= rotated.mask();
        }
    }
    result
}

fn apply_replacements(
    cells: &mut BTreeMap<Tile, CollisionCell>,
    overrides: &mut BTreeMap<Tile, CollisionCell>,
    replacements: &[CollisionCell],
    template: Option<&InstanceTemplateDefinition>,
) -> GameResult<()> {
    for cell in replacements {
        let mut cell = *cell;
        if let Some(template) = template {
            let Some(chunk) = template
                .chunks
                .iter()
                .find(|chunk| map_chunk(cell.tile, chunk, template.chunk_size).is_some())
            else {
                continue;
            };
            cell.tile = map_chunk(cell.tile, chunk, template.chunk_size)
                .ok_or_else(|| invalid_state("Unmapped transform cell."))?;
            cell.blocked_movement = rotate_mask(cell.blocked_movement, chunk.quarter_turns);
            cell.blocked_sight = rotate_mask(cell.blocked_sight, chunk.quarter_turns);
        }
        if !cells.contains_key(&cell.tile) {
            return Err(invalid_state("Collision transform cannot invent a cell."));
        }
        if overrides
            .get(&cell.tile)
            .is_some_and(|previous| previous != &cell)
        {
            return Err(invalid_state(
                "Conflicting collision replacements need a declared combined group.",
            ));
        }
        overrides.insert(cell.tile, cell);
        cells.insert(cell.tile, cell);
    }
    Ok(())
}

pub(crate) fn map_footprint(
    template: &InstanceTemplateDefinition,
    origin: Tile,
    width: u8,
    height: u8,
) -> GameResult<(Tile, u8, u8)> {
    let mut tiles = std::collections::BTreeSet::new();
    for x in 0..width {
        for y in 0..height {
            let source = origin
                .offset(i16::from(x), i16::from(y))
                .ok_or_else(|| invalid_state("Footprint coordinate overflow."))?;
            let destination = template
                .chunks
                .iter()
                .find_map(|chunk| map_chunk(source, chunk, template.chunk_size))
                .ok_or_else(|| {
                    invalid_state("Whole entity footprint must be mapped into the instance.")
                })?;
            tiles.insert(destination);
        }
    }
    let plane = tiles
        .first()
        .ok_or_else(|| invalid_state("Empty entity footprint."))?
        .plane();
    let min_x = tiles
        .iter()
        .map(|tile| tile.x())
        .min()
        .ok_or_else(|| invalid_state("Empty footprint."))?;
    let max_x = tiles
        .iter()
        .map(|tile| tile.x())
        .max()
        .ok_or_else(|| invalid_state("Empty footprint."))?;
    let min_y = tiles
        .iter()
        .map(|tile| tile.y())
        .min()
        .ok_or_else(|| invalid_state("Empty footprint."))?;
    let max_y = tiles
        .iter()
        .map(|tile| tile.y())
        .max()
        .ok_or_else(|| invalid_state("Empty footprint."))?;
    let mapped_width = max_x - min_x + 1;
    let mapped_height = max_y - min_y + 1;
    if tiles.iter().any(|tile| tile.plane() != plane)
        || tiles.len() != usize::from(mapped_width) * usize::from(mapped_height)
    {
        return Err(crate::unavailable(
            "Instance chunks split a source entity footprint across noncontiguous cells.",
        ));
    }
    Ok((
        Tile::new(min_x, min_y, plane)?,
        u8::try_from(mapped_width)
            .map_err(|_| invalid_state("Mapped footprint width overflow."))?,
        u8::try_from(mapped_height)
            .map_err(|_| invalid_state("Mapped footprint height overflow."))?,
    ))
}

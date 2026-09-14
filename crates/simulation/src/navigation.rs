use std::collections::{BTreeMap, BTreeSet, VecDeque};

use clubscape_game_types::{
    CollisionCell, Direction, GameError, GameErrorCode, GameEvent, GameResult, RegionDefinition,
    Tile,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollisionMap {
    cells: BTreeMap<Tile, CollisionCell>,
}

#[derive(Clone, Copy)]
enum Mask {
    Movement,
    Sight,
}

impl CollisionMap {
    /// Bounds are inclusive in X, Y and plane. Missing cells remain blocked.
    /// Duplicate regions/cells and cells outside their declared region are rejected.
    pub fn from_regions<'a>(
        regions: impl IntoIterator<Item = &'a RegionDefinition>,
    ) -> GameResult<Self> {
        let mut cells = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for region in regions {
            if !ids.insert(&region.id)
                || region.min.x() > region.max.x()
                || region.min.y() > region.max.y()
                || region.min.plane() > region.max.plane()
            {
                return Err(GameError::new(
                    GameErrorCode::InvalidContent,
                    format!(
                        "Region {} has duplicate identity or invalid bounds.",
                        region.id
                    ),
                ));
            }
            for cell in &region.cells {
                let tile = cell.tile;
                if tile.x() < region.min.x()
                    || tile.x() > region.max.x()
                    || tile.y() < region.min.y()
                    || tile.y() > region.max.y()
                    || tile.plane() < region.min.plane()
                    || tile.plane() > region.max.plane()
                {
                    return Err(GameError::new(
                        GameErrorCode::InvalidContent,
                        format!("Cell {tile:?} is outside region {}.", region.id),
                    ));
                }
                if cells.insert(tile, *cell).is_some() {
                    return Err(GameError::new(
                        GameErrorCode::InvalidContent,
                        format!("Multiple region cells define {tile:?}."),
                    ));
                }
            }
        }
        Ok(Self { cells })
    }

    pub fn cell(&self, tile: Tile) -> Option<&CollisionCell> {
        self.cells.get(&tile)
    }

    /// Checks both endpoints' directional masks and all four cardinal corner edges.
    pub fn can_step(&self, from: Tile, to: Tile) -> bool {
        self.can_cross(from, to, Mask::Movement)
    }

    /// Shortest eight-direction, unit-cost path, excluding start and including goal.
    /// Ties use N, E, S, W, NE, SE, SW, NW. The bound counts discovered cells,
    /// including the start; exhausting it is an explicit error, never a partial path.
    pub fn find_path(&self, start: Tile, goal: Tile, max_visited: usize) -> GameResult<Vec<Tile>> {
        if max_visited == 0 {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Path search requires a positive visited-cell bound.",
            ));
        }
        self.require_walkable(start, "source")?;
        self.require_walkable(goal, "destination")?;
        if start.plane() != goal.plane() {
            return Err(GameError::new(
                GameErrorCode::OutOfReach,
                "Walking cannot cross planes.",
            ));
        }
        if start == goal {
            return Ok(Vec::new());
        }
        let mut predecessors = BTreeMap::from([(start, start)]);
        let mut frontier = VecDeque::from([start]);
        while let Some(current) = frontier.pop_front() {
            for direction in Direction::ALL {
                let (dx, dy) = direction.offset();
                let Some(next) = current.offset(dx, dy) else {
                    continue;
                };
                if predecessors.contains_key(&next) || !self.can_step(current, next) {
                    continue;
                }
                if predecessors.len() >= max_visited {
                    return Err(GameError::new(
                        GameErrorCode::Blocked,
                        format!("Path search reached its {max_visited}-cell bound."),
                    ));
                }
                predecessors.insert(next, current);
                if next == goal {
                    return reconstruct_path(&predecessors, start, goal);
                }
                frontier.push_back(next);
            }
        }
        Err(GameError::new(
            GameErrorCode::Blocked,
            format!("No reachable path from {start:?} to {goal:?}."),
        ))
    }

    /// Integer, symmetric center-to-center supercover traversal.
    /// Sight masks are independent of walkability and movement masks.
    /// A corner crossing checks both adjoining routes and diagonal sight flags.
    pub fn line_of_sight(&self, from: Tile, to: Tile) -> bool {
        if from.plane() != to.plane()
            || !self.cells.contains_key(&from)
            || !self.cells.contains_key(&to)
        {
            return false;
        }
        let dx = u64::from(from.x().abs_diff(to.x()));
        let dy = u64::from(from.y().abs_diff(to.y()));
        let step_x = sign(from.x(), to.x());
        let step_y = sign(from.y(), to.y());
        let mut crossed_x = 0_u64;
        let mut crossed_y = 0_u64;
        let mut current = from;
        for _ in 0..(dx + dy) {
            if current == to {
                return true;
            }
            let next_x_crossing = (2 * crossed_x + 1) * dy;
            let next_y_crossing = (2 * crossed_y + 1) * dx;
            let (move_x, move_y) = match next_x_crossing.cmp(&next_y_crossing) {
                std::cmp::Ordering::Less => (step_x, 0),
                std::cmp::Ordering::Greater => (0, step_y),
                std::cmp::Ordering::Equal => (step_x, step_y),
            };
            let Some(next) = current.offset(move_x, move_y) else {
                return false;
            };
            if !self.can_cross(current, next, Mask::Sight) {
                return false;
            }
            crossed_x += u64::from(move_x != 0);
            crossed_y += u64::from(move_y != 0);
            current = next;
        }
        current == to
    }

    /// Consumes one walking or at most two running steps for one caller-owned tick.
    /// A rejected prefix leaves both position and path unchanged; callers can replan.
    /// Emits one movement event per traversed tile and does not invent energy costs.
    pub fn step_path(
        &self,
        position: &mut Tile,
        path: &mut Vec<Tile>,
        running: bool,
    ) -> GameResult<Vec<GameEvent>> {
        self.require_walkable(*position, "source")?;
        let limit = if running { 2 } else { 1 };
        let mut current = *position;
        let mut events = Vec::new();
        for next in path.iter().take(limit) {
            if !self.can_step(current, *next) {
                return Err(GameError::new(
                    GameErrorCode::Blocked,
                    format!("Path step from {current:?} to {next:?} is blocked or nonadjacent."),
                ));
            }
            current = *next;
            events.push(GameEvent::Moved { tile: current });
        }
        path.drain(..events.len());
        *position = current;
        Ok(events)
    }

    fn can_cross(&self, from: Tile, to: Tile, mask: Mask) -> bool {
        if !self.edge_clear(from, to, mask) {
            return false;
        }
        let Some(direction) = direction_between(from, to) else {
            return false;
        };
        let (dx, dy) = direction.offset();
        if dx == 0 || dy == 0 {
            return true;
        }
        let (Some(across_x), Some(across_y)) = (from.offset(dx, 0), from.offset(0, dy)) else {
            return false;
        };
        self.edge_clear(from, across_x, mask)
            && self.edge_clear(across_x, to, mask)
            && self.edge_clear(from, across_y, mask)
            && self.edge_clear(across_y, to, mask)
    }

    fn edge_clear(&self, from: Tile, to: Tile, mask: Mask) -> bool {
        let Some(direction) = direction_between(from, to) else {
            return false;
        };
        let (Some(source), Some(destination)) = (self.cells.get(&from), self.cells.get(&to)) else {
            return false;
        };
        let (source_mask, destination_mask) = match mask {
            Mask::Movement => {
                if !source.walkable || !destination.walkable {
                    return false;
                }
                (source.blocked_movement, destination.blocked_movement)
            }
            Mask::Sight => (source.blocked_sight, destination.blocked_sight),
        };
        source_mask & direction.mask() == 0 && destination_mask & direction.opposite().mask() == 0
    }

    fn require_walkable(&self, tile: Tile, endpoint: &str) -> GameResult<()> {
        let cell = self.cells.get(&tile).ok_or_else(|| {
            GameError::new(
                GameErrorCode::OutOfReach,
                format!("Path {endpoint} {tile:?} has no listed collision cell."),
            )
        })?;
        if !cell.walkable {
            return Err(GameError::new(
                GameErrorCode::Blocked,
                format!("Path {endpoint} {tile:?} is not walkable."),
            ));
        }
        Ok(())
    }
}

fn direction_between(from: Tile, to: Tile) -> Option<Direction> {
    if from.distance(to) != Some(1) {
        return None;
    }
    Direction::ALL.into_iter().find(|direction| {
        let (dx, dy) = direction.offset();
        from.offset(dx, dy) == Some(to)
    })
}

fn sign(from: u16, to: u16) -> i16 {
    match from.cmp(&to) {
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => -1,
    }
}

fn reconstruct_path(
    predecessors: &BTreeMap<Tile, Tile>,
    start: Tile,
    goal: Tile,
) -> GameResult<Vec<Tile>> {
    let mut reversed = Vec::new();
    let mut current = goal;
    for _ in 0..predecessors.len() {
        if current == start {
            reversed.reverse();
            return Ok(reversed);
        }
        reversed.push(current);
        current = *predecessors.get(&current).ok_or_else(|| {
            GameError::new(GameErrorCode::InvalidInput, "Path predecessor is missing.")
        })?;
    }
    Err(GameError::new(
        GameErrorCode::InvalidInput,
        "Path predecessor chain is cyclic.",
    ))
}

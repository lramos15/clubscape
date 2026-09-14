use clubscape_game_types::{Direction, GameErrorCode::*, GameEvent};
use clubscape_simulation::navigation::CollisionMap;

use crate::support::*;

#[test]
fn unlisted_tiles_are_blocked_even_inside_region_bounds() {
    let mut region = region("sparse", tile(0, 0, 0), tile(2, 2, 0));
    region
        .cells
        .retain(|cell| cell.tile == tile(0, 0, 0) || cell.tile == tile(2, 2, 0));
    let map = CollisionMap::from_regions([&region]).unwrap();
    assert!(map.cell(tile(1, 1, 0)).is_none());
    assert!(!map.can_step(tile(0, 0, 0), tile(1, 1, 0)));
    assert!(!map.line_of_sight(tile(0, 0, 0), tile(2, 2, 0)));
    assert_error(map.find_path(tile(0, 0, 0), tile(2, 2, 0), 20), Blocked);
}

#[test]
fn empty_region_definitions_do_not_create_a_walkable_default_map() {
    let mut region = region("empty", tile(0, 0, 0), tile(1, 1, 0));
    region.cells.clear();
    let map = CollisionMap::from_regions([&region]).unwrap();
    assert!(!map.can_step(tile(0, 0, 0), tile(1, 0, 0)));
    assert!(!map.line_of_sight(tile(0, 0, 0), tile(0, 0, 0)));
    assert_error(map.find_path(tile(0, 0, 0), tile(0, 0, 0), 1), OutOfReach);
}

#[test]
fn every_canonical_mask_bit_and_bilateral_movement_edge_is_observed() {
    let center = tile(2, 2, 0);
    for (direction, expected_mask) in [
        (Direction::North, 1),
        (Direction::East, 2),
        (Direction::South, 4),
        (Direction::West, 8),
        (Direction::NorthEast, 16),
        (Direction::SouthEast, 32),
        (Direction::SouthWest, 64),
        (Direction::NorthWest, 128),
    ] {
        assert_eq!(direction.mask(), expected_mask);
        let (dx, dy) = direction.offset();
        let neighbor = center.offset(dx, dy).unwrap();
        let mut region = region("edges", tile(0, 0, 0), tile(4, 4, 0));
        let map = CollisionMap::from_regions([&region]).unwrap();
        assert!(map.can_step(center, neighbor));
        cell_mut(&mut region, center).blocked_movement = expected_mask;
        let map = CollisionMap::from_regions([&region]).unwrap();
        assert!(!map.can_step(center, neighbor));
        assert!(!map.can_step(neighbor, center));
        cell_mut(&mut region, center).blocked_movement = 0;
        cell_mut(&mut region, neighbor).blocked_movement = direction.opposite().mask();
        let map = CollisionMap::from_regions([&region]).unwrap();
        assert!(!map.can_step(center, neighbor));
        assert!(!map.can_step(neighbor, center));
    }
}

#[test]
fn diagonal_movement_checks_both_cardinal_routes_and_both_sides_of_every_edge() {
    let from = tile(1, 1, 0);
    let to = tile(2, 2, 0);
    let across_x = tile(2, 1, 0);
    let across_y = tile(1, 2, 0);
    for (edge_from, edge_to, direction) in [
        (from, across_x, Direction::East),
        (across_x, to, Direction::North),
        (from, across_y, Direction::North),
        (across_y, to, Direction::East),
    ] {
        for on_destination in [false, true] {
            let mut region = region("corners", tile(0, 0, 0), tile(3, 3, 0));
            let (cell, mask) = if on_destination {
                (edge_to, direction.opposite().mask())
            } else {
                (edge_from, direction.mask())
            };
            cell_mut(&mut region, cell).blocked_movement = mask;
            let map = CollisionMap::from_regions([&region]).unwrap();
            assert!(
                !map.can_step(from, to),
                "{edge_from:?} {edge_to:?} {on_destination}"
            );
            assert!(!map.can_step(to, from));
        }
    }
}

#[test]
fn missing_or_unwalkable_corner_cells_block_diagonals() {
    for corner in [tile(2, 1, 0), tile(1, 2, 0)] {
        for missing in [false, true] {
            let mut region = region("corner_cells", tile(0, 0, 0), tile(3, 3, 0));
            if missing {
                region.cells.retain(|cell| cell.tile != corner);
            } else {
                cell_mut(&mut region, corner).walkable = false;
            }
            let map = CollisionMap::from_regions([&region]).unwrap();
            assert!(!map.can_step(tile(1, 1, 0), tile(2, 2, 0)));
        }
    }
}

#[test]
fn different_planes_are_never_adjacent_or_connected_by_walking_or_sight() {
    let region = region("planes", tile(0, 0, 0), tile(2, 2, 1));
    let map = CollisionMap::from_regions([&region]).unwrap();
    for destination in [tile(0, 0, 1), tile(1, 0, 1)] {
        assert!(!map.can_step(tile(0, 0, 0), destination));
        assert!(!map.line_of_sight(tile(0, 0, 0), destination));
        assert_error(map.find_path(tile(0, 0, 0), destination, 50), OutOfReach);
    }
}

#[test]
fn adjacent_listed_cells_can_cross_region_boundaries() {
    let left = region("left", tile(0, 0, 0), tile(1, 2, 0));
    let right = region("right", tile(2, 0, 0), tile(3, 2, 0));
    let map = CollisionMap::from_regions([&left, &right]).unwrap();
    assert!(map.can_step(tile(1, 1, 0), tile(2, 1, 0)));
    assert!(map.can_step(tile(1, 1, 0), tile(2, 2, 0)));
    assert!(map.line_of_sight(tile(0, 1, 0), tile(3, 1, 0)));
    assert_eq!(
        map.find_path(tile(1, 1, 0), tile(2, 1, 0), 20).unwrap(),
        vec![tile(2, 1, 0)]
    );
}

#[test]
fn reversed_region_bounds_and_out_of_bounds_cells_are_invalid_content() {
    let original = region("bounds", tile(1, 1, 1), tile(2, 2, 2));
    for min in [tile(3, 1, 1), tile(1, 3, 1), tile(1, 1, 3)] {
        let mut region = original.clone();
        region.min = min;
        assert_error(CollisionMap::from_regions([&region]), InvalidContent);
    }
    for outside in [
        tile(0, 1, 1),
        tile(3, 1, 1),
        tile(1, 0, 1),
        tile(1, 3, 1),
        tile(1, 1, 0),
        tile(1, 1, 3),
    ] {
        let mut region = original.clone();
        region.cells.first_mut().unwrap().tile = outside;
        assert_error(CollisionMap::from_regions([&region]), InvalidContent);
    }
}

#[test]
fn duplicate_region_ids_or_cells_cannot_silently_override_collision() {
    let first = region("first", tile(0, 0, 0), tile(1, 1, 0));
    let mut second = region("second", tile(2, 0, 0), tile(3, 1, 0));
    second.id = first.id.clone();
    assert_error(
        CollisionMap::from_regions([&first, &second]),
        InvalidContent,
    );
    let overlap = region("overlap", tile(1, 1, 0), tile(2, 2, 0));
    assert_error(
        CollisionMap::from_regions([&first, &overlap]),
        InvalidContent,
    );
    let mut duplicate = first.clone();
    duplicate.cells.push(*duplicate.cells.first().unwrap());
    assert_error(CollisionMap::from_regions([&duplicate]), InvalidContent);
}

#[test]
fn path_endpoints_must_be_listed_and_walkable_even_for_an_empty_path() {
    let mut region = region("endpoints", tile(1, 1, 0), tile(3, 3, 0));
    cell_mut(&mut region, tile(2, 2, 0)).walkable = false;
    let map = CollisionMap::from_regions([&region]).unwrap();
    assert_error(map.find_path(tile(0, 1, 0), tile(1, 1, 0), 20), OutOfReach);
    assert_error(map.find_path(tile(1, 1, 0), tile(4, 1, 0), 20), OutOfReach);
    assert_error(map.find_path(tile(0, 1, 0), tile(0, 1, 0), 20), OutOfReach);
    assert_error(map.find_path(tile(2, 2, 0), tile(1, 1, 0), 20), Blocked);
    assert_error(map.find_path(tile(1, 1, 0), tile(2, 2, 0), 20), Blocked);
    assert_error(map.find_path(tile(2, 2, 0), tile(2, 2, 0), 20), Blocked);
    assert_eq!(
        map.find_path(tile(1, 1, 0), tile(1, 1, 0), 1).unwrap(),
        vec![]
    );
}

#[test]
fn pathfinding_goes_around_walls_without_straight_line_or_corner_skips() {
    let start = tile(0, 1, 0);
    let goal = tile(4, 1, 0);
    let mut region = region("detour", tile(0, 0, 0), tile(4, 3, 0));
    for y in 0..=2 {
        cell_mut(&mut region, tile(2, y, 0)).walkable = false;
    }
    let map = CollisionMap::from_regions([&region]).unwrap();
    let path = map.find_path(start, goal, 20).unwrap();
    assert_eq!(path.len(), 6);
    assert_eq!(path.last(), Some(&goal));
    assert!(path.contains(&tile(2, 3, 0)));
    let mut previous = start;
    for next in path {
        assert!(map.can_step(previous, next));
        previous = next;
    }
}

#[test]
fn unreachable_paths_return_an_error_not_a_partial_route() {
    let mut region = region("sealed", tile(0, 0, 0), tile(4, 4, 0));
    for x in 0..=4 {
        cell_mut(&mut region, tile(x, 2, 0)).walkable = false;
    }
    let map = CollisionMap::from_regions([&region]).unwrap();
    let error = map.find_path(tile(0, 0, 0), tile(4, 4, 0), 25).unwrap_err();
    assert_eq!(error.code, Blocked);
    assert!(error.message.contains("No reachable path"));
}

#[test]
fn search_budget_is_explicit_bounded_and_counts_the_start() {
    let region = region("budget", tile(0, 0, 0), tile(2, 2, 0));
    let map = CollisionMap::from_regions([&region]).unwrap();
    let start = tile(0, 0, 0);
    let goal = tile(0, 1, 0);
    assert_error(map.find_path(start, goal, 0), InvalidInput);
    let error = map.find_path(start, goal, 1).unwrap_err();
    assert_eq!(error.code, Blocked);
    assert!(error.message.contains("bound"));
    assert_eq!(map.find_path(start, goal, 2).unwrap(), vec![goal]);
    assert_eq!(map.find_path(start, start, 1).unwrap(), vec![]);
    assert_eq!(map.find_path(start, goal, usize::MAX).unwrap(), vec![goal]);
}

#[test]
fn equal_length_paths_have_stable_direction_order_independent_of_cell_order() {
    let mut region = region("ties", tile(0, 0, 0), tile(2, 2, 0));
    let expected = vec![tile(1, 0, 0), tile(2, 1, 0)];
    for _ in 0..16 {
        let map = CollisionMap::from_regions([&region]).unwrap();
        assert_eq!(
            map.find_path(tile(0, 0, 0), tile(2, 1, 0), 9).unwrap(),
            expected
        );
        region.cells.reverse();
    }
}

#[test]
fn directional_walls_force_a_real_detour_in_pathfinding() {
    let mut region = region("directional_path", tile(0, 0, 0), tile(3, 3, 0));
    let start = tile(1, 1, 0);
    let goal = tile(2, 1, 0);
    cell_mut(&mut region, start).blocked_movement = Direction::East.mask();
    let map = CollisionMap::from_regions([&region]).unwrap();
    let path = map.find_path(start, goal, 16).unwrap();
    assert_eq!(path.len(), 3);
    let mut previous = start;
    for next in path {
        assert!(map.can_step(previous, next));
        previous = next;
    }
    assert_eq!(previous, goal);
}

#[test]
fn world_coordinate_edges_do_not_wrap_or_panic() {
    let low = region("low", tile(0, 0, 0), tile(1, 1, 0));
    let high = region("high", tile(16382, 16382, 0), tile(16383, 16383, 0));
    let map = CollisionMap::from_regions([&low, &high]).unwrap();
    assert_eq!(
        map.find_path(tile(16383, 16383, 0), tile(16382, 16382, 0), 4)
            .unwrap(),
        vec![tile(16382, 16382, 0)]
    );
    assert_eq!(
        map.find_path(tile(0, 0, 0), tile(1, 1, 0), 4).unwrap(),
        vec![tile(1, 1, 0)]
    );
    assert!(!map.can_step(tile(0, 0, 0), tile(16383, 16383, 0)));
    assert_error(
        map.find_path(tile(0, 0, 0), tile(16383, 16383, 0), 8),
        Blocked,
    );
}

#[test]
fn sight_is_separate_from_walkability_and_movement_masks() {
    let mut region = region("sight_water", tile(0, 0, 0), tile(4, 2, 0));
    for cell in &mut region.cells {
        cell.walkable = false;
        cell.blocked_movement = u8::MAX;
    }
    let map = CollisionMap::from_regions([&region]).unwrap();
    assert!(map.line_of_sight(tile(0, 1, 0), tile(4, 1, 0)));
    assert!(map.line_of_sight(tile(0, 0, 0), tile(4, 2, 0)));
    assert!(!map.can_step(tile(0, 1, 0), tile(1, 1, 0)));
    cell_mut(&mut region, tile(2, 1, 0)).blocked_sight = Direction::East.mask();
    let map = CollisionMap::from_regions([&region]).unwrap();
    assert!(!map.line_of_sight(tile(0, 1, 0), tile(4, 1, 0)));
}

#[test]
fn a_sight_only_wall_does_not_block_movement() {
    let mut region = region("sight_only", tile(0, 0, 0), tile(1, 0, 0));
    cell_mut(&mut region, tile(0, 0, 0)).blocked_sight = Direction::East.mask();
    let map = CollisionMap::from_regions([&region]).unwrap();
    assert!(map.can_step(tile(0, 0, 0), tile(1, 0, 0)));
    assert!(!map.line_of_sight(tile(0, 0, 0), tile(1, 0, 0)));
    assert!(!map.line_of_sight(tile(1, 0, 0), tile(0, 0, 0)));
}

#[test]
fn every_sight_direction_checks_source_and_destination_masks() {
    let center = tile(2, 2, 0);
    for direction in Direction::ALL {
        let (dx, dy) = direction.offset();
        let neighbor = center.offset(dx, dy).unwrap();
        for on_destination in [false, true] {
            let mut region = region("sight_edges", tile(0, 0, 0), tile(4, 4, 0));
            if on_destination {
                cell_mut(&mut region, neighbor).blocked_sight = direction.opposite().mask();
            } else {
                cell_mut(&mut region, center).blocked_sight = direction.mask();
            }
            let map = CollisionMap::from_regions([&region]).unwrap();
            assert!(!map.line_of_sight(center, neighbor));
            assert!(!map.line_of_sight(neighbor, center));
        }
    }
}

#[test]
fn sight_checks_each_corner_route_and_requires_listed_corner_cells() {
    let from = tile(1, 1, 0);
    let to = tile(2, 2, 0);
    for (cell, mask) in [
        (from, Direction::East.mask()),
        (tile(2, 1, 0), Direction::North.mask()),
        (from, Direction::North.mask()),
        (tile(1, 2, 0), Direction::East.mask()),
        (to, Direction::West.mask()),
        (to, Direction::South.mask()),
    ] {
        let mut region = region("sight_corners", tile(0, 0, 0), tile(3, 3, 0));
        cell_mut(&mut region, cell).blocked_sight = mask;
        let map = CollisionMap::from_regions([&region]).unwrap();
        assert!(!map.line_of_sight(from, to));
        assert!(!map.line_of_sight(to, from));
    }
    let mut region = region("sight_missing_corner", tile(0, 0, 0), tile(3, 3, 0));
    region.cells.retain(|cell| cell.tile != tile(2, 1, 0));
    let map = CollisionMap::from_regions([&region]).unwrap();
    assert!(!map.line_of_sight(from, to));
}

#[test]
fn supercover_sight_does_not_skip_a_crossed_cell_on_a_shallow_line() {
    let mut region = region("supercover", tile(0, 0, 0), tile(4, 2, 0));
    let map = CollisionMap::from_regions([&region]).unwrap();
    assert!(map.line_of_sight(tile(0, 0, 0), tile(4, 2, 0)));
    region.cells.retain(|cell| cell.tile != tile(1, 0, 0));
    let map = CollisionMap::from_regions([&region]).unwrap();
    assert!(!map.line_of_sight(tile(0, 0, 0), tile(4, 2, 0)));
    assert!(!map.line_of_sight(tile(4, 2, 0), tile(0, 0, 0)));
}

#[test]
fn sight_requires_both_endpoints_and_handles_same_tile_without_crossing_edges() {
    let mut region = region("sight_bounds", tile(1, 1, 0), tile(2, 2, 0));
    cell_mut(&mut region, tile(1, 1, 0)).blocked_sight = u8::MAX;
    let map = CollisionMap::from_regions([&region]).unwrap();
    assert!(map.line_of_sight(tile(1, 1, 0), tile(1, 1, 0)));
    assert!(!map.line_of_sight(tile(0, 1, 0), tile(1, 1, 0)));
    assert!(!map.line_of_sight(tile(1, 1, 0), tile(3, 1, 0)));
    assert!(!map.line_of_sight(tile(1, 1, 0), tile(1, 1, 1)));
}

#[test]
fn sight_is_symmetric_for_all_pairs_through_mixed_directional_walls() {
    let mut region = region("symmetric", tile(0, 0, 0), tile(4, 4, 0));
    cell_mut(&mut region, tile(2, 1, 0)).blocked_sight =
        Direction::North.mask() | Direction::NorthWest.mask();
    cell_mut(&mut region, tile(1, 2, 0)).blocked_sight = Direction::East.mask();
    cell_mut(&mut region, tile(3, 3, 0)).blocked_sight = Direction::SouthEast.mask();
    let map = CollisionMap::from_regions([&region]).unwrap();
    for from in &region.cells {
        for to in &region.cells {
            assert_eq!(
                map.line_of_sight(from.tile, to.tile),
                map.line_of_sight(to.tile, from.tile),
                "{:?} {:?}",
                from.tile,
                to.tile,
            );
        }
    }
}

#[test]
fn path_stepping_walks_one_tile_or_runs_at_most_two_per_tick() {
    let region = region("ticks", tile(0, 0, 0), tile(4, 0, 0));
    let map = CollisionMap::from_regions([&region]).unwrap();
    let mut position = tile(0, 0, 0);
    let mut path = vec![tile(1, 0, 0), tile(2, 0, 0), tile(3, 0, 0), tile(4, 0, 0)];
    assert_eq!(
        map.step_path(&mut position, &mut path, false).unwrap(),
        vec![GameEvent::Moved {
            tile: tile(1, 0, 0)
        }]
    );
    assert_eq!(position, tile(1, 0, 0));
    assert_eq!(path.len(), 3);
    assert_eq!(
        map.step_path(&mut position, &mut path, true).unwrap(),
        vec![
            GameEvent::Moved {
                tile: tile(2, 0, 0)
            },
            GameEvent::Moved {
                tile: tile(3, 0, 0)
            }
        ],
    );
    assert_eq!(position, tile(3, 0, 0));
    assert_eq!(path, vec![tile(4, 0, 0)]);
    assert_eq!(
        map.step_path(&mut position, &mut path, true).unwrap().len(),
        1
    );
    assert_eq!(position, tile(4, 0, 0));
    assert!(path.is_empty());
    assert!(
        map.step_path(&mut position, &mut path, true)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn invalid_running_prefix_is_atomic_and_does_not_teleport_or_consume_path() {
    let region = region("bad_prefix", tile(0, 0, 0), tile(4, 1, 1));
    let map = CollisionMap::from_regions([&region]).unwrap();
    for invalid_second in [tile(3, 0, 0), tile(2, 0, 1), tile(1, 0, 0), tile(1, 2, 0)] {
        let mut position = tile(0, 0, 0);
        let mut path = vec![tile(1, 0, 0), invalid_second];
        let before = (position, path.clone());
        assert_error(map.step_path(&mut position, &mut path, true), Blocked);
        assert_eq!((position, path), before);
    }
}

#[test]
fn walking_validates_only_this_ticks_prefix_and_rejects_the_later_bad_step() {
    let region = region("walk_prefix", tile(0, 0, 0), tile(4, 0, 0));
    let map = CollisionMap::from_regions([&region]).unwrap();
    let mut position = tile(0, 0, 0);
    let mut path = vec![tile(1, 0, 0), tile(4, 0, 0)];
    map.step_path(&mut position, &mut path, false).unwrap();
    assert_eq!(position, tile(1, 0, 0));
    let before = (position, path.clone());
    assert_error(map.step_path(&mut position, &mut path, false), Blocked);
    assert_eq!((position, path), before);
}

#[test]
fn stale_path_and_invalid_source_are_rejected_before_mutation() {
    let mut region = region("stale", tile(0, 0, 0), tile(2, 0, 0));
    let map = CollisionMap::from_regions([&region]).unwrap();
    let mut position = tile(0, 0, 0);
    let mut path = map.find_path(position, tile(2, 0, 0), 3).unwrap();
    cell_mut(&mut region, tile(1, 0, 0)).blocked_movement = Direction::East.mask();
    let blocked = CollisionMap::from_regions([&region]).unwrap();
    let before = (position, path.clone());
    assert_error(blocked.step_path(&mut position, &mut path, true), Blocked);
    assert_eq!((position, path.clone()), before);
    position = tile(3, 0, 0);
    let before = (position, path.clone());
    assert_error(
        blocked.step_path(&mut position, &mut path, false),
        OutOfReach,
    );
    assert_eq!((position, path), before);
}

#[test]
fn all_open_grid_pairs_have_shortest_chebyshev_paths_and_legal_steps() {
    let region = region("all_pairs", tile(0, 0, 0), tile(4, 4, 0));
    let map = CollisionMap::from_regions([&region]).unwrap();
    for from in &region.cells {
        for to in &region.cells {
            let path = map.find_path(from.tile, to.tile, 25).unwrap();
            assert_eq!(
                path.len(),
                usize::from(from.tile.distance(to.tile).unwrap())
            );
            let mut previous = from.tile;
            for next in path {
                assert!(map.can_step(previous, next));
                previous = next;
            }
            assert_eq!(previous, to.tile);
        }
    }
}

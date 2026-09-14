"""Explicit native-coordinate collision reconstruction; scene data is never relocated."""

from array import array
from collections import Counter, deque

from common import ASSET_PREFIX, SOURCE, load, region_id, source_record, tile


DIRECTIONS = ((0, 1, 1), (1, 0, 2), (0, -1, 4), (-1, 0, 8),
              (1, 1, 16), (1, -1, 32), (-1, -1, 64), (-1, 1, 128))
SOURCE_DIRECTIONS = ((-1, 1, 1), (0, 1, 2), (1, 1, 4), (1, 0, 8),
                     (1, -1, 16), (0, -1, 32), (-1, -1, 64), (-1, 0, 128))
SOURCE_TO_CANONICAL = {1: 128, 2: 1, 4: 16, 8: 2, 16: 32, 32: 4, 64: 64, 128: 8}
OPPOSITE = {1: 4, 2: 8, 4: 1, 8: 2, 16: 64, 32: 128, 64: 16, 128: 32}
SOURCE_OPPOSITE = {1: 16, 2: 32, 4: 64, 8: 128, 16: 1, 32: 2, 64: 4, 128: 8}
FULL_MOVEMENT = 0x100 | 0x40000 | 0x200000
FULL_SIGHT = 0x20000


def canonical_mask(flags):
    return sum(value for bit, value in SOURCE_TO_CANONICAL.items() if flags & bit)


def wall_edges(kind, orientation):
    if kind == 0:
        return ((128, 2, 8, 32)[orientation],)
    if kind in (1, 3):
        return ((1, 4, 16, 64)[orientation],)
    if kind == 2:
        sides = (128, 2, 8, 32)
        return sides[orientation], sides[(orientation + 1) % 4]
    return ()


def clipped_footprint(definition, orientation):
    x, y = definition["sizeX"], definition["sizeY"]
    return (y, x) if orientation & 1 else (x, y)


class World:
    def __init__(self, inputs, omit_openable_doors=False, omit_placements=()):
        self.inputs = inputs
        self.raw = {value["region_id"]: value for value in (
            load(path) for path in sorted((SOURCE / "world").glob("*.json.gz")))}
        self.flags = {number: array("I", [0]) * 16384 for number in self.raw}
        self.support = {number: bytearray(16384) for number in self.raw}
        self.height_plane = {number: bytearray(16384) for number in self.raw}
        self.statistics = Counter()
        self.placements = []
        self.openable_doors = []
        self.outside_clipping = Counter()
        self.placement_tiles = {(row[1], row[2], row[3])
                                for raw in self.raw.values() for row in raw["placements"]}
        omitted = set(omit_placements)
        for number, raw in self.raw.items():
            if raw["dimensions"] != [4, 64, 64]:
                raise ValueError(f"Unexpected source square dimensions: {number}")
            for key in ("heights", "underlay_ids", "overlay_ids", "overlay_shapes",
                        "overlay_rotations", "tile_settings", "encoded_heights"):
                if len(raw[key]) != 16384:
                    raise ValueError(f"Incomplete source tile array {number}/{key}")
            for p in range(4):
                for x in range(64):
                    for y in range(64):
                        index = self.index(p, x, y)
                        bridge = bool(raw["tile_settings"][self.index(1, x, y)] & 2)
                        visual_plane = p + int(bridge)
                        self.height_plane[number][index] = min(visual_plane, 3)
                        if visual_plane < 4:
                            visual_index = self.index(visual_plane, x, y)
                            self.support[number][index] = bool(
                                raw["underlay_ids"][visual_index] or raw["overlay_ids"][visual_index])
                        if raw["tile_settings"][index] & 1:
                            collision_plane = p - int(bridge)
                            if collision_plane >= 0:
                                self.flags[number][self.index(collision_plane, x, y)] |= 0x200000
                                self.statistics["floor_blocks"] += 1
                        if bridge and p == 0:
                            self.statistics["bridge_columns"] += 1
            for row in raw["placements"]:
                number_id, x, y, plane, kind, orientation = row
                if not (raw["base_x"] <= x < raw["base_x"] + 64 and
                        raw["base_y"] <= y < raw["base_y"] + 64 and
                        0 <= plane < 4 and 0 <= kind <= 22 and 0 <= orientation < 4):
                    raise ValueError(f"Invalid original placement {row}")
                self.placements.append((number, row))
                definition = inputs.collections["object"][number_id]
                operations = [op["text"] for op in definition["ops"]["ops"] if op]
                is_door = kind in (0, 1, 2, 3) and "Open" in operations and (
                    "door" in definition["name"].lower() or "gate" in definition["name"].lower())
                if is_door:
                    self.openable_doors.append(inputs.object_spawn_id(row))
                if tuple(row) not in omitted and not (omit_openable_doors and is_door):
                    self.add_object(number, row, definition)
        self.statistics["source_regions"] = len(self.raw)
        self.statistics["explicit_full_cells"] = 16384 * len(self.raw)
        self.statistics["source_placements"] = len(self.placements)
        self.statistics["openable_wall_placements"] = len(self.openable_doors)
        self.statistics["clipping_past_source_envelope"] = sum(self.outside_clipping.values())

    @staticmethod
    def index(plane, x, y):
        return (plane * 64 + x) * 64 + y

    def locate(self, x, y, plane):
        number = ((x // 64) << 8) | (y // 64)
        if number not in self.raw or not 0 <= plane < 4:
            return None
        return number, self.index(plane, x & 63, y & 63)

    def add_flag(self, x, y, plane, flag):
        located = self.locate(x, y, plane)
        if located:
            number, index = located
            self.flags[number][index] |= flag
        else:
            self.outside_clipping[(x, y, plane)] += 1

    def add_object(self, number, row, definition):
        _, x, y, plane, kind, orientation = row
        settings = self.raw[number]["tile_settings"]
        collision_plane = plane - int(bool(settings[self.index(1, x & 63, y & 63)] & 2))
        if collision_plane < 0:
            self.statistics["below_bridge_placements"] += 1
            return
        interact_type = 0 if definition["isHollow"] else definition["interactType"]
        if not interact_type:
            return
        projectile = definition["blocksProjectile"] and not definition["isHollow"]
        if kind in (0, 1, 2, 3):
            for edge in wall_edges(kind, orientation):
                dx, dy, _ = next(d for d in SOURCE_DIRECTIONS if d[2] == edge)
                other = SOURCE_OPPOSITE[edge]
                self.add_flag(x, y, collision_plane, edge | ((edge << 9) if projectile else 0))
                self.add_flag(x + dx, y + dy, collision_plane,
                              other | ((other << 9) if projectile else 0))
            self.statistics["walls"] += 1
        elif kind == 22:
            if interact_type == 1:
                self.add_flag(x, y, collision_plane, 0x40000)
                self.statistics["blocking_floor_decorations"] += 1
        elif kind == 9 or kind in (10, 11) or 12 <= kind <= 21:
            size_x, size_y = clipped_footprint(definition, orientation)
            for dx in range(size_x):
                for dy in range(size_y):
                    self.add_flag(x + dx, y + dy, collision_plane,
                                  0x100 | (FULL_SIGHT if projectile else 0))
            self.statistics["blocking_objects"] += 1
        else:
            self.statistics["nonclipping_wall_decorations"] += 1

    def cell(self, x, y, plane):
        located = self.locate(x, y, plane)
        if located is None:
            return None
        number, index = located
        flags = self.flags[number][index]
        source_plane = self.height_plane[number][index]
        return {
            "tile": tile(x, y, plane),
            "height": self.raw[number]["heights"][self.index(source_plane, x & 63, y & 63)],
            "walkable": bool(self.support[number][index]) and not bool(flags & FULL_MOVEMENT),
            "blocked_movement": 255 if flags & FULL_MOVEMENT or not self.support[number][index]
                                else canonical_mask(flags),
            "blocked_sight": 255 if flags & FULL_SIGHT else canonical_mask(flags >> 9),
        }

    def in_navigation_envelope(self, x, y):
        return any(rect["min_x"] <= x <= rect["max_x"] and rect["min_y"] <= y <= rect["max_y"]
                   for rect in self.inputs.selection["navigation_envelopes"])

    def region(self, number, full=False):
        raw = self.raw[number]
        cells = []
        for plane in range(4):
            for local_x in range(64):
                x = raw["base_x"] + local_x
                for local_y in range(64):
                    y = raw["base_y"] + local_y
                    if not full and (not self.in_navigation_envelope(x, y) or
                                     (plane > 0 and not self.support[number][self.index(plane, local_x, local_y)]
                                      and (x, y, plane) not in self.placement_tiles)):
                        continue
                    cells.append(self.cell(x, y, plane))
        if not cells:
            return None
        return {
            "id": region_id(raw["base_x"], raw["base_y"]),
            "name": f"Source map square {number}",
            "min": tile(min(c["tile"]["x"] for c in cells), min(c["tile"]["y"] for c in cells),
                        min(c["tile"]["plane"] for c in cells)),
            "max": tile(max(c["tile"]["x"] for c in cells), max(c["tile"]["y"] for c in cells),
                        max(c["tile"]["plane"] for c in cells)),
            "cells": cells,
            "source_map_squares": [number],
            "scene_asset": f"{ASSET_PREFIX}region.{number}",
            "source": [
                source_record(f"assets/source/osrs/cache2695/world/{number}.json.gz",
                              "All original terrain, tile settings, bridge/roof flags and scenery remain "
                              "in the source scene; X east/Y north/128 source units per tile."),
                source_record("research/m1-bindings/code-sources.json#collision_algorithm",
                              "Source clipping reconstruction; upper-plane cells without an explicit floor "
                              "are nonwalkable. Dynamic door/morph and instance state is not observed.",
                              "inference", "collision-projection-v1"),
            ],
        }

    def can_step(self, start, end):
        x, y, plane = start
        ex, ey, ep = end
        dx, dy = ex - x, ey - y
        if ep != plane or (dx, dy) not in [(a, b) for a, b, _ in DIRECTIONS]:
            return False
        before, after = self.cell(x, y, plane), self.cell(ex, ey, plane)
        if not before or not after or not before["walkable"] or not after["walkable"]:
            return False
        mask = next(bit for a, b, bit in DIRECTIONS if (a, b) == (dx, dy))
        if before["blocked_movement"] & mask or after["blocked_movement"] & OPPOSITE[mask]:
            return False
        if dx and dy:
            side_x, side_y = (x + dx, y, plane), (x, y + dy, plane)
            return (self.can_step(start, side_x) and self.can_step(start, side_y) and
                    self.can_step(side_x, end) and self.can_step(side_y, end))
        return True

    def path(self, start, goal, maximum=180000, allowed=None):
        if allowed is not None and (start not in allowed or goal not in allowed):
            return None
        if not self.cell(*start) or not self.cell(*start)["walkable"]:
            return None
        frontier, previous = deque([start]), {start: None}
        while frontier and len(previous) <= maximum:
            current = frontier.popleft()
            if current == goal:
                result = []
                while current != start:
                    result.append(current)
                    current = previous[current]
                return list(reversed(result))
            for dx, dy, _ in DIRECTIONS:
                following = current[0] + dx, current[1] + dy, current[2]
                if (following not in previous and (allowed is None or following in allowed)
                        and self.can_step(current, following)):
                    previous[following] = current
                    frontier.append(following)
        return None

#!/usr/bin/env python3
"""Verify source-layout route dependencies, keeping dynamic-door assumptions explicit."""

import json
from copy import deepcopy
from itertools import product

from common import BINDINGS, CONTENT, Inputs, canonical, load, position, sha, write
from geometry import World
from travel import adjacent


class ContentCollisionMap(World):
    def __init__(self, content):
        self.cells = {position(cell["tile"]): deepcopy(cell) for region in content["regions"].values() for cell in region["cells"]}
        self.source_cells = deepcopy(self.cells)
        self.transforms = content["mechanics"]["object_transforms"]
        self.groups = content["mechanics"]["collision_groups"]

    def cell(self, x, y, plane):
        return self.cells.get((x, y, plane))

    def select_transforms(self, selection):
        if set(selection) != set(self.transforms):
            raise ValueError("Every source transform requires an explicit state selection")
        replacements, members = [], set()
        for group in self.groups.values():
            if members.intersection(group["transforms"]):
                raise ValueError("A transform belongs to overlapping collision groups")
            members.update(group["transforms"])
            if group["states"]["status"] != "bound":
                raise ValueError("Selected source collision group is unresolved")
            wanted = {identifier: selection[identifier] for identifier in group["transforms"]}
            matches = [state for state in group["states"]["value"] if state["selection"] == wanted]
            if len(matches) != 1:
                raise ValueError("Missing or duplicate combined source collision selection")
            replacements.append(matches[0]["collision"])
        for identifier in sorted(set(self.transforms) - members):
            replacements.append(self.transforms[identifier]["states"][selection[identifier]]["collision"])
        cells, written = deepcopy(self.source_cells), set()
        for replacement in replacements:
            for cell in replacement:
                point = position(cell["tile"])
                if point not in cells or point in written:
                    raise ValueError("Collision selection synthesized or overwrote an ungrouped source cell")
                written.add(point)
                cells[point] = deepcopy(cell)
        self.cells = cells


def check_group_products(content):
    transforms = content["mechanics"]["object_transforms"]
    count, mixed = 0, 0
    for group in content["mechanics"]["collision_groups"].values():
        identifiers = sorted(group["transforms"])
        expected = set(product(*[tuple(transforms[identifier]["states"]) for identifier in identifiers]))
        states = group["states"]["value"]
        actual = {tuple(state["selection"][identifier] for identifier in identifiers) for state in states}
        if actual != expected or len(states) != len(expected):
            raise ValueError("Combined source clipping does not cover the exact transform state product")
        initial = next(state for state in states if all(
            state["selection"][identifier] == transforms[identifier]["initial"] for identifier in identifiers))
        coverage = {position(cell["tile"]) for cell in initial["collision"]}
        for state in states:
            if {position(cell["tile"]) for cell in state["collision"]} != coverage or len(state["collision"]) != len(coverage):
                raise ValueError("Combined source clipping changed its cell coverage")
            count += 1
            mixed += len(set(state["selection"].values())) > 1
    return count, mixed


def check_group_source_cells(content, inputs):
    transforms = content["mechanics"]["object_transforms"]
    rows = {identifier: content["spawns"][definition["spawn"]] for identifier, definition in transforms.items()}
    source_rows = {
        identifier: (content["objects"][spawn["kind"]["object"]]["source_id"], *position(spawn["tile"]),
                     spawn["placement"]["shape"], spawn["placement"]["quarter_turns"])
        for identifier, spawn in rows.items()
    }
    source = World(inputs, omit_placements=source_rows.values())
    coverage = {position(cell["tile"]) for group in content["mechanics"]["collision_groups"].values()
                for state in group["states"]["value"] for cell in state["collision"]}
    base_flags = {point: source.flags[number][index] for point in coverage
                  for number, index in [source.locate(*point)]}
    checked = 0
    for group in content["mechanics"]["collision_groups"].values():
        for combined in group["states"]["value"]:
            for point, flags in base_flags.items():
                number, index = source.locate(*point)
                source.flags[number][index] = flags
            for identifier, definition in transforms.items():
                state = definition["states"][combined["selection"].get(identifier, definition["initial"])]
                spawn, original = rows[identifier], source_rows[identifier]
                if state["tile"] != spawn["tile"] or state["object"] != spawn["kind"]["object"]:
                    raise ValueError("Door source identity/origin changed")
                row = (*original[:-1], state["placement"]["quarter_turns"])
                square, _ = source.locate(*position(spawn["tile"]))
                source.add_object(square, row, inputs.collections["object"][original[0]])
            for cell in combined["collision"]:
                if source.cell(**cell["tile"]) != cell:
                    raise ValueError("Combined clipping differs from independent source object insertion")
                checked += 1
    return checked


def check_travel_destinations(content):
    cells = {position(cell["tile"]): (identifier, cell) for identifier, region in content["regions"].items()
             for cell in region["cells"]}
    destinations = []
    for identifier, travel in content["mechanics"]["travels"].items():
        definition = travel["destination"]
        if definition["status"] != "bound":
            raise ValueError("Required source travel destination is unresolved")
        value = definition["value"]
        if value["kind"] == "fixed":
            locations = [("fixed", value["location"])]
        elif value["kind"] == "experience":
            locations = list(value["branches"].items())
        elif value["kind"] == "previous_respawn":
            locations = [("source_respawn", content["mechanics"]["death"]["respawn"]["value"])]
        else:
            raise ValueError("Unmodeled source travel destination kind")
        for branch, location in locations:
            point = position(location["tile"])
            if point not in cells or cells[point][0] != location["region"] or not cells[point][1]["walkable"]:
                raise ValueError("Player arrival is not a walkable explicit source coordinate")
            if location["instance"] is not None and location["instance"] not in content["mechanics"]["instances"]:
                raise ValueError("Player arrival references an absent source instance")
            destinations.append({"travel": identifier, "branch": branch, "location": location})
    return destinations


def verify():
    inputs = Inputs()
    content = load(CONTENT / "game-content.json.gz")
    closed, doors_open = ContentCollisionMap(content), ContentCollisionMap(content)
    transforms = content["mechanics"]["object_transforms"]
    initial_selection = {identifier: definition["initial"] for identifier, definition in transforms.items()}
    open_selection = dict.fromkeys(transforms, "object_state.open")
    selections, mixed = check_group_products(content)
    source_cells = check_group_source_cells(content, inputs)
    travel_destinations = check_travel_destinations(content)
    for definition in transforms.values():
        initial = definition["states"][definition["initial"]]
        if any(closed.cell(**cell["tile"]) != cell for cell in initial["collision"]):
            raise ValueError("Transform's initial geometry differs from immutable source navigation")
    doors_open.select_transforms(open_selection)
    restored = ContentCollisionMap(content)
    restored.select_transforms(open_selection)
    restored.select_transforms(initial_selection)
    if restored.cells != closed.cells:
        raise ValueError("Door open/close roundtrip changed source geometry")
    locations = load(BINDINGS / "location-bindings.json")
    transit = {record["id"]: record for record in load(BINDINGS / "travel-bindings.json")["links"]}
    allowed = {position(cell["tile"]) for region in content["regions"].values() for cell in region["cells"]}
    anchors = {name: adjacent(doors_open, position(value["tile"]))
               for name, value in locations.items()}
    for name, record in transit.items():
        for index, point in enumerate(record["landing_candidates"]):
            anchors[f"transit.{name}.{index}"] = tuple(point)
    anchors["location.tutorial.rat_pen"] = (3105, 9514, 0)
    routes = [
        ("tutorial_start_to_survival", "location.tutorial.start_house", "location.tutorial.survival"),
        ("tutorial_survival_to_chef", "location.tutorial.survival", "location.tutorial.kitchen"),
        ("tutorial_chef_to_quest_guide", "location.tutorial.kitchen", "location.tutorial.quest_house"),
        ("tutorial_quest_guide_to_ladder", "location.tutorial.quest_house", "transit.tutorial_quest_ladder.0"),
        ("tutorial_ladder_to_mining", "transit.tutorial_quest_ladder.1", "location.tutorial.mine"),
        ("tutorial_mining_to_combat", "location.tutorial.mine", "location.tutorial.rat_cave"),
        ("tutorial_combat_to_rat_pen", "location.tutorial.rat_cave", "location.tutorial.rat_pen"),
        ("tutorial_combat_to_bank_ladder", "location.tutorial.rat_cave", "transit.tutorial_combat_ladder.0"),
        ("tutorial_bank_ladder_to_bank", "transit.tutorial_combat_ladder.1", "location.tutorial.bank"),
        ("tutorial_bank_to_account", "location.tutorial.bank", "location.tutorial.account_room"),
        ("tutorial_account_to_chapel", "location.tutorial.account_room", "location.tutorial.chapel"),
        ("tutorial_chapel_to_magic", "location.tutorial.chapel", "location.tutorial.magic_house"),
        ("arrival_to_cook", "location.lumbridge.castle_arrival", "location.lumbridge.kitchen"),
        ("cook_to_bank_stairs", "location.lumbridge.kitchen", "transit.castle_south_lower.0"),
        ("castle_first_floor_connection", "transit.castle_south_lower.1", "transit.castle_south_upper.0"),
        ("castle_top_floor_to_bank", "transit.castle_south_upper.1", "location.lumbridge.bank"),
        ("castle_to_shop", "location.lumbridge.castle_arrival", "location.lumbridge.general_store"),
        ("castle_to_swamp_copper", "location.lumbridge.castle_arrival", "location.lumbridge.east_swamp_mine"),
        ("castle_across_river_to_goblins", "location.lumbridge.castle_arrival", "location.lumbridge.east_bridge_goblins"),
        ("castle_to_west_coop", "location.lumbridge.castle_arrival", "location.lumbridge.west_coop"),
        ("coop_to_west_dairy_cow", "location.lumbridge.west_coop", "location.lumbridge.north_cows"),
        ("castle_across_river_to_east_cow", "location.lumbridge.castle_arrival", "location.lumbridge.east_cows"),
        ("coop_to_wheat", "location.lumbridge.west_coop", "location.lumbridge.wheat"),
        ("wheat_to_mill", "location.lumbridge.wheat", "location.lumbridge.mill_ground"),
        ("mill_ground_to_first_ladder", "location.lumbridge.mill_ground", "transit.mill_lower.0"),
        ("mill_first_to_second_ladder", "transit.mill_lower.1", "transit.mill_upper.0"),
        ("mill_second_ladder_to_hopper", "transit.mill_upper.1", "location.lumbridge.mill_top"),
        ("cook_to_cellar_trapdoor", "location.lumbridge.kitchen", "transit.lumbridge_cellar.0"),
        ("cellar_ladder_to_bucket_area", "transit.lumbridge_cellar.1", "location.lumbridge.cellar"),
        ("castle_to_death_entrance", "location.lumbridge.castle_arrival", "location.lumbridge.death_entrance"),
    ]
    results = []
    for name, first, second in routes:
        start, goal = anchors[first], anchors[second]
        path = doors_open.path(start, goal, maximum=len(allowed) + 1, allowed=allowed) if start and goal else None
        static = closed.path(start, goal, maximum=len(allowed) + 1, allowed=allowed) if start and goal else None
        results.append({
            "id": name, "from": first, "to": second, "start": start, "goal": goal,
            "source_doors_open_path_exists": path is not None,
            "static_closed_path_exists": static is not None,
            "steps": len(path) if path is not None else None,
            "path_sha256": sha(canonical(path)) if path is not None else None,
            "within_explicit_runtime_cells": all(point in allowed for point in path) if path is not None else False,
        })
    report = {
        "schema_version": 3, "game_content_sha256": sha((CONTENT / "game-content.json.gz").read_bytes()),
        "routes": results, "source_transit_pairs": list(transit),
        "connected_with_source_doors_open": sum(result["source_doors_open_path_exists"] for result in results),
        "required_walk_segments": len(results),
        "door_transform_definitions": len(transforms),
        "combined_collision_groups": len(content["mechanics"]["collision_groups"]),
        "complete_combined_selections": selections,
        "mixed_leaf_selections": mixed,
        "combined_cells_checked_by_source_object_insertion": source_cells,
        "walkable_player_travel_destinations": travel_destinations,
        "door_roundtrip_preserves_source_cells": True,
        "scope": "Source geometry connectivity through actual content3 CombinedCollisionState selections, "
                 "not an empty map or wholesale door deletion. Open/close restores exact source cells. "
                 "This topology check does not bypass access guards in gameplay, execute unbound source timing "
                 "or certify inferred hinge/arrival/renderer fidelity.",
        "separate_runtime_acceptance": [
            "Execute experience-branch tutorial departure/Home Teleport and reconciliation through the live server.",
            "Execute Death's Office instance entry/exit and first-item-losing-death state through the live server.",
        ],
        "runtime_journey_passed": False, "presentation_approved": False,
    }
    write(BINDINGS / "route-validation.json", report, True)
    print(json.dumps({"connected": report["connected_with_source_doors_open"],
                      "required": len(results),
                      "failed": [result["id"] for result in results if not result["source_doors_open_path_exists"]],
                      "runtime_journey_passed": False}))
    return report


if __name__ == "__main__":
    result = verify()
    raise SystemExit(0 if result["connected_with_source_doors_open"] == result["required_walk_segments"] else 1)

#!/usr/bin/env python3
"""Verify source-layout route dependencies, keeping dynamic-door assumptions explicit."""

import json
from copy import deepcopy

from common import BINDINGS, CONTENT, Inputs, canonical, load, position, sha, write
from geometry import World
from travel import adjacent


class ContentCollisionMap(World):
    def __init__(self, content):
        self.cells = {position(cell["tile"]): deepcopy(cell) for region in content["regions"].values() for cell in region["cells"]}

    def cell(self, x, y, plane):
        return self.cells.get((x, y, plane))

    def transform(self, state):
        for cell in state["collision"]:
            point = position(cell["tile"])
            if point not in self.cells:
                raise ValueError("Transform attempted to synthesize an unlisted navigation cell")
            self.cells[point] = deepcopy(cell)


def verify():
    inputs = Inputs()
    content = load(CONTENT / "game-content.json.gz")
    closed, doors_open = ContentCollisionMap(content), ContentCollisionMap(content)
    transforms = content["mechanics"]["object_transforms"]
    for definition in transforms.values():
        initial = definition["states"][definition["initial"]]
        if any(closed.cell(**cell["tile"]) != cell for cell in initial["collision"]):
            raise ValueError("Transform's initial geometry differs from immutable source navigation")
        doors_open.transform(definition["states"]["object_state.open"])
    restored = ContentCollisionMap(content)
    for definition in transforms.values():
        restored.transform(definition["states"]["object_state.open"])
    for definition in transforms.values():
        restored.transform(definition["states"][definition["initial"]])
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
        "schema_version": 1, "game_content_sha256": sha((CONTENT / "game-content.json.gz").read_bytes()),
        "routes": results, "source_transit_pairs": list(transit),
        "connected_with_source_doors_open": sum(result["source_doors_open_path_exists"] for result in results),
        "required_walk_segments": len(results),
        "door_transform_definitions": len(transforms),
        "door_roundtrip_preserves_source_cells": True,
        "scope": "Source geometry connectivity through the actual schema2 ObjectTransform open-state collisions, "
                 "not an empty map or wholesale door deletion. Open/close restores exact source cells. "
                 "This topology check does not bypass access guards in gameplay, execute unbound source timing "
                 "or certify inferred hinge/arrival/renderer fidelity.",
        "unrepresented_connections": [
            "Experience-branch tutorial departure/Home Teleport and reconciliation",
            "Death's Office live instance entry/exit and first-item-losing-death state",
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

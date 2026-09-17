"""Pair actual source stairs/ladders; keep doors and live instance copies distinct."""

from common import (
    bound, counter_guard, location, region_id, source_record, tile,
    tutorial_at, unique_sources, unresolved,
)
from mechanics import completed_counter
from spawns import interaction


PAIRINGS = (
    ("tutorial_quest_ladder", (9726, 3088, 3119, 0), (9725, 3088, 9519, 0),
     "Climb-down", "Climb-up", "transition.tutorial.quest_ladder"),
    ("tutorial_combat_ladder", (9727, 3111, 9526, 0), (9728, 3111, 3126, 0),
     "Climb-up", "Climb-down", "transition.tutorial.combat_ladder"),
    ("lumbridge_cellar", (14880, 3209, 3216, 0), (17385, 3209, 9616, 0),
     "Climb-down", "Climb-up", None),
    ("mill_lower", (12964, 3164, 3307, 0), (12965, 3164, 3307, 1),
     "Climb-up", "Climb-down", None),
    ("mill_upper", (12965, 3164, 3307, 1), (12966, 3164, 3307, 2),
     "Climb-up", "Climb-down", None),
    ("castle_south_lower", (56230, 3204, 3207, 0), (16672, 3204, 3207, 1),
     "Climb-up", "Climb-down", None),
    ("castle_south_upper", (16672, 3204, 3207, 1), (56231, 3205, 3208, 2),
     "Climb-up", "Climb-down", None),
    ("castle_north_lower", (56230, 3204, 3229, 0), (16672, 3204, 3229, 1),
     "Climb-up", "Climb-down", None),
    ("castle_north_upper", (16672, 3204, 3229, 1), (56231, 3205, 3229, 2),
     "Climb-up", "Climb-down", None),
    ("castle_south_top_floor", (56230, 3204, 3207, 0), (56231, 3205, 3208, 2),
     "Top-floor", "Bottom-floor", None),
    ("castle_north_top_floor", (56230, 3204, 3229, 0), (56231, 3205, 3229, 2),
     "Top-floor", "Bottom-floor", None),
)


def adjacent(world, point, radius=2):
    x, y, plane = point
    points = sorted(((cx, cy, plane) for cx in range(x - radius, x + radius + 1)
                     for cy in range(y - radius, y + radius + 1)),
                    key=lambda p: (max(abs(p[0] - x), abs(p[1] - y)),
                                   abs(p[0] - x) + abs(p[1] - y), p))
    return next((p for p in points if (cell := world.cell(*p)) and cell["walkable"]), None)


def build_travel(inputs, world, content):
    spawns = content["spawns"]
    object_numbers = {inputs.object_id(number): number for number in inputs.collections["object"]}
    objects = {(object_numbers[spawn["kind"]["object"]],
                spawn["tile"]["x"], spawn["tile"]["y"], spawn["tile"]["plane"]): identifier
               for identifier, spawn in spawns.items() if spawn["kind"]["kind"] == "object"}
    records, missing = [], []
    for name, first, second, forward, backward, transition in PAIRINGS:
        if first not in objects or second not in objects:
            missing.append({"id": name, "reason": "Required exact source transit object is absent from runtime anchors.",
                            "source_endpoints": [list(first), list(second)]})
            continue
        first_landing, second_landing = adjacent(world, first[1:]), adjacent(world, second[1:])
        if name.startswith("castle_south"):
            first_landing, second_landing = (3205, 3209, first[3]), (3205, 3209, second[3])
        elif name.startswith("castle_north"):
            first_landing, second_landing = (3205, 3228, first[3]), (3205, 3228, second[3])
        for candidate in (first_landing, second_landing):
            if candidate and not world.cell(*candidate)["walkable"]:
                raise ValueError(f"Source staircase landing candidate is clipped: {name}/{candidate}")
        if not first_landing or not second_landing:
            missing.append({"id": name, "reason": "No explicit walkable landing beside the actual source object."})
            continue
        for source, destination, action_name, direction in (
            (first, second_landing, forward, "forward"),
            (second, first_landing, backward, "backward"),
        ):
            identifier = objects[source]
            if name == "tutorial_quest_ladder":
                guard = counter_guard(completed_counter("transition.tutorial.quest_explanation"))
            elif name == "tutorial_combat_ladder":
                guard = counter_guard(completed_counter("transition.tutorial.ranged_kill"))
            else:
                guard = tutorial_at("stage.tutorial.mainland")
            travel_id = f"travel.{name}.{direction}"
            provenance = [source_record(
                "research/m1-bindings/travel-bindings.json#" + name,
                "Matched original source objects and planes, with an explicit collision-valid adjacent landing candidate. "
                "Neither the landing nor server animation/channel phase is claimed as observed.",
                "inference", "240/cache2695")]
            content["mechanics"]["travels"][travel_id] = {
                "id": travel_id, "guard": guard,
                "destination": bound({"kind": "fixed", "location": location(tile(*destination))}, provenance),
                "channel_ticks": unresolved("Exact source stair/ladder server transit phase is not encoded in cache placement data.", provenance),
                "cooldown_ticks": bound(0, provenance), "cooldown_start": bound("completed", provenance),
                "interruptions": ["combat", "logout", "movement", "another_action"],
                "completion_effects": [], "source": provenance,
            }
            replacement = interaction(action_name, {"kind": "travel_via", "travel": travel_id}, guard, reach=1)
            actions = spawns[identifier]["interactions"]
            if not any(action["name"] == action_name for action in actions):
                raise ValueError(f"Transit action {action_name} is not on source object {source}")
            spawns[identifier]["interactions"] = [
                replacement if action["name"] == action_name else action for action in actions]
            spawns[identifier]["source"] = unique_sources(spawns[identifier]["source"] + [
                source_record("research/m1-bindings/travel-bindings.json#" + name,
                              "Both transit object origins/floors are actual source placements. Adjacent landing "
                              "is a reversible collision-valid candidate, not an observed arrival or timing.",
                              "inference", "m1-bindings-v1")])
        records.append({
            "id": name, "source_endpoints": [list(first), list(second)],
            "spawn_endpoints": [objects[first], objects[second]],
            "actions": [forward, backward],
            "landing_candidates": [list(first_landing), list(second_landing)],
            "classification": "inference",
            "source_relationship": "Matched source staircase planes or actual surface/underground ladder origins; "
                                   "no resource or building relocation.",
            "acceptance_impact": "Confirm exact landing tiles, animation/timing and source access guards in live reference.",
        })
    return records, missing


def location_anchors(inputs, world, spawns, travel):
    npc_spawns = {spawn["kind"]["npc"]: spawn for spawn in spawns.values()
                  if spawn["kind"]["kind"] == "npc" and not spawn["id"].rsplit(".", 1)[-1].startswith("p")}
    npc_locations = {
        "tutorial.start_house": "npc.gielinor_guide", "tutorial.survival": "npc.survival_expert",
        "tutorial.kitchen": "npc.master_chef", "tutorial.quest_house": "npc.quest_guide",
        "tutorial.mine": "npc.mining_instructor", "tutorial.rat_cave": "npc.combat_instructor",
        "tutorial.account_room": "npc.account_guide", "tutorial.chapel": "npc.brother_brace",
        "tutorial.magic_house": "npc.magic_instructor", "lumbridge.kitchen": "npc.cook",
        "lumbridge.general_store": "npc.shopkeeper",
        "lumbridge.jon_arrival": "npc.adventurer_jon", "death.office": "npc.death",
    }
    locations = {}
    for name, npc in npc_locations.items():
        if npc in npc_spawns:
            spawn = npc_spawns[npc]
            locations["location." + name] = {
                "tile": spawn["tile"], "region": spawn["region"],
                "source_binding": spawn["id"], "classification": "inference",
                "purpose": "Source-area navigation anchor, not a certified player spawn, NPC observation or camera.",
            }
    static = {
        "tutorial.bank": (3119, 3123, 0), "lumbridge.castle_arrival": (3222, 3218, 0),
        "lumbridge.cellar": (3210, 9616, 0), "lumbridge.bank": (3208, 3220, 2),
        "lumbridge.east_swamp_mine": (3228, 3146, 0), "lumbridge.west_coop": (3172, 3300, 0),
        "lumbridge.north_cows": (3171, 3317, 0), "lumbridge.east_cows": (3253, 3275, 0),
        "lumbridge.wheat": (3160, 3296, 0), "lumbridge.mill_ground": (3165, 3307, 0),
        "lumbridge.mill_middle": (3165, 3307, 1), "lumbridge.mill_top": (3165, 3307, 2),
        "lumbridge.east_bridge_goblins": (3246, 3235, 0), "lumbridge.death_entrance": (3237, 3194, 0),
    }
    for name, point in static.items():
        candidate = adjacent(world, point)
        if candidate is None:
            raise ValueError(f"No explicit source-area anchor for {name}")
        locations["location." + name] = {
            "tile": tile(*candidate), "region": region_id(*candidate[:2]),
            "source_candidate": list(point), "classification": "inference",
            "purpose": "Route checkpoint beside source-bound objects. Not an observed arrival or camera.",
        }
    return locations

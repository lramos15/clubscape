"""Bind source location rows and original objects without inventing an NPC spawn table."""

from collections import Counter

from common import (
    BINDINGS, always, item_stack, load, region_id,
    source_record, tile, tutorial_at, unique_sources,
)


NPC_PAGES = {
    "npc.gielinor_guide": "Gielinor Guide", "npc.survival_expert": "Survival Expert",
    "npc.master_chef": "Master Chef", "npc.quest_guide": "Quest Guide",
    "npc.mining_instructor": "Mining Instructor", "npc.account_guide": "Account Guide",
    "npc.brother_brace": "Brother Brace", "npc.magic_instructor": "Magic Instructor",
    "npc.adventurer_jon": "Adventurer Jon", "npc.lumbridge_guide": "Lumbridge Guide",
    "npc.cook": "Cook (Lumbridge)", "npc.death": "Death (NPC)",
    "npc.millie_miller": "Millie Miller", "npc.gillie_groats": "Gillie Groats",
    "npc.shopkeeper": "Shop keeper (Lumbridge)", "npc.shop_assistant": "Shop assistant (Lumbridge)",
    "npc.ironman_tutor.mainland": "Ironman tutor",
}
ROW_NPCS = {
    "Giant rat (Tutorial Island)": "npc.tutorial_rat",
    "Chicken (Tutorial Island)": "npc.tutorial_chicken",
    "Goblin": "npc.goblin.level_2",
    "Fishing spot (Tutorial Island)": "npc.tutorial.fishing_spot",
}
RUNTIME_OBJECT_PREFIXES = (
    "object.tutorial.", "object.tree.", "object.range.", "object.furnace.",
    "object.anvil.", "object.rock.", "object.altar.", "object.mill.",
    "object.lumbridge.", "object.death.", "object.water_source", "object.wheat",
    "object.dairy_cow",
)


def unavailable(reason):
    return {"kind": "unavailable", "reason": reason}


def interaction(name, action, guard=None, reach=1):
    return {"name": name, "reach": reach, "guard": guard or always(), "action": action}


def dialogue_id(npc):
    return "dialogue." + npc.removeprefix("npc.")


def marker_candidate(world, marker, size):
    x, y, plane = marker["tile"]
    radius = marker["radius"] or 0
    radius_x = marker.get("square_x", 0) // 2 if marker.get("square_x") else radius
    radius_y = marker.get("square_y", 0) // 2 if marker.get("square_y") else radius
    choices = sorted(((cx, cy, plane) for cx in range(x - radius_x, x + radius_x + 1)
                      for cy in range(y - radius_y, y + radius_y + 1)),
                     key=lambda point: (max(abs(point[0] - x), abs(point[1] - y)),
                                        abs(point[0] - x) + abs(point[1] - y), point))
    for point in choices:
        if all((cell := world.cell(point[0] + dx, point[1] + dy, plane)) and cell["walkable"]
               for dx in range(size) for dy in range(size)):
            return point
    return None


def build_spawns(inputs, world, regions):
    result, bindings, rejected = {}, {}, []
    cells = {(cell["tile"]["x"], cell["tile"]["y"], cell["tile"]["plane"])
             for region in regions.values() for cell in region["cells"]}
    duplicate_layers = Counter()
    object_keys = set()
    for number, row in sorted(world.placements, key=lambda entry: (entry[0], entry[1])):
        oid, x, y, plane, kind, rotation = row
        object_id = inputs.object_id(oid)
        raw = inputs.collections["object"][oid]
        operations = [op["text"] for op in raw["ops"]["ops"] if op]
        door = kind <= 3 and any(name in operations for name in ("Open", "Close")) and any(
            name in raw["name"].lower() for name in ("door", "gate"))
        if (x, y, plane) not in cells or not (object_id.startswith(RUNTIME_OBJECT_PREFIXES) or door):
            continue
        key = (oid, x, y, plane)
        if key in object_keys:
            duplicate_layers[key] += 1
            continue
        object_keys.add(key)
        identifier = inputs.object_spawn_id(row)
        actions = []
        for name in operations:
            reason = "This ancillary source interaction is outside the currently bound working methods; source scenery is retained."
            action = unavailable(reason)
            guard = always()
            if object_id == "object.lumbridge.bank_booth" and name == "Bank":
                action = {"kind": "bank"}
                guard = tutorial_at("stage.tutorial.mainland")
            elif object_id.startswith("object.wheat") and name == "Pick":
                action = {"kind": "effects", "effects": [{"kind": "give_items", "items": [item_stack("item.grain")]}]}
                guard = tutorial_at("stage.tutorial.mainland")
            elif object_id.startswith("object.dairy_cow") and name == "Milk":
                action = {"kind": "production", "recipes": ["recipe.cooks.milk"]}
                guard = tutorial_at("stage.tutorial.mainland")
            elif object_id == "object.furnace.tutorial" and name == "Use":
                action = {"kind": "production", "recipes": ["recipe.smelting.bronze"]}
                guard = stage_from(inputs, "stage.tutorial.smelt_bronze")
            elif object_id == "object.anvil.tutorial" and name == "Smith":
                action = {"kind": "production", "recipes": ["recipe.smithing.bronze_dagger"]}
                guard = stage_from(inputs, "stage.tutorial.anvil_open")
            elif object_id.startswith("object.tree.") or object_id.startswith("object.rock."):
                action = unavailable("Source gather binding must be attached before this placement is usable.")
            elif "bank" in object_id and object_id.startswith("object.tutorial"):
                action = unavailable("Source contextual bank binding must be attached before this placement is usable.")
            elif "mill" in object_id and "ladder" not in object_id:
                action = unavailable("Source mill counter/morph binding must be attached before this placement is usable.")
            elif "door" in object_id or "gate" in object_id or name in ("Open", "Close"):
                action = unavailable("No source-backed opening/closing state is assigned to this ancillary placement; its real clipping is retained.")
            actions.append(interaction(name, action, guard))
        result[identifier] = {
            "id": identifier, "region": region_id(x, y), "tile": tile(x, y, plane),
            "facing": (3, 0, 1, 2)[rotation],
            "placement": {"shape": kind, "quarter_turns": rotation,
                          "layer": "wall" if kind < 4 else "wall_decoration" if kind < 9 else
                                   "game_object" if kind < 22 else "floor_decoration"},
            "kind": {"kind": "object", "object": object_id},
            "interactions": actions,
            "source": [source_record(f"assets/source/osrs/cache2695/world/{number}.json.gz#placements",
                                     f"Exact object placement {row}; source type/orientation remain in world-bindings. "
                                     "A cache object is not an NPC spawn.")],
        }
        bindings[identifier] = {
            "source_kind": "cache_object_placement", "source_id": oid,
            "source_map_square": number, "source_tile": [x, y, plane],
            "source_placement_type": kind, "source_orientation": rotation,
            "source_interaction_side_mask": raw["blockingMask"],
            "morph_ids": raw["configChangeDest"], "varbit": raw["varbitID"], "varp": raw["varpID"],
            "classification": "verified_reference",
        }
    facts = load(BINDINGS / "wiki-facts.json")
    for npc, page in NPC_PAGES.items():
        markers = [marker for marker in facts["map_anchors"] if marker["page"] == page and
                   world.in_navigation_envelope(*marker["tile"][:2])]
        if not markers:
            rejected.append({"npc": npc, "gap": "npc_origin", "reason": "No applicable normal-area map anchor."})
            continue
        marker = markers[0]
        size = inputs.collections["npc"][inputs.selection["npcs"][npc]]["size"]
        candidate = marker_candidate(world, marker, size)
        if npc == "npc.death" and tuple(marker["tile"]) in cells:
            candidate = tuple(marker["tile"])
        if candidate is None or candidate not in cells:
            rejected.append({"npc": npc, "gap": "npc_origin", "marker": marker,
                             "reason": "Source marker has no valid explicit cell in its stated radius; not relocated."})
            continue
        identifier = "spawn." + npc[4:]
        source = [inputs.wiki(page, "Map anchor, not an observed NPC origin. The source marker and any within-radius "
                              "walkable candidate selection are retained in spawn-bindings.", "inference")]
        add_npc(result, identifier, npc, candidate, source)
        bindings[identifier] = {"source_kind": "wiki_map_anchor_candidate", "marker": marker,
                               "candidate": list(candidate), "classification": "inference",
                               "acceptance_impact": "Exact server origin/wander/facing is not verified."}
        if npc == "npc.death":
            bindings[identifier]["stationary_anchor"] = True
            bindings[identifier]["acceptance_impact"] = "Stationary Death map pin is retained without clearing chair/world clipping; live instance copy and anchor policy are unverified."
    inferred = (
        ("npc.combat_instructor", (3105, 9508, 0),
         "Vannaka page has a 2020 map marker only. Candidate is the normal-area translation indicated "
         "by paired normal/2020 rat location rows (+1408,-3008), not an observed normal origin."),
        ("npc.tutorial.banker", (3122, 3125, 0),
         "Candidate beside the actual tutorial bank booths, using the source NPC3318 definition. "
         "Cache booths do not certify an NPC origin."),
    )
    for npc, point, note in inferred:
        if point not in cells or not world.cell(*point)["walkable"]:
            rejected.append({"npc": npc, "gap": "npc_origin", "candidate": list(point), "reason": "Candidate is source-clipped."})
            continue
        identifier = "spawn." + npc[4:]
        add_npc(result, identifier, npc, point, [source_record(
            "research/m1-bindings/wiki-facts.json", note, "inference", "pinned-wiki-revisions")])
        bindings[identifier] = {"source_kind": "explicit_candidate", "candidate": list(point),
                               "classification": "inference", "acceptance_impact": note}
    for row in facts["coordinate_rows"]:
        npc = ROW_NPCS.get(row["page"])
        if npc is None or (npc == "npc.goblin.level_2" and row["level"] != "2"):
            continue
        for point in row["tiles"]:
            if tuple(point) not in cells:
                rejected.append({"npc": npc, "point": point, "gap": "npc_origin",
                                 "reason": "Wiki row falls outside explicit navigation cells."})
                continue
            identifier = "spawn." + npc[4:] + f".{point[0]}.{point[1]}.p{point[2]}"
            source_status = "inference" if npc == "npc.goblin.level_2" else "verified_reference"
            source = [inputs.wiki(row["page"], row["evidence_scope"], source_status)]
            add_npc(result, identifier, npc, point, source)
            bindings[identifier] = {
                "source_kind": "wiki_location_row", "row": row, "candidate": list(point),
                "classification": source_status,
                "walkable_anchor": world.cell(*point)["walkable"],
                "variant_note": "3028 is a reversible level-2 visual-variant candidate, not a per-tile ID observation."
                                if npc == "npc.goblin.level_2" else "Used normal source variant; alternate/unused variants excluded.",
            }
    for row in facts["coordinate_rows"]:
        item = {"Pot": "item.pot", "Bucket": "item.bucket", "Egg": "item.egg"}.get(row["page"])
        if not item or row["row_kind"] != "ItemSpawnLine":
            continue
        for x, y, plane in row["tiles"]:
            if (x, y, plane) not in cells:
                continue
            identifier = f"spawn.{item[5:]}.{x}.{y}.p{plane}"
            result[identifier] = {
                "id": identifier, "region": region_id(x, y), "tile": tile(x, y, plane), "facing": 0,
                "placement": None,
                "kind": {"kind": "item", "stack": item_stack(item), "respawn_ticks": 25 if item == "item.egg" else 100},
                "interactions": [],
                "source": [inputs.wiki(row["page"], "Exact source ItemSpawnLine and source infobox respawn ticks; "
                                      "the ground-item lifecycle must create/take this item, not a repeatable grant interaction.")],
            }
            bindings[identifier] = {"source_kind": "wiki_item_spawn_row", "row": row, "classification": "verified_reference"}
    return result, bindings, {
        "unbound_npc_candidates": rejected,
        "object_layer_collisions": [{"source_id": key[0], "tile": list(key[1:]), "extra_layers": value}
                                    for key, value in sorted(duplicate_layers.items())],
        "layer_policy": "A runtime interaction anchor is shared where object/type layers collide; every original "
                        "source placement remains individually bound in full world-bindings, not discarded.",
    }


def add_npc(result, identifier, npc, point, source):
    actions = [interaction("Talk-to", {"kind": "dialogue", "dialogue": dialogue_id(npc)})]
    if npc in ("npc.tutorial_rat", "npc.tutorial_chicken", "npc.goblin.level_2", "npc.chicken"):
        actions = [interaction("Attack", unavailable("No working combat scope is assigned to this source NPC variant."))]
    elif npc == "npc.tutorial.fishing_spot":
        actions = [interaction("Net", unavailable("Source netting binding must be attached before this placement is usable."))]
    elif npc in ("npc.shopkeeper", "npc.shop_assistant"):
        actions.append(interaction("Trade", unavailable("Source contextual shop binding must be attached before this placement is usable.")))
    elif npc == "npc.tutorial.banker":
        actions.append(interaction("Bank", unavailable("Source contextual bank binding must be attached before this placement is usable.")))
    x, y, plane = point
    if identifier in result:
        raise ValueError(f"Duplicate source-bound NPC spawn {identifier}")
    result[identifier] = {
        "id": identifier, "region": region_id(x, y), "tile": tile(x, y, plane), "facing": 0,
        "placement": None, "kind": {"kind": "npc", "npc": npc}, "interactions": actions,
        "source": unique_sources(source + [source_record(
            "research/m1-bindings/spawn-bindings.json#" + identifier,
            "Facing zero is an unresolved initial-facing candidate, not a measured orientation. "
            "Wiki location evidence and cache identity are kept separate.",
            "inference", "m1-bindings-v1")]),
    }


def stage_from(inputs, first, include_mainland=False):
    states = inputs.rules["tutorial"]["states"]
    index = next(index for index, state in enumerate(states) if state["id"] == first)
    stages = [state["id"] for state in states[index:] if include_mainland or state["id"] != "stage.tutorial.mainland"]
    return {"kind": "any", "guards": [tutorial_at(stage) for stage in stages]}

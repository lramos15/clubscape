"""Bounded server-owned music history and original morph-variable source bindings."""
import re

from common import ROOT, canonical, load, sha, source_record

INPUT = ROOT / "research/interface-contracts/audio-authority/inputs.json"
MENU_GROUPS = {2, 62, 64, 76, 144, 145, 163, 327}
LUMBRIDGE_GROUPS = MENU_GROUPS - {62, 144}
EQUIPMENT_SCOPE = {841, 877, 882, 1009, 1171, 1173, 1205, 1237, 1265, 1277, 1351, 1949}


def source(notes, status="verified_reference"):
    return [source_record(str(INPUT.relative_to(ROOT)), notes, status, sha(INPUT.read_bytes()))]


def contains(area, tile):
    if tile["plane"] != area["plane"]:
        return False
    x, y = tile["x"] * 2, tile["y"] * 2
    for polygon in area["polygons"]:
        inside = False
        for (ax, ay), (bx, by) in zip(polygon, polygon[1:] + polygon[:1]):
            cross = (x - ax) * (by - ay) - (y - ay) * (bx - ax)
            if cross == 0 and min(ax, bx) <= x <= max(ax, bx) and min(ay, by) <= y <= max(ay, by):
                return True
            if (ay > y) != (by > y) and (cross < 0) == (by > ay):
                inside = not inside
        if inside:
            return True
    return False


def nodes(value):
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from nodes(child)
    elif isinstance(value, list):
        for child in value:
            yield from nodes(child)


def conserved_proof(content, counter, stage, travel, area):
    writers = [node for node in nodes(content) if node.get("kind") == "set_counter"
               and node.get("counter") == counter]
    expected = {"kind": "set_counter", "counter": counter, "value": {"type": "boolean", "value": True}}
    if writers != [expected]:
        raise ValueError(f"Conserved audio fact has changed writers: {counter}")
    edges = [edge for edge in content["tutorial"][stage]["transitions"] if expected in list(nodes(edge))]
    if len(edges) != 1 or edges[0]["event"] != "teleport" or edges[0]["target"] != travel:
        raise ValueError("A conserved music fact is not its original completed travel")
    conditions = list(nodes(edges[0]["guard"]))
    if {"kind": "teleport", "phase": "completed", "travel": travel} not in conditions:
        raise ValueError("An accepted travel request is not proof of actual arrival")
    if counter.endswith("quest_ladder.completed"):
        destination = content["mechanics"]["travels"][travel]["destination"]["value"]
        if destination["kind"] != "fixed" or destination["location"]["instance"] is not None:
            raise ValueError("Tutorial cave completion lost its fixed ordinary-world landing")
        tiles = [destination["location"]["tile"]]
    else:
        tiles = [condition["tile"] for condition in conditions
                 if condition.get("kind") == "within" and condition["distance"] == 0]
    if not tiles or any(not contains(area, tile) for tile in tiles):
        raise ValueError("A conserved music fact does not establish its actual source location")
    definition = content["mechanics"]["counters"][counter]
    if definition["scope"] != "character" or definition["initial"] != {"type": "boolean", "value": False}:
        raise ValueError("Music history proof changed its actual generic counter ownership/default")
    return {"counter": counter, "event": "teleport", "phase": "completed", "travel": travel,
            "landings": tiles, "source_edge_sha256": sha(canonical(edges[0]))}


def bind_audio_authority(content):
    inputs = load(INPUT)
    if set(inputs["menu_groups"]) != MENU_GROUPS or inputs["tracks"]["62"]["automatic_unlock"] != 1:
        raise ValueError("Wrong original M1 menu or automatic track")
    tracks, areas = {}, {}
    geography = {area["name"]: area for area in inputs["geography"]["areas"]}
    for name, source_name in [("lumbridge", "Lumbridge"), ("tutorial_cave", "Scape Cave")]:
        row = geography[source_name]
        polygons = [[[int(x * 2), int(y * 2)] for x, y in polygon] for polygon in row["polygons"]]
        if any(coordinate * 2 != int(coordinate * 2) for polygon in row["polygons"] for point in polygon for coordinate in point):
            raise ValueError("Source music boundary is not on the captured half-tile lattice")
        areas[name] = {"plane": row["plane"], "polygons": polygons, "source": source(
            "Dated public source music geometry, corroborated by original track hints. "
            "Ordinary-world tile/plane containment is explicitly qualified server trigger inference, "
            "not an observed native server polygon program.", "inference")}
    proofs = {
        "tutorial_cave": conserved_proof(content, "counter.tutorial.quest_ladder.completed",
            "stage.tutorial.quest_ladder", "travel.tutorial_quest_ladder.forward", areas["tutorial_cave"]),
        "lumbridge": conserved_proof(content, "counter.tutorial.departed",
            "stage.tutorial.teleport_channel", "travel.tutorial.departure", areas["lumbridge"]),
    }
    for group in sorted(MENU_GROUPS):
        native = inputs["tracks"][str(group)]
        area, counter = None, None
        sources = source(f"Original cache table44 row{native['row']}, source group{group}; "
                         f"hint '{native['hint']}', automatic_unlock={native['automatic_unlock']}.")
        if group in LUMBRIDGE_GROUPS:
            title = {"76": "Harmony (music track)", "327": "Dream (music track)"}.get(str(group), native["name"])
            page = inputs["frozen_pages"].get(title) or inputs["supplemental_pages"].get(title)
            if page is None or not re.search(
                r"\|unlockdetail\s*=\s*Unlocked when the player first arrives in Lumbridge\.", page["text"]
            ) or native["hint"] != "in Lumbridge.":
                raise ValueError(f"No independent first-arrival unlock evidence for {group}")
            area, counter = "lumbridge", "counter.tutorial.departed"
            sources += [source_record(page["url"], "This individual track explicitly unlocks on first arrival "
                "in Lumbridge. Modern playlist membership and client playback preferences are NOT used as "
                "unlock authority.", "verified_reference", str(page.get("revision", page["url"].split("oldid=")[-1])))]
        elif group == 144:
            page = inputs["frozen_pages"]["Scape Cave"]
            if "|unlockdetail = Unlocked on the cave on [[Tutorial Island]]." not in page["text"]:
                raise ValueError("Native Varrock hint alone cannot establish the M1 tutorial cave trigger")
            area, counter = "tutorial_cave", "counter.tutorial.quest_ladder.completed"
            sources += [source_record(page["url"], "Pinned source explicitly identifies Tutorial Island cave "
                "unlock, despite the native hint naming Varrock Sewers.", "verified_reference", "15333690")]
        if counter:
            sources += [source_record(
                "research/m1-bindings/audio-authority-bindings.json#conserved_facts." + area,
                "The existing character boolean " + counter +
                " is set only by a guarded completed teleport with the audited actual source landing. "
                "It conserves a positive historical fact across movement/restart; no stage-name integer is guessed.",
                "inference", proofs[area]["source_edge_sha256"])]
        tracks[str(group)] = {"group": group, "name": native["name"], "automatic": native["automatic_unlock"] == 1,
                              "area": area, "conserved": counter, "source": sources}
    equipment = {item["source_id"] for item in content["items"].values() if item["equipment"]}
    if equipment != EQUIPMENT_SCOPE:
        raise ValueError("Expanded M1 equipment requires requalifying native no-talisman access facts")
    variable_source = source("Original ABYSSAL_WARP491 bits2/4 are RC_NO_TALLY_REQUIRED_WATER609 / FIRE611. "
        "Dated Water/Fire tiara and Mysterious ruins references tie no-talisman entry to the appropriate "
        "equipped tiara/attuned Hat of the Eye. The exact checked twelve-item M1 equipment universe has "
        "none, so only those bits are projected as zero. Other491 bits and outside-M1 equipment are NOT inferred.",
        "inference")
    result = {
        "version": 1, "profile": "source_audio.m1.cache2695.v1", "areas": areas, "tracks": tracks,
        "varps": {"491": {"binding": "source_varp.abyssal_warp.m1_water_fire",
                         "fields": [{"lsb": bit, "width": 1, "cases": [{"guard": {"kind": "always"}, "value": 0}]}
                                    for bit in (2, 4)], "source": variable_source}},
        "source": source("Eight published M1 menu identities and bounded append-only authority, separate "
                         "from preferences/playback. Title0 is not a menu grant."),
    }
    proof = {
        "schema_version": 1, "input_sha256": sha(INPUT.read_bytes()), "menu_groups": sorted(MENU_GROUPS),
        "conserved_facts": proofs, "equipment_scope_source_ids": sorted(equipment),
        "native_varp": {"id": 491, "known_bits": 20, "value": 0},
        "normal_initial_position": content["initial_state"]["tile"], "title_group_excluded": 0,
        "modern_membership_is_unlock_proof": False, "client_preferences_grant_tracks": False,
        "source_geometry_classification": inputs["geography"]["classification"],
        "source_trigger_qualification": "Explicit per-track arrival hints plus dated polygons and audited completed-travel facts.",
        "legacy_history": "Explicit legacy_untracked origin, positive conserved confirmations only; absence remains unknown.",
        "source_assets_modified": False, "milestone_accepted": False,
    }
    return result, proof

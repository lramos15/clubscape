#!/usr/bin/env python3
"""Check authored IDs, references, geometry and source graph bindings, not gameplay acceptance."""

from bisect import bisect_right
from collections import Counter
import gzip
import json
import os
import re
import subprocess

from common import BINDINGS, CONTENT, ROOT, Inputs, load, position, sha, write
from geometry import World


REGISTRIES = {
    "item": "items", "skill": "skills", "region": "regions", "spawn": "spawns", "object": "objects",
    "npc": "npcs", "recipe": "recipes", "dialogue": "dialogues", "quest": "quests",
    "interface": "interfaces", "shop": "shops",
}
EVENTS = {"interacted", "dialogue_selected", "interface_opened", "gathered", "produced", "equipped",
          "xp_gained", "hit", "defeated", "moved", "died", "recovered", "tutorial_advanced", "quest_advanced",
          "appearance_confirmed", "experience_selected", "interface_closed", "interface_presented",
          "setting_changed", "inspected", "production_resolved", "combat_resolved", "npc_killed",
          "spell_resolved", "teleport", "temporary_object_created", "object_transformed", "counter_changed",
          "death_occurred", "death_topic_completed", "recovery_completed", "grave_expired"}
ID = re.compile(r"[a-z][a-z0-9_-]*(?:\.[a-z0-9_-]+)+\Z")


def require(value, message):
    if not value:
        raise ValueError(message)


def check_content(content, inputs, world, bindings):
    report = inspect_content(content, inputs, world, bindings)
    require(report["asset_closure_passed"],
            f"Unpublished asset references: {report['unpublished_asset_references']}")
    return report


def inspect_content(content, inputs, world, bindings):
    """Inspect source structure and report publication separately; CLI callers still fail either gate."""
    ids = {kind: set(content[field]) for kind, field in REGISTRIES.items()}
    ids["slot"] = set(content["equipment_slots"])
    mechanics_fields = {"counter": "counters", "grant": "grants", "entitlement": "entitlements",
                        "reconciliation": "reconciliations", "transform": "object_transforms",
                        "temporary_object": "temporary_objects", "ground_policy": "ground_policies",
                        "instance_template": "instances", "travel": "travels", "experience": "experiences",
                        "style": "combat_styles", "spell": "spells", "projectile": "projectiles",
                        "prayer": "prayers", "value_provider": "value_providers",
                        "traversal": "traversal", "collision_group": "collision_groups"}
    ids.update({kind: set(content["mechanics"][field]) for kind, field in mechanics_fields.items()})
    ids["object_state"] = {state for transform in content["mechanics"]["object_transforms"].values() for state in transform["states"]}
    require(content["schema_version"] == 4, "Expected actual content schema4")
    ids["stage"] = set(content["tutorial"])
    for quest in content["quests"].values():
        ids["stage"].update(quest["journal"])
    source_counts = Counter()
    unpublished = set()
    for kind, field in REGISTRIES.items():
        require(content[field], f"Empty {field}")
        source_numbers = set()
        for identifier, definition in content[field].items():
            require(ID.fullmatch(identifier) and identifier.startswith(kind + "."), f"Invalid {kind} ID {identifier}")
            require(definition["id"] == identifier, f"Map key mismatch {identifier}")
            if "source_id" in definition and definition["source_id"] is not None:
                require(definition["source_id"] not in source_numbers, f"Duplicate source ID in {field}")
                source_numbers.add(definition["source_id"])
            require(definition["source"], f"Missing source {identifier}")
    reference_fields = {"noted_variant": "item", "unnoted_variant": "item", "equipment_slots": "slot",
                        "occupied_slots": "slot", "target_objects": "object", "tools": "item",
                        "recipes": "recipe", "tutorial_stage": "stage", "initial_stage": "stage",
                        "completed_stage": "stage"}
    reference_fields.update({kind: kind for kind in ids})
    reference_fields.pop("instance_template", None)
    def reference(value, kind, path):
        if value is None:
            return
        if isinstance(value, list):
            for item in value:
                reference(item, kind, path)
        elif isinstance(value, str):
            require(value in ids[kind], f"Dangling {kind} reference at {path}: {value}")
    def visit(value, path="content"):
        if isinstance(value, list):
            for index, item in enumerate(value):
                visit(item, f"{path}/{index}")
        elif isinstance(value, dict):
            if "event" in value and "guard" in value:
                require(value["event"] in EVENTS, f"Unrepresented event at {path}")
                if value["target"]:
                    kind = value["target"].split(".", 1)[0]
                    require(kind in ids and value["target"] in ids[kind], f"Dangling event target at {path}")
            if "reference" in value and "status" in value:
                require(value["status"] in ("verified_reference", "inference", "approved_adaptation"),
                        f"Fixture/unknown provenance at {path}")
                require(value["reference"] and value["revision"] and value["notes"], f"Empty provenance at {path}")
                source_counts[value["status"]] += 1
            for key, item in value.items():
                if key in reference_fields and not (path == "content" and key in content):
                    reference(item, reference_fields[key], path + "/" + key)
                if key in ("asset", "scene_asset", "animation", "sound") and item is not None:
                    require(isinstance(item, str) and ID.fullmatch(item) and item.startswith("asset."),
                            f"Invalid asset reference at {path}: {item}")
                    if item not in inputs.assets:
                        unpublished.add((item, path))
                if key == "quantity":
                    require(type(item) is int and 0 < item <= 2147483647, f"Invalid item quantity at {path}")
                if key == "name" and value.get("kind") == "flag":
                    require(item in content["initial_state"]["flags"], f"Uninitialized flag {item}")
                visit(item, path + "/" + key)
    visit(content)
    for item in content["items"].values():
        if item["noted_variant"]:
            note = content["items"][item["noted_variant"]]
            require(note["unnoted_variant"] == item["id"] and note["stackable"], f"Nonreciprocal note {item['id']}")
            require(not item["stackable"], f"Stackable base has note mapping {item['id']}")
        if item["equipment"]:
            occupied = item["equipment"]["occupied_slots"]
            require(len(set(occupied)) == len(occupied) and item["equipment"]["slot"] in occupied, "Invalid equipment occupancy")
    cells = {}
    walkable, blocked = 0, 0
    for region in content["regions"].values():
        for cell in region["cells"]:
            point = position(cell["tile"])
            require(point not in cells, f"Duplicate collision cell {point}")
            require(all(region["min"][key] <= cell["tile"][key] <= region["max"][key] for key in ("x", "y", "plane")),
                    f"Out-of-bounds collision cell {point}")
            require(cell == world.cell(*point), f"Runtime cell differs from actual source reconstruction: {point}")
            require(0 <= cell["blocked_movement"] <= 255 and 0 <= cell["blocked_sight"] <= 255, "Invalid directional mask")
            cells[point] = region["id"]
            walkable += cell["walkable"]
            blocked += not cell["walkable"]
    initial = content["initial_state"]
    require(initial["tutorial_stage"] == inputs.rules["tutorial"]["start_ref"], "Tutorial was precompleted")
    require(initial["quest_points"] == 0 and initial["run_energy"] == 10000, "Invalid normal initial QP/run units")
    require(len(initial["inventory"]["slots"]) == 28 and all(value is None for value in initial["inventory"]["slots"]),
            "Non-normal fresh inventory")
    require(initial["bank"]["slots"] == [] and not initial["equipment"], "Fresh containers silently normalized/seeded")
    require(cells.get(position(initial["tile"])) == initial["region"] and world.cell(**initial["tile"])["walkable"],
            "Initial source-area candidate is not explicitly walkable")
    require(set(initial["skills"]) == set(content["skills"]) and len(initial["skills"]) == 24, "Missing normal skill state")
    for skill, state in initial["skills"].items():
        definition = content["skills"][skill]
        thresholds = definition["xp_thresholds_tenths"]
        require(len(thresholds) == 99 and thresholds[0] == 0 and thresholds == sorted(set(thresholds)), "Invalid XP thresholds")
        require(bisect_right(thresholds, state["xp_tenths"]) == state["current_level"], "Initial level/XP mismatch")
    for quest, state in initial["quests"].items():
        require(state["stage"] == content["quests"][quest]["initial_stage"], "Quest was precompleted")
    occupied, nonwalking = set(), []
    source_placements = {tuple(row) for _, row in world.placements}
    for identifier, spawn in content["spawns"].items():
        point = position(spawn["tile"])
        require(cells.get(point) == spawn["region"], f"Spawn lacks exact region/cell ownership: {identifier}")
        require(0 <= spawn["facing"] <= 7, "Noncanonical spawn facing")
        kind = spawn["kind"]
        definition = kind.get("object", kind.get("npc", str(kind.get("stack"))))
        key = point, kind["kind"], definition
        require(key not in occupied, f"Duplicate runtime anchor: {identifier}")
        occupied.add(key)
        require(len({action["name"] for action in spawn["interactions"]}) == len(spawn["interactions"]), "Duplicate interaction name")
        if kind["kind"] == "object":
            binding = bindings["spawns"][identifier]
            raw = (binding["source_id"], *binding["source_tile"],
                   binding["source_placement_type"], binding["source_orientation"])
            require(raw in source_placements, f"Relocated/synthetic object {identifier}")
        elif kind["kind"] == "npc" and not world.cell(*point)["walkable"]:
            nonwalking.append(identifier)
        for action in spawn["interactions"]:
            if action["action"]["kind"] == "attack":
                require(content["npcs"][kind["npc"]]["combat"] is not None, "Attack without a complete NPC combat definition")
    for dialogue in content["dialogues"].values():
        nodes = {node["id"]: node for node in dialogue["nodes"]}
        require(len(nodes) == len(dialogue["nodes"]), "Duplicate dialogue node")
        require(all(entry in nodes for entry in dialogue["entry_nodes"]), "Dangling dialogue entry")
        for node in nodes.values():
            require(len({choice["id"] for choice in node["choices"]}) == len(node["choices"]), "Duplicate choice in node")
            for choice in node["choices"]:
                require(choice["next_node"] is None or choice["next_node"] in nodes, "Dangling next dialogue node")
    for label, source_name in (("tutorial", "tutorial"), ("cooks", "cooks-assistant")):
        edges = {record["id"]: record for record in inputs.rules[source_name]["transitions"]}
        require({record["id"] for record in bindings[label]} == set(edges), f"Source {label} graph lost edges")
        require(len(bindings[label]) == len(edges), f"Duplicate source {label} edge")
        for bound in bindings[label]:
            original = edges[bound["id"]]
            require((bound["from"], bound["to"]) == (original["from_ref"], original["to_ref"]), "Source graph was collapsed")
            require(bound["status"] == "blocked" or bool(bound["runtime_hooks"]), "Unimplemented edge labeled projected")
    require(len(content["tutorial"]) == 71 and len(content["quests"]["quest.cooks_assistant"]["journal"]) == 10,
            "Full source state sets were shortened")
    require(content["items"]["item.pickaxe.bronze"]["source_id"] == 1265 and
            content["npcs"]["npc.tutorial_rat"]["source_id"] == 3313 and
            content["npcs"]["npc.tutorial_chicken"]["source_id"] == 3316 and
            content["npcs"]["npc.cook"]["source_id"] == 4626, "Wrong required source variant")
    counts = {field: len(content[field]) for field in REGISTRIES.values()}
    counts["mechanics"] = {field: len(content["mechanics"][field]) for field in mechanics_fields.values()}
    counts.update({
        "runtime_collision_cells": len(cells), "runtime_walkable_cells": walkable, "runtime_blocked_cells": blocked,
        "equipment_slots": len(content["equipment_slots"]), "tutorial_states": len(content["tutorial"]),
        "tutorial_source_transitions": len(bindings["tutorial"]),
        "tutorial_projected_edges": sum(record["status"] != "blocked" for record in bindings["tutorial"]),
        "tutorial_blocked_edges": sum(record["status"] == "blocked" for record in bindings["tutorial"]),
        "cooks_states": 10, "cooks_source_transitions": len(bindings["cooks"]),
        "cooks_projected_edges": sum(record["status"] != "blocked" for record in bindings["cooks"]),
        "death_source_states": 4, "death_source_transitions": 6,
        "source_transit_pairs": len(bindings["travel"]), "source_rules": 52,
        "normal_note_pairs": sum(bool(item["noted_variant"]) for item in content["items"].values()),
    })
    return {
        "schema_version": 1, "counts": counts, "structural_checks_passed": True,
        "asset_closure_passed": not unpublished,
        "unpublished_asset_references": [{"asset": asset, "path": path} for asset, path in sorted(unpublished)],
        "source_status_occurrences": dict(source_counts),
        "source_npc_nonwalkable_anchors": nonwalking,
        "exact_geometry_scope": "Explicit imported cells and source-bound object placement equality, not an observed live clipping dump.",
        "runtime_compile_passed": False, "gameplay_executed": False, "presentation_approved": False,
        "source_binding_inventory": "research/m1-bindings/unresolved-bindings.json",
        "execution_requirements": "research/m1-bindings/contract-gaps.json",
        "scope": "ID/reference/note/initial-state/source-geometry/graph-preservation checks only. "
                 "Not a replacement for the real strict content compiler or the live/headless journey.",
    }


def main():
    inputs = Inputs()
    world = World(inputs)
    content = load(CONTENT / "game-content.json.gz")
    graph = load(BINDINGS / "graph-bindings.json")
    bindings = {
        **graph, "spawns": load(BINDINGS / "spawn-bindings.json.gz")["spawns"],
        "travel": load(BINDINGS / "travel-bindings.json")["links"],
    }
    manifest = load(CONTENT / "manifest.json")
    for output in manifest["outputs"]:
        require(sha((CONTENT.parents[1] / output["path"]).read_bytes()) == output["sha256"],
                f"Generated output changed: {output['path']}")
    report = inspect_content(content, inputs, world, bindings)
    report["asset_closure_passed"] = report["asset_closure_passed"] and manifest["asset_closure_passed"]
    report["unpublished_asset_ids"] = manifest["unpublished_asset_ids"]
    artifact = manifest["compiled_artifact"]
    compressed = (ROOT / artifact["path"]).read_bytes()
    require(sha(compressed) == artifact["sha256"] and sha(gzip.decompress(compressed)) == artifact["uncompressed_sha256"],
            "Compiled artifact hash mismatch")
    work = ROOT / "tools/m1-content/.local"
    work.mkdir(parents=True, exist_ok=True)
    input_path = work / "schema-input.json"
    input_path.write_bytes(gzip.decompress((CONTENT / "game-content.json.gz").read_bytes()))
    result = subprocess.run(
        ["cargo", "run", "--quiet", "--locked", "--offline", "--manifest-path",
         str(ROOT / "tools/m1-content/schema-check/Cargo.toml"), "--", str(input_path)],
        cwd=ROOT, env={**os.environ, "CARGO_TARGET_DIR": str(work / "schema-target"), "TMPDIR": str(work)},
        text=True, capture_output=True)
    if not result.stdout.strip():
        raise subprocess.CalledProcessError(result.returncode or 1, result.args, result.stdout, result.stderr)
    schema = json.loads(result.stdout)
    require(schema["artifact_reloaded"] and schema["artifact_sha256"] == artifact["uncompressed_sha256"],
            "Real compiler/library roundtrip mismatch")
    write(BINDINGS / "schema-validation.json", schema, pretty=True)
    report["runtime_compile_passed"] = True
    report["artifact_reloaded"] = True
    report["engine_constructed"] = schema["engine_constructed"]
    report["native_source_policy_probes"] = schema["native_source_policy_probes"]
    report["native_ui_control_probes"] = schema["native_ui_control_probes"]
    report["native_source_policy_checks_passed"] = schema["native_source_policy_probes"]["passed"]
    report["engine_validation_exit_code"] = result.returncode
    report["engine_validation_diagnostic"] = result.stderr
    write(BINDINGS / "check-result.json", report, pretty=True)
    print(json.dumps({"structural_checks_passed": True, "strict_runtime_compiler_passed": True,
                      "asset_closure_passed": report["asset_closure_passed"],
                      "unpublished_asset_ids": report["unpublished_asset_ids"],
                      "artifact_reloaded": True, "artifact_sha256": artifact["uncompressed_sha256"],
                      "engine_constructed": schema["engine_constructed"],
                      "native_source_policy_checks_passed": schema["native_source_policy_probes"]["passed"],
                      "unresolved_bindings": len(schema["unresolved_bindings"]), "gameplay_executed": False}))
    if result.returncode or not schema["native_source_policy_probes"]["passed"] or not schema["native_ui_control_probes"]["passed"]:
        raise SystemExit(result.returncode or 1)
    require(report["asset_closure_passed"],
            f"Source/engine checks do not waive missing published assets: {report['unpublished_asset_ids']}")


if __name__ == "__main__":
    main()

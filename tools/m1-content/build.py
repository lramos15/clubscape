#!/usr/bin/env python3
"""Build source-bound M1 GameContent and full source geometry; never claim runtime acceptance."""

import argparse
from collections import Counter, defaultdict
from pathlib import Path
import subprocess
import sys

from common import (
    ASSET_PREFIX, BINDINGS, CONTENT, JOURNEY, PUBLICATION, ROOT, SOURCE, Inputs, canonical, load,
    position, sha, unique_sources, write,
)
from definitions import (
    WEAR_SLOTS, build_interfaces, build_items, build_npcs, build_objects, build_recipes,
    build_shop, build_skills, equipment_slots,
)
from geometry import World
from progression import (
    build_cooks, build_dialogues, build_initial_state, build_tutorial, finish_dialogues,
    learning_quest,
)
from spawns import build_spawns
from travel import build_travel, location_anchors
from mechanics import base_mechanics
from travel_policies import wire_travel_policies
from world_mechanics import build_doors, wire_world
from runtime_application import apply_source_bindings


def input_lock(inputs):
    published = load(ROOT / "assets/manifests/osrs/cache2695-published.json")
    indexed = {record["path"]: record for record in published["published_files"]}
    paths = list((SOURCE / "world").glob("*.json.gz"))
    paths += list((SOURCE / "collections").glob("*.json.gz"))
    paths += list(JOURNEY.glob("*.json"))
    paths += [ROOT / "assets/manifests/osrs/cache2695-full-bundle.json.gz",
              ROOT / "assets/manifests/osrs/cache2695-published.json"]
    extension = inputs.publication
    closure_request = load(ROOT / extension["request"]["path"])
    extension_records = [
        *extension["published_files"], *extension["collection_extensions"].values(),
        *[extension[key] for key in ("base_bundle", "base_publication", "merged_inventory",
                                    "extraction_inventory", "request", "closure_report", "dependency_graph")],
        closure_request["product_definitions_snapshot"], closure_request["selection"], closure_request["decoder_lock"],
    ]
    for record in extension_records:
        previous = indexed.get(record["path"])
        if previous and (previous["sha256"], previous["size_bytes"]) != (record["sha256"], record["size_bytes"]):
            raise ValueError(f"Conflicting publication input hashes: {record['path']}")
        indexed[record["path"]] = record
    paths += [ROOT / record["path"] for record in extension_records]
    paths += [PUBLICATION, ROOT / "tools/cache-import/content_closure.py", ROOT / "tools/cache-import/import_cache.py",
              ROOT / "research/current-source/m1-content-closure-validation.json"]
    paths += [ROOT / f"research/current-source/{name}.json" for name in
              ("selection", "extraction-contract", "m1-request")]
    paths += [BINDINGS / name for name in ("selection.json", "definitions.json.gz", "wiki-sources.json",
                                         "wiki-facts.json", "code-sources.json", "runtime-source-facts.json",
                                         "profile-v2.json", "application-item-definitions.json.gz")]
    paths += [ROOT / "research/runtime-bindings" / name for name in (
        "resolutions.json", "application-context.json", "sources.json", "oracles.json",
        "death-values.json", "profile-resolutions.json", "inputs/guide-prices.json.gz")]
    paths += [ROOT / "tools/runtime-bindings/apply.py", ROOT / "milestones/m1-goblin-loot-approval.json",
              ROOT / "spec/adaptations.md"]
    paths += list((ROOT / "crates/game-types/src").glob("*.rs"))
    paths += sorted((ROOT / "tools/m1-content").glob("*.py"))
    paths += sorted((ROOT / "tools/m1-content").glob("*.java"))
    records = []
    for path in sorted(set(paths)):
        data = path.read_bytes()
        name = str(path.relative_to(ROOT))
        record = {"path": name, "sha256": sha(data), "bytes": len(data)}
        if name in indexed and (record["sha256"] != indexed[name]["sha256"]
                                or record["bytes"] != indexed[name]["size_bytes"]):
            raise ValueError(f"Published source input changed: {name}")
        records.append(record)
    bundle_record = published["full_extraction_bundle"]
    if sha((ROOT / bundle_record["path"]).read_bytes()) != bundle_record["sha256"]:
        raise ValueError("Full source-worker extraction manifest changed")
    return {"schema_version": 1, "inputs": records, "aggregate_sha256": sha(canonical(records)),
            "source_selection": load(ROOT / "research/current-source/selection.json")["selection_id"]}


def assemble(inputs, world, revision):
    regions = {region["id"]: region for number in sorted(world.raw) if (region := world.region(number))}
    items, item_bindings, excluded_items = build_items(inputs)
    npcs, npc_bindings = build_npcs(inputs)
    interfaces, interface_bindings = build_interfaces(inputs)
    spawns, spawn_bindings, placement_issues = build_spawns(inputs, world, regions)
    dialogues = build_dialogues(inputs)
    content = {
        "schema_version": 2, "revision": revision, "baseline": inputs.selection["baseline"],
        "items": items, "skills": build_skills(inputs), "objects": build_objects(inputs),
        "npcs": npcs, "interfaces": interfaces, "regions": regions, "spawns": spawns,
        "recipes": build_recipes(inputs), "shops": build_shop(inputs, items),
        "dialogues": dialogues, "tutorial": {},
        "quests": {"quest.learning_the_ropes": learning_quest(inputs)},
        "equipment_slots": equipment_slots(inputs),
        "initial_state": None, "mechanics": base_mechanics(inputs),
    }
    bindings = {
        "items": item_bindings, "npcs": npc_bindings, "interfaces": interface_bindings,
        "spawns": spawn_bindings, "placement_issues": placement_issues, "excluded_items": excluded_items,
    }
    wire_world(inputs, world, content, bindings)
    build_doors(inputs, world, content, bindings)
    travel, travel_issues = build_travel(inputs, world, content)
    bindings.update({"travel": travel, "travel_issues": travel_issues})
    wire_travel_policies(inputs, world, content)
    tutorial, tutorial_bindings = build_tutorial(inputs, world, content, bindings)
    content["tutorial"] = tutorial
    cooks, cook_bindings = build_cooks(inputs, content)
    content["quests"]["quest.cooks_assistant"] = cooks
    finish_dialogues(inputs, content)
    content["initial_state"] = build_initial_state(inputs, content)
    bindings.update({"tutorial": tutorial_bindings, "cooks": cook_bindings,
                     "locations": location_anchors(inputs, world, content["spawns"], travel)})
    bindings["locations"]["location.death.player_arrival"] = {
        "tile": content["mechanics"]["death"]["first_office"]["value"]["tile"],
        "region": "region.osrs.12633", "classification": "inference",
        "purpose": "Walkable player arrival, distinct from nonwalking Death NPC anchor; source capture remains required.",
    }
    def normalize_sources(value):
        if isinstance(value, dict):
            for key, child in value.items():
                if key == "source" and isinstance(child, list) and all(
                        isinstance(record, dict) and "reference" in record for record in child):
                    value[key] = unique_sources(child)
                else:
                    normalize_sources(child)
        elif isinstance(value, list):
            for child in value:
                normalize_sources(child)
    normalize_sources(content)
    content, application = apply_source_bindings(content, inputs)
    bindings["application"] = application
    return content, bindings


def object_bindings(inputs):
    result = {}
    for number, raw in sorted(inputs.collections["object"].items()):
        result[inputs.object_id(number)] = {
            "source_id": number, "asset": inputs.asset("object", number), "source_name": raw["name"],
            "models": raw["objectModels"], "model_types": raw["objectTypes"],
            "source_size": [raw["sizeX"], raw["sizeY"]],
            "model_scale": [raw["modelSizeX"], raw["modelSizeHeight"], raw["modelSizeY"]],
            "model_offset": [raw["offsetX"], raw["offsetHeight"], raw["offsetY"]],
            "recolor_from": raw["recolorToFind"], "recolor_to": raw["recolorToReplace"],
            "retexture_from": raw["retextureToFind"], "retexture_to": raw["textureToReplace"],
            "is_rotated": raw["isRotated"], "contour": raw["contouredGround"],
            "animation_id": raw["animationID"], "ambient_sound": raw["ambientSoundId"],
            "ambient_sound_ids": raw["ambientSoundIds"], "clip_type": raw["interactType"],
            "projectile_clip": raw["blocksProjectile"], "hollow": raw["isHollow"],
            "interaction_side_mask": raw["blockingMask"], "varbit": raw["varbitID"], "varp": raw["varpID"],
            "morph_ids": raw["configChangeDest"],
        }
    return result


def mechanics_bindings(inputs, content):
    source_to_runtime = {
        "object.tutorial.fishing_spot": ["npc.tutorial.fishing_spot"],
        "npc.dairy_cow": ["object.dairy_cow", "object.dairy_cow.east"],
        "object.rock.copper": ["object.rock.copper", "object.rock.copper.11161", "object.rock.copper.tutorial"],
        "object.rock.tin": ["object.rock.tin", "object.rock.tin.11361", "object.rock.tin.tutorial"],
        "object.tree.normal": ["object.tree.normal", "object.tree.tutorial", "object.tree.tutorial.9731", "object.tree.tutorial.9733"],
    }
    by_definition = defaultdict(list)
    for identifier, spawn in content["spawns"].items():
        kind = spawn["kind"]
        definition = kind.get("object", kind.get("npc"))
        if definition:
            by_definition[definition].append(identifier)
    result = []
    for file in ("activities", "cooks-assistant"):
        for source in inputs.rules[file]["rules"]:
            reference = source.get("object_ref", source.get("npc_ref"))
            if source["id"] == "rule.cooks.milk":
                reference = "npc.dairy_cow"
            if source["id"] == "rule.cooks.grain":
                reference = "object.wheat"
            targets = source_to_runtime.get(reference, [reference] if reference else [])
            record = {
                "id": source["id"], "source": f"research/journey-rules/{file}.json#{source['id']}",
                "source_rule_sha256": sha(canonical(source)), "source_rule": source,
                "target_definitions": targets,
                "spawns": sorted(identifier for target in targets for identifier in by_definition.get(target, [])),
                "recipe_ids": [identifier for identifier in content["recipes"]
                               if identifier == source["id"].replace("rule.", "recipe.", 1)],
            }
            if source.get("roll", {}).get("formula_ref") == "formula.skilling.success":
                roll = source["roll"]
                record["success_count_by_level_1_through_99"] = [
                    max(0, min(256, 1 + (roll["low"] * (99 - level) + roll["high"] * (level - 1) + 49) // 98))
                    for level in range(1, 100)]
                record["success_count_scope"] = "Source arithmetic projection, not a sampled or guaranteed runtime result."
            result.append(record)
    return {
        "schema_version": 1, "rules": result, "semantic_corrections": inputs.selection["semantic_corrections"],
        "formulas": inputs.rules["activities"]["formulas"],
        "loot_tables": inputs.rules["activities"]["loot_tables"],
        "death_graph": inputs.rules["activities"]["death_graph"],
        "unresolved_semantic_items": inputs.selection["unresolved_items"],
        "source_interaction_policy": "A source rule binding is not an engine implementation or an approval. "
                                     "Unrepresented rules remain blocked in contract-gaps.json.",
    }


def checkpoint_bindings(inputs, content, bindings):
    source = inputs.rules["expected-scenarios"]
    tutorial_by_id = {record["id"]: record for record in bindings["tutorial"]}
    edges = []
    for edge in inputs.rules["tutorial"]["transitions"]:
        record = tutorial_by_id[edge["id"]]
        event = edge["event_ref"]
        targets = []
        for hook in record["runtime_hooks"]:
            if hook["kind"] == "dialogue_choice":
                npc = "npc." + hook["dialogue"].removeprefix("dialogue.")
                targets.extend(spawn["id"] for spawn in content["spawns"].values()
                               if spawn["kind"].get("npc") == npc)
            elif hook["kind"] == "source_bound_travel":
                targets.append(hook["spawn"])
        edges.append({
            "source_transition": edge["id"], "expected_stage_before": edge["from_ref"],
            "source_event": event, "expected_stage_after": edge["to_ref"],
            "bound_spawn_ids": sorted(set(targets)), "runtime_hooks": record["runtime_hooks"],
            "status": record["status"], "gaps": record["gaps"],
        })
    ingredient_ids = {
        "egg": next(identifier for identifier, spawn in content["spawns"].items()
                    if spawn["kind"].get("stack", {}).get("item") == "item.egg"
                    and spawn["tile"]["x"] == 3172 and spawn["tile"]["y"] == 3301),
        "pot": next(identifier for identifier, spawn in content["spawns"].items()
                    if spawn["kind"].get("stack", {}).get("item") == "item.pot"
                    and spawn["tile"]["x"] == 3209 and spawn["tile"]["y"] == 3214),
        "bucket": next(identifier for identifier, spawn in content["spawns"].items()
                       if spawn["kind"].get("stack", {}).get("item") == "item.bucket"
                       and spawn["tile"]["x"] == 3216 and spawn["tile"]["y"] == 9625),
    }
    def object_at(object_id, x, y, plane=0):
        return next(identifier for identifier, spawn in content["spawns"].items()
                    if spawn["kind"].get("object") == object_id and position(spawn["tile"]) == (x, y, plane))
    cook_route = [
        {"intent_kind": "take_ground_item", "resolve_ground_instance_from_spawn": ingredient_ids["pot"],
         "expect_owned_item": "item.pot", "source_route": "route.cooks.pot_spawn"},
        {"intent": {"kind": "interact", "target": object_at("object.lumbridge.kitchen_trapdoor", 3209, 3216),
                    "action": "Climb-down"}, "expect_region": "region.osrs.12950", "expect_plane": 0},
        {"intent_kind": "take_ground_item", "resolve_ground_instance_from_spawn": ingredient_ids["bucket"],
         "expect_owned_item": "item.bucket", "source_route": "route.cooks.bucket_spawn"},
        {"intent_kind": "take_ground_item", "resolve_ground_instance_from_spawn": ingredient_ids["egg"],
         "expect_owned_item": "item.egg", "source_route": "route.cooks.egg"},
        {"intent": {"kind": "interact", "target": object_at("object.dairy_cow", 3172, 3317), "action": "Milk"},
         "recipe": "recipe.cooks.milk", "consumed": "item.bucket", "produced": "item.milk.bucket"},
        {"intent": {"kind": "interact", "target": object_at("object.wheat", 3160, 3296), "action": "Pick"},
         "expect_owned_item": "item.grain", "source_route": "route.cooks.flour"},
        {"intent": {"kind": "interact", "target": object_at("object.mill.ladder_lower", 3164, 3307), "action": "Climb-up"},
         "expect_plane": 1},
        {"intent": {"kind": "interact", "target": object_at("object.mill.ladder_middle", 3164, 3307, 1), "action": "Climb-up"},
         "expect_plane": 2},
        {"intent": {"kind": "interact", "target": object_at("object.mill.hopper", 3166, 3307, 2), "action": "Fill"},
         "expect_hopper": "grain", "consumed": "item.grain", "gap": "mill_state"},
        {"intent": {"kind": "interact", "target": object_at("object.mill.controls", 3166, 3305, 2), "action": "Operate"},
         "expect_hopper": None, "expect_flour_units_delta": 1, "gap": "mill_state"},
        {"intent": {"kind": "interact", "target": object_at("object.mill.ladder_upper", 3164, 3307, 2), "action": "Climb-down"},
         "expect_plane": 1},
        {"intent": {"kind": "interact", "target": object_at("object.mill.ladder_middle", 3164, 3307, 1), "action": "Climb-down"},
         "expect_plane": 0},
        {"target": object_at("object.mill.flour_bin", 3166, 3306), "source_action": "Empty",
         "required_item": "item.pot", "produced": "item.flour.pot", "expect_flour_units_delta": -1,
         "gap": "mill_state", "morph_note": "Parent1781 must resolve to the source per-player full/empty bin, not be silently replaced."},
        {"intent": {"kind": "interact", "target": "spawn.cook", "action": "Talk-to"},
         "expect_quest": "quest.cooks_assistant", "delivery_graph": "research/m1-bindings/graph-bindings.json#cooks"},
    ]
    return {
        "schema_version": 1,
        "status": "source_reference_requirements_not_executed",
        "source_oracles": "research/journey-rules/expected-scenarios.json",
        "source_oracles_sha256": sha(canonical(source)),
        "oracle_counts": {name: len(source[name]) for name in (
            "formula_cases", "guard_cases", "graph_cases", "outcome_cases",
            "checkpoint_requirements", "journey_requirements")},
        "tutorial": edges, "cooks_legitimate_route": cook_route,
        "source_checkpoints": source["checkpoint_requirements"],
        "source_journeys": source["journey_requirements"],
        "delivery_orders": source["quest_delivery_orders"],
        "input_policy": "Use real Walk/Interact/SelectDialogue/OpenInterface/Produce and dynamic ground-item IDs. "
                        "Resolve/approach the specified source targets through collision and guarded travel. "
                        "Do not submit result events, seed stages, inject RNG in production, grant ingredients or "
                        "skip unavailable mill/death dependencies. These checkpoints remain requirements.",
    }


def build(args):
    inputs = Inputs()
    lock = input_lock(inputs)
    world = World(inputs)
    revision = "m1.source-backed.v2." + lock["aggregate_sha256"][:16]
    content, bindings = assemble(inputs, world, revision)
    from check import check_content
    checks = check_content(content, inputs, world, bindings)
    outputs = []
    def emit(path, value, pretty=False):
        record = write(path, value, pretty)
        outputs.append(record)
        return record
    emit(BINDINGS / "input-lock.json", lock, True)
    emit(CONTENT / "game-content.json.gz", content)
    bindings["application"]["content_compressed_sha256"] = sha((CONTENT / "game-content.json.gz").read_bytes())
    emit(ROOT / "research/runtime-bindings/application-result.json", bindings["application"], True)
    geometry_records = []
    for number in sorted(world.raw):
        geometry_records.append(emit(CONTENT / f"geometry/{number}.json.gz", world.region(number, full=True)))
    emit(BINDINGS / "item-bindings.json", {"items": bindings["items"], "excluded": bindings["excluded_items"]}, True)
    emit(BINDINGS / "npc-bindings.json", bindings["npcs"], True)
    emit(BINDINGS / "interface-bindings.json", bindings["interfaces"], True)
    emit(BINDINGS / "spawn-bindings.json.gz", {"spawns": bindings["spawns"], "issues": bindings["placement_issues"]})
    emit(BINDINGS / "travel-bindings.json", {"links": bindings["travel"], "unresolved": bindings["travel_issues"]}, True)
    emit(BINDINGS / "door-bindings.json", {"groups": bindings["doors"], "rat_pen_cells": bindings["rat_pen_cells"]}, True)
    emit(BINDINGS / "location-bindings.json", bindings["locations"], True)
    emit(BINDINGS / "graph-bindings.json", {"tutorial": bindings["tutorial"], "cooks": bindings["cooks"],
                                          "death": inputs.rules["activities"]["death_graph"]}, True)
    emit(BINDINGS / "mechanics-bindings.json.gz", mechanics_bindings(inputs, content))
    emit(CONTENT / "journey-checkpoints.json", checkpoint_bindings(inputs, content, bindings), True)
    emit(CONTENT / "slot-bindings.json", {
        "slots": [{"id": f"slot.{name}", "source_wear_position": number, "normal_functional_slot": True}
                  for number, name in WEAR_SLOTS.items()],
        "source": "Original item wearPos1/2/3 and research/journey-rules/vocabulary.json#slots",
        "equipment_fitting": "Not approved; ring/ammunition remain functional without an invented visible mesh.",
    }, True)
    emit(BINDINGS / "policy-bindings.json", {
        "initial_state": inputs.rules["initial-state"],
        "tutorial_grants": inputs.rules["tutorial"]["grant_definitions"],
        "recovery_handlers": inputs.rules["tutorial"]["recovery_handlers"],
        "bank_seed_hook": inputs.rules["tutorial"]["bank_seed_hook"],
        "departure": next(rule for rule in inputs.rules["activities"]["rules"] if rule["id"] == "rule.tutorial.departure"),
        "quest_rewards": inputs.rules["cooks-assistant"]["rewards"],
        "decisions": inputs.rules["decisions"],
        "selection_policy": "Source assumptions/conflicts are carried verbatim; none is silently promoted to approval or an observed result.",
    }, True)
    all_placements = []
    seen = Counter()
    for square, row in sorted(world.placements, key=lambda entry: (entry[0], entry[1])):
        identifier = inputs.object_spawn_id(row)
        seen[identifier] += 1
        key = identifier if seen[identifier] == 1 else identifier + f".occurrence{seen[identifier]}"
        all_placements.append([key, square, *row, identifier if identifier in content["spawns"] else None])
    emit(BINDINGS / "world-bindings.json.gz", {
        "schema_version": 1,
        "coordinate_system": inputs.selection["coordinate_system"],
        "objects": object_bindings(inputs),
        "placement_columns": ["placement_id", "source_square", "object_source_id", "x", "y", "plane", "type", "orientation", "runtime_interaction_anchor"],
        "placements": all_placements,
        "regions": [{"source_square": number, "asset": f"{ASSET_PREFIX}region.{number}",
                     "full_geometry": f"content/m1/geometry/{number}.json.gz",
                     "native_bounds": [raw["base_x"], raw["base_y"], raw["base_x"] + 63, raw["base_y"] + 63],
                     "source_placements": len(raw["placements"])} for number, raw in sorted(world.raw.items())],
        "source_scene_policy": "All151019 cache placements, including scenery outside navigation bounds, retained. "
                               "Original scene assets, not generated placeholders, carry terrain/material/roof/bridge data. "
                               "This is not an approved visibility radius, fence, renderer or source screenshot.",
        "collision_statistics": dict(world.statistics),
        "clipping_outside_source_bounds": [{"tile": list(point), "contributions": count}
                                          for point, count in sorted(world.outside_clipping.items())],
    })
    asset_ids = set()
    def collect(value):
        if isinstance(value, dict):
            for key, item in value.items():
                if key in ("asset", "scene_asset", "animation", "sound") and isinstance(item, str):
                    asset_ids.add(item)
                else:
                    collect(item)
        elif isinstance(value, list):
            for item in value:
                collect(item)
    collect(content)
    for category in ("items", "npcs"):
        for binding in bindings[category].values():
            asset_ids.add(binding["source_asset"])
            asset_ids.update(f"{ASSET_PREFIX}model.{number}" for number in binding["models"])
    for binding in bindings["interfaces"].values():
        asset_ids.update(f"{ASSET_PREFIX}interface.{number}" for number in binding["source_groups"])
    missing_assets = set(asset_ids - inputs.assets.keys())
    additional_assets = set()
    for identifier in bindings["application"]["item_extensions"]:
        extra = bindings["items"][identifier]
        additional_assets.add(extra["source_asset"])
        additional_assets.update(f"{ASSET_PREFIX}model.{number}" for number in extra["models"])
    if missing_assets - additional_assets:
        raise ValueError(f"Product asset references absent from validated merged catalog: {sorted(missing_assets - additional_assets)}")
    emit(CONTENT / "asset-references.json", {
        "assets": [{"id": identifier, "kind": inputs.assets[identifier]["kind"],
                    "outputs": inputs.assets[identifier]["outputs"]} for identifier in sorted(asset_ids - missing_assets)],
        "manifest": inputs.catalog_path,
        "publications": [inputs.publication["base_publication"]["path"], str(PUBLICATION.relative_to(ROOT))],
        "collection_extensions": inputs.publication["collection_extensions"],
        "verified_definition_extensions": {
            "path": "research/m1-bindings/application-item-definitions.json.gz",
            "sha256": sha((BINDINGS / "application-item-definitions.json.gz").read_bytes()),
            "items": bindings["application"]["item_extensions"],
            "unpublished_asset_ids": sorted(missing_assets),
            "scope": "New owner-approved3-dose potion identity and reciprocal note only. Original assets are not fabricated; "
                     "source-worker publication of these exact additional IDs remains a separate graphical hookup.",
        },
        "output_path_policy": "Asset outputs keep their canonical extraction-relative paths. Resolve committed "
                              "closure bytes with published_files[].extraction_path -> path in the additive publication; "
                              "definition provenance names the exact original or extension collection shard.",
        "source_closure_missing": {
            "item_definition_ids": [value["source_id"] for value in bindings["items"].values() if not value["asset_available"]],
            "npc_definition_ids": [value["source_id"] for value in bindings["npcs"].values() if not value["asset_available"]],
            "model_ids": sorted({model for category in ("items", "npcs") for value in bindings[category].values()
                                 for model in value["missing_model_ids"]}),
            "interface_groups": sorted({group for value in bindings["interfaces"].values()
                                        for group in value["missing_source_groups"]}),
        },
        "source_data_not_presentation_approval": True,
    })
    emit(BINDINGS / "structural-validation.json", checks, True)
    manifest = {
        "schema_version": 1, "revision": revision, "baseline": content["baseline"],
        "input_aggregate_sha256": lock["aggregate_sha256"],
        "build_command": "python3 tools/m1-content/build.py",
        "check_command": "python3 tools/m1-content/check.py",
        "compiler_command": "python3 tools/m1-content/compile.py --compiler-manifest crates/content/Cargo.toml",
        "outputs": outputs, "counts": checks["counts"],
        "source_geometry": dict(world.statistics),
        "content_schema_version": 2, "artifact_version": 2, "runtime_schema_version": 1,
        "runtime_ready": False, "source_gameplay_verified": False,
        "presentation_approved": False, "milestone_accepted": False,
        "readiness_dependency": "research/m1-bindings/contract-gaps.json",
        "source_application": {
            "audit": "research/runtime-bindings/application-result.json",
            "source_bindings_consumed": bindings["application"]["bound_path_count"],
            "coupled_updates": bindings["application"]["coupled_update_count"],
            "approved_adaptation": bindings["application"]["approved_loot"]["adaptation"],
            "residuals": bindings["application"]["residuals"],
            "remaining_selector_hooks": bindings["application"]["remaining_selector_hooks"],
        },
        "scope_policy": inputs.selection["navigation_scope_note"],
    }
    write(CONTENT / "manifest.json", manifest, True)
    if args.json_output:
        write(args.json_output, content)
    subprocess.run([sys.executable, str(ROOT / "tools/m1-content/compile.py")], cwd=ROOT, check=True)
    compiler = load(BINDINGS / "compiler-validation.json")
    manifest.update({"runtime_compile_passed": compiler["runtime_compile_passed"],
                     "compiled_artifact": compiler["artifact"],
                     "unresolved_binding_count": compiler["unresolved_binding_count"]})
    write(CONTENT / "manifest.json", manifest, True)
    print(__import__("json").dumps({"revision": revision, "counts": checks["counts"],
                                   "runtime_ready": False, "structural_checks": "passed",
                                   "runtime_compile_passed": True,
                                   "unresolved_binding_count": compiler["unresolved_binding_count"]}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json-output", type=Path, help="Optional explicit uncompressed GameContent path.")
    args = parser.parse_args()
    if args.json_output and not any(args.json_output.resolve().is_relative_to(directory)
                                    for directory in (CONTENT, BINDINGS, ROOT / "tools/m1-content")):
        parser.error("All build outputs must be inside an owned M1 directory.")
    build(args)


if __name__ == "__main__":
    main()

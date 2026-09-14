#!/usr/bin/env python3
"""Verify the bounded asset refresh without certifying or changing M1 behavior."""

import argparse
import json

from common import BINDINGS, CONTENT, PUBLICATION, ROOT, Inputs, canonical, load, sha, write


BASELINE = BINDINGS / "asset-refresh-baseline.json"
CATALOG_SHA256 = "5f429fecea1b2526a146adb8e96c6c564343cd248f3a12c75911e17c57054b7b"
REQUESTED = {"item_definition_ids": 72, "model_ids": 68, "npc_definition_ids": 6, "interface_groups": 13}
KINDS = {"item_definition_ids": "item", "model_ids": "model", "npc_definition_ids": "npc", "interface_groups": "interface"}


def behavior_projection(content):
    def project(value):
        if isinstance(value, dict):
            return {key: project(child) for key, child in value.items() if key not in ("source", "asset")}
        if isinstance(value, list):
            return [project(child) for child in value]
        return value
    result = project(content)
    result.pop("revision")
    return result


def file_hashes(paths):
    return {str(path.relative_to(ROOT)): sha(path.read_bytes()) for path in sorted(paths)}


def record_baseline():
    if BASELINE.exists():
        raise ValueError("The frozen pre-refresh baseline already exists; do not overwrite it.")
    content = load(CONTENT / "game-content.json.gz")
    manifest = load(CONTENT / "manifest.json")
    unresolved = load(BINDINGS / "unresolved-bindings.json")
    if manifest["compiled_artifact"]["uncompressed_sha256"] != "88a6f32810712b2f64e2e9a8baa5cdfe0f817e49ac8352f4e4d656c9be995e68":
        raise ValueError("Pre-refresh artifact differs from the supplied parent baseline")
    if unresolved["count"] != 111:
        raise ValueError("Pre-refresh behavior binding inventory differs from the current parent baseline")
    baseline = {
        "schema_version": 1,
        "commit": "34c8ef4",
        "revision": manifest["revision"],
        "compiled_artifact": manifest["compiled_artifact"],
        "content_compressed_sha256": sha((CONTENT / "game-content.json.gz").read_bytes()),
        "behavior_sha256": sha(canonical(behavior_projection(content))),
        "counts": manifest["counts"],
        "unresolved_bindings": unresolved["bindings"],
        "source_geometry_hashes": file_hashes((CONTENT / "geometry").glob("*.json.gz")),
        "source_only_files": file_hashes(BINDINGS / name for name in (
            "graph-bindings.json", "door-bindings.json", "travel-bindings.json", "location-bindings.json",
            "policy-bindings.json", "profile-v2.json", "selection.json")),
        "baseline_asset_references": load(CONTENT / "asset-references.json")["source_closure_missing"],
        "normalization": "Only revision, source provenance and nullable asset references are excluded from "
                         "the behavior fingerprint. Every other gameplay/schema/geometry value is preserved.",
    }
    write(BASELINE, baseline, pretty=True)
    print(json.dumps({"baseline_recorded": True, "behavior_sha256": baseline["behavior_sha256"], "unresolved": 111}))


def verify():
    inputs = Inputs()  # The shared loader validates all catalog, publication, shard and source-output hashes.
    baseline = load(BASELINE)
    content = load(CONTENT / "game-content.json.gz")
    manifest = load(CONTENT / "manifest.json")
    references = load(CONTENT / "asset-references.json")
    request = load(ROOT / inputs.publication["request"]["path"])
    closure = load(ROOT / inputs.publication["closure_report"]["path"])
    unresolved = load(BINDINGS / "unresolved-bindings.json")
    require(sha((ROOT / inputs.catalog_path).read_bytes()) == CATALOG_SHA256, "Wrong merged source catalog")
    require(sha(canonical(behavior_projection(content))) == baseline["behavior_sha256"], "Non-asset content behavior changed")
    require(manifest["counts"] == baseline["counts"], "Bounded refresh changed content/geometry/graph counts")
    require(unresolved["count"] == 111 and
            [(record["path"], record["reason"]) for record in unresolved["bindings"]] ==
            [(record["path"], record["reason"]) for record in baseline["unresolved_bindings"]],
            "Bounded refresh changed another worker's behavior bindings")
    for name, expected in {**baseline["source_geometry_hashes"], **baseline["source_only_files"]}.items():
        require(sha((ROOT / name).read_bytes()) == expected, f"Protected source content changed: {name}")
    require(references["manifest"] == inputs.catalog_path, "Product still points at the old source catalog")
    require(all(not values for values in references["source_closure_missing"].values()), "Product asset closure is not empty")
    require(set(references["source_closure_missing"]) == set(REQUESTED), "Missing closure category")
    require(closure["requested"] == REQUESTED and closure["resolved"] == REQUESTED, "Wrong frozen source closure request")
    required = set(request["required_asset_ids"])
    actual = {record["id"] for record in references["assets"]}
    require(actual == required and len(actual) == 5254, "Product does not expose exactly the requested real source asset set")
    require(len(actual) == len(references["assets"]), "Duplicate product asset reference")
    resolved = {}
    for category, expected_count in REQUESTED.items():
        identifiers = request["reported_missing_ids"][category]
        require(len(identifiers) == expected_count, "Frozen missing-ID list changed")
        require(identifiers == baseline["baseline_asset_references"][category], "Closure roots differ from parent baseline")
        resolved[category] = sum(inputs.asset(KINDS[category], number) in actual for number in identifiers)
    require(resolved == REQUESTED, "A requested original asset is still not connected")
    for category, kind in (("items", "item"), ("npcs", "npc"), ("objects", "object")):
        for identifier, definition in content[category].items():
            asset = inputs.asset(kind, definition["source_id"])
            require(asset is not None and definition["asset"] == asset, f"Unresolved or fabricated asset on {identifier}")
            if kind in ("item", "npc"):
                locator = f"{inputs.collection_sources[asset]}#{asset}"
                require(any(record["reference"] == locator for record in definition["source"]), f"Wrong source shard on {identifier}")
    item_bindings = load(BINDINGS / "item-bindings.json")["items"]
    npc_bindings = load(BINDINGS / "npc-bindings.json")
    interface_bindings = load(BINDINGS / "interface-bindings.json")
    require(all(record["asset_available"] and not record["missing_model_ids"] for record in
                [*item_bindings.values(), *npc_bindings.values()]), "Definition/model binding still unavailable")
    require(all(not record["missing_source_groups"] for record in interface_bindings.values()), "Interface binding still unavailable")
    for record in references["assets"]:
        require(record["outputs"] == inputs.assets[record["id"]]["outputs"], "Canonical source model/output links were altered")
    lock = load(BINDINGS / "input-lock.json")
    locked = {record["path"]: record for record in lock["inputs"]}
    expected_inputs = [
        *inputs.publication["published_files"], *inputs.publication["collection_extensions"].values(),
        *[inputs.publication[key] for key in ("base_bundle", "base_publication", "merged_inventory",
                                            "extraction_inventory", "request", "closure_report", "dependency_graph")],
        request["product_definitions_snapshot"],
    ]
    for record in expected_inputs:
        path = record["path"]
        require(path in locked and locked[path]["sha256"] == record["sha256"] and
                locked[path]["bytes"] == record["size_bytes"], f"Missing/mismatched published input lock: {path}")
        require(sha((ROOT / path).read_bytes()) == record["sha256"], f"Published source changed: {path}")
    require(str(PUBLICATION.relative_to(ROOT)) in locked, "Additive publication is not input-hashed")
    source_report = load(ROOT / "research/current-source/m1-content-closure-validation.json")
    for record in source_report["outputs"].values():
        if isinstance(record, dict) and "path" in record and "sha256" in record:
            require(sha((ROOT / record["path"]).read_bytes()) == record["sha256"],
                    f"Authoritative source closure output changed: {record['path']}")
    protected = source_report["protected_input_integrity"]
    require(sha((ROOT / protected["path"]).read_bytes()) == protected["sha256"], "Protected-source evidence changed")
    protected = load(ROOT / protected["path"])
    for record in [*protected["unchanged_locks"].values(), protected["reference_sets"], protected["audio_runtime_manifest"]]:
        require(sha((ROOT / record["path"]).read_bytes()) == record["sha256"], f"Protected source lock changed: {record['path']}")
    report = {
        "schema_version": 1, "task": "m1-content-asset-refresh", "result": "passed",
        "scope": "Asset/provenance refresh only; not full M1 source or gameplay certification.",
        "revision": content["revision"], "compiled_artifact": manifest["compiled_artifact"],
        "merged_catalog": inputs.publication["merged_inventory"],
        "publication_sha256": sha(PUBLICATION.read_bytes()),
        "requested": REQUESTED, "resolved": resolved, "remaining_missing_inputs": [],
        "product_assets_resolved": len(actual), "merged_inventory_records": len(inputs.assets),
        "new_original_assets": len(closure["new_asset_ids"]), "new_original_outputs": len(inputs.publication["published_files"]),
        "original_catalog_records_preserved": inputs.publication["existing_inventory_records_preserved"],
        "original_source_files_validated": protected["original_published_files_unchanged"],
        "protected_source_closure_evidence": source_report["protected_input_integrity"],
        "protected_reference_image_count": protected["original_reference_images_unchanged"],
        "protected_audio_file_count": protected["audio_runtime_files_unchanged"],
        "behavior_sha256": baseline["behavior_sha256"], "behavior_and_geometry_unchanged": True,
        "bound_wind_strike_table": content["mechanics"]["combat_styles"]["style.magic.wind_strike"]["maximum_hit"],
        "unresolved_behavior_bindings_unchanged": unresolved["count"],
        "runtime_compile_passed": manifest["runtime_compile_passed"],
        "gameplay_executed": False, "presentation_approved": False,
        "remaining_data_hookup": "Consumers resolve the merged catalog and its additive publication/shards; "
                                 "111 behavior policies and precise runtime selectors remain with their owners. "
                                 "Original assets are not scene/render/audio acceptance.",
    }
    write(BINDINGS / "asset-refresh-validation.json", report, pretty=True)
    print(json.dumps({"asset_refresh": "passed", "requested_resolved": resolved, "product_assets": len(actual),
                      "missing": [], "behavior_unchanged": True, "unresolved_behavior_bindings": unresolved["count"]}))
    return report


def require(condition, message):
    if not condition:
        raise ValueError(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--record-baseline", action="store_true",
                        help="Freeze the supplied pre-refresh parent artifact before regenerating.")
    arguments = parser.parse_args()
    if arguments.record_baseline:
        record_baseline()
    else:
        verify()


if __name__ == "__main__":
    main()

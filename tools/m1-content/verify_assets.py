#!/usr/bin/env python3
"""Verify the bounded asset refresh without certifying or changing M1 behavior."""

import argparse
import json

from common import BINDINGS, CONTENT, PUBLICATION, ROOT, Inputs, canonical, load, sha, write


BASELINE = BINDINGS / "asset-refresh-baseline.json"
CATALOG_SHA256 = "a6bcdca0fe8288b4b586a11537a646c7e745d49c874aa1329e508639a939381e"
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
    report = inspect_assets()
    require(report["current_asset_closure_passed"],
            f"Current product still requires original asset publication: {report['new_verified_definition_publication_needed']}")
    return report


def inspect_assets():
    inputs = Inputs()  # The shared loader validates all catalog, publication, shard and source-output hashes.
    baseline = load(BASELINE)
    content = load(CONTENT / "game-content.json.gz")
    ui_profile = content.get("ui")
    if ui_profile is not None:
        from ui4 import CONTAINER_ITEMS, legacy_content
        from verify_ui4 import verify_ui4
        verify_ui4(content)
        content = legacy_content(content)
    manifest = load(CONTENT / "manifest.json")
    references = load(CONTENT / "asset-references.json")
    ancestor = next(publication for _, publication in inputs.publication_chain
                    if publication.get("merged_inventory", {}).get("path", "").endswith("cache2695-content-v2-bundle.json.gz"))
    request = load(ROOT / ancestor["request"]["path"])
    closure = load(ROOT / ancestor["closure_report"]["path"])
    potion_request = load(ROOT / inputs.publication["request"]["path"])
    potion_closure = load(ROOT / inputs.publication["closure_report"]["path"])
    unresolved = load(BINDINGS / "unresolved-bindings.json")
    application_path = BINDINGS / "application-result.json"
    application = load(application_path) if application_path.exists() else None
    extensions = application["item_extensions"] if application else {}
    require(sha((ROOT / inputs.catalog_path).read_bytes()) == CATALOG_SHA256, "Wrong merged source catalog")
    if application:
        require(application["baseline_behavior_sha256"] == baseline["behavior_sha256"], "Application changed its original behavior boundary")
        require(application["runtime3"]["content_behavior_sha256"] == sha(canonical(behavior_projection(content))),
                "Non-target behavior changed after audited source application")
        require(application["content_compressed_sha256"] == sha((CONTENT / "game-content.json.gz").read_bytes()),
                "Application audit describes another content artifact")
        require(application["bound_path_count"] == 104 and application["coupled_update_count"] == 7,
                "Incomplete canonical source application")
        for key, count in baseline["counts"].items():
            if key not in ("items", "normal_note_pairs", "mechanics", "recipes"):
                current_count = len(content["interfaces"]) if key == "interfaces" and ui_profile else manifest["counts"][key]
                require(current_count == count, f"Source application changed non-target count {key}")
        require(set(extensions) == {"item.energy_potion.three_dose", "item.energy_potion.three_dose.noted"},
                "Unapproved item-extension scope")
    else:
        require(sha(canonical(behavior_projection(content))) == baseline["behavior_sha256"], "Non-asset content behavior changed")
        require(manifest["counts"] == baseline["counts"], "Bounded refresh changed content/geometry/graph counts")
        require(unresolved["count"] == 111 and
                [(record["path"], record["reason"]) for record in unresolved["bindings"]] ==
                [(record["path"], record["reason"]) for record in baseline["unresolved_bindings"]],
                "Bounded refresh changed another worker's behavior bindings")
    for name, expected in {**baseline["source_geometry_hashes"], **baseline["source_only_files"]}.items():
        if application and name in ("research/m1-bindings/selection.json", "research/m1-bindings/door-bindings.json"):
            continue  # Verified item closure and explicit v3 combined-door records have separate audits.
        require(sha((ROOT / name).read_bytes()) == expected, f"Protected source content changed: {name}")
    require(references["manifest"] == inputs.catalog_path, "Product still points at the old source catalog")
    missing = references["source_closure_missing"]
    if ui_profile:
        require(set(missing["item_definition_ids"]).issubset(set(CONTAINER_ITEMS.values()))
                and set(missing["model_ids"]).issubset({561, 2548, 2747, 8234})
                and not missing["npc_definition_ids"] and not missing["interface_groups"],
                "A previously closed source asset became missing outside the exact UI extension")
    else:
        require(all(not values for values in missing.values()), "Product asset closure is not empty")
    require(set(references["source_closure_missing"]) == set(REQUESTED), "Missing closure category")
    require(closure["requested"] == REQUESTED and closure["resolved"] == REQUESTED, "Wrong frozen source closure request")
    required = set(potion_request["required_asset_ids"])
    actual = {record["id"] for record in references["assets"]}
    ui_assets = {ui_profile["appearance_base"]["value"]["asset"]} if ui_profile else set()
    require(actual == required | ui_assets and len(required) == 5257, "Product changed the exact original source asset boundary")
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
            if identifier in extensions:
                require(asset is not None and definition["asset"] == asset, "Published source potion asset was not bound")
                require(any(record["reference"] == extensions[identifier]["source_definition"] for record in definition["source"]),
                        "New original item lost its exact definition provenance")
            require(asset is not None and definition["asset"] == asset, f"Unresolved or fabricated asset on {identifier}")
            if kind in ("item", "npc"):
                locator = f"{inputs.collection_sources[asset]}#{asset}"
                require(any(record["reference"] == locator for record in definition["source"]), f"Wrong source shard on {identifier}")
    item_bindings = load(BINDINGS / "item-bindings.json")["items"]
    npc_bindings = load(BINDINGS / "npc-bindings.json")
    interface_bindings = load(BINDINGS / "interface-bindings.json")
    require(all(record["asset_available"] and not record["missing_model_ids"] for identifier, record in
                list(item_bindings.items()) + list(npc_bindings.items()) if not ui_profile or identifier not in CONTAINER_ITEMS),
            "Previously completed definition/model binding became unavailable")
    require(all(not record["missing_source_groups"] for record in interface_bindings.values()), "Interface binding still unavailable")
    for record in references["assets"]:
        require(record["outputs"] == inputs.assets[record["id"]]["outputs"], "Canonical source model/output links were altered")
    lock = load(BINDINGS / "input-lock.json")
    locked = {record["path"]: record for record in lock["inputs"]}
    expected_inputs = []
    for path, publication in inputs.publication_chain:
        require(str(path.relative_to(ROOT)) in locked, "Publication ancestor is not input-hashed")
        expected_inputs.extend(publication["published_files"])
        expected_inputs.extend(publication.get("collection_extensions", {}).values())
        expected_inputs.extend(publication[key] for key in ("base_bundle", "base_publication", "merged_inventory",
                                            "extraction_inventory", "request", "closure_report", "dependency_graph") if key in publication)
        if "request" in publication:
            expected_inputs.append(load(ROOT / publication["request"]["path"])["product_definitions_snapshot"])
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
    for record in [*protected["unchanged_locks"].values(), protected["reference_sets"]]:
        require(sha((ROOT / record["path"]).read_bytes()) == record["sha256"], f"Protected source lock changed: {record['path']}")
    audio_context = load(BINDINGS / "runtime3-input-context.json")["audio_runtime"]
    historical_audio = protected["audio_runtime_manifest"]
    require(historical_audio["path"] == audio_context["path"] and
            historical_audio["sha256"] == audio_context["historical_manifest_sha256"],
            "Historical audio input boundary changed")
    audio_bytes = (ROOT / audio_context["path"]).read_bytes()
    require(sha(audio_bytes) == audio_context["current_manifest_sha256"] and
            len(audio_bytes) == audio_context["current_manifest_bytes"],
            "Current upstream audio differs from the exact audited publication")
    audio = json.loads(audio_bytes)
    require(len(audio["assets"]) == audio_context["current_rendered_asset_count"], "Current audio publication count changed")
    for record in audio["assets"]:
        require(sha((ROOT / record["path"]).read_bytes()) == record["sha256"],
                f"Current upstream audio output changed: {record['path']}")
    report = {
        "schema_version": 1, "task": "m1-content-asset-refresh",
        "result": "passed" if not any(missing.values()) else "blocked_new_assets",
        "scope": "Prior original asset/provenance closure preserved; current UI publication is a separate mandatory gate. Not M1 acceptance.",
        "revision": content["revision"], "compiled_artifact": manifest["compiled_artifact"],
        "merged_catalog": inputs.publication["merged_inventory"],
        "publication_sha256": sha(PUBLICATION.read_bytes()),
        "requested": REQUESTED, "resolved": resolved, "remaining_missing_inputs": [],
        "product_assets_resolved": len(required), "current_published_asset_references": len(actual),
        "current_asset_closure_passed": not any(missing.values()), "merged_inventory_records": len(inputs.assets),
        "new_original_assets": len(closure["new_asset_ids"]) + len(potion_closure["new_asset_ids"]),
        "new_original_outputs": sum(len(publication["published_files"]) for _, publication in inputs.publication_chain[1:]),
        "potion_requested_resolved": potion_closure["resolved"],
        "publication_chain": [str(path.relative_to(ROOT)) for path, _ in inputs.publication_chain],
        "original_catalog_records_preserved": ancestor["existing_inventory_records_preserved"],
        "original_source_files_validated": protected["original_published_files_unchanged"],
        "protected_source_closure_evidence": source_report["protected_input_integrity"],
        "protected_reference_image_count": protected["original_reference_images_unchanged"],
        "historical_audio_file_count": protected["audio_runtime_files_unchanged"],
        "current_audio_files_hash_validated": len(audio["assets"]),
        "audio_publication_evolution": audio_context,
        "behavior_sha256": baseline["behavior_sha256"], "behavior_and_geometry_unchanged": application is None,
        "geometry_and_prior_asset_closure_preserved": True,
        "audited_source_application": str(application_path.relative_to(ROOT)) if application else None,
        "new_verified_definition_publication_needed": missing,
        "bound_wind_strike_table": content["mechanics"]["combat_styles"]["style.magic.wind_strike"]["maximum_hit"],
        "unresolved_behavior_bindings_unchanged": unresolved["count"],
        "runtime_compile_passed": manifest["runtime_compile_passed"],
        "gameplay_executed": False, "presentation_approved": False,
        "remaining_data_hookup": "Consumers resolve the merged catalog and its additive publication/shards; "
                                 "Canonical source application/selectors are checked separately. Potion3010/3011, "
                                 "original model2697 and source placeholder19365 are now published and bound. "
                                 "Original assets are not scene/render/audio acceptance.",
    }
    write(BINDINGS / "asset-refresh-validation.json", report, pretty=True)
    print(json.dumps({"prior_asset_closure": "preserved", "requested_resolved": resolved, "product_assets": len(actual),
                      "prior_request_missing": [], "new_verified_definition_publication_needed":
                      references["source_closure_missing"] if application else {},
                      "behavior_unchanged": application is None,
                      "source_application_audited": application is not None,
                      "unresolved_behavior_bindings": unresolved["count"]}))
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

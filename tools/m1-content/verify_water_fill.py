#!/usr/bin/env python3
"""Verify exact source deltas and run real native water cases for both isolated source profiles."""

import gzip
import json
import os
import subprocess

from common import CONTENT, ROOT, load, sha, write
from build_water_legacy import compile_profile, compressed
from water_fill import DIRECTORY, baseline_content, baselines, verify_delta


def compatibility():
    profiles = []
    for name, expected in baselines()["profiles"].items():
        source = baseline_content(name)
        if any("item_on_target" in recipe for recipe in source["recipes"].values()):
            raise ValueError("An old source archive acquired a new item-on declaration")
        data, compiler, command = compile_profile(source, name + "-baseline")
        raw_hash, packed_hash = sha(data), sha(compressed(data))
        if (raw_hash != expected["raw_artifact_sha256"]
                or packed_hash != expected["compressed_artifact_sha256"]):
            raise ValueError("The optional rule changed an old source artifact: " + name)
        profiles.append({
            "profile": name, "source_input": expected["input"], "source_sha256": expected["source_sha256"],
            "raw_sha256": raw_hash, "compressed_sha256": packed_hash, "exact_recompile": True,
            "new_field_absent_and_omitted": True, "compiler_command": command,
            "compiler_versions": [compiler["schema_version"], compiler["artifact_version"]],
        })
    report = {"schema_version": 1, "passed": True, "profiles": profiles,
              "source_archives_changed": False, "migration_admitted": False}
    write(DIRECTORY / "item-on-compatibility.json", report, True)
    return report


def native(profile, manifest_path, source_path):
    manifest = load(manifest_path)
    proof = verify_delta(load(source_path))
    artifact = manifest["compiled_artifact"]
    packed = (ROOT / artifact["path"]).read_bytes()
    data = gzip.decompress(packed)
    if sha(packed) != artifact["sha256"] or sha(data) != artifact["uncompressed_sha256"]:
        raise ValueError("Water native probe input does not match its exact profile pin")
    work = ROOT / "tools/m1-content/.local/water-fill" / ("native-" + profile)
    work.mkdir(parents=True, exist_ok=True)
    raw = work / "source.csc"
    raw.write_bytes(data)
    command = [
        "cargo", "run", "--quiet", "--locked", "--offline",
        "--manifest-path", str(ROOT / "tools/m1-content/schema-check/Cargo.toml"),
        "--example", "water_fill", "--", str(raw),
    ]
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, env={
        **os.environ, "CARGO_TARGET_DIR": str(work / "target"), "TMPDIR": str(work),
    })
    report = {
        "schema_version": 1, "profile": profile, "command": command, "exit_code": result.returncode,
        "stderr": result.stderr, "artifact": artifact, "semantic_delta": proof,
        "native": json.loads(result.stdout) if result.stdout.strip() else None,
        "passed": False, "migration_admitted": False,
    }
    if report["native"] is not None:
        output = report["native"]
        if output["artifact_sha256"] != artifact["uncompressed_sha256"] or output["revision"] != manifest["revision"]:
            raise ValueError("Native test loaded a stale or different source profile")
        report["passed"] = result.returncode == 0 and output["water"]["passed"]
    write(DIRECTORY / (profile + "-native.json"), report, True)
    return report


def main():
    old_profiles = compatibility()
    profiles = [
        native("current5b-water", CONTENT / "manifest.json", CONTENT / "game-content.json.gz"),
        native("legacy5e-water", CONTENT / "legacy5e-water/manifest.json",
               CONTENT / "legacy5e-water/game-content.json.gz"),
    ]
    report = {
        "schema_version": 1, "passed": all(row["passed"] for row in profiles),
        "profiles": profiles, "actual_account_read_or_changed": False, "migration_admitted": False,
        "old_source_profiles_byte_exact": old_profiles["passed"],
        "scope": "Strict source artifacts and controlled native components only; not account, server, browser or presentation acceptance.",
    }
    write(DIRECTORY / "validation.json", report, True)
    facts = load(DIRECTORY / "facts.json")
    write(DIRECTORY / "handoff.json", {
        "schema_version": 1, "task": "M1-WATER-ITEM-ON-DISPATCH", "approval": baselines()["approval"],
        "status": "ready_for_separate_review" if report["passed"] else "blocked_on_item_on_only_binding",
        "profiles": [{"profile": row["profile"], "source_baseline": row["semantic_delta"]["base"],
                      "candidate_artifact": row["artifact"],
                      "changed_json_paths": row["semantic_delta"]["changed_json_paths"],
                      "unchanged_remainder_sha256": row["semantic_delta"]["remainder_semantic_sha256"],
                      "native_passed": row["passed"]} for row in profiles],
        "source_behavior": {"input": facts["input"], "output": facts["output"], "byproducts": [],
                            "xp_rewards": facts["xp_rewards"], "single_ticks": 1, "first_ticks": 1,
                            "repeat_ticks": 1, "menu_delay": 0,
                            "cadence_qualification": facts["cadence_qualification"],
                            "object_menu_operations": [], "actor_motion": facts["actor_motion"]},
        "source_contact_candidates": profiles[0]["semantic_delta"]["source_contact_candidates"],
        "contact_qualification": profiles[0]["semantic_delta"]["contact_qualification"],
        "preserved_existing_unknowns": profiles[0]["semantic_delta"]["preexisting_source_unknowns_preserved"],
        "broader_profile_differences": "research/water-fill/consumer-baseline-differences.json",
        "native_evidence": ["research/water-fill/current5b-water-native.json",
                            "research/water-fill/legacy5e-water-native.json"],
        "old_source_profile_compatibility": "research/water-fill/item-on-compatibility.json",
        "dispatch_contract": {
            "field": "RecipeDefinition.item_on_target: Option<SourceBinding<ItemOnTargetRule>>",
            "serialization": "Absent is omitted, preserving exact old5b and5e artifact bytes.",
            "rule": "Source reach and guard for exactly one inventory conversion, no object menu or automatic batch.",
            "admission": "Start/projection/pending checks share single-mode and target admission; original recipes retain Production menu guards.",
            "menu": "Source sink14868 interactions remain empty. Mixed menu/item-on declarations are rejected.",
            "implementation_hashes": {name: sha((ROOT / name).read_bytes()) for name in (
                "crates/game-types/src/content.rs", "crates/world-engine/src/activities.rs",
                "crates/world-engine/src/permissions.rs", "crates/world-engine/src/validation.rs")},
        },
        "remaining_seam": None if report["passed"] else "Inspect the explicit failing native case; no permission, geometry, timing or menu fallback is allowed.",
        "driver": {"files": ["tools/simulator/src/journey/plan.rs", "tools/simulator/src/journey/source.rs"],
                   "source_clear_candidates": True, "arbitrary_east_side_removed": True,
                   "preflight_refuses_missing_dispatch_before_inputs_or_reacquisition": True,
                   "existing_flour_and_bucket_are_reused": True,
                   "actual_driver_or_network_pump_run": False},
        "migration_admitted": False, "private_state_read": False, "gameplay_or_presentation_accepted": False,
        "parent_action": "Director reviews the exact legacy5e+water versus current5b+water pins and conservation results, "
                         "then separately tests/fences any authorized same-world upgrade. This implementation performs no migration. "
                         "No prior progress, reward, acquisition or unknown operation may be repeated.",
    }, True)
    print(json.dumps({"water_native_passed": report["passed"], "profiles": [
        {"profile": row["profile"], "passed": row["passed"], "exit_code": row["exit_code"],
         "raw_artifact_sha256": row["artifact"]["uncompressed_sha256"],
         "positive_passed": row["native"]["water"]["positive_passed"] if row["native"] else None,
         "negative_passed": row["native"]["water"]["negative_passed"] if row["native"] else None}
        for row in profiles], "migration_admitted": False}))
    raise SystemExit(0 if report["passed"] else 1)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Strictly compile the exact public legacy source plus only the audited water recipe."""

from copy import deepcopy
import gzip
import json
import os
import subprocess

from common import CONTENT, ROOT, canonical, load, sha, write
from runtime_application import changed_paths
from water_fill import DIRECTORY, RECIPE, baseline_content, baselines, recipe_definition, verify_delta


def compile_profile(content, profile):
    work = ROOT / "tools/m1-content/.local/water-fill" / profile
    work.mkdir(parents=True, exist_ok=True)
    source = work / "game-content.json"
    source.write_bytes(canonical(content))
    artifact = work / "game-content.csc"
    command = [
        "cargo", "run", "--quiet", "--locked", "--offline",
        "--manifest-path", str(ROOT / "crates/content/Cargo.toml"), "-p", "clubscape-content", "--",
        "--input", str(source), "--output", str(artifact),
    ]
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, env={
        **os.environ, "CARGO_TARGET_DIR": str(work / "target"), "TMPDIR": str(work),
    })
    if result.returncode:
        write(DIRECTORY / (profile + "-compiler-failure.json"), {
            "profile": profile, "command": command, "exit_code": result.returncode,
            "stdout": result.stdout, "stderr": result.stderr, "source_sha256": sha(source.read_bytes()),
            "runtime_compile_passed": False,
        }, True)
        raise subprocess.CalledProcessError(result.returncode, command, result.stdout, result.stderr)
    report = json.loads(result.stdout)
    data = artifact.read_bytes()
    if (not report["ok"] or report["artifact_version"] != 4 or report["schema_version"] != 4
            or report["sha256"] != sha(data)):
        raise ValueError("Actual legacy-profile compiler output failed identity/version verification")
    return data, report, command


def compressed(data):
    result = bytearray(gzip.compress(data, compresslevel=9, mtime=0))
    result[9] = 255
    return bytes(result)


def exact_differences(before, after):
    def value_at(root, pointer):
        value = root
        for part in pointer:
            if isinstance(value, dict) and part not in value:
                return {"present": False}
            value = value[part]
        result = {"present": True, "sha256": sha(canonical(value))}
        if len(canonical(value)) <= 256:
            result["value"] = value
        return result
    return [
        {**row, "json_pointer": "/" + "/".join(str(part).replace("~", "~0").replace("/", "~1") for part in row["pointer"]),
         "before": value_at(before, row["pointer"]), "after": value_at(after, row["pointer"])}
        for row in changed_paths(before, after)
    ]


def build():
    original = baseline_content("legacy5e")
    baseline = baselines()["profiles"]["legacy5e"]
    old_bytes, old_report, old_command = compile_profile(original, "legacy5e-baseline")
    if (sha(old_bytes) != baseline["raw_artifact_sha256"]
            or sha(compressed(old_bytes)) != baseline["compressed_artifact_sha256"]):
        raise ValueError("Current strict compiler does not preserve the exact legacy source artifact")
    candidate = deepcopy(original)
    candidate["recipes"][RECIPE] = recipe_definition()
    delta_hash = sha(canonical({"baseline": baseline["raw_artifact_sha256"], "recipe": candidate["recipes"][RECIPE]}))
    candidate["revision"] += ".water." + delta_hash[:16]
    proof = verify_delta(candidate)
    data, compiler, command = compile_profile(candidate, "legacy5e-water")
    if compiler["validation"]["unresolved_bindings"] != old_report["validation"]["unresolved_bindings"]:
        raise ValueError("Water introduced or discarded an unrelated legacy source binding")
    output = CONTENT / "legacy5e-water"
    output.mkdir(parents=True, exist_ok=True)
    source_record = write(output / "game-content.json.gz", candidate)
    packed = compressed(data)
    artifact = output / "game-content.csc.gz"
    artifact.write_bytes(packed)
    artifact_record = {
        "path": str(artifact.relative_to(ROOT)), "bytes": len(packed), "sha256": sha(packed),
        "uncompressed_bytes": len(data), "uncompressed_sha256": sha(data), "artifact_version": 4, "schema_version": 4,
    }
    comparison = exact_differences(original, baseline_content("current5b"))
    write(DIRECTORY / "consumer-baseline-differences.json", {
        "schema_version": 1, "from": baseline, "to": baselines()["profiles"]["current5b"],
        "differences": comparison, "difference_count": len(comparison),
        "exact_value_policy": "Small values are inline. Every larger value is available at its exact pointer in the two "
                              "committed public source inputs and is content-hashed; nothing is silently normalized.",
        "migration_admitted": False,
    }, True)
    manifest = {
        "schema_version": 1, "profile": "legacy5e-plus-water-candidate", "revision": candidate["revision"],
        "content_schema_version": 4, "artifact_version": 4, "source_baseline": baseline,
        "source_input": source_record, "compiled_artifact": artifact_record,
        "compiler_commands": {"baseline": old_command, "candidate": command},
        "exact_original_artifact_recompiled": True, "runtime_compile_passed": True,
        "compiler_counts": compiler["counts"], "unresolved_bindings": compiler["validation"]["unresolved_bindings"],
        "semantic_delta": proof, "item_on_dispatch_declared": True,
        "native_execution_evidence": "research/water-fill/legacy5e-water-native.json",
        "generator": "python3 tools/m1-content/build_water_legacy.py",
        "current5b_ui_audio_actor_metadata_imported": False,
        "migration_admitted": False, "actual_account_read_or_changed": False,
        "source_observation_claimed": False, "presentation_or_milestone_accepted": False,
    }
    write(output / "manifest.json", manifest, True)
    write(DIRECTORY / "legacy5e-application.json", manifest, True)
    print(json.dumps({"legacy5e_exact_recompile": True, "legacy5e_water_artifact": artifact_record,
                      "changed_json_paths": proof["changed_json_paths"], "broader_5b_difference_count": len(comparison),
                      "item_on_dispatch_declared": True, "native_execution_claimed": False,
                      "migration_admitted": False}))
    return manifest


if __name__ == "__main__":
    build()

#!/usr/bin/env python3
"""Invoke the actual independent content compiler in Runtime mode; preserve failures."""

import argparse
import gzip
import json
import os
from pathlib import Path
import subprocess

from common import BINDINGS, CONTENT, ROOT, canonical, sha, write


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler-manifest", type=Path, default=ROOT / "crates/content/Cargo.toml")
    args = parser.parse_args()
    work = ROOT / "tools/m1-content/.local/compiler"
    work.mkdir(parents=True, exist_ok=True)
    manifest = args.compiler_manifest.resolve()
    if not manifest.is_file():
        parser.error(f"Actual compiler is not integrated at {manifest}; supply its real Cargo.toml, not a replacement validator.")
    source = CONTENT / "game-content.json.gz"
    data = gzip.decompress(source.read_bytes())
    input_path = work / "game-content.json"
    input_path.write_bytes(data)
    output_path = work / "m1.csc"
    command = [
        "cargo", "run", "--quiet", "--locked", "--offline", "--manifest-path", str(manifest),
        "-p", "clubscape-content", "--", "--input", str(input_path), "--output", str(output_path),
    ]
    environment = {**os.environ, "CARGO_TARGET_DIR": str(work / "target"), "TMPDIR": str(work)}
    result = subprocess.run(command, cwd=ROOT, env=environment, text=True, capture_output=True)
    implementation = [{"path": str(path.relative_to(manifest.parent)), "sha256": sha(path.read_bytes())}
                      for path in sorted((manifest.parent / "src").rglob("*.rs"))]
    record = {
        "schema_version": 1,
        "command": command, "input_sha256": sha(data),
        "input_compressed_sha256": sha(source.read_bytes()),
        "compiler_manifest": str(manifest), "compiler_manifest_sha256": sha(manifest.read_bytes()),
        "compiler_sources": implementation,
        "compiler_sources_aggregate_sha256": sha(canonical(implementation)),
        "shared_game_types_content_sha256": sha((ROOT / "crates/game-types/src/content.rs").read_bytes()),
        "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
        "runtime_compile_passed": result.returncode == 0,
        "gameplay_executed": False, "presentation_approved": False,
        "note": "Actual clubscape-content Runtime CLI, no --test-fixture, permissive mode, "
                "field-stripping, synthetic content or substitute validator.",
    }
    if result.returncode == 0:
        artifact = output_path.read_bytes()
        parsed = json.loads(result.stdout)
        if parsed["schema_version"] != 2 or parsed["artifact_version"] != 2 or sha(artifact) != parsed["sha256"]:
            raise ValueError("Actual compiler artifact identity/version mismatch")
        compressed = bytearray(gzip.compress(artifact, compresslevel=9, mtime=0))
        compressed[9] = 255
        committed_path = CONTENT / "game-content.csc.gz"
        committed_path.write_bytes(compressed)
        record["artifact_sha256"] = sha(artifact)
        record["artifact"] = {"path": str(committed_path.relative_to(ROOT)), "bytes": len(compressed),
                              "sha256": sha(compressed), "uncompressed_bytes": len(artifact),
                              "uncompressed_sha256": sha(artifact), "artifact_version": 2, "schema_version": 2}
        decoded = json.loads(data)
        unresolved = []
        def find_bindings(value, path=""):
            if isinstance(value, dict):
                if value.get("status") == "unresolved" or value.get("kind") == "unresolved":
                    unresolved.append({"path": path, "reason": value["reason"], "source": value["source"]})
                for key, child in value.items():
                    find_bindings(child, (path + "." if path else "") + key)
            elif isinstance(value, list):
                for index, child in enumerate(value):
                    find_bindings(child, f"{path}[{index}]")
        find_bindings(decoded)
        reported = parsed["validation"]["unresolved_bindings"]
        if {value["path"] for value in unresolved} != set(reported):
            raise ValueError("Compiler unresolved-binding report differs from authored source binding paths")
        record["unresolved_binding_count"] = len(unresolved)
        write(BINDINGS / "unresolved-bindings.json", {
            "schema_version": 2, "content_sha256": sha(source.read_bytes()),
            "runtime_compile_passed": True, "runtime_success_claimed": False,
            "bindings": unresolved, "count": len(unresolved),
            "profile_candidates": "research/m1-bindings/profile-v2.json",
        }, True)
    write(BINDINGS / "compiler-validation.json", record, True)
    print(json.dumps({
        "runtime_compile_passed": record["runtime_compile_passed"],
        "exit_code": result.returncode,
        "diagnostic": result.stderr[:2400] if result.returncode else None,
        "artifact": record.get("artifact"),
        "unresolved_binding_count": record.get("unresolved_binding_count"),
        "record": "research/m1-bindings/compiler-validation.json",
    }))
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()

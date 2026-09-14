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
        record["artifact_sha256"] = sha(output_path.read_bytes())
    write(BINDINGS / "compiler-validation.json", record, True)
    print(json.dumps({
        "runtime_compile_passed": record["runtime_compile_passed"],
        "exit_code": result.returncode,
        "diagnostic": (result.stderr or result.stdout)[:2400],
        "record": "research/m1-bindings/compiler-validation.json",
    }))
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()

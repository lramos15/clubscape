#!/usr/bin/env python3
"""Export neutral render buffers from the pinned original runtime (tools/render-assets).

This never renders a candidate image. It compiles the Java exporter together with the
existing source-capture bootstrap (read-only reuse) and runs it against the hash-verified
original cache, writing chunked binary buffers plus a manifest under assets/compiled/render.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools/render-assets"
CAPTURE_TOOL = ROOT / "tools/source-capture"
LOCAL = ROOT / ".local/render-assets"
DEFAULT_OUTPUT = ROOT / "assets/compiled/render"


def load_capture_module():
    spec = importlib.util.spec_from_file_location("source_capture", CAPTURE_TOOL / "capture.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def sha(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def normalize(value):
    """Gson round-trips integers as doubles; restore integral numbers."""
    if isinstance(value, float) and value.is_integer() and abs(value) < 2**53:
        return int(value)
    if isinstance(value, dict):
        return {k: normalize(v) for k, v in value.items()}
    if isinstance(value, list):
        return [normalize(v) for v in value]
    return value


def verify_manifest(output: Path) -> dict:
    manifest = json.loads((output / "manifest.json").read_text())
    if manifest["source_cache_id"] != 2695 or manifest["source_revision"] != 240 or manifest["brightness"] != 0.8:
        raise ValueError("Render asset manifest identity differs from the approved source selection")
    checked = 0
    optional_prefixes = ("models/baked/", "tables.bin")
    for name, record in manifest["files"].items():
        path = output / name
        if not path.is_file():
            if name.startswith(optional_prefixes):
                continue
            raise ValueError(f"Missing exported buffer {name}")
        if path.stat().st_size != record["size_bytes"] or sha(path) != record["sha256"]:
            raise ValueError(f"Exported buffer changed: {name}")
        checked += 1
    return {"schema_version": 1, "result": "passed", "files": checked,
            "manifest_sha256": sha(output / "manifest.json"), "candidate_render_acceptance": False}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--profile", default="all",
                        choices=["all", "tables", "palette", "textures", "models", "npcs", "scenes", "prune-textures"])
    parser.add_argument("--verify-only", action="store_true")
    parser.add_argument("--java-home", type=Path, default=Path.home() / ".local/share/jdks/temurin-17.0.20.1+1")
    parser.add_argument("extra", nargs="*", help="Profile-specific arguments passed to the Java exporter")
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to(ROOT):
        raise ValueError("Export output must stay inside this worktree")
    if args.verify_only:
        print(json.dumps(verify_manifest(args.output), separators=(",", ":")))
        return 0
    capture = load_capture_module()
    if args.source is None:
        reused = ROOT.parent / "m1-runtime-inputs/.local/current-source"
        args.source = reused if reused.exists() else ROOT / ".local/current-source"
    libraries = capture.prepare(args.source)
    classes, home, scratch = LOCAL / "classes", LOCAL / "java-home", LOCAL / "java-work"
    for directory in [classes, home, scratch, args.output]:
        directory.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(str(path) for path in libraries)
    java, javac = args.java_home / "bin/java", args.java_home / "bin/javac"
    sources = sorted((TOOL / "java").glob("*.java")) + sorted(CAPTURE_TOOL.glob("*.java"))
    subprocess.run([str(javac), "--release", "17", "-nowarn", "-cp", cp, "-d", str(classes), *map(str, sources)], check=True, cwd=ROOT)
    command = [str(java), "-ea", "-Xmx6g", "-Djava.awt.headless=true",
               "--add-opens=java.base/java.lang=ALL-UNNAMED",
               "-Duser.home=" + str(home), "-Djava.io.tmpdir=" + str(scratch),
               "-cp", str(classes) + os.pathsep + cp, "RenderExport",
               str(capture.LOCAL / "cache"), str(args.output.resolve()), args.profile, *args.extra]
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=1800,
                            env=dict(os.environ, TMPDIR=str(scratch), TEMP=str(scratch), TMP=str(scratch)))
    log = result.stdout + result.stderr
    (LOCAL / "export.log").write_text(log)
    if result.returncode:
        print(log[-8000:])
    result.check_returncode()
    for line in log.splitlines():
        if line.startswith(("TABLES", "PALETTE", "TEXTURES", "MODEL", "NPCS", "SCENE", "RENDER_EXPORT_OK")):
            print(line)
    manifest_path = args.output / "manifest.json"
    manifest = normalize(json.loads(manifest_path.read_text()))
    manifest["brightness"] = 0.8
    manifest_path.write_text(json.dumps(manifest, indent=1) + "\n")
    print(json.dumps(verify_manifest(args.output), separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"render-assets: {error}", file=sys.stderr)
        sys.exit(1)

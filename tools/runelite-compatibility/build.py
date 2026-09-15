#!/usr/bin/env python3
"""Bounded Java build with generated schema bindings; no upstream client patch."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

from prepare import ROOT, LOCAL, DEFAULT_SOURCE, digest, verify


def build(java_home, source, *, report_path=None, history_path=None):
    report_path = report_path or ROOT / "research/runelite-feasibility/build.json"
    history_path = history_path or ROOT / "research/runelite-feasibility/build-history.json"
    home = LOCAL / "build-home"
    home.mkdir(parents=True, exist_ok=True)
    cache = LOCAL / "cache-2695"
    cache.mkdir(exist_ok=True)
    records = json.loads((ROOT / "research/current-source/cache-files.json").read_text())
    for record in records:
        original = source / "cache-2695" / record["name"]
        target = cache / record["name"]
        if not target.exists():
            verify(original, record)
            shutil.copyfile(original, target)
        verify(target, record)
    classes = LOCAL / "classes"
    classes.mkdir(exist_ok=True)
    files = sorted((ROOT / "runelite/compatibility").glob("*.java"))
    files += sorted((LOCAL / "generated").rglob("*.java"))
    files += sorted((ROOT / "runelite/integration-tests").glob("*.java"))
    files += [ROOT / "tools/source-capture/MutedAnimationAudio.java"]
    classpath = (LOCAL / "classpath.txt").read_text()
    arguments = [
        str(java_home / "bin/javac"), "-J-Xmx2048m", "-J-XX:ActiveProcessorCount=2",
        f"-J-Duser.home={home}", f"-J-Djava.io.tmpdir={home}",
        "--release", "17", "-encoding", "UTF-8", "-cp", classpath,
        "-d", str(classes), *map(str, files),
    ]
    result = subprocess.run(arguments, cwd=ROOT, capture_output=True, text=True, timeout=180)
    (LOCAL / "build.log").write_text(result.stdout + result.stderr)
    report = {
        "command": arguments, "exit_code": result.returncode,
        "inputs": [{"path": str(p.relative_to(ROOT)), "sha256": digest(p)} for p in files],
        "upstream_modifications": [], "compatibility_verified": False,
    }
    report_path.write_text(json.dumps(report, indent=2) + "\n")
    history = json.loads(history_path.read_text()) if history_path.exists() else []
    history.append({"command": arguments, "exit_code": result.returncode,
                    "input_hashes": {entry["path"]: entry["sha256"] for entry in report["inputs"]},
                    "diagnostic": (result.stdout + result.stderr)[-14000:]})
    history_path.write_text(json.dumps(history, indent=2) + "\n")
    if result.returncode:
        print((result.stdout + result.stderr)[-14000:])
    else:
        print(json.dumps({"javac_exit": 0, "sources": len(files), "upstream_classes_modified": 0}))
    return result.returncode


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--java-home", type=Path, required=True)
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--report", type=Path, default=ROOT / "research/runelite-feasibility/build.json")
    parser.add_argument("--history", type=Path, default=ROOT / "research/runelite-feasibility/build-history.json")
    args = parser.parse_args()
    report, history = args.report.resolve(), args.history.resolve()
    if not all(path.is_relative_to(ROOT / "research/runelite-feasibility") for path in [report, history]):
        parser.error("Build reports must remain in the owned research directory.")
    raise SystemExit(build(args.java_home.resolve(), args.source.resolve(), report_path=report, history_path=history))

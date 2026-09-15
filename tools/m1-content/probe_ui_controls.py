#!/usr/bin/env python3
"""Inspect pinned native control definitions without writing frozen UI or source assets."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def read(path):
    return json.loads(path.read_text())


def verified(path, record):
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    if digest != record["sha256"] or path.stat().st_size != record["size_bytes"]:
        raise ValueError(f"Pinned source input differs: {path}")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--tooling", type=Path, required=True)
    parser.add_argument("--java-home", type=Path, required=True)
    args = parser.parse_args()
    work = ROOT / ".local/evidence/ui-controls"
    for name in ("classes", "java-home", "java-work"):
        (work / name).mkdir(parents=True, exist_ok=True)
    lock = read(ROOT / "tools/cache-import/dependencies.json")
    selection = read(ROOT / "research/current-source/selection.json")
    capture = read(ROOT / "tools/source-capture/dependencies.json")
    records = [lock["decoder"], *lock["libraries"], *selection["runtime"]["artifacts"],
               *capture["additional_artifacts"]]
    libraries = list(dict.fromkeys(verified(args.tooling / r["name"], r) for r in records))
    source_records = read(ROOT / "research/current-source/cache-files.json")
    for record in source_records:
        verified(args.source / record["name"], record)
    cp = os.pathsep.join(map(str, libraries))
    env = dict(os.environ, TMPDIR=str(work / "java-work"))
    sources = sorted((ROOT / "tools/source-capture").glob("*.java"))
    subprocess.run([str(args.java_home / "bin/javac"), "--release", "17", "-cp", cp,
                    "-d", str(work / "classes"), *map(str, sources),
                    str(ROOT / "tools/m1-content/NativeControlProbe.java")],
                   cwd=ROOT, env=env, check=True)
    subprocess.run([str(args.java_home / "bin/java"), "-ea", "-Xmx2g",
                    "-Djava.awt.headless=true",
                    f"-Duser.home={work / 'java-home'}",
                    f"-Djava.io.tmpdir={work / 'java-work'}",
                    "-cp", str(work / "classes") + os.pathsep + cp,
                    "NativeControlProbe", str(args.source), str(work / "native-probe.json")],
                   cwd=ROOT, env=env, check=True, timeout=180)
    for record in source_records:
        verified(args.source / record["name"], record)


if __name__ == "__main__":
    main()

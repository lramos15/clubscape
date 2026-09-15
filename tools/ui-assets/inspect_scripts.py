#!/usr/bin/env python3
"""Read selected original UI scripts or find direct widget-group references."""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess

from prepare import ROOT, TOOL, WORK, read, verified


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selectors", nargs="+", help="Script ID, enum:ID or widget:GROUP")
    parser.add_argument("--source", type=Path, default=ROOT.parent / "m1-runtime-inputs/.local/current-source/cache-2695")
    parser.add_argument("--tooling", type=Path, default=ROOT.parent / "m1-source-captures/.local/source-capture/tooling")
    args = parser.parse_args()
    records = [read(ROOT / "tools/cache-import/dependencies.json")["decoder"],
               *read(ROOT / "tools/cache-import/dependencies.json")["libraries"],
               *read(ROOT / "research/current-source/selection.json")["runtime"]["artifacts"],
               *read(ROOT / "tools/source-capture/dependencies.json")["additional_artifacts"]]
    libraries = list(dict.fromkeys(str(verified(args.tooling / record["name"], record)) for record in records))
    for record in read(ROOT / "research/current-source/cache-files.json"):
        verified(args.source / record["name"], record)
    for name in ("inspect", "scripts", "java-work", "java-home"):
        (WORK / name).mkdir(parents=True, exist_ok=True)
    classpath = os.pathsep.join([str(WORK / "inspect"), *libraries])
    environment = dict(os.environ, TMPDIR=str(WORK / "java-work"))
    subprocess.run(["javac", "--release", "17", "-cp", classpath, "-d", str(WORK / "inspect"),
                    *map(str, sorted((ROOT / "tools/source-capture").glob("*.java"))),
                    str(TOOL / "UiScriptDump.java")], cwd=ROOT, env=environment, check=True)
    subprocess.run(["java", "-Xmx2g", "-Djava.awt.headless=true",
                    "--add-opens=java.base/java.lang=ALL-UNNAMED",
                    f"-Duser.home={WORK / 'java-home'}", f"-Djava.io.tmpdir={WORK / 'java-work'}",
                    "-cp", classpath, "UiScriptDump", str(args.source), str(WORK / "scripts"),
                    *args.selectors], cwd=ROOT, env=environment, check=True, timeout=180)


if __name__ == "__main__":
    main()

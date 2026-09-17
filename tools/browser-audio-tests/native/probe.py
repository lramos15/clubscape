#!/usr/bin/env python3
"""Run hash-pinned, isolated original audio-policy probes without a game session."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
TOOLS = ROOT / "tools/browser-audio-tests/native"
WORK = TOOLS / ".run"
RESEARCH = ROOT / "research/browser-audio-policy"


def record(path):
    return {"size_bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


def artifact_paths(reuse):
    sys.path.insert(0, str(ROOT / "tools/audio-import"))
    import audio_import
    records = audio_import.artifact_records(audio_import.locks())
    result = []
    for item in records:
        path = reuse / "tooling" / item["name"]
        audio_import.verify(path, item)
        result.append(path)
    return result


def compact_provenance(value):
    if isinstance(value, list):
        return [compact_provenance(item) for item in value]
    if not isinstance(value, dict):
        return value
    result = {key: compact_provenance(item) for key, item in value.items() if key != "file_ids"}
    if "file_ids" in value:
        encoded = json.dumps(value["file_ids"], separators=(",", ":")).encode()
        result["file_id_count"] = len(value["file_ids"])
        result["file_ids_sha256"] = hashlib.sha256(encoded).hexdigest()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["preferences", "position", "music", "catalog", "scripts", "objects", "pcm", "disassemble"])
    parser.add_argument("--reuse", type=Path, default=Path("../m1-audio-bindings/.local/audio-import"))
    parser.add_argument("--cache", type=Path, default=Path("../m1-runtime-inputs/.local/current-source/cache-2695"))
    parser.add_argument("--classes", nargs="*", default=[])
    args = parser.parse_args()
    if Path.cwd().resolve() != ROOT:
        raise ValueError("Run from this worktree root.")
    for part in ["classes", "java-home", "work", "bytecode"]:
        (WORK / part).mkdir(parents=True, exist_ok=True)
    RESEARCH.mkdir(parents=True, exist_ok=True)
    jars = artifact_paths(args.reuse)
    if args.mode == "disassemble":
        for name in args.classes:
            if not name.replace("$", "").isalnum():
                raise ValueError("Expected an explicit native class name.")
            with (WORK / "bytecode" / f"{name}.txt").open("w") as output:
                subprocess.run(["javap", "-J-XX:-UsePerfData", "-c", "-p", "-classpath",
                                os.pathsep.join(map(str, jars)), name], stdout=output, check=True, timeout=30)
        print("Disassembled " + ", ".join(args.classes))
        return
    java_options = [
        "-XX:-UsePerfData", "-XX:ActiveProcessorCount=2", "-Xmx1g", "-Djava.awt.headless=true",
        f"-Djava.io.tmpdir={WORK / 'work'}", f"-Duser.home={WORK / 'java-home'}",
    ]
    source = [
        ROOT / "tools/audio-import/SourceAudio.java",
        ROOT / "tools/audio-import/BindingCache.java",
        ROOT / "tools/audio-import/BindingExtract.java",
        ROOT / "tools/source-capture/FixturePreferenceWrites.java",
        TOOLS / "NativeArchive.java",
        TOOLS / "NativeScriptPolicy.java",
        TOOLS / "NativeObjects.java",
        TOOLS / "NativeMusicPcm.java",
        TOOLS / "NativePolicyProbe.java",
    ]
    subprocess.run([
        "javac", *["-J" + option for option in java_options], "--release", "17", "-proc:none",
        "-cp", os.pathsep.join(map(str, jars)), "-d", str(WORK / "classes"), *map(str, source),
    ], check=True, timeout=120)
    output = WORK / f"{args.mode}.json"
    result = subprocess.run([
        "java", *java_options, "-cp", os.pathsep.join(map(str, [WORK / "classes", *jars])),
        "NativePolicyProbe", args.mode, str(args.reuse / "inputs"), str(args.cache), str(output),
    ], capture_output=True, text=True, timeout=240)
    (WORK / f"{args.mode}.log").write_text(result.stdout + result.stderr)
    if result.returncode:
        raise RuntimeError(f"Native {args.mode} probe failed:\n{result.stderr[-6000:]}")
    value = compact_provenance(json.loads(output.read_text()))
    value["input_artifacts"] = [{"name": path.name, **record(path)} for path in jars]
    value["probe_sources"] = [{"path": str(path.relative_to(ROOT)), **record(path)} for path in source]
    value["runner"] = {"path": str(Path(__file__).resolve().relative_to(ROOT)), **record(Path(__file__))}
    (RESEARCH / f"native-{args.mode}.json").write_text(json.dumps(value, indent=2) + "\n")
    print(json.dumps({"mode": args.mode, "result": value["result"], "cases": len(value["cases"])}))


if __name__ == "__main__":
    main()

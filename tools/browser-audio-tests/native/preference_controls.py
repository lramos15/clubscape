#!/usr/bin/env python3
"""Inspect and execute bounded original preference/control policy, reusing locked local inputs."""

import importlib.util
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
WORK = TOOLS / ".run/preference-controls"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["inspect", "run"], default="run", nargs="?")
    args = parser.parse_args()
    if Path.cwd().resolve() != ROOT:
        raise ValueError("Run from the assigned audio worktree root.")
    spec = importlib.util.spec_from_file_location("native_audio_tools", TOOLS / "probe.py")
    native = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(native)
    jars = native.artifact_paths(Path("../m1-audio-bindings/.local/audio-import"))
    for name in ["classes", "home", "work"]:
        (WORK / name).mkdir(parents=True, exist_ok=True)
    options = [
        "-XX:-UsePerfData", "-XX:ActiveProcessorCount=2", "-Xmx1g", "-Djava.awt.headless=true",
        f"-Duser.home={WORK/'home'}", f"-Djava.io.tmpdir={WORK/'work'}",
    ]
    sources = [
        ROOT / "tools/audio-import/SourceAudio.java",
        ROOT / "tools/audio-import/BindingCache.java",
        ROOT / "tools/audio-import/BindingExtract.java",
        ROOT / "tools/source-capture/FixturePreferenceWrites.java",
        ROOT / "tools/source-capture/FixtureVarcs.java",
        *sorted(TOOLS.glob("*.java")),
    ]
    subprocess.run([
        "javac", *["-J"+value for value in options], "--release", "17", "-proc:none",
        "-cp", os.pathsep.join(map(str,jars)), "-d", str(WORK/"classes"), *map(str,sources),
    ], check=True, timeout=120, capture_output=True)
    runs = []
    for name in (["first", "second"] if args.mode == "run" else ["inspect"]):
        output = WORK / name
        result = subprocess.run([
            "java", *options, "-cp", os.pathsep.join(map(str,[WORK/"classes",*jars])),
            "NativePreferenceControls", "../m1-runtime-inputs/.local/current-source/cache-2695", str(output), args.mode,
        ], capture_output=True, text=True, timeout=120)
        (WORK / f"{name}.log").write_text(result.stdout + result.stderr)
        if result.returncode or "Client error:" in result.stderr:
            raise RuntimeError(f"Original control execution failed:\n{result.stderr[-9000:]}")
        if args.mode == "run":
            runs.append(native.compact_provenance(json.loads((output/"native.json").read_text())))
    if args.mode == "inspect":
        print(json.dumps({"mode":"inspect","controlled_native_states":0}))
        return
    assert runs[0] == runs[1], "The same twelve controlled native states differed across fresh JVMs."
    evidence = runs[1]
    assert evidence["controlled_native_states"] == 12
    definitions = json.loads((WORK/"second/definitions.json").read_text())
    identities = json.loads((WORK/"second/music-identities.json").read_text())
    destination = ROOT/"research/browser-audio-policy/native-preference-controls.json"
    evidence.update({
        "schema_version":1, "runtime":"unmodified injected-client-1.12.38",
        "game_build":240, "cache_id":2695,
        "scope":"Twelve controlled client-policy states, not accounts, gameplay, speakers or M1 acceptance.",
        "native_artifacts":[{"name":path.name,**native.record(path)} for path in jars],
        "probe_sources":[{"path":str(path.relative_to(ROOT)),**native.record(path)}
                         for path in [*sources,Path(__file__).resolve()]],
        "source_scripts":definitions["scripts"], "source_varbits":definitions["varbits"],
        "music_identities":identities,
        "repeatability":{"fresh_jvms":2, "unique_controlled_states":12, "same_twelve_state_results":True,
            "native_result_sha256":hashlib.sha256(json.dumps(runs[0],sort_keys=True,separators=(",",":")).encode()).hexdigest()},
        "bounds":{"path":"research/browser-audio-policy/bounds.json",
                  **native.record(ROOT/"research/browser-audio-policy/bounds.json")},
        "skip_boundary":"9292 guards the UI action and queues2266 only. The server-side next-row chooser is not client bytecode. Browser Skip uses the already-qualified internal selection policy and the executed original3201/rj.bc request path, not claimed measured network-response time.",
        "external_account":False, "personal_preferences_read":False, "hardware_audio_opened":False,
        "source_pack_modified":False, "acceptance":False,
    })
    destination.write_text(json.dumps(evidence,indent=2)+"\n")
    print(json.dumps({"result":evidence["result"],"controlled_native_states":12,
        "scripts":len(definitions["scripts"]), "output":str(destination.relative_to(ROOT)),
        "sha256":hashlib.sha256(destination.read_bytes()).hexdigest()}))


if __name__ == "__main__":
    main()

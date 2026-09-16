#!/usr/bin/env python3
"""Replay the unchanged original normal camera with controlled offline input."""
from __future__ import annotations

import argparse
import gzip
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools/camera-source"
LOCAL = ROOT / ".local/camera-source"
OUT = ROOT / "research/camera-source"
sys.path.insert(0, str(ROOT / "tools/cache-import"))
import import_cache as cache

spec = importlib.util.spec_from_file_location("original_capture", ROOT / "tools/source-capture/capture.py")
original = importlib.util.module_from_spec(spec)
spec.loader.exec_module(original)
SCENARIOS = ("initial", "keyboard", "clamps", "mouse", "follow", "follow-threshold",
             "frame-rates", "resize", "terrain", "wheel", "orbit", "region-shift",
             "zoom-separation", "script-init-scan")


def prepare(source: Path) -> list[Path]:
    selection = cache.read_json(cache.DEFAULT_SELECTION)
    lock = cache.read_json(ROOT / "tools/cache-import/dependencies.json")
    capture_lock = cache.read_json(ROOT / "tools/source-capture/dependencies.json")
    cache.verify_cache(selection, source / "cache-2695")
    inputs = [(source / "tooling" / row["name"], LOCAL / "tooling" / row["name"], row)
              for row in [lock["decoder"], *lock["libraries"]]]
    inputs += [(source / row["name"], LOCAL / "tooling" / row["name"], row)
               for row in selection["runtime"]["artifacts"]]
    resources = ROOT.parent / "m1-source-captures/.local/source-capture/tooling"
    inputs += [(resources / row["name"], LOCAL / "tooling" / row["name"], row)
               for row in capture_lock["additional_artifacts"]]
    libraries = [target for _, target, _ in inputs]
    inputs += [(source / "cache-2695" / row["name"], LOCAL / "cache" / row["name"], row)
               for row in cache.cache_files(selection)]
    for original_path, target, record in inputs:
        cache.checked_file(original_path, record)
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(original_path, target)
        cache.checked_file(target, record)
    cache.verify_cache(selection, LOCAL / "cache")
    return libraries


def native_errors(text: str) -> list[str]:
    errors = original.native_render_errors(text)
    lines = text.splitlines()
    for n, line in enumerate(lines):
        owner = re.search(r" in '([^']+)'", line)
        if "thrown in" in line and "method <" in line and owner and re.fullmatch(r"[a-z]{1,3}|rl\d+|client", owner[1]):
            entry = "\n".join(lines[max(0, n - 2):n + 2])
            if entry not in errors:
                errors.append(entry)
    return errors


def validate(directory: Path) -> dict:
    manifest = cache.read_json(directory / "manifest.json")
    names = [row["scenario"] for row in manifest["traces"]]
    if len(names) != len(set(names)) or any(name not in SCENARIOS for name in names):
        raise ValueError("Invalid or duplicate camera trace scenario")
    for name in ("source_selection", "cache_disk_manifest", "decoder_lock", "runtime_resource_lock"):
        row = manifest[name]
        cache.checked_file(ROOT / row["path"], row)
    for row in manifest["original_helpers"] + manifest["probe_sources"]:
        cache.checked_file(ROOT / row["path"], row)
    states = 0
    for entry in manifest["traces"]:
        trace_path = cache.checked_file(cache.within(ROOT, ROOT / entry["trace"]["path"]), entry["trace"])
        value = cache.read_json(trace_path)
        if value["scenario"] != entry["scenario"] or len(value["traces"]) != entry["states"]:
            raise ValueError("Native camera trace identity/count changed")
        if value["network_pump"] or value["original_jar_modified"] or value["authenticated_source_initial_state"]:
            raise ValueError("Invalid original-camera source qualification")
        log = cache.checked_file(ROOT / entry["native_runlog"]["path"], entry["native_runlog"])
        if entry["native_errors"] or native_errors(gzip.decompress(log.read_bytes()).decode()):
            raise ValueError("Original native camera errors must not be hidden")
        states += len(value["traces"])
    return {"result": "passed", "scenarios": len(names), "states": states,
            "source_account_used": False, "production_or_acceptance_claimed": False}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=ROOT.parent / "m1-runtime-inputs/.local/current-source")
    parser.add_argument("--scenario", choices=SCENARIOS, action="append")
    parser.add_argument("--all", action="store_true")
    parser.add_argument("--verify-only", action="store_true")
    parser.add_argument("--output", type=Path, default=OUT / "traces")
    parser.add_argument("--java-home")
    args = parser.parse_args()
    output = cache.within(ROOT, args.output)
    if not (output.is_relative_to(OUT) or output.is_relative_to(LOCAL)):
        raise ValueError("Camera evidence must not overwrite production or frozen references")
    if args.verify_only:
        print(json.dumps(validate(output), separators=(",", ":")))
        return 0
    scenarios = list(SCENARIOS) if args.all else args.scenario or ["initial"]
    if len(set(scenarios)) != len(scenarios):
        raise ValueError("Duplicate requested camera scenario")
    libraries = prepare(args.source)
    artifact_records = [cache.file_record(path) for path in libraries]
    classes, home, work = (LOCAL / name for name in ("classes", "java-home", "java-work"))
    for path in (classes, home, work, output, output / "logs"):
        path.mkdir(parents=True, exist_ok=True)
    java, javac = cache.java_tools(args.java_home)
    cp = os.pathsep.join(map(str, libraries))
    helpers = sorted((ROOT / "tools/source-capture").glob("*.java"))
    subprocess.run([javac, "--release", "17", "-cp", cp, "-d", str(classes), *map(str, helpers),
                    str(ROOT / "tools/source-layer-capture/LayerCapture.java"), str(TOOL / "CameraProbe.java")],
                   cwd=ROOT, check=True)
    traces = []
    for scenario in scenarios:
        command = [java, "-ea", "-Xmx3g", "-Djava.awt.headless=true",
                   "--add-opens=java.base/java.lang=ALL-UNNAMED", "-Xlog:exceptions=info",
                   "-Duser.home=" + str(home), "-Djava.io.tmpdir=" + str(work),
                   "-cp", str(classes) + os.pathsep + cp, "CameraProbe",
                   str(LOCAL / "cache"), str(work), scenario, str(output / (scenario + ".json"))]
        completed = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=240,
                                   env=dict(os.environ, TMPDIR=str(work), TMP=str(work), TEMP=str(work)))
        log = completed.stdout + completed.stderr
        log_path = output / "logs" / (scenario + ".log.gz")
        log_path.write_bytes(gzip.compress(log.encode(), mtime=0))
        if completed.returncode:
            raise ValueError(f"Native camera probe failed; {log_path}\n{log[-6500:]}")
        failures = native_errors(log)
        if failures:
            raise ValueError("Native errors occurred:\n" + "\n".join(failures[:5]))
        trace = output / (scenario + ".json")
        result = cache.read_json(trace)
        traces.append({"scenario": scenario, "states": len(result["traces"]),
                       "trace": cache.file_record(trace), "native_runlog": cache.file_record(log_path),
                       "jvm_arguments": command[1:command.index("-cp")],
                       "replay_arguments": command[command.index("CameraProbe") + 1:],
                       "native_errors": failures})
        if scenario == "script-init-scan":
            for name in ["camera-script-initializers.json", "short-script-inputs.json"]:
                shutil.copyfile(work / name, output / name)
        print(json.dumps({"scenario": scenario, "states": len(result["traces"]),
                          "trace": cache.file_record(trace)}, separators=(",", ":")))
    selection = cache.read_json(cache.DEFAULT_SELECTION)
    cache.verify_cache(selection, LOCAL / "cache")
    cache.verify_cache(selection, args.source / "cache-2695")
    for path, record in zip(libraries, artifact_records):
        cache.checked_file(path, record)
    manifest = {"schema_version": 1, "classification": "Controlled offline original normal-camera observations",
                "source_revision": 240, "source_cache_id": 2695, "traces": traces,
                "source_selection": cache.file_record(cache.DEFAULT_SELECTION),
                "cache_disk_manifest": cache.file_record(ROOT / "research/current-source/cache-files.json"),
                "decoder_lock": cache.file_record(ROOT / "tools/cache-import/dependencies.json"),
                "runtime_resource_lock": cache.file_record(ROOT / "tools/source-capture/dependencies.json"),
                "runtime_artifacts": artifact_records,
                "original_helpers": [cache.file_record(path) for path in helpers]
                    + [cache.file_record(ROOT / "tools/source-layer-capture/LayerCapture.java")],
                "probe_sources": [cache.file_record(path) for path in sorted(TOOL.glob("*")) if path.is_file()],
                "original_cache_and_jar_unchanged": True, "source_account_used": False,
                "network_pump": False, "source_spawn_camera_defaults_observed": False,
                "presentation_or_gameplay_acceptance": False}
    cache.write_json(output / "manifest.json", manifest)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"camera-source: {error}", file=sys.stderr)
        raise SystemExit(1)

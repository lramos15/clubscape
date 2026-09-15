#!/usr/bin/env python3
"""Supplementary original-runtime layer captures, without modifying the frozen reference pack."""
from __future__ import annotations

import argparse
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import gzip
import re

ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools/source-layer-capture"
FROZEN = ROOT / "tools/source-capture"
LOCAL = ROOT / ".local/source-layer-capture"
OUTPUT = ROOT / "assets/reference/osrs240/m1-dynamic"
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "tools/cache-import"))
import import_cache as cache

SPEC = importlib.util.spec_from_file_location("frozen_capture", FROZEN / "capture.py")
frozen = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(frozen)
import checks


def output_directory(path: Path) -> Path:
    result = cache.within(ROOT, path)
    if not (result.is_relative_to(OUTPUT) or result.is_relative_to(LOCAL)):
        raise ValueError("Dynamic captures must stay under m1-dynamic or the private source-layer scratch directory")
    return result


def case_selection(contract: dict, requested: list[str] | None) -> list[dict]:
    cases = contract["cases"]
    ids = [case["id"] for case in cases]
    if not cases or len(cases) > 12 or len(set(ids)) != len(ids) or any(not re.fullmatch(r"[a-z0-9-]+", name) for name in ids):
        raise ValueError("Invalid, duplicate, escaping or excessive dynamic case IDs")
    if requested and len(set(requested)) != len(requested):
        raise ValueError("Duplicate requested dynamic case")
    selected = [case for case in cases if requested is None or case["id"] in requested]
    if not selected or (requested and set(requested) != {case["id"] for case in selected}):
        raise ValueError("Unknown or empty dynamic case selection")
    return selected


def render_failures(log: str) -> list[str]:
    errors = frozen.native_render_errors(log)
    lines = log.splitlines()
    for index, line in enumerate(lines):
        owner = re.search(r" in '([^']+)'", line)
        if "thrown in" in line and "method <" in line and owner and re.fullmatch(r"[a-z]{1,3}|rl\d+", owner[1]):
            context = "\n".join(lines[max(0, index - 2):index + 2])
            if context not in errors:
                errors.append(context)
    return errors


def copy_verified(source: Path, target: Path, record: dict) -> Path:
    cache.checked_file(source, record)
    cache.within(ROOT, target)
    if not target.exists():
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    return cache.checked_file(target, record)


def prepare(source: Path, resource_source: Path) -> list[Path]:
    selection = cache.read_json(cache.DEFAULT_SELECTION)
    cache.verify_cache(selection, source / "cache-2695")
    lock = cache.read_json(ROOT / "tools/cache-import/dependencies.json")
    libraries = []
    for record in [lock["decoder"], *lock["libraries"]]:
        libraries.append(copy_verified(source / "tooling" / record["name"], LOCAL / "tooling" / record["name"], record))
    for record in selection["runtime"]["artifacts"]:
        libraries.append(copy_verified(source / record["name"], LOCAL / "tooling" / record["name"], record))
    for record in cache.read_json(FROZEN / "dependencies.json")["additional_artifacts"]:
        libraries.append(copy_verified(resource_source / record["name"], LOCAL / "tooling" / record["name"], record))
    for record in cache.cache_files(selection):
        copy_verified(source / "cache-2695" / record["name"], LOCAL / "cache" / record["name"], record)
    cache.verify_cache(selection, LOCAL / "cache")
    cache.verify_cache(selection, source / "cache-2695")
    return libraries


def validate(directory: Path) -> dict:
    manifest = cache.read_json(directory / "case-index.json")
    cache.checked_file(ROOT / manifest["case_contract"]["path"], manifest["case_contract"])
    for name in ("source_selection", "disk_file_manifest", "base_dependency_lock", "frozen_capture_dependency_lock",
                 "approved_reference_pack", "comparison_policy", "native_hud_input_contract"):
        cache.checked_file(ROOT / manifest[name]["path"], manifest[name])
    for record in manifest["source_capture_helpers"] + manifest["supplement_capture_sources"]:
        cache.checked_file(ROOT / record["path"], record)
    for log in manifest["original_runlogs"]:
        path = cache.checked_file(cache.within(ROOT, ROOT / log["path"]), log)
        if log["native_errors"] or render_failures(gzip.decompress(path.read_bytes()).decode()):
            raise ValueError("Original native runlog contains renderer/script failures")
    records = manifest["cases"]
    if not records or len(records) > 12:
        raise ValueError("Dynamic reference case count is outside the bounded request")
    seen = set()
    for record in records:
        if record["id"] in seen:
            raise ValueError("Duplicate dynamic reference ID")
        seen.add(record["id"])
        image = record["capture"]
        path = cache.within(directory, directory / image["path"])
        cache.checked_file(path, image)
        if frozen.png_dimensions(path.read_bytes()) != (1920, 1080):
            raise ValueError("Original dynamic capture dimensions changed")
        if image["authenticated_source_journey"] or not image["png_roundtrip_exact"]:
            raise ValueError("Invalid source classification or capture integrity")
        if image["nonbackground_pixels"] < 20000:
            raise ValueError("Blank original native layer capture")
        state_path = directory / (record["id"] + ".json")
        if cache.read_json(state_path) != record:
            raise ValueError("Per-case original control-state file differs from case index")
    result = checks.verify_all(directory, manifest)
    expected = cache.read_json(directory / "pair-metrics.json") if (directory / "pair-metrics.json").exists() else None
    if expected is not None and json.loads(json.dumps(result)) != expected:
        raise ValueError("Source pair integrity metrics changed")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=ROOT.parent / "m1-consumable-assets/.local/current-source")
    parser.add_argument("--resources", type=Path, default=ROOT.parent / "m1-native-hud/.local/source-capture/tooling")
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument("--case", action="append")
    parser.add_argument("--probe", action="store_true")
    parser.add_argument("--baseline-check", action="store_true", help="Replay only the existing frozen native inventory frame, not a new reference case")
    parser.add_argument("--verify-only", action="store_true")
    parser.add_argument("--java-home")
    args = parser.parse_args()
    output = output_directory(args.output)
    if args.verify_only:
        result = validate(output)
        print(json.dumps({"result": result["result"], "cases": result["cases"], "pairs": len(result["pairs"]),
                          "candidate_compared": False, "new_presentation_approval": False}, separators=(",", ":")))
        return 0
    cases = cache.read_json(TOOL / "cases.json")
    contract_path = TOOL / "cases.json"
    if args.baseline_check:
        if args.output == OUTPUT:
            output = LOCAL / "frozen-hud-replay"
        cases = {**cases, "cases": [{"id": "native-inventory", "camera": "lumbridge-castle-plaza", "layer": "baseline",
                                     "hide_roofs": False, "player_tile": [3222, 3218], "plane": 0}], "pairs": []}
        contract_path = LOCAL / "frozen-hud-replay-contract.json"
        cache.write_json(contract_path, cases)
    selected = case_selection(cases, args.case)
    libraries = prepare(args.source, args.resources)
    java, javac = cache.java_tools(args.java_home)
    classes, home, scratch = (LOCAL / name for name in ("classes", "java-home", "java-work"))
    for path in (classes, home, scratch, output):
        path.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(map(str, libraries))
    subprocess.run([javac, "--release", "17", "-cp", cp, "-d", str(classes),
                    *map(str, sorted(FROZEN.glob("*.java"))), *map(str, sorted(TOOL.glob("*.java")))],
                   check=True, cwd=ROOT)
    results = []
    logs = []
    for case in selected:
        command = [java, "-ea", "-Xmx3g", "-Djava.awt.headless=true",
                   "--add-opens=java.base/java.lang=ALL-UNNAMED", "-Xlog:exceptions=info",
                   "-Duser.home=" + str(home), "-Djava.io.tmpdir=" + str(scratch),
                   "-cp", str(classes) + os.pathsep + cp, "LayerCapture", str(LOCAL / "cache"),
                   str(output), str(contract_path), case["id"], "probe" if args.probe else "capture"]
        completed = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=180,
                                   env=dict(os.environ, TMPDIR=str(scratch), TEMP=str(scratch), TMP=str(scratch)))
        log = completed.stdout + completed.stderr
        log_path = LOCAL / (case["id"] + ".log")
        log_path.write_text(log)
        if completed.returncode:
            raise ValueError(f"Native {case['id']} failed ({completed.returncode}); {log_path}\n{log[-6500:]}")
        errors = render_failures(log)
        if errors:
            raise ValueError(f"Native errors in {case['id']}: " + "\n".join(errors[:4]))
        if not args.probe:
            archived = output / "logs" / (case["id"] + ".log.gz")
            archived.parent.mkdir(parents=True, exist_ok=True)
            archived.write_bytes(gzip.compress(log.encode(), mtime=0))
            logs.append({"case_id": case["id"], **cache.file_record(archived), "native_errors": errors,
                         "jvm_arguments": command[1:command.index("-cp")],
                         "replay_arguments": command[command.index("LayerCapture") + 1:]})
        print("\n".join(line for line in log.splitlines() if line.startswith(("LAYER_", "CAPTURE", "PROBE_"))))
        if not args.probe:
            results.append(cache.read_json(output / (case["id"] + ".json")))
    selection = cache.read_json(cache.DEFAULT_SELECTION)
    cache.verify_cache(selection, LOCAL / "cache")
    cache.verify_cache(selection, args.source / "cache-2695")
    for path in libraries:
        frozen.verify(path, next(record for record in
            [cache.read_json(ROOT / "tools/cache-import/dependencies.json")["decoder"],
             *cache.read_json(ROOT / "tools/cache-import/dependencies.json")["libraries"],
             *selection["runtime"]["artifacts"],
             *cache.read_json(FROZEN / "dependencies.json")["additional_artifacts"]]
            if record["name"] == path.name))
    if not args.probe:
        index = {"schema_version": 1, "reference_set_id": "osrs240.m1-dynamic.v1",
                 "classification": cases["classification"], "cases": results,
                 "case_contract": cache.file_record(contract_path),
                 "pairs": [pair for pair in cases["pairs"] if pair["before"] in {c["id"] for c in selected}
                           and pair["after"] in {c["id"] for c in selected}],
                 "source_capture_helpers": [cache.file_record(path) for path in sorted(FROZEN.glob("*.java"))],
                 "supplement_capture_sources": [cache.file_record(path) for path in sorted(TOOL.glob("*")) if path.is_file()],
                 "original_runlogs": logs,
                 "source_selection": cache.file_record(cache.DEFAULT_SELECTION),
                 "approved_reference_pack": cache.file_record(ROOT / "research/reference-pack/v1/manifest.json"),
                 "comparison_policy": cache.file_record(ROOT / "research/reference-pack/v1/comparison-policy.json"),
                 "native_hud_input_contract": cache.file_record(output / "hud-input-contract.json"),
                 "disk_file_manifest": cache.file_record(ROOT / "research/current-source/cache-files.json"),
                 "base_dependency_lock": cache.file_record(ROOT / "tools/cache-import/dependencies.json"),
                 "frozen_capture_dependency_lock": cache.file_record(FROZEN / "dependencies.json"),
                 "runtime_artifacts": [cache.file_record(path) for path in libraries],
                 "fixture_seed": cases["seed"],
                 "captured_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                 "source_cache_id": 2695, "cache_files_unchanged": True, "gpu_plugin_enabled": False,
                 "authenticated_source_gameplay": False, "candidate_compared": False,
                 "frozen_pack_modified": False, "new_presentation_approval": False}
        cache.write_json(output / "case-index.json", index)
        metrics = checks.verify_all(output, index)
        cache.write_json(output / "pair-metrics.json", metrics)
        if args.baseline_check:
            native = cache.read_json(ROOT / "assets/reference/osrs240/native-hud/captures.json")
            expected = next(record for record in native["captures"] if record["path"] == "hud/native-inventory.png")
            actual = results[0]["capture"]
            if (actual["sha256"], actual["pixel_argb32_be_sha256"]) != (expected["sha256"], expected["pixel_argb32_be_sha256"]):
                raise ValueError(f"Frozen native HUD baseline differs: actual={actual['sha256']} expected={expected['sha256']}")
            print("FROZEN_NATIVE_HUD_REPLAY exact original inventory pixels; native full-HUD zoom410")
        print(json.dumps({"result": metrics["result"], "cases": metrics["cases"], "pairs": len(metrics["pairs"]),
                          "candidate_compared": False}, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"source-layer-capture: {error}", file=sys.stderr)
        raise SystemExit(1)

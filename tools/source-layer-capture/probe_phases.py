#!/usr/bin/env python3
"""Read original scenery phases while replaying three unchanged source references.

Run: python3 tools/source-layer-capture/probe_phases.py
Verify published sidecars: add --verify-only.
Repeat independently: add --output .local/source-layer-phases/repeat.
No original image, case index, helper, animation target or RNG sequence is edited.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import zipfile

sys.dont_write_bytecode = True
import capture as original
import checks

cache = original.cache
ROOT = original.ROOT
TOOL = ROOT / "tools/source-layer-capture"
LOCAL = ROOT / ".local/source-layer-phases"
OUTPUT = original.OUTPUT / "phases"
EVIDENCE = ROOT / "research/source-layers/phases"
INDEX_SHA = "dde300e30ff909e539c283fa7053c673abc9428ce357894f0727be08da8ad557"
CASE_IDS = ("tutorial-door-closed", "tutorial-door-open", "tutorial-roofs-hidden")
TARGETS = {(24969, 3095, 3102, 0): (10, 2, 477),
           (196, 3096, 3105, 0): (4, 3, 481),
           (196, 3096, 3110, 0): (4, 1, 481)}
STAGES = ("after-original-maploader", "before-original-draw", "after-original-draw")


def selected_output(path: Path) -> Path:
    path = cache.within(ROOT, path)
    if not (path.is_relative_to(OUTPUT) or path.is_relative_to(LOCAL)):
        raise ValueError("Phase sidecars/replays must not overwrite original reference paths")
    return path


def old_index() -> dict:
    path = original.OUTPUT / "case-index.json"
    if cache.digest(path) != INDEX_SHA:
        raise ValueError("The admitted original case index changed")
    value = cache.read_json(path)
    original.validate(original.OUTPUT)
    return value


def signed_product(value: int, multiplier: int) -> int:
    result = (value * multiplier) & 0xffffffff
    return result - 0x100000000 if result >= 0x80000000 else result


def validate_case(sidecar: dict, source: dict) -> None:
    if sidecar["case_id"] not in CASE_IDS or sidecar["case_id"] != source["id"]:
        raise ValueError("Unadmitted original phase case")
    if sidecar["classification"] != "controlled offline original-client rendering":
        raise ValueError("Phase classification changed")
    if sidecar["candidate_images_or_frame_guesses_read"] or sidecar["animation_state_modified_by_probe"]:
        raise ValueError("Probe changed animation state or used candidate evidence")
    if sidecar["authenticated_source_gameplay"] or sidecar["new_presentation_approval"]:
        raise ValueError("Source observation must not imply gameplay/approval")
    expected_image = source["capture"]
    replay = sidecar["source_image_replay"]
    if (replay["png_sha256"], replay["native_argb32_be_sha256"]) != (
            expected_image["sha256"], expected_image["pixel_argb32_be_sha256"]):
        raise ValueError("Original source image/native pixels changed during observation")
    if not replay["per_case_native_state_unchanged"]:
        raise ValueError("Original per-case state changed")
    rows = sidecar["observations"]
    if len(rows) != len(TARGETS) * len(STAGES) or sidecar["observation_phases"] != list(STAGES):
        raise ValueError("Incomplete original pre/post observations")
    seen = {}
    for row in rows:
        key = (row["source_object_id"], *row["world_tile"])
        stage = row["observation_phase"]
        if key not in TARGETS or stage not in STAGES or (stage, key) in seen:
            raise ValueError("Duplicate, missing or foreign native placement observation")
        seen[(stage, key)] = row
        if (row["placement_type"], row["orientation"], row["sequence_id"]) != TARGETS[key]:
            raise ValueError("Source placement/sequence identity changed")
        if row["native_api_anim_cycle"] != -1 or "Unavailable" not in row["native_api_anim_cycle_availability"]:
            raise ValueError("Unavailable original getAnimCycle must not be invented")
        if row["source_cycle"] != signed_product(row["source_cycle_raw"], 1612595797):
            raise ValueError("Original source-cycle decode differs")
        if row["source_cycle"] != source["capture"]["settings"]["game_cycle"]:
            raise ValueError("Observed scene clock differs from the original replay state")
        if row["last_update_cycle"] != signed_product(row["last_update_cycle_raw"], 1618438999):
            raise ValueError("Original last-update cycle decode differs")
        if row["pending_source_cycle_delta"] != row["source_cycle"] - row["last_update_cycle"]:
            raise ValueError("Native pending cycle delta differs")
        if (row["source_cycle_decode_multiplier"], row["last_update_decode_multiplier"],
                row["rendering_controller"]) != (1612595797, 1618438999, "dy.ac"):
            raise ValueError("Native clock/controller binding metadata changed")
        active = row["active_controller"]
        if (active["frame_decode_multiplier"], active["frame_cycle_decode_multiplier"],
                active["sequence_decode_multiplier"]) != (292569817, -1399668821, 1684838611):
            raise ValueError("Verified native controller multipliers changed")
        if active["frame"] != signed_product(active["frame_raw"], 292569817) or active["frame"] != row["native_frame"]:
            raise ValueError("Original native frame getter/field differs")
        if active["frame_cycle"] != signed_product(active["frame_cycle_raw"], -1399668821):
            raise ValueError("Original within-frame cycle decode differs")
        if active["sequence_id"] != signed_product(active["sequence_raw"], 1684838611):
            raise ValueError("Original sequence decode differs")
        if active["sequence_id"] != row["sequence_id"] or active["sequence_api_id"] != row["sequence_id"]:
            raise ValueError("Original sequence getter and controller disagree")
        lengths = [7, 6, 6, 6, 1] if key[0] == 24969 else [7, 6, 6, 6, 6]
        if active["frame_lengths"] != lengths or not 0 <= active["frame"] < len(lengths):
            raise ValueError("Original sequence/frame identity is invalid")
        if not 0 <= active["frame_cycle"] <= lengths[active["frame"]]:
            raise ValueError("Native frame-cycle observation exceeds source frame duration")
    for target in TARGETS:
        before = seen[("before-original-draw", target)]
        after = seen[("after-original-draw", target)]
        if before["source_cycle"] != after["source_cycle"]:
            raise ValueError("Probe advanced the source scene clock")
        if after["last_update_cycle"] != after["source_cycle"]:
            raise ValueError("Required visible placement was not updated by the original draw")


def validate(directory: Path) -> dict:
    old = old_index()
    sources = {row["id"]: row for row in old["cases"]}
    index = cache.read_json(directory / "phase-index.json")
    if index["original_case_index"]["sha256"] != INDEX_SHA:
        raise ValueError("Phase index refers to a different original reference set")
    if [row["case_id"] for row in index["cases"]] != list(CASE_IDS):
        raise ValueError("Phase index changed the bounded case selection")
    for record in index["probe_sources"]:
        cache.checked_file(ROOT / record["path"], record)
    if {Path(record["path"]).name for record in index["probe_sources"]} != {
            "SceneryPhaseProbe.java", "probe_phases.py", "test_phases.py"}:
        raise ValueError("Incomplete exact probe source hashes")
    if index["native_class_sha256"].keys() != {"dy.class", "qr.class", "ou.class", "rd.class"}:
        raise ValueError("Missing original native class identities")
    for entry in index["cases"]:
        path = cache.checked_file(cache.within(ROOT, ROOT / entry["sidecar"]["path"]), entry["sidecar"])
        validate_case(cache.read_json(path), sources[entry["case_id"]])
        log_path = cache.checked_file(cache.within(ROOT, ROOT / entry["runlog"]["path"]), entry["runlog"])
        if original.render_failures(gzip.decompress(log_path.read_bytes()).decode()):
            raise ValueError("Original native errors occurred during phase observation")
    return {"result": "passed", "cases": 3, "placements_per_case": 3, "observations": 27,
            "original_png_and_native_pixel_hashes_unchanged": True,
            "candidate_evidence_used": False, "animation_targets_modified": False}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--source", type=Path, default=ROOT.parent / "m1-consumable-assets/.local/current-source")
    parser.add_argument("--resources", type=Path, default=ROOT.parent / "m1-native-hud/.local/source-capture/tooling")
    parser.add_argument("--output", type=Path, default=OUTPUT)
    parser.add_argument("--verify-only", action="store_true")
    parser.add_argument("--java-home")
    args = parser.parse_args()
    output = selected_output(args.output)
    if args.verify_only:
        print(json.dumps(validate(output), separators=(",", ":")))
        return 0
    baseline = old_index()
    sources = {row["id"]: row for row in baseline["cases"]}
    libraries = original.prepare(args.source, args.resources)
    java, javac = cache.java_tools(args.java_home)
    classes, home, scratch = (LOCAL / name for name in ("classes", "java-home", "java-work"))
    for path in (classes, home, scratch, output, EVIDENCE / "logs"):
        path.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(map(str, libraries))
    frozen_java = [ROOT / record["path"] for record in baseline["source_capture_helpers"]]
    subprocess.run([javac, "--release", "17", "-cp", cp, "-d", str(classes),
                    *map(str, frozen_java), str(TOOL / "LayerCapture.java"), str(TOOL / "SceneryPhaseProbe.java")],
                   cwd=ROOT, check=True)
    entries = []
    for case_id in CASE_IDS:
        replay = LOCAL / "replays" / case_id
        replay.mkdir(parents=True, exist_ok=True)
        observations = replay / "observations.json"
        command = [java, "-ea", "-Xmx3g", "-Djava.awt.headless=true",
                   "--add-opens=java.base/java.lang=ALL-UNNAMED", "-Xlog:exceptions=info",
                   "-Duser.home=" + str(home), "-Djava.io.tmpdir=" + str(scratch),
                   "-cp", str(classes) + os.pathsep + cp, "SceneryPhaseProbe",
                   str(original.LOCAL / "cache"), str(replay), str(TOOL / "cases.json"), case_id, str(observations)]
        run = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=180,
                             env=dict(os.environ, TMPDIR=str(scratch), TMP=str(scratch), TEMP=str(scratch)))
        text = run.stdout + run.stderr
        log = output / "logs" / (case_id + ".log.gz")
        log.parent.mkdir(parents=True, exist_ok=True)
        log.write_bytes(gzip.compress(text.encode(), mtime=0))
        if run.returncode or original.render_failures(text):
            raise ValueError(f"Native phase observation failed: {case_id}; {log}\n{text[-6000:]}")
        actual = cache.read_json(replay / (case_id + ".json"))
        expected = sources[case_id]
        if actual != expected:
            raise ValueError(f"Read-only probe changed original per-case native state: {case_id}")
        image = replay / actual["capture"]["path"]
        old_image = original.OUTPUT / expected["capture"]["path"]
        if image.read_bytes() != old_image.read_bytes():
            raise ValueError(f"Read-only probe changed original PNG bytes: {case_id}")
        checks.image_for(replay, actual)
        sidecar = cache.read_json(observations)
        sidecar["source_image_replay"] = {
            "original_path": str(old_image.relative_to(ROOT)),
            "png_sha256": cache.digest(image),
            "native_argb32_be_sha256": actual["capture"]["pixel_argb32_be_sha256"],
            "per_case_native_state_unchanged": True, "replay_png_published_as_new_case": False,
        }
        validate_case(sidecar, expected)
        sidecar_path = output / (case_id + ".phases.json")
        cache.write_json(sidecar_path, sidecar)
        entries.append({"case_id": case_id, "sidecar": cache.file_record(sidecar_path),
                        "runlog": cache.file_record(log), "original_image": sidecar["source_image_replay"],
                        "jvm_arguments": command[1:command.index("-cp")],
                        "replay_arguments": command[command.index("SceneryPhaseProbe") + 1:]})
        print("ORIGINAL_PHASE_REPLAY", case_id, sidecar["source_image_replay"]["png_sha256"])
    selection = cache.read_json(cache.DEFAULT_SELECTION)
    cache.verify_cache(selection, original.LOCAL / "cache")
    cache.verify_cache(selection, args.source / "cache-2695")
    old_index()
    artifacts = [cache.file_record(path) for path in libraries]
    for artifact, expected in zip(artifacts, baseline["runtime_artifacts"]):
        if (artifact["sha256"], artifact["size_bytes"]) != (expected["sha256"], expected["size_bytes"]):
            raise ValueError("Original runtime artifact changed")
    jar = next(path for path in libraries if path.name == "injected-client-1.12.38.jar")
    with zipfile.ZipFile(jar) as zipped:
        classes_hashes = {name: hashlib.sha256(zipped.read(name)).hexdigest()
                          for name in ("dy.class", "qr.class", "ou.class", "rd.class")}
    index = {
        "schema_version": 1, "classification": "controlled offline original-client rendering",
        "original_case_index": cache.file_record(original.OUTPUT / "case-index.json"),
        "cases": entries, "observation_phases": list(STAGES),
        "probe_sources": [cache.file_record(TOOL / name) for name in
                          ("SceneryPhaseProbe.java", "probe_phases.py", "test_phases.py") if (TOOL / name).exists()],
        "original_helper_sources": baseline["source_capture_helpers"] + baseline["supplement_capture_sources"],
        "runtime_artifacts": artifacts, "native_class_sha256": classes_hashes,
        "source_cache_disk_manifest": cache.file_record(ROOT / "research/current-source/cache-files.json"),
        "input_rng": {"seed": 0, "native_randomized_placement_phases": "Observed unchanged; not assigned or searched."},
        "candidate_images_or_guessed_phases_used": False, "old_reference_files_modified": False,
        "source_cache_unchanged": True, "authenticated_source_gameplay": False,
        "new_gameplay_presentation_or_performance_acceptance": False,
    }
    cache.write_json(output / "phase-index.json", index)
    print(json.dumps(validate(output), separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"source-phase-probe: {error}", file=sys.stderr)
        raise SystemExit(1)

#!/usr/bin/env python3
"""Fail-closed, offline original-client audio conversion for the selected M1 cache."""

import argparse
from array import array
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import urllib.request
import wave

from codec import Flac
import midi
from signal_analysis import measure
import source_map

ROOT = Path(__file__).resolve().parents[2]
WORK = Path(".local/audio-import")
TOOLS = Path("tools/audio-import")
RESEARCH = Path("research/audio-source")
ASSETS = Path("assets/source/osrs/audio-runtime")
MANIFEST = Path("assets/manifests/osrs/audio-runtime.json")


def json_bytes(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def write_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(json_bytes(value))


def compressed_json(path):
    return json.loads(gzip.decompress(Path(path).read_bytes()))


def file_record(path, relative=None):
    path = Path(path)
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    return {"path": str(relative if relative is not None else path), "size_bytes": path.stat().st_size, "sha256": digest}


def checked_path(root, relative):
    relative = str(relative)
    path = PurePosixPath(relative)
    if path.is_absolute() or not path.parts or "\\" in relative or any(part in ("", ".", "..") for part in path.parts):
        raise ValueError(f"Unsafe source/output path: {relative}")
    result = Path(root).joinpath(*path.parts)
    if not result.resolve().is_relative_to(Path(root).resolve()):
        raise ValueError(f"Source/output path escapes its declared root: {relative}")
    return result


def verify(path, expected):
    path = Path(path)
    if not path.is_file():
        raise ValueError(f"Missing required input: {path}")
    actual = file_record(path)
    if actual["sha256"] != expected["sha256"] or (
        "size_bytes" in expected and actual["size_bytes"] != expected["size_bytes"]
    ):
        raise ValueError(f"Input integrity mismatch: {path}; expected {expected['sha256']}, got {actual['sha256']}")
    return actual


def copy_verified(source, target, expected):
    verify(source, expected)
    target = Path(target)
    if target.exists():
        verify(target, expected)
        return
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)
    verify(target, expected)


def locks():
    lock = json.loads((TOOLS / "dependencies.json").read_text())
    for key in ("runtime_selection", "extraction_inventory", "supplementary_decoder_lock"):
        record = lock[key]
        verify(record["path"], record)
    return lock


def artifact_records(lock):
    extra = json.loads(Path(lock["supplementary_decoder_lock"]["path"]).read_text())
    records = {}
    for record in lock["artifacts"] + [extra["decoder"]] + extra["libraries"]:
        name = record["name"]
        if name in records and record["sha256"] != records[name]["sha256"]:
            raise ValueError(f"Conflicting pinned dependency: {name}")
        records[name] = record
    return [records[name] for name in sorted(records)]


def download_artifact(record, target):
    if not record.get("url", "").startswith("https://"):
        raise ValueError(f"Pinned local decoder is unavailable; reproduce tools/cache-import first: {record['name']}")
    request = urllib.request.Request(record["url"], headers={"User-Agent": "ClubScape-original-audio-import/1"})
    with urllib.request.urlopen(request, timeout=90) as response:
        raw = response.read(record["size_bytes"] + 1)
    if len(raw) != record["size_bytes"] or hashlib.sha256(raw).hexdigest() != record["sha256"]:
        raise ValueError(f"Downloaded artifact failed integrity: {record['name']}")
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(raw)


def java_options():
    for name in ("java-home", "java-work", "classes"):
        (WORK / name).mkdir(parents=True, exist_ok=True)
    return [
        "-XX:-UsePerfData", "-XX:ActiveProcessorCount=2", "-Xmx1g",
        f"-Djava.io.tmpdir={(WORK / 'java-work').resolve()}",
        f"-Duser.home={(WORK / 'java-home').resolve()}", "-Djava.awt.headless=true",
    ]


def classpath(lock):
    jars = []
    for record in artifact_records(lock):
        path = WORK / "tooling" / record["name"]
        verify(path, record)
        jars.append(str(path))
    return os.pathsep.join([str(WORK / "classes"), *jars])


def run_java(lock, name, *args):
    subprocess.run(["java", *java_options(), "-cp", classpath(lock), name, *map(str, args)], check=True, timeout=300)


def prepare(args):
    lock = locks()
    WORK.mkdir(parents=True, exist_ok=True)
    for record in artifact_records(lock):
        target = WORK / "tooling" / record["name"]
        if target.exists():
            verify(target, record)
            continue
        candidates = [args.artifacts / record["name"], args.artifacts / "tooling" / record["name"]]
        source = next((path for path in candidates if path.exists()), None)
        if source is not None:
            copy_verified(source, target, record)
        elif args.fetch_artifacts:
            download_artifact(record, target)
        else:
            raise ValueError(f"Missing pinned dependency {record['name']}; supply --artifacts or retry with --fetch-artifacts")
    version = subprocess.run(["java", *java_options(), "-version"], capture_output=True, text=True, check=True).stderr
    if 'version "17.' not in version:
        raise ValueError("The source adapter is locked to JDK17")
    subprocess.run(
        ["javac", *["-J" + value for value in java_options()], "-proc:none", "--release", "17",
         "-cp", classpath(lock), "-d", str(WORK / "classes"),
         str(TOOLS / "SourceAudio.java"), str(TOOLS / "CacheAudioInputs.java")],
        check=True, timeout=120,
    )
    disk_records = json.loads(Path("research/current-source/cache-files.json").read_text())
    private_cache = WORK / "cache-2695"
    for record in disk_records:
        target = private_cache / record["name"]
        if target.exists():
            verify(target, record)
        else:
            copy_verified(checked_path(args.cache, record["name"]), target, record)
    request_record = file_record(RESEARCH / "request.json")
    supplement_root = WORK / "supplements" / request_record["sha256"][:16]
    run_java(lock, "CacheAudioInputs", private_cache, RESEARCH / "request.json", supplement_root)
    # Store never touches another worktree; recheck its private disk copy after the read.
    for record in disk_records:
        verify(private_cache / record["name"], record)
    supplement = json.loads((supplement_root / "supplement.json").read_text())
    if (RESEARCH / "extra-inputs.json.gz").exists() and compressed_json(RESEARCH / "extra-inputs.json.gz") != supplement:
        raise ValueError("Supplementary source extraction differs from the committed audio input record")
    bundle = compressed_json(lock["extraction_inventory"]["path"])
    catalog = {}
    inputs = WORK / "inputs"
    for record in bundle["records"]:
        if record["kind"] not in ("sound", "music", "music-sample", "music-patch"):
            continue
        for output in record["outputs"]:
            relative = output["path"]
            copy_verified(checked_path(args.extracted, relative), checked_path(inputs, relative), output)
            catalog[relative] = output
    for output in supplement["outputs"]:
        relative = output["path"]
        if relative in catalog and catalog[relative]["sha256"] != output["sha256"]:
            raise ValueError(f"Conflicting original extracted input: {relative}")
        copy_verified(checked_path(supplement_root, relative), checked_path(inputs, relative), output)
        catalog[relative] = output
    write_json(WORK / "prepared.json", {
        "schema_version": 1, "request": request_record, "java_version": version.strip(),
        "inputs": catalog, "supplement_root": str(supplement_root),
        "dependencies": file_record(TOOLS / "dependencies.json"),
    })
    run_java(lock, "CacheAudioInputs", "check-layouts", private_cache, WORK / "prepared.json", WORK)
    layout = json.loads((WORK / "layout-check.json").read_text())
    if any(files != [0] for files in layout["group_file_ids"].values()):
        raise ValueError("A selected sound has additional/digital source files; do not silently substitute legacy file0")
    verify_inputs()
    print(f"Prepared {len(catalog)} checksum-verified source inputs; original cache unchanged.")


def verify_inputs():
    prepared = json.loads((WORK / "prepared.json").read_text())
    verify(RESEARCH / "request.json", prepared["request"])
    verify(TOOLS / "dependencies.json", prepared["dependencies"])
    expected = prepared["inputs"]
    root = WORK / "inputs"
    actual = {path.relative_to(root).as_posix() for path in root.rglob("*") if path.is_file()}
    if actual != set(expected):
        raise ValueError(f"Unexpected/missing staged inputs: {sorted(actual.symmetric_difference(expected))[:8]}")
    for relative, record in expected.items():
        verify(checked_path(root, relative), record)
    return prepared


def jobs_from_inputs(request, inputs):
    tracks = []
    for index, name in ((6, "music"), (11, "jingles")):
        for record in request[name]:
            group = record["id"]
            info = midi.describe((inputs / f"audio/{name}/{group}.mid").read_bytes())
            if not info["note_on_count"] or not 0.05 <= info["duration_seconds"] <= 600:
                raise ValueError("Empty or unbounded original source track")
            tracks.append({"index": index, "group": group, "expected_engine_end_frame": info["expected_engine_end_frame"]})
    sounds = sorted(int(path.name) for path in (inputs / "raw/4").iterdir() if path.is_dir())
    return {"tracks": tracks, "sound_ids": sounds}


def sound_definition(inputs, group):
    extra = inputs / f"definitions/sound/{group}.json"
    if extra.exists():
        return json.loads(extra.read_text())
    return compressed_json(inputs / f"audio/sounds/{group}.json.gz")


def verify_reads(report, prepared):
    reads = report["reads"] if isinstance(report["reads"], list) else [report["reads"]]
    for part in reads:
        for record in part.values():
            relative = f"raw/{record['index']}/{record['group']}/{record['file']}.bin"
            expected = prepared["inputs"].get(relative)
            if expected is None or expected["sha256"] != record["sha256"] or expected["size_bytes"] != record["size_bytes"]:
                raise ValueError(f"Original runtime consumed an unlocked input: {relative}")


def reference_pcm_float_hashes(samples, channels):
    scale = 32768 if samples.typecode == "h" else 2147483648
    return [hashlib.sha256(array("f", (value / scale for value in samples[channel::channels])).tobytes()).hexdigest()
            for channel in range(channels)]


def convert(_args):
    lock = locks()
    prepared = verify_inputs()
    request = json.loads((RESEARCH / "request.json").read_text())
    inputs = WORK / "inputs"
    jobs = jobs_from_inputs(request, inputs)
    write_json(WORK / "jobs.json", jobs)
    native = WORK / "native"
    run_java(lock, "SourceAudio", inputs, WORK / "jobs.json", native)
    codec = Flac()
    files = []
    silences = []
    evidence = {}
    identities = {(6, row["id"]): row for row in request["music"]}
    identities.update({(11, row["id"]): row for row in request["jingles"]})
    specs = [(row["index"], row["group"]) for row in jobs["tracks"]] + [(4, source_id) for source_id in jobs["sound_ids"]]
    for index, group in specs:
        kind = {4: "sfx", 6: "music", 11: "jingle"}[index]
        key = f"{kind}-{group}"
        report = json.loads((native / f"{key}.json").read_text())
        if report["index"] != index or report["group"] != group:
            raise ValueError("Original source decoder returned a different identity")
        verify_reads(report, prepared)
        source_relative = f"raw/{index}/{group}/0.bin"
        source = prepared["inputs"][source_relative]
        if index != 4 and report["source_sha256"] != source["sha256"]:
            raise ValueError("Original track source hash mismatch")
        if report["sample_rate"] != 22050 or not 0 < report["frames"] <= 22050 * 601:
            raise ValueError("Original audio has invalid sample count/rate")
        if index == 4:
            bounds = source_map.effect_bounds(sound_definition(inputs, group))
            if report["frames"] != bounds["expected_frames"]:
                raise ValueError(f"Original SFX duration differs from independent source definitions: {group}")
            if report["loop_start_frame"] != bounds["loop_start_ms"] * 22050 // 1000 or report["loop_end_frame"] != bounds["loop_end_ms"] * 22050 // 1000:
                raise ValueError(f"Original SFX loop boundaries differ: {group}")
            loop = {key: report[key] for key in ("loop_start_frame", "loop_end_frame", "ping_pong_loop")}
            if not 0 <= loop["loop_start_frame"] <= report["frames"] or not 0 <= loop["loop_end_frame"] <= report["frames"]:
                raise ValueError("Original SFX loop falls outside its actual PCM")
            loop["event_repeat_count"] = None
            report["independent_definition_bounds"] = bounds
            with wave.open(str(native / f"{key}.wav"), "rb") as source_wave:
                pcm = source_wave.readframes(source_wave.getnframes())
                if source_wave.getnframes() != report["frames"] or source_wave.getsampwidth() != 2 or source_wave.getnchannels() != 1:
                    raise ValueError("Original SFX PCM format mismatch")
            if hashlib.sha256(pcm).hexdigest() != report["pcm_s16le_sha256"]:
                raise ValueError("Original SFX PCM changed before packaging")
            if not any(pcm):
                if group not in request["verified_silent_sound_ids"]:
                    raise ValueError(f"Undocumented silent original sound: {group}")
                silences.append({
                    "asset_id": f"asset.source.osrs.cache2695.audio-runtime.sfx.{group}",
                    "source_index": 4, "source_group": group, "source_file": 0,
                    "source_input": source, "frames": report["frames"], "sample_rate": report["sample_rate"],
                    "duration_seconds": report["frames"] / report["sample_rate"],
                    "pcm_s16le_sha256": report["pcm_s16le_sha256"],
                    "kind": "verified-source-silence", "playable_output": None,
                    "note": "Actual original synth output is silence; retain the source event probability without substituting a sound or renormalizing other choices.",
                    "provenance_record": key,
                })
                evidence[key] = report
                continue
            if group in request["verified_silent_sound_ids"]:
                raise ValueError("Previously verified source silence unexpectedly became audible")
        else:
            published = (inputs / f"audio/{'music' if index == 6 else 'jingles'}/{group}.mid").read_bytes()
            report["midi_event_comparison"] = midi.compare_runtime(published, (native / f"{key}.runtime.mid").read_bytes())
            report["midi"] = midi.describe(published)
            if report["preview"] or report["device_clipped_samples"] or not report["original_device_quantization_verified"] or not report["original_player_scheduling_used"]:
                raise ValueError(f"Preview, clipped or unverified original music output: {key}")
            if report["engine_end_frame"] != report["midi"]["expected_engine_end_frame"] or report["frames"] != report["engine_end_frame"] + 22050:
                raise ValueError("Original full-track duration/release mismatch")
            loop = {
                "export_native_loop": False,
                "source_start_tick": 0,
                "source_end_tick": report["midi"]["end_tick"],
                "source_engine_end_frame": report["engine_end_frame"],
                "release_tail_frames": 22050,
                "source_session_repeat_policy": None,
                "note": "One original native pass plus1s release. Not a seamless-loop asset; do not loop the padded file or assume a source music-player repeat/transition policy.",
            }
        relative = ASSETS / kind / f"{group}.flac"
        staged = WORK / "pack" / kind / f"{group}.flac"
        samples, info, encoding = codec.encode(native / f"{key}.wav", staged, effect=index == 4)
        if encoding["source_pcm_s16le_sha256"] != report["pcm_s16le_sha256"]:
            raise ValueError("Packaging changed the original source PCM")
        signal = measure(samples, info.samplerate, info.channels)
        if signal["frames"] != report["frames"]:
            raise ValueError("Measured output frame count mismatch")
        if signal["nonzero_fraction"] <= 0 or signal["distinct_amplitudes"] < 16:
            raise ValueError("Source conversion is silent or has implausibly little signal detail")
        record = {
            "asset_id": f"asset.source.osrs.cache2695.audio-runtime.{kind}.{group}",
            "kind": kind, "source_index": index, "source_group": group, "source_file": 0,
            "name": identities.get((index, group), {}).get("name", f"Original sound {group}"),
            "source_input": source,
            **file_record(staged, relative),
            "encoding": encoding, "signal": signal, "loop": loop,
            "reference_pcm_float32_channel_sha256": reference_pcm_float_hashes(samples, info.channels),
            "source_decode_verified": True, "live_trigger_and_presentation_accepted": False,
            "provenance_record": key,
        }
        files.append(record)
        evidence[key] = report
    mapping = source_map.build(inputs, request, set(jobs["sound_ids"]))
    supplement = json.loads((Path(prepared["supplement_root"]) / "supplement.json").read_text())
    layout = json.loads((WORK / "layout-check.json").read_text())
    if set(map(int, layout["group_file_ids"])) != set(jobs["sound_ids"]) or any(value != [0] for value in layout["group_file_ids"].values()):
        raise ValueError("Original source file-layout verification is incomplete")
    # Publish only after every dependency, original PCM relationship and measured output passed.
    for record in files:
        destination = Path(record["path"])
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(WORK / "pack" / record["kind"] / f"{record['source_group']}.flac", destination)
        verify(destination, record)
    RESEARCH.mkdir(parents=True, exist_ok=True)
    for name, value in (("conversion-evidence.json.gz", evidence), ("extra-inputs.json.gz", supplement)):
        (RESEARCH / name).write_bytes(gzip.compress(json_bytes(value), compresslevel=9, mtime=0))
    write_json(RESEARCH / "source-map.json", mapping)
    write_json(RESEARCH / "layout-check.json", layout)
    counts = {kind: sum(record["kind"] == kind for record in files) for kind in ("music", "jingle", "sfx")}
    tool_files = [TOOLS / name for name in (
        "audio_import.py", "SourceAudio.java", "CacheAudioInputs.java", "codec.py",
        "midi.py", "signal_analysis.py", "source_map.py", "dependencies.json",
    )]
    manifest = {
        "schema_version": 1,
        "status": "Original source audio decoded and losslessly packaged; source binding gaps and owner/browser acceptance remain explicit.",
        "source_selection": request["source_selection"], "cache_id": 2695, "game_build": 240,
        "runtime": "Original injected1.12.38; current16-bit music and SFX algorithms; no legacy soundfont",
        "dependency_lock": file_record(TOOLS / "dependencies.json"),
        "request": file_record(RESEARCH / "request.json"),
        "java_environment": prepared["java_version"],
        "python_environment": sys.version.split()[0],
        "conversion_tools": [file_record(path) for path in tool_files],
        "source_bundle": lock["extraction_inventory"],
        "supplementary_inputs": file_record(RESEARCH / "extra-inputs.json.gz"),
        "conversion_evidence": file_record(RESEARCH / "conversion-evidence.json.gz"),
        "source_map": file_record(RESEARCH / "source-map.json"),
        "source_file_layouts": file_record(RESEARCH / "layout-check.json"),
        "identity_references": file_record(RESEARCH / "references.json"),
        "settings": {
            "sample_rate": 22050, "native_device_block_frames": 512,
            "music_synth_master_volume": 128, "music_channels": 2,
            "music_release_tail_seconds": 1,
            "sfx_source_bit_depth": 16, "sfx_container_bit_depth": 24, "sfx_export_gain": 0.5,
            "sfx_gain_note": "Exact, reversible power-of-two attenuation, with all16 source bits retained. Original synth saturation is counted and preserved; no new clipping, limiting, resampling, generic samples or normalization.",
            "volume_calibrated_to_source_session": False,
        },
        "counts": counts,
        "verified_silent_source_effects": len(silences),
        "total_frames": sum(record["signal"]["frames"] for record in files),
        "total_duration_seconds": round(sum(record["signal"]["duration_seconds"] for record in files), 9),
        "total_size_bytes": sum(record["size_bytes"] for record in files),
        "source_recordings": False, "owner_reference_pack_approved": False,
        "runtime_browser_audio_accepted": False, "clubscape_presentation_implemented": False,
        "remaining_bindings": mapping["remaining_observations"],
        "notices": ["assets/source/osrs/audio-runtime/NOTICE.txt", "tools/audio-import/THIRD_PARTY_NOTICES.txt"],
        "assets": files,
        "source_silences": silences,
    }
    write_json(MANIFEST, manifest)
    validate(None)
    print(f"Published {counts}; {manifest['total_duration_seconds']:.3f}s; {manifest['total_size_bytes']} bytes.")


def validate(_args):
    lock = locks()
    manifest = json.loads(MANIFEST.read_text())
    if manifest["owner_reference_pack_approved"] or manifest["runtime_browser_audio_accepted"] or manifest["clubscape_presentation_implemented"]:
        raise ValueError("Source preparation must not assert presentation acceptance")
    for key in ("dependency_lock", "request", "supplementary_inputs", "conversion_evidence", "source_map", "source_file_layouts", "identity_references"):
        record = manifest[key]
        verify(record["path"], record)
    for record in manifest["conversion_tools"]:
        verify(record["path"], record)
    codec = Flac()
    evidence = compressed_json(manifest["conversion_evidence"]["path"])
    seen = set()
    for record in manifest["assets"]:
        path = checked_path(ROOT, record["path"])
        if not path.is_relative_to((ROOT / ASSETS).resolve()) or record["asset_id"] in seen:
            raise ValueError("Invalid/duplicate audio output identity")
        seen.add(record["asset_id"])
        verify(path, record)
        samples, info = codec.read(path)
        encoding = record["encoding"]
        if hashlib.sha256(samples.tobytes()).hexdigest() != encoding["decoded_pcm_sha256"]:
            raise ValueError("Actual audio decode differs from the recorded PCM")
        recovered = array("h", (value >> 15 for value in samples)) if record["kind"] == "sfx" else samples
        source_digest = hashlib.sha256(recovered.tobytes()).hexdigest()
        if source_digest != encoding["source_pcm_s16le_sha256"] or source_digest != evidence[record["provenance_record"]]["pcm_s16le_sha256"]:
            raise ValueError("Original-source PCM relationship does not hold")
        measured = measure(samples, info.samplerate, info.channels)
        if measured != record["signal"]:
            raise ValueError("Actual signal measurements differ from the manifest")
        if reference_pcm_float_hashes(samples, info.channels) != record["reference_pcm_float32_channel_sha256"]:
            raise ValueError("Reference float representation differs from the verified PCM")
    counts = {kind: sum(record["kind"] == kind for record in manifest["assets"]) for kind in ("music", "jingle", "sfx")}
    if counts != manifest["counts"] or counts["music"] != 5:
        raise ValueError("Audio asset count mismatch")
    for record in manifest["source_silences"]:
        expected = hashlib.sha256(bytes(record["frames"] * 2)).hexdigest()
        if record["pcm_s16le_sha256"] != expected or evidence[record["provenance_record"]]["pcm_s16le_sha256"] != expected:
            raise ValueError("Recorded source silence is not the original all-zero PCM")
    if len(manifest["source_silences"]) != manifest["verified_silent_source_effects"]:
        raise ValueError("Source silence count mismatch")
    write_json(RESEARCH / "validation.json", {
        "schema_version": 1, "manifest": file_record(MANIFEST),
        "validation_command": "python3 tools/audio-import/audio_import.py validate",
        "result": "passed", "actual_decoded_outputs": counts,
        "all_output_and_decoded_pcm_hashes_verified": True,
        "all_signals_measured_nonempty_and_below_full_scale": True,
        "all_original_pcm_relationships_exact": True,
        "verified_silent_source_effects_not_counted_as_playable": manifest["verified_silent_source_effects"],
        "total_duration_seconds": manifest["total_duration_seconds"],
        "source_recordings_or_human_perceptual_comparison": False,
        "live_browser_playback_or_source_trigger_acceptance": False,
        "owner_reference_pack_approval": False,
    })
    print(f"Validated {sum(counts.values())} actual decoded signals and exact original-PCM relationships.")


def main():
    if Path.cwd().resolve() != ROOT:
        raise ValueError(f"Run this tool from its worktree root: {ROOT}")
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    preparation = commands.add_parser("prepare")
    preparation.add_argument("--extracted", type=Path, default=Path(".local/current-source/extracted"))
    preparation.add_argument("--artifacts", type=Path, default=Path(".local/current-source"))
    preparation.add_argument("--cache", type=Path, default=Path(".local/current-source/cache-2695"))
    preparation.add_argument("--fetch-artifacts", action="store_true")
    commands.add_parser("convert")
    commands.add_parser("validate")
    commands.add_parser("verify-inputs")
    args = parser.parse_args()
    {"prepare": prepare, "convert": convert, "validate": validate,
     "verify-inputs": lambda _: print(f"Verified {len(verify_inputs()['inputs'])} staged inputs.")}[args.command](args)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"audio-import: {error}", file=sys.stderr)
        sys.exit(2)

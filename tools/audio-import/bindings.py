#!/usr/bin/env python3
"""Reproduce the bounded M1 cue audit and necessary incremental audio correction."""

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace

import audio_import as audio

WORK = Path(".local/audio-bindings")


def java(reuse, class_name, *args):
    jars = [str(reuse / "tooling" / record["name"]) for record in audio.artifact_records(audio.locks())]
    options = [
        "-XX:-UsePerfData", "-XX:ActiveProcessorCount=2", "-Xmx1g", "-Djava.awt.headless=true",
        f"-Djava.io.tmpdir={(WORK / 'work').resolve()}",
        f"-Duser.home={(WORK / 'java-home').resolve()}",
    ]
    cp = os.pathsep.join([str(WORK / "classes"), *jars])
    subprocess.run(["java", *options, "-cp", cp, class_name, *map(str, args)], check=True, timeout=180)


def prepare_tools(reuse):
    for name in ("work", "java-home", "classes"):
        (WORK / name).mkdir(parents=True, exist_ok=True)
    jars = []
    for record in audio.artifact_records(audio.locks()):
        path = reuse / "tooling" / record["name"]
        audio.verify(path, record)
        jars.append(str(path))
    subprocess.run([
        "javac", "-J-XX:-UsePerfData", f"-J-Djava.io.tmpdir={(WORK / 'work').resolve()}",
        f"-J-Duser.home={(WORK / 'java-home').resolve()}", "--release", "17", "-proc:none",
        "-cp", os.pathsep.join(jars), "-d", str(WORK / "classes"),
        *[str(audio.TOOLS / name) for name in
          ("BindingCache.java", "BindingExtract.java", "SourceAudio.java", "NativeBindingProbe.java")],
    ], check=True, timeout=120)


def verify_cache(cache):
    records = json.loads(Path("research/current-source/cache-files.json").read_text())
    for record in records:
        audio.verify(cache / record["name"], record)
    return records


def audit(args):
    prepare_tools(args.reuse)
    verify_cache(args.cache)
    java(args.reuse, "BindingExtract", args.cache, audio.RESEARCH / "bindings-request.json", WORK / "extracted")
    java(args.reuse, "NativeBindingProbe", args.reuse / "inputs", WORK / "native-queue-evidence.json")
    verify_cache(args.cache)
    print("Native cue metadata, full script audit and queue probes complete; original cache unchanged.")


def publish_audio(args):
    lock = audio.locks()
    previous = json.loads(audio.MANIFEST.read_text())
    old_effects = {record["asset_id"]: record["sha256"] for record in previous["assets"] if record["kind"] == "sfx"}
    audio.write_json(WORK / "previous-manifest.json", previous)
    audio.WORK.mkdir(parents=True, exist_ok=True)
    for record in audio.artifact_records(lock):
        original = args.reuse / "tooling" / record["name"]
        audio.verify(original, record)
        target = audio.WORK / "tooling" / record["name"]
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.exists():
            target.symlink_to(original.resolve())
        audio.verify(target, record)
    old_inputs = json.loads((args.reuse / "prepared.json").read_text())["inputs"]
    for relative, record in old_inputs.items():
        source = audio.checked_path(args.reuse / "inputs", relative)
        target = audio.checked_path(audio.WORK / "inputs", relative)
        audio.verify(source, record)
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.exists():
            os.link(source, target)
        audio.verify(target, record)
    # A private disk copy is necessary for the unchanged standard supplemental decoder's write-open Store.
    for record in verify_cache(args.cache):
        audio.copy_verified(args.cache / record["name"], audio.WORK / "cache-2695" / record["name"], record)
    subprocess.run([
        "javac", *["-J" + option for option in audio.java_options()], "--release", "17", "-proc:none",
        "-cp", audio.classpath(lock), "-d", str(audio.WORK / "classes"),
        str(audio.TOOLS / "CacheAudioInputs.java"), str(audio.TOOLS / "SourceAudio.java"),
    ], check=True, timeout=120)
    supplement_root = WORK / "updated-supplement"
    audio.run_java(lock, "CacheAudioInputs", audio.WORK / "cache-2695",
                   audio.RESEARCH / "request.json", supplement_root)
    supplement = json.loads((supplement_root / "supplement.json").read_text())
    old_supplement = audio.compressed_json(audio.RESEARCH / "extra-inputs.json.gz")
    expanded = {record["path"]: record for record in supplement["outputs"]}
    for record in old_supplement["outputs"]:
        if record["path"] not in expanded or expanded[record["path"]]["sha256"] != record["sha256"]:
            raise ValueError("Incremental binding inputs changed a previously verified source payload")
    (audio.RESEARCH / "extra-inputs.json.gz").write_bytes(
        gzip.compress(audio.json_bytes(supplement), compresslevel=9, mtime=0))
    audio.prepare(SimpleNamespace(
        extracted=args.extracted, artifacts=args.reuse / "tooling", cache=args.cache, fetch_artifacts=False))
    audio.convert(SimpleNamespace(reuse_existing_sfx=True))
    updated = json.loads(audio.MANIFEST.read_text())
    new_effects = {record["asset_id"]: record["sha256"] for record in updated["assets"] if record["kind"] == "sfx"}
    if any(new_effects.get(key) != digest for key, digest in old_effects.items()):
        raise ValueError("An existing SFX changed during the musical-only correction")
    if updated["source_silences"] != previous["source_silences"]:
        raise ValueError("The original weighted silence changed")
    changed = [
        row["asset_id"] for row in updated["assets"] if row["kind"] != "sfx"
        and next(old["sha256"] for old in previous["assets"] if old["asset_id"] == row["asset_id"]) != row["sha256"]
    ]
    audio.write_json(audio.RESEARCH / "musical-startup-correction.json", {
        "schema_version": 1,
        "reason": "Original native dg.ay initializes every MidiPcmStream with channel9 bank128; the earlier renderer omitted it.",
        "native_call": "nu.ap(9,128,-27396)",
        "musical_outputs_checked": 35, "changed_musical_asset_ids": changed,
        "existing_sfx_byte_hashes_preserved": len(old_effects),
        "new_sfx_asset_ids": sorted(set(new_effects) - set(old_effects)),
        "silent_source_entries_preserved": True,
        "previous_manifest_sha256": hashlib.sha256(audio.json_bytes(previous)).hexdigest(),
        "updated_manifest_sha256": audio.file_record(audio.MANIFEST)["sha256"],
        "reference_pack_requires_parent_refresh": True,
        "presentation_accepted": False,
    })
    print(f"Corrected {len(changed)} musical payloads; preserved all {len(old_effects)} existing SFX.")


def main():
    if Path.cwd().resolve() != audio.ROOT:
        raise ValueError("Run from this worktree's root")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("audit", "publish-audio"))
    parser.add_argument("--reuse", type=Path, default=Path("../m1-audio-inputs/.local/audio-import"))
    parser.add_argument("--cache", type=Path, default=Path("../m1-runtime-inputs/.local/current-source/cache-2695"))
    parser.add_argument("--extracted", type=Path, default=Path("../m1-runtime-inputs/.local/current-source/extracted"))
    args = parser.parse_args()
    {"audit": audit, "publish-audio": publish_audio}[args.command](args)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as exception:
        print(f"audio-bindings: {exception}", file=sys.stderr)
        sys.exit(2)

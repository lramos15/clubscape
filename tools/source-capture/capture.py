#!/usr/bin/env python3
"""Run isolated original-runtime rendering fixtures; never a replacement renderer."""
from __future__ import annotations

import argparse
import collections
import datetime
import errno
import hashlib
import json
import os
from pathlib import Path
import shutil
import re
import struct
import subprocess
import sys
import urllib.request
import zlib

ROOT = Path(__file__).resolve().parents[2]
LOCAL = ROOT / ".local/source-capture"
TOOL = ROOT / "tools/source-capture"
HUD_OUTPUT = ROOT / "assets/reference/osrs240/native-hud"


def sha(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify(path: Path, record: dict) -> Path:
    if not path.is_file() or path.stat().st_size != record["size_bytes"] or sha(path) != record["sha256"]:
        raise ValueError(f"Missing/corrupt pinned source input: {path}")
    return path


def link(source: Path, target: Path, record: dict) -> Path:
    verify(source, record)
    target.parent.mkdir(parents=True, exist_ok=True)
    if not target.exists():
        try:
            os.link(source, target)
        except OSError as error:
            if error.errno != errno.EXDEV:
                raise
            shutil.copyfile(source, target)
    return verify(target, record)


def prepare(source: Path) -> list[Path]:
    selection = json.loads((ROOT / "research/current-source/selection.json").read_text())
    lock = json.loads((ROOT / "tools/cache-import/dependencies.json").read_text())
    if selection["runtime"]["version"] != "1.12.38" or selection["cache"]["id"] != 2695:
        raise ValueError("These native member bindings are locked to injected1.12.38/cache2695; re-verify them before changing inputs.")
    libraries = []
    for record in [lock["decoder"], *lock["libraries"]]:
        libraries.append(link(source / "tooling" / record["name"], LOCAL / "tooling" / record["name"], record))
    for record in selection["runtime"]["artifacts"]:
        libraries.append(link(source / record["name"], LOCAL / "tooling" / record["name"], record))
    capture_lock = json.loads((TOOL / "dependencies.json").read_text())
    for record in capture_lock["additional_artifacts"]:
        target = LOCAL / "tooling" / record["name"]
        reused = ROOT.parent / "m1-source-captures/.local/source-capture/tooling" / record["name"]
        if not target.exists() and reused.exists():
            link(reused, target, record)
        if not target.exists():
            partial = target.with_suffix(".jar.part")
            try:
                with urllib.request.urlopen(record["url"], timeout=120) as response, partial.open("xb") as stream:
                    size = 0
                    while chunk := response.read(1024 * 1024):
                        size += len(chunk)
                        if size > record["size_bytes"]:
                            raise ValueError("Runtime artifact exceeds pinned size")
                        stream.write(chunk)
                verify(partial, record)
                partial.replace(target)
            finally:
                partial.unlink(missing_ok=True)
        libraries.append(verify(target, record))
    for record in json.loads((ROOT / "research/current-source/cache-files.json").read_text()):
        link(source / "cache-2695" / record["name"], LOCAL / "cache" / record["name"], record)
    return libraries


def png_dimensions(data: bytes) -> tuple[int, int]:
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("Not PNG")
    offset, dimensions, compressed, ended = 8, None, bytearray(), False
    while offset < len(data):
        if offset + 12 > len(data):
            raise ValueError("Truncated PNG")
        size = struct.unpack_from(">I", data, offset)[0]
        end = offset + 12 + size
        if end > len(data):
            raise ValueError("Truncated PNG payload")
        kind = data[offset + 4:offset + 8]
        payload = data[offset + 8:offset + 8 + size]
        if zlib.crc32(kind + payload) != struct.unpack_from(">I", data, offset + 8 + size)[0]:
            raise ValueError("Corrupt PNG chunk")
        if kind == b"IHDR":
            if size != 13:
                raise ValueError("Invalid image header")
            width, height, depth, color, compression, filtering, interlace = struct.unpack(">IIBBBBB", payload)
            if width <= 0 or height <= 0 or width * height > 16_000_000 or (depth, color, compression, filtering, interlace) != (8, 6, 0, 0, 0):
                raise ValueError("Unsupported or invalid RGBA8 capture")
            dimensions = width, height
        elif kind == b"IDAT":
            compressed.extend(payload)
        elif kind == b"IEND":
            ended = end == len(data)
        offset = end
    if dimensions is None or not compressed or not ended:
        raise ValueError("Incomplete PNG")
    stride = 1 + dimensions[0] * 4
    expected = stride * dimensions[1]
    inflater = zlib.decompressobj()
    try:
        raw = inflater.decompress(compressed, expected + 1)
    except zlib.error as error:
        raise ValueError("Corrupt PNG image data") from error
    if not inflater.eof or inflater.unused_data or len(raw) != expected or any(raw[i] > 4 for i in range(0, len(raw), stride)):
        raise ValueError("Invalid PNG scanlines")
    return dimensions


def native_render_errors(log: str) -> list[str]:
    errors = []
    lines = log.splitlines()
    errors.extend(line for line in lines if "ERROR injected-client - Client error:" in line)
    for index, line in enumerate(lines):
        if "thrown in " not in line or "method <" not in line:
            continue
        owner = re.search(r" in '([^']+)'", line)
        if not owner or not re.fullmatch(r"[a-z]{1,3}|rl\d+", owner.group(1)):
            continue
        context = "\n".join(lines[max(0, index - 2):index + 2])
        if re.search(r"NullPointerException|IndexOutOfBoundsException|ArithmeticException|IllegalStateException|OutOfMemoryError", context):
            errors.append(context)
    return errors


def validate_hud_record(record: dict) -> None:
    source, settings = record["source"], record["settings"]
    if source["root_interface"] != 161 or settings["layout"] != "Original Resizable-Classic group161":
        raise ValueError("HUD is not the original Resizable-Classic root")
    if not settings["resized"] or settings["network_transport_connected"] or settings["login_handler_invoked"]:
        raise ValueError("Native HUD fixture state/transport contract violated")
    links = {int(node["parent_component"]): node["interface_group"] for node in source["component_links"]}
    for component, group in [(96, 162), (33, 160)]:
        if links.get((161 << 16) | component) != group:
            raise ValueError("Native chatbox/orbs interface is missing from the full frame")
    actual_tabs = sorted((parent & 65535) - 76 for parent in links
                         if parent >> 16 == 161 and 76 <= (parent & 65535) <= 89)
    if actual_tabs != settings["enabled_tab_slots"]:
        raise ValueError("Declared unlock family disagrees with native interface attachments")
    regions = {region["name"]: region for region in source["native_ui_regions"]}
    if set(regions) != {"minimap", "chat", "sidebar", "active-panel"}:
        raise ValueError("Incomplete native HUD pixel-region coverage")
    for region in regions.values():
        x, y, width, height = region["bounds"]
        if width <= 0 or height <= 0 or x < 0 or y < 0 or x + width > 1920 or y + height > 1080:
            raise ValueError("Native HUD widget bounds are invalid")
        if region["colors"] < 5 or region["nonblack_pixels"] < 100:
            raise ValueError("A required native HUD region is blank")
    active = [widget for widget in source["visible_widgets"] if widget["id"] >> 16 == source["active_interface"]]
    if not active:
        raise ValueError("Active native interface has no visible source widgets")
    texts = " ".join(widget["text"] for widget in active if widget["text"])
    group = source["active_interface"]
    if group == 149 and sum(widget["item"] >= 0 for widget in active) < 12:
        raise ValueError("Native inventory item widgets were not populated")
    if group == 320 and "Total level: 33" not in texts:
        raise ValueError("Native skill values are missing")
    if group == 593 and not all(text in texts for text in ["Bronze sword", "Stab", "Lunge", "Slash", "Block"]):
        raise ValueError("Native weapon and source combat-category data disagree")
    if group == 399 and not all(text in texts for text in ["Cook's Assistant", "Completed: 0/187", "Quest Points: 0/347"]):
        raise ValueError("Native quest content/counters are incomplete")
    if group == 12 and "The Bank of Gielinor" not in texts:
        raise ValueError("Native bank frame did not initialize")
    if group == 300 and ("General Store" not in texts or sum(widget["item"] >= 0 for widget in active) < 8):
        raise ValueError("Native shop frame/stock did not initialize")
    if group == 231 and not any(widget["type"] == 6 for widget in active):
        raise ValueError("Native instructor portrait was omitted")


def validate(directory: Path) -> dict:
    manifest = json.loads((directory / "captures.json").read_text())
    if manifest["owner_reference_pack_approved"] or manifest.get("gpu_plugin_enabled"):
        raise ValueError("Unexpected approval or non-stock GPU-plugin state")
    records = manifest["captures"]
    if manifest.get("profile") == "hud" and not records:
        raise ValueError("Native HUD experiment has not produced a complete capture yet")
    counts = collections.Counter()
    paths = set()
    animation_groups = collections.defaultdict(list)
    for record in records:
        relative = record["path"]
        path = (directory / relative).resolve()
        if not path.is_relative_to(directory.resolve()) or relative in paths:
            raise ValueError("Escaping or duplicate capture path")
        paths.add(relative)
        verify(path, record)
        if png_dimensions(path.read_bytes()) != (record["width"], record["height"]):
            raise ValueError("Capture dimension mismatch")
        if not record["png_roundtrip_exact"] or record["nonbackground_pixels"] <= 0 or record["nonbackground_colors"] < 2:
            raise ValueError("Blank/invalid native capture")
        if record["authenticated_source_journey"]:
            raise ValueError("Offline fixture must not claim authenticated journey evidence")
        counts[record["kind"]] += 1
        if record["kind"] == "original-runtime-item-icon":
            if (record["width"], record["height"]) != (36, 32):
                raise ValueError("Native item icon size changed")
        elif (record["width"], record["height"]) != (1920, 1080):
            raise ValueError("Primary source fixture resolution changed")
        if record["kind"] == "original-runtime-model":
            geometry = record["settings"]["geometry"]
            if geometry["vertices"] <= 0 or geometry["faces"] <= 0:
                raise ValueError("Empty native model")
            for minimum, maximum in zip(geometry["bounds_min"], geometry["bounds_max"]):
                if minimum > maximum or abs(minimum) > 100000 or abs(maximum) > 100000:
                    raise ValueError("Invalid native model transform")
            if "sequence_id" in record["source"]:
                animation_groups[record["source"]["sequence_id"]].append(record)
        if record["kind"] == "original-runtime-scene-fixture":
            if record["source"]["tiles"] < 10000 or record["source"]["placed_object_tile_references"] < 1000:
                raise ValueError("Missing visible source scenery")
            settings = record["settings"]
            if settings["angle_units_per_turn"] != 16384 or settings["camera_height_offset_from_ground"] >= 0:
                raise ValueError("Native scene camera calibration is invalid")
            if settings["camera_local_units"][1] != settings["source_focal_ground_height"] + settings["camera_height_offset_from_ground"]:
                raise ValueError("Source elevation was not preserved")
            local = settings["camera_local_units"]
            readback = settings["native_camera_readback"]
            if [readback["Client.getCameraX"], readback["Client.getCameraZ"], readback["Client.getCameraY"]] != local:
                raise ValueError("Actual native camera differs from declared projection")
            if settings["camera_world_tile_xz"] != [record["source"]["base_x"] + local[0] // 128,
                                                    record["source"]["base_y"] + local[2] // 128]:
                raise ValueError("Source scene coordinates were relocated")
        if record["kind"] == "original-runtime-hud-fixture":
            validate_hud_record(record)
    for sequence, frames in animation_groups.items():
        lengths = frames[0]["source"]["frame_lengths_client_cycles"]
        if sorted(frame["source"]["frame_index"] for frame in frames) != list(range(len(lengths))):
            raise ValueError(f"Sequence {sequence} has incomplete frame coverage")
        if len({frame["settings"]["geometry"]["vertex_xyz_float32_be_sha256"] for frame in frames}) < 2:
            raise ValueError(f"Sequence {sequence} is a static model masquerading as animation")
    if manifest["profile"] == "all":
        expected = {"original-runtime-model": 58, "original-runtime-item-icon": 24,
                    "original-runtime-component-fixture": 1, "original-runtime-scene-fixture": 5,
                    "original-runtime-title-state-fixture": 5}
        if dict(counts) != expected:
            raise ValueError(f"Incomplete capture set: {dict(counts)}")
    if manifest["profile"] == "hud":
        names = {"native-inventory", "native-equipment", "native-skills", "native-combat", "native-prayer",
                 "native-magic", "native-quest-list", "native-bank", "native-shop", "native-guide-dialogue",
                 "family-guide", "family-survival", "family-quest-guide", "family-combat", "family-prayer", "family-magic"}
        if paths != {f"hud/{name}.png" for name in names} or counts != {"original-runtime-hud-fixture": 16}:
            raise ValueError("Incomplete native HUD/panel/progression-family capture set")
    return {"schema_version": 1, "result": "passed", "captures": len(records), "counts": dict(counts),
            "complete_native_animation_cycles": sorted(animation_groups),
            "manifest_sha256": sha(directory / "captures.json"),
            "owner_reference_pack_approved": False, "source_journey_verified": False}


def input_record(path: Path) -> dict:
    return {"path": str(path.relative_to(ROOT)), "size_bytes": path.stat().st_size, "sha256": sha(path)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--profile", choices=["all", "models", "scenes", "title", "hud"], default="all")
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--verify-only", action="store_true", help="Validate captured files without loading the runtime/cache")
    parser.add_argument("--java-home", type=Path,
                        default=Path.home() / ".local/share/jdks/temurin-17.0.20.1+1")
    args = parser.parse_args()
    if args.output is None:
        args.output = HUD_OUTPUT if args.profile == "hud" else ROOT / "assets/reference/osrs240"
    if not args.output.resolve().is_relative_to(ROOT):
        raise ValueError("Capture output must stay inside this worktree")
    if args.verify_only:
        print(json.dumps(validate(args.output), separators=(",", ":")))
        return 0
    if args.source is None:
        reused = ROOT.parent / "m1-runtime-inputs/.local/current-source"
        args.source = reused if reused.exists() else ROOT / ".local/current-source"
    libraries = prepare(args.source)
    classes, home, scratch = LOCAL / "classes", LOCAL / "java-home", LOCAL / "java-work"
    for directory in [classes, home, scratch, args.output]:
        directory.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(str(path) for path in libraries)
    java, javac = args.java_home / "bin/java", args.java_home / "bin/javac"
    subprocess.run([str(javac), "--release", "17", "-cp", cp, "-d", str(classes),
                    *map(str, sorted(TOOL.glob("*.java")))], check=True, cwd=ROOT)
    command = [str(java), "-ea", "-Xmx3g", "-Djava.awt.headless=true",
               "--add-opens=java.base/java.lang=ALL-UNNAMED", "-Dclubscape.capture.seed=" + str(args.seed),
               "-Xlog:exceptions=info",
               "-Duser.home=" + str(home), "-Djava.io.tmpdir=" + str(scratch),
               "-cp", str(classes) + os.pathsep + cp, "OriginalCapture",
               str(LOCAL / "cache"), str(args.source / "extracted"), str(args.output.resolve()), args.profile]
    try:
        result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=240,
                                env=dict(os.environ, TMPDIR=str(scratch), TEMP=str(scratch), TMP=str(scratch)))
    except subprocess.TimeoutExpired as error:
        partial = error.stdout or b""
        (LOCAL / "capture.log").write_text(partial.decode(errors="replace") if isinstance(partial, bytes) else partial)
        raise
    log = result.stdout + result.stderr
    (LOCAL / "capture.log").write_text(log)
    if result.returncode:
        print(log[-6000:])
    result.check_returncode()
    render_errors = native_render_errors(log)
    if render_errors:
        raise ValueError("Original renderer threw swallowed errors; capture rejected:\n" + "\n".join(render_errors[:4]))
    validation = validate(args.output)
    for record in json.loads((ROOT / "research/current-source/cache-files.json").read_text()):
        verify(LOCAL / "cache" / record["name"], record)
    provenance = {
        "schema_version": 1, "captured_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "runtime_source_selection": input_record(ROOT / "research/current-source/selection.json"),
        "base_dependency_lock": input_record(ROOT / "tools/cache-import/dependencies.json"),
        "capture_dependency_lock": input_record(TOOL / "dependencies.json"),
        "runtime_artifacts": [input_record(path) for path in libraries],
        "capture_sources": [input_record(path) for path in sorted(TOOL.glob("*")) if path.is_file()],
        "jvm_arguments": [part for part in command[1:command.index("-cp")]],
        "source_cache_id": 2695, "source_cache_sha256": "8f6bd170d2f97e310aaa7700626f7140647a97ce926195ef7625d738f4aa4157",
        "fixture_seed": args.seed, "profile": args.profile, "native_render_exceptions": render_errors,
        "input_hashes_unchanged_after_capture": True, "personal_runelite_state_read": False,
        "terms_accepted": False, "authenticated": False, "owner_reference_pack_approved": False,
        "validation": validation,
    }
    (args.output / "provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")
    print(json.dumps(validation, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"source-capture: {error}", file=sys.stderr)
        sys.exit(1)

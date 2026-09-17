#!/usr/bin/env python3
"""Reproduce the pinned original-source import. No account, terms acceptance, or renderer."""

from __future__ import annotations

import argparse
import collections
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import urllib.request
import zipfile
import zlib

ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools/cache-import"
DEFAULT_SELECTION = ROOT / "research/current-source/selection.json"
LOCAL = ROOT / ".local/current-source"


class InputError(ValueError):
    pass


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def checked_file(path: Path, record: dict) -> Path:
    if not path.is_file():
        raise InputError(f"Missing input: {path}")
    if path.stat().st_size != record["size_bytes"]:
        raise InputError(f"Size mismatch: {path}")
    if digest(path) != record["sha256"]:
        raise InputError(f"SHA-256 mismatch: {path}")
    return path


def within(root: Path, path: Path) -> Path:
    resolved = path.resolve()
    if not resolved.is_relative_to(root.resolve()):
        raise InputError(f"Path escapes output root: {path}")
    return resolved


def unique_mapping(pairs: list[tuple]) -> dict:
    result = {}
    for key, value in pairs:
        if key in result:
            raise InputError(f"Duplicate JSON input key: {key}")
        result[key] = value
    return result


def read_json(path: Path):
    if path.suffix == ".gz":
        with gzip.open(path, "rt", encoding="utf-8") as stream:
            return json.load(stream, object_pairs_hook=unique_mapping)
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_mapping)


def write_json(path: Path, value) -> dict:
    path = within(ROOT, path)
    path.parent.mkdir(parents=True, exist_ok=True)
    data = (json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
    if path.suffix == ".gz":
        data = gzip.compress(data, mtime=0)
    path.write_bytes(data)
    return file_record(path)


def file_record(path: Path) -> dict:
    path = within(ROOT, path)
    return {
        "path": str(path.relative_to(ROOT)),
        "size_bytes": path.stat().st_size,
        "sha256": digest(path),
    }


def download(record: dict, destination: Path) -> Path:
    destination = within(ROOT, destination)
    if destination.exists():
        return checked_file(destination, record)
    destination.parent.mkdir(parents=True, exist_ok=True)
    partial = destination.with_name(destination.name + ".part")
    if partial.exists():
        raise InputError(f"Remove or inspect the interrupted download first: {partial}")
    try:
        request = urllib.request.Request(record["url"], headers={"User-Agent": "ClubScape-source-import/1"})
        with urllib.request.urlopen(request, timeout=120) as response, partial.open("xb") as stream:
            length = 0
            while chunk := response.read(1024 * 1024):
                length += len(chunk)
                if length > record["size_bytes"]:
                    raise InputError(f"Download exceeds pinned advertised size: {record['url']}")
                stream.write(chunk)
        checked_file(partial, record)
        partial.replace(destination)
        return destination
    finally:
        partial.unlink(missing_ok=True)


def cache_files(selection: dict) -> list[dict]:
    record = selection["cache"]["disk_files_manifest"]
    return read_json(checked_file(ROOT / record["path"], record))


def verify_cache(selection: dict, directory: Path) -> None:
    files = cache_files(selection)
    for record in files:
        checked_file(directory / record["name"], record)
    expected = {record["name"] for record in files}
    empty_indexes = {f"main_file_cache.idx{i}" for i in selection["cache"].get("zero_group_indexes", [])}
    for path in directory.iterdir():
        if path.name not in expected and not (path.name in empty_indexes and path.is_file() and path.stat().st_size == 0):
            raise InputError(f"Untracked cache input: {path}")


def unpack_cache(archive: Path, directory: Path, files: list[dict]) -> None:
    directory = within(ROOT, directory)
    expected = {record["name"]: record for record in files}
    seen = set()
    with zipfile.ZipFile(archive) as zipped:
        for entry in zipped.infolist():
            if entry.is_dir() and entry.filename == "cache/":
                continue
            parts = Path(entry.filename).parts
            if len(parts) != 2 or parts[0] != "cache" or parts[1] not in expected or parts[1] in seen:
                raise InputError(f"Unexpected or duplicated cache ZIP entry: {entry.filename}")
            record = expected[parts[1]]
            if entry.file_size != record["size_bytes"]:
                raise InputError(f"Unpacked size differs from frozen metadata: {entry.filename}")
            seen.add(parts[1])
            path = within(directory, directory / parts[1])
            if path.exists():
                checked_file(path, record)
                continue
            directory.mkdir(parents=True, exist_ok=True)
            partial = path.with_name(path.name + ".part")
            if partial.exists():
                raise InputError(f"Interrupted unpack file exists: {partial}")
            try:
                with zipped.open(entry) as source, partial.open("xb") as destination:
                    length = 0
                    while chunk := source.read(1024 * 1024):
                        length += len(chunk)
                        if length > record["size_bytes"]:
                            raise InputError(f"Unpacked input exceeds pinned size: {entry.filename}")
                        destination.write(chunk)
                checked_file(partial, record)
                partial.replace(path)
            finally:
                partial.unlink(missing_ok=True)
    if seen != expected.keys():
        raise InputError(f"Cache ZIP is incomplete: {sorted(expected.keys() - seen)}")


def fetch(selection: dict, directory: Path) -> None:
    cache = selection["cache"]["download"]
    archive = download(cache, LOCAL / Path(cache["path"]).name)
    unpack_cache(archive, directory, cache_files(selection))
    verify_cache(selection, directory)
    for artifact in selection["runtime"]["artifacts"]:
        download(artifact, LOCAL / artifact["name"])
    print(f"Verified complete cache {selection['cache']['id']} and original runtime artifacts")


def reuse(selection: dict, source: Path, directory: Path) -> dict:
    source_cache = source / f"cache-{selection['cache']['id']}"
    if source_cache.resolve() == directory.resolve():
        raise InputError("Reuse requires a separate destination cache, not the read-only source")
    verify_cache(selection, source_cache)
    lock = read_json(TOOL / "dependencies.json")
    copies = [(source_cache / r["name"], directory / r["name"], r) for r in cache_files(selection)]
    copies += [(source / r["name"], LOCAL / r["name"], r) for r in selection["runtime"]["artifacts"]]
    copies += [(source / "tooling" / r["name"], LOCAL / "tooling" / r["name"], r)
               for r in [lock["decoder"], *lock["libraries"]]]
    for original, target, record in copies:
        checked_file(original, record)
        target = within(ROOT, target)
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(original, target)
        checked_file(target, record)
    verify_cache(selection, source_cache)
    verify_cache(selection, directory)
    result = {"result": "passed", "files_copied_or_reverified": len(copies),
              "source_directory": str(source.resolve()), "network_requests": 0,
              "cache_copied_not_hardlinked": True, "original_cache_unchanged": True}
    write_json(LOCAL / "reuse.json", result)
    return result


def prepare(selection: dict, upstream: Path) -> list[Path]:
    lock = read_json(TOOL / "dependencies.json")
    target = LOCAL / "tooling"
    target.mkdir(parents=True, exist_ok=True)
    decoder = lock["decoder"]
    path = target / decoder["name"]
    if not path.exists():
        built = upstream / decoder["built_artifact"]
        if not built.is_file():
            raise InputError(
                f"Missing pinned decoder {built}. Build the locked source in .local as documented in "
                "tools/cache-import/README.md, then pass --runelite-source; no global installation is needed."
            )
        checked_file(built, decoder)
        shutil.copyfile(built, path)
    checked_file(path, decoder)
    result = [path]
    for library in lock["libraries"]:
        path = target / library["name"]
        if not path.exists():
            group, artifact, version = library["maven"].split(":")
            installed = Path.home() / ".gradle/caches/modules-2/files-2.1" / group / artifact / version
            candidates = sorted(installed.glob("*/" + library["name"])) if installed.is_dir() else []
            reusable = next((candidate for candidate in candidates if digest(candidate) == library["sha256"]), None)
            if reusable:
                checked_file(reusable, library)
                shutil.copyfile(reusable, path)
            else:
                download(library, path)
        result.append(checked_file(path, library))
    for artifact in selection["runtime"]["artifacts"]:
        result.append(checked_file(LOCAL / artifact["name"], artifact))
    return result


def java_tools(home: str | None) -> tuple[str, str]:
    candidates = [
        home, os.environ.get("JAVA_HOME_17"), os.environ.get("JAVA_HOME"),
        str(Path.home() / ".local/share/jdks/temurin-17.0.20.1+1"),
    ]
    for candidate in candidates:
        if candidate:
            java, javac = Path(candidate) / "bin/java", Path(candidate) / "bin/javac"
            if java.is_file() and javac.is_file():
                return str(java), str(javac)
    java, javac = shutil.which("java"), shutil.which("javac")
    if not java or not javac:
        raise InputError("A JDK supporting --release 17 is required; pass --java-home.")
    return java, javac


def compile_tools(selection: dict, upstream: Path, home: str | None) -> tuple[str, str]:
    libraries = prepare(selection, upstream)
    java, javac = java_tools(home)
    classes = LOCAL / "classes"
    classes.mkdir(parents=True, exist_ok=True)
    classpath = os.pathsep.join(map(str, libraries))
    command = [javac, "--release", "17", "-cp", classpath, "-d", str(classes)]
    command += [str(TOOL / name) for name in ["CacheExtractor.java", "RuntimeProbe.java", "CacheIntegrityTest.java"]]
    subprocess.run(command, check=True, cwd=ROOT)
    return java, str(classes) + os.pathsep + classpath


def run_java(java: str, classpath: str, arguments: list[str], log: Path, timeout: int = 300) -> dict:
    home, scratch = ROOT / ".local/java-home", ROOT / ".local/java-work"
    home.mkdir(parents=True, exist_ok=True)
    scratch.mkdir(parents=True, exist_ok=True)
    command = [
        java, "-ea", "-Xmx3g", "-Djava.awt.headless=true", "-Dfile.encoding=UTF-8",
        "-Duser.home=" + str(home), "-Djava.io.tmpdir=" + str(scratch), "-cp", classpath, *arguments,
    ]
    environment = dict(os.environ, TMPDIR=str(scratch), TMP=str(scratch), TEMP=str(scratch))
    result = subprocess.run(command, cwd=ROOT, env=environment, text=True, capture_output=True, timeout=timeout)
    log.parent.mkdir(parents=True, exist_ok=True)
    log.write_text(result.stdout + result.stderr, encoding="utf-8")
    if result.returncode:
        raise InputError(f"Source tool failed (exit {result.returncode}); {log}\n{(result.stdout + result.stderr)[-5000:]}")
    print(result.stdout.strip())
    return {"exit_code": result.returncode, "log": file_record(log)}


def validate_model(value: dict, statistics: dict | None = None) -> None:
    model = value["model"]
    vertices, faces = model["vertexCount"], model["faceCount"]
    if type(vertices) is not int or type(faces) is not int or vertices < 0 or faces < 0 or (vertices == 0 and faces):
        raise InputError("Invalid geometry counts")
    for axis in ["vertexX", "vertexY", "vertexZ"]:
        if len(model[axis]) != vertices or any(type(n) is not int or abs(n) > 1_000_000 for n in model[axis]):
            raise InputError("Invalid model vertex array/bounds")
    for indices in ["faceIndices1", "faceIndices2", "faceIndices3"]:
        if len(model[indices]) != faces or any(type(n) is not int or n < 0 or n >= vertices for n in model[indices]):
            raise InputError("Model triangle index is out of bounds")
    for field in ["faceColors", "faceTextures", "faceTransparencies", "faceRenderTypes", "faceRenderPriorities"]:
        if model.get(field) is not None and len(model[field]) != faces:
            raise InputError(f"Invalid face attribute cardinality: {field}")
    for field in ["packedVertexGroups", "animayaGroups", "animayaScales"]:
        if model.get(field) is not None and len(model[field]) != vertices:
            raise InputError(f"Invalid vertex attribute cardinality: {field}")
    for field in ["packedTransparencyVertexGroups", "textureCoords", "faceZOffsets"]:
        if model.get(field) is not None and len(model[field]) != faces:
            raise InputError(f"Invalid face attribute cardinality: {field}")
    if statistics is not None:
        minimum = [min(model[axis], default=0) for axis in ["vertexX", "vertexY", "vertexZ"]]
        maximum = [max(model[axis], default=0) for axis in ["vertexX", "vertexY", "vertexZ"]]
        if (statistics["vertices"], statistics["faces"], statistics["bounds_min"], statistics["bounds_max"]) != (
                vertices, faces, minimum, maximum):
            raise InputError("Model counts/native bounds differ from decoded geometry")
        if statistics["source_empty_mesh"] != (vertices == 0 or faces == 0):
            raise InputError("Source empty-mesh flag differs from decoded geometry")


def validate_frame(value: dict) -> None:
    count = value["translatorCount"]
    skeleton = value["framemap"]
    length = skeleton["length"]
    if count < 0 or length != len(skeleton["types"]) or length != len(skeleton["frameMaps"]):
        raise InputError("Invalid original frame/skeleton transform counts")
    for field in ["indexFrameIds", "translator_x", "translator_y", "translator_z"]:
        if len(value[field]) != count:
            raise InputError(f"Frame transform cardinality differs: {field}")
    if any(index < 0 or index >= length for index in value["indexFrameIds"]):
        raise InputError("Frame transform index is outside the original skeleton")


def validate_region(value: dict, objects: set[int]) -> None:
    if value["dimensions"] != [4, 64, 64]:
        raise InputError("Incorrect region dimensions")
    for field in [
        "heights", "underlay_ids", "overlay_ids", "overlay_shapes", "overlay_rotations",
        "tile_settings", "encoded_heights",
    ]:
        if len(value[field]) != 16384:
            raise InputError(f"Invalid terrain array length: {field}")
    if any(n % 8 or not -65536 <= n <= 0 for n in value["heights"]):
        raise InputError("Implausible native terrain height")
    for object_id, x, y, plane, kind, orientation in value["placements"]:
        if object_id not in objects:
            raise InputError(f"Placed object is not in the source dependency closure: {object_id}")
        if not value["base_x"] <= x < value["base_x"] + 64 or not value["base_y"] <= y < value["base_y"] + 64:
            raise InputError("Relocated or out-of-bounds source placement")
        if not 0 <= plane <= 3 or not 0 <= orientation <= 3 or not 0 <= kind <= 63:
            raise InputError("Invalid source placement flags")


def validate_png(data: bytes) -> tuple[int, int]:
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise InputError("Not an original decoded PNG")
    offset, dimensions, ended, compressed = 8, None, False, bytearray()
    while offset < len(data):
        if offset + 12 > len(data):
            raise InputError("Truncated PNG chunk")
        size = struct.unpack_from(">I", data, offset)[0]
        end = offset + 12 + size
        if end > len(data):
            raise InputError("Truncated PNG data")
        kind = data[offset + 4:offset + 8]
        payload = data[offset + 8:offset + 8 + size]
        crc = struct.unpack_from(">I", data, offset + 8 + size)[0]
        if zlib.crc32(kind + payload) != crc:
            raise InputError("Corrupt PNG chunk CRC")
        if kind == b"IHDR":
            if size != 13:
                raise InputError("Invalid PNG image header")
            width, height, depth, color, *_ = struct.unpack(">IIBBBBB", payload)
            if width <= 0 or height <= 0 or width * height > 64_000_000 or depth != 8 or color != 6:
                raise InputError("Invalid RGBA8 sprite dimensions/format")
            dimensions = width, height
        if kind == b"IEND":
            ended = end == len(data)
        if kind == b"IDAT":
            compressed.extend(payload)
        offset = end
    if dimensions is None or not ended or not compressed:
        raise InputError("Incomplete PNG")
    stride = 1 + dimensions[0] * 4
    expected = stride * dimensions[1]
    inflater = zlib.decompressobj()
    try:
        raw = inflater.decompress(compressed, expected + 1)
    except zlib.error as error:
        raise InputError("Corrupt PNG image data") from error
    if not inflater.eof or inflater.unused_data or len(raw) != expected or any(raw[i] > 4 for i in range(0, len(raw), stride)):
        raise InputError("Invalid PNG RGBA scanlines")
    return dimensions


def validate_bundle(directory: Path) -> dict:
    manifest = read_json(directory / "bundle.json")
    records = manifest["records"]
    ids = [record["asset_id"] for record in records]
    if len(set(ids)) != len(ids) or any(not identity.startswith("asset.source.osrs.") for identity in ids):
        raise InputError("Duplicate or unreserved source asset ID")
    objects = {int(r["asset_id"].split(".")[-1]) for r in records if r["kind"] == "object"}
    by_id = {record["asset_id"]: record for record in records}
    prefix = f"asset.source.osrs.cache{manifest['cache_id']}."
    sprite_statistics = {int(r["asset_id"].split(".")[-1]): r["statistics"] for r in records if r["kind"] == "sprite"}
    checked = set()
    source_payloads = set()
    counters = collections.Counter()
    nonempty_models = 0
    for record in records:
        counters[record["kind"]] += 1
        decoded = None
        for output in record["outputs"]:
            path = within(directory, directory / output["path"])
            if output["path"] in checked:
                raise InputError(f"Duplicate extraction output path: {output['path']}")
            checked_file(path, output)
            checked.add(output["path"])
            if "json_sha256" in output:
                raw = gzip.decompress(path.read_bytes())
                if len(raw) != output["json_size_bytes"] or hashlib.sha256(raw).hexdigest() != output["json_sha256"]:
                    raise InputError(f"Decoded JSON integrity mismatch: {path}")
            if path.name.endswith((".json", ".json.gz")):
                decoded = read_json(path)
            if path.suffix == ".png":
                dimensions = validate_png(path.read_bytes())
                stats = record["statistics"]
                if dimensions != (stats["atlas_width"], stats["atlas_height"]):
                    raise InputError("Decoded sprite dimensions differ from the manifest")
            if path.suffix == ".mid":
                data = path.read_bytes()
                if data[:8] != b"MThd\x00\x00\x00\x06" or struct.unpack_from(">H", data, 10)[0] == 0:
                    raise InputError("Empty/malformed MIDI")
        if record["kind"] == "model":
            validate_model(decoded, record["statistics"])
            nonempty_models += bool(decoded["model"]["vertexCount"] and decoded["model"]["faceCount"])
        elif record["kind"] == "frame":
            validate_frame(decoded)
        elif record["kind"] == "region":
            validate_region(decoded, objects)
            counters["placed_objects"] += len(decoded["placements"])
        elif record["kind"] == "font":
            if len(decoded["advances"]) != 256 or sum(decoded["advances"]) <= 0 or decoded["ascent"] <= 0:
                raise InputError("Invalid native font metrics")
            sprite = sprite_statistics.get(decoded["glyph_sprite_group"])
            if not sprite or sprite["frames"] != 256 or sprite["nontransparent_pixels"] <= 0:
                raise InputError("Missing/empty original font glyph atlas")
        for source in record["source"]:
            if source["group_key"] not in manifest["groups"] or source["size_bytes"] <= 0:
                raise InputError("Missing source provenance or empty source payload")
            raw = f"raw/{source['group_key']}/{source['file']}.bin"
            if raw not in source_payloads:
                checked_file(within(directory, directory / raw), source)
                source_payloads.add(raw)
    if dict(counters) != manifest["counts"]:
        raise InputError("Extraction counts do not match the records")
    content_closure = manifest.get("scope") == "content-asset-closure"
    if content_closure:
        required = ["model", "sprite", "font"]
        for kind, key in [("npc", "npc_ids"), ("item", "item_ids"), ("interface", "interface_groups")]:
            if manifest["request"][key] or manifest["request"]["audit_existing"][key]:
                required.append(kind)
    else:
        required = ["model", "region", "sprite", "font", "sequence", "frame", "music", "sound", "npc", "object"]
    for kind in required:
        if counters[kind] <= 0:
            raise InputError(f"No actual decoded source data for {kind}")
    if nonempty_models == 0:
        raise InputError("No renderable original model was decoded")
    representatives = [] if content_closure else [
        ("object", "tree", "objectModels"), ("npc", "goblin", "models"), ("npc", "penguin", "models")]
    for kind, key, field in representatives:
        identifier = manifest["request"]["representatives"][key][kind + "_id"]
        record = by_id[prefix + kind + "." + str(identifier)]
        definition = read_json(directory / next(o["path"] for o in record["outputs"] if o["path"].endswith(".json")))
        if not definition[field]:
            raise InputError(f"Representative {key} has no original model")
        for model_id in definition[field]:
            statistics = by_id[prefix + "model." + str(model_id)]["statistics"]
            if statistics["vertices"] <= 0 or statistics["faces"] <= 0:
                raise InputError(f"Representative {key} has an empty original model")
    return {
        "schema_version": 1, "result": "passed", "counts": dict(counters),
        "nonempty_models": nonempty_models, "verified_output_files": len(checked),
        "verified_original_payloads": len(source_payloads), "verified_source_groups": len(manifest["groups"]),
        "bundle": file_record(directory / "bundle.json"),
        "source_capture_or_gameplay_acceptance": False,
    }


def publish(directory: Path) -> dict:
    validation = validate_bundle(directory)
    bundle = read_json(directory / "bundle.json")
    if bundle.get("scope") == "content-asset-closure":
        raise InputError("Use publish-closure for additive content assets; the original publication must not be replaced")
    tag = f"cache{bundle['cache_id']}"
    destination = ROOT / "assets/source/osrs" / tag
    manifests = ROOT / "assets/manifests/osrs"
    destination.mkdir(parents=True, exist_ok=True)
    manifests.mkdir(parents=True, exist_ok=True)
    published = []
    records = bundle["records"]
    by_id = {record["asset_id"]: record for record in records}
    representatives = bundle["request"]["representatives"]
    model_ids = set()
    for kind, key, field in [("object", "tree", "objectModels"), ("npc", "goblin", "models"), ("npc", "penguin", "models")]:
        identifier = representatives[key][kind + "_id"]
        record = by_id[f"asset.source.osrs.{tag}.{kind}.{identifier}"]
        definition = read_json(directory / next(o["path"] for o in record["outputs"] if o["path"].endswith(".json")))
        model_ids.update(definition[field])
    aggregated = {"object", "npc", "item", "sequence", "frame", "skeleton", "sound", "underlay", "overlay", "texture"}
    collections_by_kind = {kind: {} for kind in aggregated}
    copy_kinds = {"region", "interface", "font", "sprite", "music", "title", "cached-animation"}
    for record in records:
        kind = record["kind"]
        for output in record["outputs"]:
            relative = output["path"]
            if kind in aggregated and relative.endswith((".json", ".json.gz")):
                collections_by_kind[kind][record["asset_id"]] = read_json(directory / relative)
            representative_model = kind == "model" and int(record["asset_id"].split(".")[-1]) in model_ids
            copy = (kind in copy_kinds and not relative.startswith("raw/")) or representative_model
            copy |= kind == "cached-animation"
            copy |= kind == "sound" and relative.startswith("raw/")
            copy |= kind in {"font", "music"} and relative.startswith("raw/")
            if copy:
                target = within(destination, destination / relative)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(directory / relative, target)
                published.append({**file_record(target), "source_asset_id": record["asset_id"]})
    for kind, values in sorted(collections_by_kind.items()):
        published.append({
            **write_json(destination / "collections" / (kind + ".json.gz"), values),
            "source_kind": kind, "source_records": len(values),
            "transformation": "JSON aggregation only; values retain their decoded source fields.",
        })
    full_bundle = manifests / f"{tag}-full-bundle.json.gz"
    full_bundle.write_bytes(gzip.compress((directory / "bundle.json").read_bytes(), mtime=0))
    manifest = {
        "schema_version": 1, "source_selection": "research/current-source/selection.json",
        "full_extraction_bundle": file_record(full_bundle),
        "full_extraction_reproduction": "python3 tools/cache-import/import_cache.py extract",
        "full_extraction_default_directory": ".local/current-source/extracted",
        "published_scope": "All selected terrain/placements/object definitions; UI/fonts/music; representative original meshes. Full model and audio-sample closure is in the hash-indexed reproducible extraction, not a whole cache committed to Git.",
        "representatives": representatives, "published_files": published,
        "full_validation": validation, "closure": bundle["closure"],
        "owner_reference_pack_approved": False, "stock_scene_captures": False,
        "clubscape_visual_audio_gameplay_or_runelite_acceptance": False,
    }
    write_json(manifests / f"{tag}-published.json", manifest)
    return {"published_files": len(published), "published_bytes": sum(p["size_bytes"] for p in published),
            "full_bundle_manifest": file_record(full_bundle)}


def validate_published(selection: dict) -> dict:
    tag = f"cache{selection['cache']['id']}"
    manifest = read_json(ROOT / "assets/manifests/osrs" / f"{tag}-published.json")
    inventory_record = manifest["full_extraction_bundle"]
    inventory_path = checked_file(within(ROOT, ROOT / inventory_record["path"]), inventory_record)
    inventory_bytes = gzip.decompress(inventory_path.read_bytes())
    if hashlib.sha256(inventory_bytes).hexdigest() != manifest["full_validation"]["bundle"]["sha256"]:
        raise InputError("Full source inventory has changed")
    inventory = json.loads(inventory_bytes)
    if inventory["cache_id"] != selection["cache"]["id"] or inventory["revision"] != selection["cache"]["build"]:
        raise InputError("Published inputs disagree with frozen source identity")
    object_ids = {int(record["asset_id"].split(".")[-1]) for record in inventory["records"] if record["kind"] == "object"}
    paths = set()
    for record in manifest["published_files"]:
        path = within(ROOT / "assets/source/osrs" / tag, ROOT / record["path"])
        if record["path"] in paths:
            raise InputError("Duplicate published asset path")
        paths.add(record["path"])
        checked_file(path, record)
        if path.suffix == ".png":
            validate_png(path.read_bytes())
        elif path.parent.name == "models" and path.name.endswith(".json.gz"):
            validate_model(read_json(path))
        elif path.parent.name == "world":
            validate_region(read_json(path), object_ids)
    result = {"result": "passed", "published_files": len(paths),
              "original_source_inventory_records": len(inventory["records"]),
              "source_capture_or_gameplay_acceptance": False}
    extension = ROOT / "assets/manifests/osrs" / f"{tag}-content-v2-published.json"
    if extension.exists():
        import content_closure
        result["content_closure"] = content_closure.validate_publication(extension)
        if content_closure.POTION_PUBLICATION.exists():
            result["potion_assets"] = content_closure.validate_publication(content_closure.POTION_PUBLICATION)
        if content_closure.CONSUMABLE_PUBLICATION.exists():
            result["consumable_assets"] = content_closure.validate_publication(content_closure.CONSUMABLE_PUBLICATION)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["fetch", "reuse", "prepare", "verify", "scan", "extract", "probe", "validate",
                                          "publish", "validate-published", "test-integrity", "plan-closure",
                                          "extract-closure", "publish-closure", "validate-closure",
                                          "validate-closure-request", "plan-potions", "plan-consumables"])
    parser.add_argument("--selection", type=Path, default=DEFAULT_SELECTION)
    parser.add_argument("--cache", type=Path, default=LOCAL / "cache-2695")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--request", type=Path)
    parser.add_argument("--reuse-source", type=Path)
    parser.add_argument("--definitions", type=Path, help="Read-only original UI definition snapshot for plan-consumables")
    parser.add_argument("--runelite-source", type=Path,
                        default=Path.home() / ".cache/clubscape/upstream/runelite")
    parser.add_argument("--java-home")
    args = parser.parse_args()
    selection = read_json(args.selection)
    closure_command = args.command.endswith("-closure") or args.command == "validate-closure-request"
    if args.output is None:
        args.output = LOCAL / ("content-closure" if closure_command else "extracted")
    if args.request is None:
        args.request = ROOT / "research/current-source" / ("m1-content-closure-request.json" if closure_command else "m1-request.json")
    cache, output = within(ROOT, args.cache), within(ROOT, args.output)
    if args.command == "fetch":
        fetch(selection, cache)
    elif args.command == "reuse":
        if args.reuse_source is None:
            raise InputError("reuse requires --reuse-source pointing to verified existing current-source inputs")
        print(json.dumps(reuse(selection, args.reuse_source.resolve(), cache), separators=(",", ":")))
    elif args.command == "plan-closure":
        import content_closure
        result = content_closure.plan(args.request)
        print(json.dumps(result, separators=(",", ":")))
    elif args.command == "plan-potions":
        import content_closure
        print(json.dumps(content_closure.plan(content_closure.POTION_REQUEST, potions=True), separators=(",", ":")))
    elif args.command == "plan-consumables":
        import content_closure
        if args.definitions is None:
            raise InputError("plan-consumables requires --definitions pointing to the original UI item snapshot")
        print(json.dumps(content_closure.plan_consumables(args.definitions), separators=(",", ":")))
    elif args.command == "validate-closure-request":
        import content_closure
        request = read_json(args.request)
        content_closure.validate_request(request)
        print(json.dumps({"result": "passed", "request": file_record(args.request),
                          "required_product_asset_ids": len(request["required_asset_ids"])}, separators=(",", ":")))
    elif args.command == "prepare":
        libraries = prepare(selection, args.runelite_source)
        print(f"Verified {len(libraries)} pinned decoder/runtime dependencies")
    elif args.command == "verify":
        verify_cache(selection, cache)
        print("Complete unpacked cache integrity passed")
    elif args.command == "probe":
        configuration = selection["runtime"]["configuration"]
        checked_file(ROOT / configuration["path"], configuration)
        java, classpath = compile_tools(selection, args.runelite_source, args.java_home)
        run_java(java, classpath, ["RuntimeProbe", str(ROOT / configuration["path"])],
                 LOCAL / "runtime-probe.log", timeout=30)
    elif args.command == "test-integrity":
        verify_cache(selection, cache)
        java, classpath = compile_tools(selection, args.runelite_source, args.java_home)
        run_java(java, classpath, ["CacheIntegrityTest", str(cache)], LOCAL / "integrity-tests.log")
        verify_cache(selection, cache)
    elif args.command in {"scan", "extract", "extract-closure"}:
        verify_cache(selection, cache)
        java, classpath = compile_tools(selection, args.runelite_source, args.java_home)
        java_command = "closure" if args.command == "extract-closure" else args.command
        arguments = ["CacheExtractor", java_command, str(cache), str(output),
                     str(selection["cache"]["build"]), str(selection["cache"]["id"])]
        if args.command in {"extract", "extract-closure"}:
            request = read_json(args.request)
            if request["cache_id"] != selection["cache"]["id"] or request["game_revision"] != selection["cache"]["build"]:
                raise InputError("Request and frozen source selection disagree")
            if args.command == "extract-closure":
                import content_closure
                content_closure.validate_request(request)
            arguments.append(str(args.request.resolve()))
        run_java(java, classpath, arguments, LOCAL / (args.command + "-run.log"))
        verify_cache(selection, cache)
        if args.command in {"extract", "extract-closure"}:
            result = validate_bundle(output)
            result["tools"] = [file_record(TOOL / name) for name in ["CacheExtractor.java", "RuntimeProbe.java", "dependencies.json", "import_cache.py"]]
            if args.command == "extract-closure":
                result["content_closure"] = content_closure.validate_extraction(output)
            write_json(LOCAL / ("closure-validation.json" if closure_command else "validation.json"), result)
            print(json.dumps(result, separators=(",", ":")))
    elif args.command == "validate":
        result = validate_bundle(output)
        write_json(LOCAL / "validation.json", result)
        print(json.dumps(result, separators=(",", ":")))
    elif args.command == "publish":
        print(json.dumps(publish(output), separators=(",", ":")))
    elif args.command == "publish-closure":
        import content_closure
        print(json.dumps(content_closure.publish(output, args.request), separators=(",", ":")))
    elif args.command == "validate-closure":
        import content_closure
        print(json.dumps(content_closure.validate_extraction(output), separators=(",", ":")))
    elif args.command == "validate-published":
        print(json.dumps(validate_published(selection), separators=(",", ":")))
    return 0


if __name__ == "__main__":
    sys.modules["import_cache"] = sys.modules[__name__]
    try:
        sys.exit(main())
    except (InputError, OSError, json.JSONDecodeError, zlib.error, zipfile.BadZipFile, subprocess.SubprocessError) as error:
        print(f"cache-import: {error}", file=sys.stderr)
        sys.exit(1)

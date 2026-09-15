#!/usr/bin/env python3
"""Export neutral render buffers from the pinned original runtime (tools/render-assets).

This never renders a candidate image. It compiles the Java exporter together with the
existing source-capture bootstrap (read-only reuse) and runs it against the hash-verified
original cache, writing chunked binary buffers plus a manifest under assets/compiled/render.
"""
from __future__ import annotations

import argparse
import gzip
import io
import tarfile
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
TOOL = ROOT / "tools/render-assets"
CAPTURE_TOOL = ROOT / "tools/source-capture"
LOCAL = ROOT / ".local/render-assets"
DEFAULT_OUTPUT = ROOT / "assets/compiled/render"


def load_capture_module():
    spec = importlib.util.spec_from_file_location("source_capture", CAPTURE_TOOL / "capture.py")
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def sha(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def normalize(value):
    """Gson round-trips integers as doubles; restore integral numbers."""
    if isinstance(value, float) and value.is_integer() and abs(value) < 2**53:
        return int(value)
    if isinstance(value, dict):
        return {k: normalize(v) for k, v in value.items()}
    if isinstance(value, list):
        return [normalize(v) for v in value]
    return value


# Raw scene buffers are large (5-15 MB each) and reproducible; only their gzip form is published.
# A raw file may therefore be absent when its .gz twin verifies (see `unpack`).
COMPRESSED_PREFIXES = ("scenes/", "blocks/")
# Twins that are reproducible but not committed (world blocks: see `pack-blocks`); `unpack`
# restores them only when present, `unpack-blocks`/`verify-blocks` require them.
UNPUBLISHED_PREFIXES = ("blocks/",)


def gzip_bytes(data: bytes) -> bytes:
    """Deterministic gzip (no name, mtime 0) so the published .gz hashes are reproducible."""
    return gzip.compress(data, compresslevel=9, mtime=0)


def compress_scenes(output: Path) -> int:
    manifest_path = output / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    written = 0
    for name in sorted(manifest["files"]):
        if not name.startswith(COMPRESSED_PREFIXES) or name.endswith(".gz"):
            continue
        raw = output / name
        if not raw.is_file():
            continue
        data = raw.read_bytes()
        compressed = gzip_bytes(data)
        target = output / (name + ".gz")
        target.write_bytes(compressed)
        manifest["files"][name + ".gz"] = {
            "sha256": hashlib.sha256(compressed).hexdigest(), "size_bytes": len(compressed),
            "detail": {"encoding": "gzip", "decompressed": name, "decompressed_sha256": manifest["files"][name]["sha256"],
                       "decompressed_size_bytes": len(data)},
        }
        written += 1
    for scene in [*manifest.get("scenes", []), *manifest.get("blocks", [])]:
        for key in ("file", "models_file"):
            if key in scene and (scene[key] + ".gz") in manifest["files"]:
                scene[key + "_gz"] = scene[key] + ".gz"
    manifest_path.write_text(json.dumps(manifest, indent=1) + "\n")
    return written


def unpack_scenes(output: Path, require: tuple[str, ...] = ()) -> dict:
    """
    Restore raw buffers from their .gz twins (native tests read the raw files). Published twins
    (scenes) must be present and are always verified; twins that are deliberately unpublished
    (`blocks/`, reproducible locally or installed from the block pack) are restored when present
    and only *required* when their prefix is listed in `require`. A raw file whose hash already
    matches the manifest is left alone; a stale raw file is overwritten with the pinned bytes.
    """
    manifest = json.loads((output / "manifest.json").read_text())
    restored = fresh = skipped = 0
    for name, record in manifest["files"].items():
        if not name.endswith(".gz"):
            continue
        raw_name = record["detail"]["decompressed"]
        raw = output / raw_name
        compressed_path = output / name
        if not compressed_path.is_file():
            unpublished = name.startswith(UNPUBLISHED_PREFIXES) and not name.startswith(require)
            if unpublished:
                skipped += 1
                continue
            raise ValueError(f"Missing published buffer {name}")
        if raw.is_file() and sha(raw) == record["detail"]["decompressed_sha256"]:
            fresh += 1
            continue
        compressed = compressed_path.read_bytes()
        if hashlib.sha256(compressed).hexdigest() != record["sha256"]:
            raise ValueError(f"Published buffer changed: {name}")
        data = gzip.decompress(compressed)
        if hashlib.sha256(data).hexdigest() != record["detail"]["decompressed_sha256"]:
            raise ValueError(f"Decompressed buffer hash mismatch: {name}")
        raw.parent.mkdir(parents=True, exist_ok=True)
        raw.write_bytes(data)
        restored += 1
    return {"restored": restored, "already_pinned": fresh, "unpublished_skipped": skipped}


BLOCK_INDEX = "blocks.index.json"


def block_files(manifest: dict) -> list[str]:
    """Published (gzip) world block buffers in manifest order: what a runtime needs to stream."""
    names = []
    for block in manifest.get("blocks", []):
        for key in ("file_gz", "models_file_gz"):
            if key not in block:
                raise ValueError(f"Block {block['square']} has no published gzip twin ({key}); run --profile compress")
            names.append(block[key])
    for entry in manifest.get("minimap_blocks", []):
        names.append(entry["file"])
    return names


def pack_blocks(output: Path, dist: Path) -> dict:
    """
    Packages the 61 world blocks (+ minimap sidecars) as one deterministic tar the shell can
    publish, and writes `blocks.index.json` beside the manifest: per file key, SHA-256 and size,
    plus the pack's own hash. Consumers need only Python 3 (`--profile unpack-blocks`), not the
    source cache or a JDK.
    """
    manifest = json.loads((output / "manifest.json").read_text())
    names = block_files(manifest)
    entries = []
    for name in names:
        record = manifest["files"][name]
        path = output / name
        if not path.is_file():
            raise ValueError(f"Block buffer {name} is not exported locally; run --profile blocks/minimap first")
        if sha(path) != record["sha256"]:
            raise ValueError(f"Block buffer {name} differs from the manifest hash")
        entries.append({"file": name, "sha256": record["sha256"], "size_bytes": record["size_bytes"],
                        **({"decompressed": record["detail"]["decompressed"], "decompressed_sha256": record["detail"]["decompressed_sha256"]}
                           if record.get("detail", {}).get("encoding") == "gzip" else {})})
    dist.mkdir(parents=True, exist_ok=True)
    content_hash = hashlib.sha256("\n".join(f"{e['file']} {e['sha256']}" for e in entries).encode()).hexdigest()
    pack_name = f"clubscape-render-blocks-{content_hash[:16]}.tar"
    pack_path = dist / pack_name
    with tarfile.open(pack_path, "w", format=tarfile.PAX_FORMAT) as tar:
        for entry in entries:
            info = tarfile.TarInfo(entry["file"])
            data = (output / entry["file"]).read_bytes()
            info.size = len(data)
            info.mtime = 0
            info.mode = 0o644
            info.uid = info.gid = 0
            info.uname = info.gname = ""
            tar.addfile(info, io.BytesIO(data))
    index = {
        "schema_version": 1,
        "kind": "clubscape_render_blocks",
        "manifest_sha256": sha(output / "manifest.json"),
        "approved_reference_pack_sha256": manifest["approved_reference_pack_sha256"],
        "content_sha256": content_hash,
        "pack": {"file": pack_name, "sha256": sha(pack_path), "size_bytes": pack_path.stat().st_size},
        "squares": [b["square"] for b in manifest.get("blocks", [])],
        "files": entries,
        "install": "python3 tools/render-assets/export.py --profile unpack-blocks <pack.tar>  (verifies every hash; no source cache or JDK needed)",
        "classification": "Reproducible original-loader exports (blocks profile); runtime inputs for world streaming, not reference images.",
    }
    (output / BLOCK_INDEX).write_text(json.dumps(index, indent=1) + "\n")
    return {"pack": str(pack_path), "sha256": index["pack"]["sha256"], "size_bytes": index["pack"]["size_bytes"], "files": len(entries)}


def unpack_blocks(output: Path, pack_path: Path) -> dict:
    """Installs a block pack into the asset tree, verifying the pack and every file against `blocks.index.json`."""
    index = json.loads((output / BLOCK_INDEX).read_text())
    if sha(pack_path) != index["pack"]["sha256"]:
        raise ValueError(f"Block pack hash differs from {BLOCK_INDEX}: {pack_path}")
    expected = {e["file"]: e for e in index["files"]}
    installed = inflated = 0
    with tarfile.open(pack_path, "r") as tar:
        for member in tar.getmembers():
            entry = expected.get(member.name)
            if entry is None or not member.isfile():
                raise ValueError(f"Unexpected pack member {member.name}")
            data = tar.extractfile(member).read()
            if hashlib.sha256(data).hexdigest() != entry["sha256"] or len(data) != entry["size_bytes"]:
                raise ValueError(f"Pack member {member.name} does not match its pinned hash")
            target = (output / member.name).resolve()
            if not target.is_relative_to(output.resolve()):
                raise ValueError(f"Pack member escapes the asset tree: {member.name}")
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            installed += 1
            if "decompressed" in entry:
                # The native tests read the raw block buffers; restore them next to the twins.
                raw = gzip.decompress(data)
                if hashlib.sha256(raw).hexdigest() != entry["decompressed_sha256"]:
                    raise ValueError(f"Pack member {member.name} inflates to a different buffer")
                (output / entry["decompressed"]).write_bytes(raw)
                inflated += 1
        present = {m.name for m in tar.getmembers()}
    missing = sorted(set(expected) - present)
    if missing:
        raise ValueError(f"Pack lacks pinned files: {missing[:5]}")
    return {"installed": installed, "inflated_raw": inflated, "squares": len(index["squares"])}


def verify_blocks(output: Path) -> dict:
    """Strict check that every pinned block buffer is present with its hash (unlike `--verify-only`, which treats blocks as optional)."""
    index = json.loads((output / BLOCK_INDEX).read_text())
    manifest = json.loads((output / "manifest.json").read_text())
    if index["manifest_sha256"] != sha(output / "manifest.json"):
        raise ValueError("blocks.index.json was written for a different manifest.json")
    for entry in index["files"]:
        path = output / entry["file"]
        if not path.is_file() or sha(path) != entry["sha256"] or manifest["files"][entry["file"]]["sha256"] != entry["sha256"]:
            raise ValueError(f"Pinned block buffer missing or changed: {entry['file']}")
    return {"result": "passed", "files": len(index["files"]), "squares": len(index["squares"]), "content_sha256": index["content_sha256"]}


def refresh_blocks_index(output: Path) -> dict:
    """
    Keeps `blocks.index.json` bound to the manifest it was written for. Every profile that
    rewrites `manifest.json` calls this afterwards: when the pinned block/minimap files are
    unchanged (same names and hashes) only `manifest_sha256` is refreshed and the pack stays
    valid; when their content changed the pack is regenerated from the local exports, and if
    those are missing the export FAILS instead of leaving an index that `verify-blocks` would
    reject later (an index pinning an older manifest once shipped that way).
    """
    index_path = output / BLOCK_INDEX
    if not index_path.is_file():
        return {"index": "absent"}
    index = json.loads(index_path.read_text())
    manifest = json.loads((output / "manifest.json").read_text())
    current_manifest = sha(output / "manifest.json")
    names = block_files(manifest)
    pinned = {e["file"]: e["sha256"] for e in index["files"]}
    current = {name: manifest["files"][name]["sha256"] for name in names}
    if pinned == current:
        if index["manifest_sha256"] == current_manifest:
            return {"index": "current", "manifest_sha256": current_manifest}
        index["manifest_sha256"] = current_manifest
        index_path.write_text(json.dumps(index, indent=1) + "\n")
        return {"index": "manifest_pin_refreshed", "manifest_sha256": current_manifest, "pack": index["pack"]["sha256"]}
    missing = [name for name in names if not (output / name).is_file()]
    if missing:
        raise ValueError(f"Block package content changed ({len(set(pinned) ^ set(current)) + sum(1 for n in current if n in pinned and pinned[n] != current[n])} files) "
                         f"but {len(missing)} block buffers are not exported locally (e.g. {missing[0]}); run --profile blocks/minimap then --profile pack-blocks")
    packed = pack_blocks(output, LOCAL / "dist")
    return {"index": "repacked", **packed}


def build_pose_fits(output: Path) -> dict:
    """
    `--profile pose-fits`: runs the renderer's `pose-fit-table` tool (the same per-pose
    attachment solve the runtime uses) over the M1 worn models × the required player sequences
    they are drawn in (source `lc.bd` hand overrides decide visibility; hidden combinations have
    no fit) and registers `gear/pose-fits.json` (schema 2: rotation about the grip + shift,
    penetration / surface gap / attachment gap) plus `gear/pose-fit-failures.json` (every frame
    over a target, with its legality class) in the manifest. Frames still over the fit targets
    are counted, never hidden.
    """
    cargo = shutil.which("cargo") or str(Path.home() / ".cargo/bin/cargo")
    result = subprocess.run([cargo, "run", "--release", "-p", "clubscape-renderer", "--features", "tools",
                             "--bin", "pose-fit-table", "--", str(output)],
                            cwd=ROOT, text=True, capture_output=True, check=True)
    summary = json.loads(result.stdout.strip().splitlines()[-1])
    manifest_path = output / "manifest.json"
    manifest = json.loads(manifest_path.read_text())
    table_path = output / "gear/pose-fits.json"
    failures_path = output / "gear/pose-fit-failures.json"
    manifest["files"]["gear/pose-fits.json"] = {"sha256": sha(table_path), "size_bytes": table_path.stat().st_size}
    manifest["files"]["gear/pose-fit-failures.json"] = {"sha256": sha(failures_path), "size_bytes": failures_path.stat().st_size}
    manifest["gear_pose_fits"] = {
        "file": "gear/pose-fits.json", "failures_file": "gear/pose-fit-failures.json", "schema_version": 2, "body_npc": 2063,
        "items": summary["items"], "sequences": summary["sequences"], "item_frames": summary["item_frames"],
        "item_frames_not_drawn": summary["item_frames_not_drawn"],
        "legality_frame_counts": summary["legality_frame_counts"],
        "item_frames_over_target": summary["item_frames_over_target"],
        "over_penetration": summary["over_penetration"], "over_gap": summary["over_gap"], "over_attachment": summary["over_attachment"],
        "classification": "Per-item per-pose rigid attachment fits (rotation about the grip + shift) against the penguin body; targets penetration <= 1 (carried bind box), surface gap <= 2, attachment gap <= 2 source units; precomputed runtime input, not a fit waiver",
    }
    manifest_path.write_text(json.dumps(manifest, indent=1) + "\n")
    return summary


def verify_manifest(output: Path) -> dict:
    manifest = json.loads((output / "manifest.json").read_text())
    if manifest["source_cache_id"] != 2695 or manifest["source_revision"] != 240 or manifest["brightness"] != 0.8:
        raise ValueError("Render asset manifest identity differs from the approved source selection")
    checked = 0
    # Validation bakes, trig tables and the world blocks (reproducible, ~75 MB gzip) are not
    # published; their hashes still pin the reproduction.
    optional_prefixes = ("models/baked/", "tables.bin", "blocks/")
    for name, record in manifest["files"].items():
        path = output / name
        if not path.is_file():
            if name.startswith(optional_prefixes):
                continue
            if name.startswith(COMPRESSED_PREFIXES) and (name + ".gz") in manifest["files"]:
                continue
            raise ValueError(f"Missing exported buffer {name}")
        if path.stat().st_size != record["size_bytes"] or sha(path) != record["sha256"]:
            raise ValueError(f"Exported buffer changed: {name}")
        checked += 1
    manifest_sha = sha(output / "manifest.json")
    index_path = output / BLOCK_INDEX
    block_index = "absent"
    if index_path.is_file():
        # The block package index must be bound to THIS manifest and pin exactly the manifest's
        # block/minimap hashes (file presence is `verify-blocks`' stricter job).
        index = json.loads(index_path.read_text())
        if index["manifest_sha256"] != manifest_sha:
            raise ValueError(f"{BLOCK_INDEX} was written for manifest {index['manifest_sha256'][:12]}…, current is {manifest_sha[:12]}…; run --profile pack-blocks (or any export profile, which refreshes it)")
        pinned = {e["file"]: e["sha256"] for e in index["files"]}
        current = {name: manifest["files"][name]["sha256"] for name in block_files(manifest)}
        if pinned != current:
            changed = sorted(set(pinned) ^ set(current)) or sorted(n for n in current if pinned.get(n) != current[n])
            raise ValueError(f"{BLOCK_INDEX} pins block/minimap buffers that differ from the manifest (e.g. {changed[0]}); run --profile pack-blocks")
        block_index = {"manifest_bound": True, "files": len(index["files"]), "content_sha256": index["content_sha256"], "pack_sha256": index["pack"]["sha256"]}
    return {"schema_version": 1, "result": "passed", "files": checked,
            "manifest_sha256": manifest_sha, "block_index": block_index, "candidate_render_acceptance": False}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--profile", default="all",
                        choices=["all", "tables", "palette", "textures", "models", "npcs", "scenes", "scenes-pinned", "blocks", "minimap", "anim", "dynamic", "widgets", "prune-textures", "compress", "unpack",
                                 "pack-blocks", "unpack-blocks", "verify-blocks", "pose-fits", "zoom"])
    parser.add_argument("--verify-only", action="store_true")
    parser.add_argument("--java-home", type=Path, default=Path.home() / ".local/share/jdks/temurin-17.0.20.1+1")
    parser.add_argument("extra", nargs="*", help="Profile-specific arguments passed to the Java exporter")
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to(ROOT):
        raise ValueError("Export output must stay inside this worktree")
    if args.verify_only:
        print(json.dumps(verify_manifest(args.output), separators=(",", ":")))
        return 0
    if args.profile == "compress":
        print(f"COMPRESS {compress_scenes(args.output)} buffers")
        print("BLOCK_INDEX " + json.dumps(refresh_blocks_index(args.output), separators=(",", ":")))
        print(json.dumps(verify_manifest(args.output), separators=(",", ":")))
        return 0
    if args.profile == "unpack":
        # Published twins only; unpublished block twins are restored when present, never required.
        print("UNPACK " + json.dumps(unpack_scenes(args.output), separators=(",", ":")))
        print(json.dumps(verify_manifest(args.output), separators=(",", ":")))
        return 0
    if args.profile == "pack-blocks":
        print("PACK_BLOCKS " + json.dumps(pack_blocks(args.output, LOCAL / "dist"), separators=(",", ":")))
        print(json.dumps(verify_blocks(args.output), separators=(",", ":")))
        return 0
    if args.profile == "unpack-blocks":
        if len(args.extra) != 1:
            raise ValueError("unpack-blocks needs the pack .tar path")
        print("UNPACK_BLOCKS " + json.dumps(unpack_blocks(args.output, Path(args.extra[0])), separators=(",", ":")))
        print(json.dumps(verify_blocks(args.output), separators=(",", ":")))
        return 0
    if args.profile == "verify-blocks":
        print(json.dumps(verify_blocks(args.output), separators=(",", ":")))
        return 0
    if args.profile == "pose-fits":
        print("POSE_FITS " + json.dumps(build_pose_fits(args.output), separators=(",", ":")))
        print("BLOCK_INDEX " + json.dumps(refresh_blocks_index(args.output), separators=(",", ":")))
        print(json.dumps(verify_manifest(args.output), separators=(",", ":")))
        return 0
    capture = load_capture_module()
    if args.profile in ("blocks", "minimap") and not args.extra:
        # The M1 world: every full source map square the content pack retains.
        args.extra = sorted(path.name.split(".")[0] for path in (ROOT / "content/m1/geometry").glob("*.json.gz"))
    if args.source is None:
        reused = ROOT.parent / "m1-runtime-inputs/.local/current-source"
        args.source = reused if reused.exists() else ROOT / ".local/current-source"
    libraries = capture.prepare(args.source)
    classes, home, scratch = LOCAL / "classes", LOCAL / "java-home", LOCAL / "java-work"
    for directory in [classes, home, scratch, args.output]:
        directory.mkdir(parents=True, exist_ok=True)
    cp = os.pathsep.join(str(path) for path in libraries)
    java, javac = args.java_home / "bin/java", args.java_home / "bin/javac"
    sources = sorted((TOOL / "java").glob("*.java")) + sorted(CAPTURE_TOOL.glob("*.java"))
    subprocess.run([str(javac), "--release", "17", "-nowarn", "-cp", cp, "-d", str(classes), *map(str, sources)], check=True, cwd=ROOT)
    command = [str(java), "-ea", "-Xmx6g", "-Djava.awt.headless=true",
               "--add-opens=java.base/java.lang=ALL-UNNAMED",
               "-Duser.home=" + str(home), "-Djava.io.tmpdir=" + str(scratch),
               "-cp", str(classes) + os.pathsep + cp, "RenderExport",
               str(capture.LOCAL / "cache"), str(args.output.resolve()), args.profile, *args.extra]
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=1800,
                            env=dict(os.environ, TMPDIR=str(scratch), TEMP=str(scratch), TMP=str(scratch)))
    log = result.stdout + result.stderr
    (LOCAL / "export.log").write_text(log)
    if result.returncode:
        print(log[-8000:])
    result.check_returncode()
    for line in log.splitlines():
        if line.startswith(("TABLES", "PALETTE", "TEXTURES", "MODEL", "NPCS", "NPCDEF", "ITEMDEF", "ANIM", "DYN", "GROUNDITEM", "WIDGETS", "SCENE", "BLOCK", "MINIMAP", "MAPSCENES", "RENDER_EXPORT_OK")):
            print(line)
    manifest_path = args.output / "manifest.json"
    manifest = normalize(json.loads(manifest_path.read_text()))
    manifest["brightness"] = 0.8
    manifest_path.write_text(json.dumps(manifest, indent=1) + "\n")
    if args.profile in ("all", "scenes", "blocks"):
        print(f"COMPRESS {compress_scenes(args.output)} buffers")
    # Any manifest rewrite re-binds (or regenerates) the block package index.
    print("BLOCK_INDEX " + json.dumps(refresh_blocks_index(args.output), separators=(",", ":")))
    print(json.dumps(verify_manifest(args.output), separators=(",", ":")))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"render-assets: {error}", file=sys.stderr)
        sys.exit(1)

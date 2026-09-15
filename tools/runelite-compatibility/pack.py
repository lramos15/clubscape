#!/usr/bin/env python3
"""Package unchanged compiled M1 and real source payloads for the strict server."""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import uuid

ROOT = Path(__file__).resolve().parents[2]
LOCAL = ROOT / "runelite/compatibility/artifacts"
SOURCE = ROOT.parent / "m1-runtime-inputs/.local/current-source"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def encode(value):
    return json.dumps(value, ensure_ascii=True, separators=(",", ":"), sort_keys=True).encode()


def asset_ids(value):
    result = set()
    if isinstance(value, str) and value.startswith("asset.") and len(value) < 120:
        result.add(value)
    elif isinstance(value, dict):
        for key, child in value.items():
            if key != "source":
                result.update(asset_ids(child))
    elif isinstance(value, list):
        for child in value:
            result.update(asset_ids(child))
    return result


def package(destination, source, *, report_path=None, catalog_path=None):
    report_path = report_path or ROOT / "research/runelite-feasibility/source-pack.json"
    catalog_path = catalog_path or LOCAL / "catalog.json"
    manifest = json.loads((ROOT / "content/m1/manifest.json").read_text())
    compiled = manifest["compiled_artifact"]
    compressed = (ROOT / compiled["path"]).read_bytes()
    if sha(compressed) != compiled["sha256"]:
        raise ValueError("Committed compiled source artifact hash mismatch")
    artifact = gzip.decompress(compressed)
    if sha(artifact) != compiled["uncompressed_sha256"]:
        raise ValueError("Compiled source artifact decompression mismatch")
    definition = json.loads(gzip.decompress((ROOT / "content/m1/game-content.json.gz").read_bytes()))
    if definition["schema_version"] != 3:
        raise ValueError("Expected actual integrated content schema 3")
    descriptor_path = destination / "clubscape-game.json"
    existing = json.loads(descriptor_path.read_text()) if descriptor_path.exists() else {}
    if existing and existing.get("sha256") != sha(artifact):
        raise ValueError("Canonical artifact changed; choose a new --output instead of overwriting historical inputs.")
    if report_path.exists() and json.loads(report_path.read_text()).get("artifact_sha256") != sha(artifact):
        raise ValueError("Canonical artifact changed; choose a new --report instead of relabeling old evidence.")
    if catalog_path.exists() and json.loads(catalog_path.read_text()).get("content_revision") != definition["revision"]:
        raise ValueError("Canonical content changed; choose a new --catalog instead of overwriting historical inputs.")
    destination.mkdir(parents=True, exist_ok=True)
    catalog_path.parent.mkdir(parents=True, exist_ok=True)
    (destination / "world.csc").write_bytes(artifact)
    references = json.loads((ROOT / "content/m1/asset-references.json").read_text())
    required = asset_ids(definition)
    available = {r["id"]: r for r in references["assets"]}
    if missing := required - available.keys():
        raise ValueError(f"Unresolved source catalog membership: {sorted(missing)}")
    publications = {}
    for name in references["publications"]:
        for record in json.loads((ROOT / name).read_text())["published_files"]:
            extraction = record.get("extraction_path")
            if not extraction and record["path"].startswith("assets/source/osrs/cache2695/"):
                extraction = record["path"].removeprefix("assets/source/osrs/cache2695/")
            if extraction:
                publications[extraction, record["sha256"]] = ROOT / record["path"]
    files, assets, origin_records = [], {}, []
    directory = destination / "source"
    directory.mkdir(exist_ok=True)
    for index, identifier in enumerate(sorted(required)):
        record = available[identifier]["outputs"][0]
        path = publications.get((record["path"], record["sha256"]))
        if path is None:
            path = source / "extracted" / record["path"]
        data = path.read_bytes()
        if sha(data) != record["sha256"] or len(data) != record["size_bytes"]:
            raise ValueError(f"Changed actual source payload: {identifier}")
        # Gzip is carried as binary, not mislabeled as decoded JSON.
        target = f"source/{index:x}.bin"
        url = f"/assets/{index:x}"
        (destination / target).write_bytes(data)
        files.append({"url": url, "path": target, "sha256": sha(data), "content_type": "application/octet-stream"})
        assets[identifier] = url
        origin_records.append({"id": identifier, "url": url, "source_path": record["path"],
                               "sha256": sha(data), "encoding": "gzip" if data[:2] == b"\x1f\x8b" else "identity"})
    public = {"schema_version": 1, "content_revision": definition["revision"], "source_cache": 2695,
              "artifact_sha256": sha(artifact), "assets": origin_records}
    public_bytes = encode(public)
    (destination / "manifest.json").write_bytes(public_bytes)
    files.append({"url": "/content/manifest.json", "path": "manifest.json", "sha256": sha(public_bytes),
                  "content_type": "application/json"})
    descriptor = {
        "schema_version": 1, "world_id": existing.get("world_id", str(uuid.uuid4())),
        "artifact": "world.csc", "sha256": sha(artifact),
        "content_manifest_path": "/content/manifest.json", "assets": assets,
        "readiness_profile": {
            "id": "ordinary_normal_f2p",
            "excluded_items": ["item.ensouled_goblin_head", "item.milk.bottomless_bucket"],
        },
    }
    encoded_descriptor = encode(descriptor)
    descriptor_path.write_bytes(encoded_descriptor)
    (destination / "clubscape-game-assets.json").write_bytes(encode({"schema_version": 1, "files": files}))
    penguin_collection = ROOT / "assets/source/osrs/cache2695/collections/npc.json.gz"
    penguin = json.loads(gzip.decompress(penguin_collection.read_bytes()))[
        "asset.source.osrs.cache2695.npc.2063"]
    catalog = {
        "schema_version": 1, "cache": 2695, "content_revision": definition["revision"],
        "penguin": penguin,
        "penguin_source": {"path": str(penguin_collection.relative_to(ROOT)),
                           "sha256": sha(penguin_collection.read_bytes())},
        "developer_preflight_initial_tile": definition["initial_state"]["tile"],
        "items": {key: int(value["asset"].rsplit(".", 1)[1]) for key, value in definition["items"].items()},
        "npcs": {key: int(value["asset"].rsplit(".", 1)[1]) for key, value in definition["npcs"].items()},
    }
    catalog_path.write_bytes(encode(catalog))
    lower_bound = len(encode(dict.fromkeys(required, "/assets/")))
    report = {
        "schema_version": 1, "kind": "unchanged_source_pack",
        "artifact_sha256": sha(artifact), "content_revision": definition["revision"],
        "referenced_source_assets": len(required), "source_bytes": sum(
            (destination / r["path"]).stat().st_size for r in files),
        "descriptor_bytes": len(encoded_descriptor),
        "descriptor_sha256": sha(encoded_descriptor),
        "strict_server_descriptor_limit_bytes_at_base": 256 * 1024,
        "minimum_assets_object_bytes_even_if_every_asset_shared_shortest_legal_url": lower_bound,
        "fits_base_server_descriptor_limit": len(encoded_descriptor) <= 256 * 1024,
        "source_content_changed": False, "server_guards_changed": False,
        "game_root": str(destination.relative_to(ROOT)),
        "adapter_catalog": str(catalog_path.relative_to(ROOT)),
        "compatibility_verified": False,
    }
    report_path.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=LOCAL / "game")
    parser.add_argument("--source", type=Path, default=SOURCE)
    parser.add_argument("--report", type=Path, default=ROOT / "research/runelite-feasibility/source-pack.json")
    parser.add_argument("--catalog", type=Path, default=LOCAL / "catalog.json")
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(LOCAL):
        parser.error("Game pack output must stay in the owned ignored artifacts directory.")
    report = args.report.resolve()
    catalog = args.catalog.resolve()
    if not report.is_relative_to(ROOT / "research/runelite-feasibility") or not catalog.is_relative_to(LOCAL):
        parser.error("Reports and catalogs must stay in their owned research/artifact directories.")
    package(output, args.source.resolve(), report_path=report, catalog_path=catalog)

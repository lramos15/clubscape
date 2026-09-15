#!/usr/bin/env python3
"""Publish only requested real source definitions; never game state or presentation substitutes."""
import gzip
import hashlib
import io
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
MAX_BYTES = 64 * 1024 * 1024
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "tools/cache-import"))
from content_closure import load_published_inputs, publication_chain  # noqa: E402


def digest(data):
    return hashlib.sha256(data).hexdigest()


def checked(path, expected):
    path = (ROOT / path).resolve()
    if not path.is_relative_to(ROOT) or not path.is_file() or path.stat().st_size > MAX_BYTES:
        raise ValueError("Source publication file must be bounded and inside the worktree.")
    data = path.read_bytes()
    if len(data) != expected["size_bytes"] or digest(data) != expected["sha256"]:
        raise ValueError("Source publication bytes changed.")
    return data


def main():
    data = sys.stdin.buffer.read(2 * 1024 * 1024 + 1)
    if len(data) > 2 * 1024 * 1024:
        raise ValueError("Source selection exceeds its byte budget.")
    selection = json.loads(data)
    identifiers = selection["assetIds"]
    if not isinstance(identifiers, list) or len(identifiers) > 20_000 or len(set(identifiers)) != len(identifiers):
        raise ValueError("Invalid source asset selection.")
    output = Path(selection["directory"]).resolve()
    if output == ROOT or not output.is_relative_to(ROOT):
        raise ValueError("Source delivery must stay inside the worktree.")
    destination = output / "assets/source-definitions"
    destination.mkdir(parents=True, exist_ok=False)

    publication = ROOT / "assets/manifests/osrs/cache2695-consumables-published.json"
    bundle, collections = load_published_inputs(publication)
    records = {record["asset_id"]: record for record in bundle["records"]}
    published = {}
    collection_sources = {}
    for kind in ("item", "npc", "object"):
        collection_sources[kind] = {
            identifier: ROOT / f"assets/source/osrs/cache2695/collections/{kind}.json.gz"
            for identifier in collections[kind]
        }
    for _, manifest in publication_chain(publication):
        for record in manifest["published_files"]:
            key = record.get("source_asset_id")
            if key:
                published.setdefault(key, []).append(record)
        for kind, record in manifest.get("collection_extensions", {}).items():
            if kind in collection_sources:
                shard = ROOT / record["path"]
                for identifier in json.loads(gzip.decompress(checked(record["path"], record))):
                    collection_sources[kind][identifier] = shard

    assets, files, provenance, actions = [], [], [], {}
    total = 0
    source_hashes = {}
    for ordinal, identifier in enumerate(sorted(identifiers)):
        record = records.get(identifier)
        if record is None or record["kind"] not in ("item", "npc", "object", "region"):
            raise ValueError(f"No original definition delivery for {identifier}; no replacement bytes were made.")
        kind = record["kind"]
        if kind == "region":
            original = next((entry for entry in record["outputs"] if entry["path"].endswith(".json.gz")), None)
            emitted = next((entry for entry in published.get(identifier, [])
                            if original and entry["path"].endswith("/" + original["path"])), None)
            if original is None or emitted is None:
                raise ValueError(f"Published original region data is missing: {identifier}.")
            compressed = checked(emitted["path"], emitted)
            maximum = original["json_size_bytes"]
            if maximum > MAX_BYTES:
                raise ValueError("Original region exceeds its decoded byte budget.")
            with gzip.GzipFile(fileobj=io.BytesIO(compressed)) as stream:
                body = stream.read(maximum + 1)
            if len(body) != maximum or digest(body) != original["json_sha256"]:
                raise ValueError("Original region JSON identity changed during decompression.")
            source = {"path": emitted["path"], "sha256": emitted["sha256"], "transformation": "lossless_gzip_decode"}
        else:
            definition = collections[kind].get(identifier)
            if not isinstance(definition, dict) or str(definition.get("id")) != identifier.rsplit(".", 1)[1]:
                raise ValueError("Original source definition/identifier mismatch.")
            # Collection members are original decoded definitions; no field is stripped or rewritten.
            body = (json.dumps(definition, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n").encode()
            shard = collection_sources[kind][identifier]
            relative = str(shard.relative_to(ROOT))
            if relative not in source_hashes:
                source_hashes[relative] = digest(shard.read_bytes())
            source = {"path": relative, "sha256": source_hashes[relative],
                      "selector": identifier, "transformation": "canonical_json_collection_member"}
            if kind == "item":
                options = definition.get("interfaceOptions")
                if isinstance(options, list) and all(option is None or isinstance(option, str) for option in options):
                    actions[identifier] = [option for option in options if option]
        total += len(body)
        if len(body) > MAX_BYTES or total > 512 * 1024 * 1024:
            raise ValueError("Selected original source definitions exceed the deployment budget.")
        filename = f"{ordinal}.json"
        target = destination / filename
        target.write_bytes(body)
        route = f"/assets/source-definitions/{filename}"
        sha = digest(body)
        assets.append({"id": identifier, "url": route, "sha256": sha, "bytes": len(body), "contentType": "application/json"})
        files.append({"url": route, "path": route[1:], "sha256": sha, "content_type": "application/json"})
        provenance.append({"id": identifier, "sha256": sha, "source": source})
    (output / "source-provenance.json").write_text(json.dumps({
        "kind": "original_source_definition_delivery", "assets": provenance,
        "geometryChanged": False, "presentationComplete": False,
    }, indent=2) + "\n")
    print(json.dumps({"assets": assets, "files": files, "inventoryActions": actions, "bytes": total}))


if __name__ == "__main__":
    main()

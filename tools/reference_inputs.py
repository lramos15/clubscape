#!/usr/bin/env python3
"""Retrieve or verify only explicitly hash-pinned OSRS source index inputs."""

import argparse
import hashlib
import json
import os
import re
import sys
import tempfile
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "research/first-slice-sources.json"
MAX_INPUT_BYTES = 16 * 1024 * 1024


class NoRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, file, code, message, headers, new_url):
        raise urllib.error.HTTPError(request.full_url, code, "Unexpected source redirect", headers, file)


def input_path(root, item, namespace="research/inputs/"):
    name = item.get("path")
    if not isinstance(name, str) or not name.startswith(namespace) or Path(name).is_absolute():
        raise ValueError(f"Source inputs must use relative {namespace} paths.")
    path = root / name
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError("Source input path escapes the repository.")
    current = path
    while current != root:
        if current.is_symlink():
            raise ValueError("Source input paths cannot traverse symlinks.")
        current = current.parent
    return path


def validate_manifest(manifest, root):
    if not isinstance(manifest, dict) or manifest.get("schema_version") != 1:
        raise ValueError("Unsupported source manifest schema.")
    cache = manifest.get("cache", {})
    cache_id = cache.get("id") if isinstance(cache, dict) else None
    if type(cache_id) is not int or cache_id <= 0:
        raise ValueError("Source manifest requires an identifiable cache snapshot.")
    inputs = manifest.get("inputs")
    if not isinstance(inputs, list) or not inputs:
        raise ValueError("An empty source inventory is not input coverage.")
    ids, paths = set(), set()
    for item in inputs:
        if not isinstance(item, dict) or not isinstance(item.get("id"), str) or not item["id"]:
            raise ValueError("Every input requires a stable ID.")
        path = input_path(root, item)
        if item["id"] in ids or path in paths:
            raise ValueError("Source input IDs and paths must be unique.")
        ids.add(item["id"])
        paths.add(path)
        if type(item.get("size_bytes")) is not int or not 1 <= item["size_bytes"] <= MAX_INPUT_BYTES:
            raise ValueError("Source input size is missing or exceeds the bounded retrieval budget.")
        if not isinstance(item.get("sha256"), str) or not re.fullmatch(r"[0-9a-f]{64}", item["sha256"]):
            raise ValueError("Source inputs require pre-established SHA-256 hashes.")
        for field in ("archive", "group"):
            if type(item.get(field)) is not int or item[field] < 0:
                raise ValueError("Source archive/group identities must be nonnegative integers.")
        expected_url = (
            f"https://archive.openrs2.org/caches/runescape/{cache_id}"
            f"/archives/{item['archive']}/groups/{item['group']}.dat"
        )
        if item.get("url") != expected_url:
            raise ValueError("Source URL must identify the exact pinned OpenRS2 archive/group.")
    return inputs


def validate_bytes(item, data):
    if len(data) != item["size_bytes"] or hashlib.sha256(data).hexdigest() != item["sha256"]:
        raise ValueError(f"Source input size/hash mismatch: {item['id']}; refusing to replace the pinned identity.")


def download(item):
    opener = urllib.request.build_opener(NoRedirects)
    request = urllib.request.Request(item["url"], headers={"User-Agent": "ClubScape-pinned-source-inputs/1"})
    with opener.open(request, timeout=30) as response:
        if response.status != 200:
            raise ValueError(f"Source input HTTP status {response.status}: {item['id']}")
        return response.read(item["size_bytes"] + 1)


def fetch_one(root, item, fetch=download, namespace="research/inputs/"):
    path = input_path(root, item, namespace)
    if path.exists():
        if not path.is_file() or path.stat().st_size != item["size_bytes"]:
            raise ValueError(f"Existing source input has changed: {item['id']}")
        validate_bytes(item, path.read_bytes())
        return False
    data = fetch(item)
    validate_bytes(item, data)
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent, prefix="source-", delete=False) as temporary:
        temporary_path = Path(temporary.name)
        try:
            temporary.write(data)
            temporary.flush()
            os.fsync(temporary.fileno())
            os.link(temporary_path, path)
        finally:
            temporary_path.unlink(missing_ok=True)
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["fetch", "verify"])
    args = parser.parse_args()
    try:
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        inputs = validate_manifest(manifest, ROOT)
        fetched = 0
        for item in inputs:
            if args.operation == "fetch":
                fetched += int(fetch_one(ROOT, item))
            else:
                path = input_path(ROOT, item)
                if not path.is_file() or path.stat().st_size != item["size_bytes"]:
                    raise ValueError(f"Missing or changed input {item['id']}; run just reference-fetch.")
                validate_bytes(item, path.read_bytes())
        print(json.dumps({
            "baseline": manifest["baseline"], "input_integrity": "verified",
            "verified_inputs": len(inputs), "downloaded_inputs": fetched,
            "full_inventory_verified": False, "presentation_accepted": False,
        }))
        return 0
    except (OSError, ValueError, urllib.error.URLError) as error:
        print(f"Source input error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())

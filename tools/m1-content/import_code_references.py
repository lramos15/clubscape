#!/usr/bin/env python3
"""Pin the source interface identifiers and inspectable collision algorithm references."""

import hashlib
import json
from pathlib import Path
import re
import urllib.request


ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "research/m1-bindings"
REVISION = "ac79ed8bd8926bec7bf172aa291574b4d944b0e7"
COLLISION_REVISION = "915fb55c0a2a1c000dc04a431e68f3bbb29a40cf"
URLS = {
    "interfaces": f"https://raw.githubusercontent.com/runelite/runelite/{REVISION}/runelite-api/src/main/java/net/runelite/api/gameval/InterfaceID.java",
    "collision_flags": f"https://raw.githubusercontent.com/runelite/runelite/{REVISION}/runelite-api/src/main/java/net/runelite/api/CollisionDataFlag.java",
    "collision_algorithm": f"https://raw.githubusercontent.com/open-osrs/runelite/{COLLISION_REVISION}/runescape-client/src/main/java/CollisionMap.java",
    "item_loader": f"https://raw.githubusercontent.com/runelite/runelite/{REVISION}/cache/src/main/java/net/runelite/cache/definitions/loaders/ItemLoader.java",
}


def main():
    destination = OUT / ".local/code"
    destination.mkdir(parents=True, exist_ok=True)
    manifest_path = OUT / "code-sources.json"
    previous = json.loads(manifest_path.read_text()) if manifest_path.exists() else {}
    records, interface_ids = [], {}
    for name, url in URLS.items():
        path = destination / f"{name}.java"
        if not path.exists():
            with urllib.request.urlopen(url, timeout=90) as response:
                path.write_bytes(response.read())
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        expected = next((r for r in previous.get("sources", []) if r["id"] == name), None)
        if expected and digest != expected["sha256"]:
            raise ValueError(f"Reference hash mismatch: {name}")
        records.append({"id": name, "url": url, "sha256": digest, "size_bytes": len(data)})
        if name == "interfaces":
            outer = data.decode().split("public static final class", 1)[0]
            interface_ids = {key: int(value) for key, value in re.findall(
                r"public static final int\s+([A-Z][A-Z0-9_]*)\s*=\s*(\d+)\s*;", outer)}
    output = {
        "schema_version": 1,
        "sources": records,
        "interface_group_ids": interface_ids,
        "collision_status": "inference",
        "collision_note": "Directional bits are from the pinned current RuneLite API. Wall/object "
                          "clipping is reconstructed from the published older source algorithm and "
                          "current decoded clipping fields. It is not an observed build-240 live "
                          "collision dump. Dynamic morph, door and instance state remains explicit.",
    }
    manifest_path.write_text(json.dumps(output, sort_keys=True, indent=2) + "\n")
    print(json.dumps({"interface_group_ids": len(interface_ids), "pinned_sources": len(records)}))


if __name__ == "__main__":
    main()

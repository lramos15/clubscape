#!/usr/bin/env python3
"""Acquire bounded public evidence into this task's owned, ignored directory."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import gzip
import urllib.parse
import urllib.request


ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "research/runtime-bindings"
RAW = OUT / ".local/raw"
MANIFEST = OUT / "sources.json"
API = "https://oldschool.runescape.wiki/api.php"
PAGES = [
    "Game tick", "Game tick/Action lengths", "Pathfinding", "Bread", "Shrimps",
    "Cooking", "Grave", "Death", "Death's Office", "Death/Item Recovery Fees",
    "Transcript:Death (NPC)", "Items Kept on Death", "Hitpoints", "Prayer",
    "Energy", "Experience", "Goblin", "Giant rat (Tutorial Island)",
    "Chicken (Tutorial Island)", "Chicken", "Giant rat", "Tutorial Island",
    "Learning the Ropes", "Transcript:Learning the Ropes", "Lumbridge Home Teleport",
    "Shop", "General store", "Module:Shop calculator", "Arrows", "Fire",
    "Tutorial Island bank", "Items", "Item spawn", "Projectile", "Hitsplat",
    "Auto Retaliate", "Settings", "Items Kept on Death (Interface)", "Coins",
    "Fishing spot (small net, bait)",
    "Hit delay", "Energy potion", "Combat", "Temporary skill boost",
    "Drop", "Skills", "Update:Summer Sweep Up - Hunter & Skilling",
    "Value", "High Level Alchemy", "Ashes", "Drops",
    "Money making guide/Collecting ashes",
    "Wind Strike", "Update:Death Changes", "Update:New Player Improvements Round 2",
    "Update:Grid Master Rewards, Poll & New Player Improvements",
]
GITHUB_FILES = {
    "rsmod/rsmod": [
        "api/shops/src/main/kotlin/org/rsmod/api/shops/cost/StandardGpCostCalculations.kt",
        "api/shops/src/main/kotlin/org/rsmod/api/shops/restock/ShopRestockProcess.kt",
        "api/shops/src/main/kotlin/org/rsmod/api/shops/restock/ShopRestockScript.kt",
        "api/shops/src/test/kotlin/org/rsmod/api/shops/cost/StandardGpCostCalculationBuyTest.kt",
        "api/death/src/main/kotlin/org/rsmod/api/death/PlayerDeath.kt",
        "api/death-plugin/src/main/kotlin/org/rsmod/api/death/plugin/PlayerDeathScript.kt",
        "api/player/src/main/kotlin/org/rsmod/api/player/stat/PlayerSkillXP.kt",
        "api/player/src/integration/kotlin/org/rsmod/api/player/stat/PlayerSkillXPTest.kt",
        "api/player/src/integration/kotlin/org/rsmod/api/player/stat/StatOperationsTest.kt",
        "engine/game/src/main/kotlin/org/rsmod/game/movement/MoveSpeed.kt",
        "api/game-process/src/main/kotlin/org/rsmod/api/game/process/npc/NpcMovementProcessor.kt",
        "api/game-process/src/main/kotlin/org/rsmod/api/game/process/npc/mode/NpcWanderModeProcessor.kt",
        "content/generic/generic-locs/src/main/kotlin/org/rsmod/content/generic/locs/ladders/LadderScript.kt",
        "content/generic/generic-locs/src/main/kotlin/org/rsmod/content/generic/locs/ladders/DungeonLadderScript.kt",
        "content/generic/generic-locs/src/main/kotlin/org/rsmod/content/generic/locs/staircase/SpiralStaircaseScript.kt",
        "content/other/windmill/src/main/kotlin/org/rsmod/content/other/windmill/WindmillLadderScript.kt",
    ],
    "weirdgloop/osrs-dps-calc": [
        "src/lib/NPCVsPlayerCalc.ts", "src/lib/HitDist.ts",
    ],
    "runelite/runelite": [
        "runelite-client/src/main/java/net/runelite/client/game/ItemClient.java",
        "runelite-client/src/main/java/net/runelite/client/game/ItemManager.java",
    ],
}


def request(url):
    req = urllib.request.Request(url, headers={"User-Agent": "ClubScape-public-source-binding-research/1.0"})
    with urllib.request.urlopen(req, timeout=90) as response:
        return response.read(), dict(response.headers)


def read_manifest():
    if MANIFEST.exists():
        return json.loads(MANIFEST.read_text())
    return {"schema_version": 1, "sources": []}


def record(identifier, url, data, kind, **metadata):
    RAW.mkdir(parents=True, exist_ok=True)
    manifest = read_manifest()
    path = RAW / (identifier + (".wikitext" if kind == "wiki_revision" else ".data"))
    value = {
        "id": identifier, "kind": kind, "url": url,
        "retrieved_at": datetime.now(timezone.utc).isoformat(),
        "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data),
        "local_raw": str(path.relative_to(OUT)), **metadata,
    }
    previous = next((s for s in manifest["sources"] if s["id"] == identifier), None)
    if previous:
        if previous["sha256"] != value["sha256"]:
            raise ValueError(f"Refusing to overwrite a different pinned snapshot: {identifier}")
        path.write_bytes(data)
        return previous
    path.write_bytes(data)
    manifest["sources"].append(value)
    MANIFEST.write_text(json.dumps(manifest, indent=2) + "\n")
    return value


def wiki():
    previous = json.loads((ROOT / "research/journey-rules/sources.json").read_text())
    pins = {s["page"]: s for s in previous["sources"] if s["kind"] == "wiki_revision"}
    recorded = {s.get("page"): s for s in read_manifest()["sources"] if s["kind"] == "wiki_revision"}
    requests = [p for p in PAGES if p not in recorded]
    for pinned in (True, False):
        subset = [p for p in requests if (p in pins) == pinned]
        for offset in range(0, len(subset), 25):
            titles = subset[offset:offset + 25]
            if not titles:
                continue
            params = {
                "action": "query", "format": "json", "formatversion": 2,
                "prop": "revisions", "rvprop": "ids|timestamp|content", "rvslots": "main",
                ("revids" if pinned else "titles"):
                    "|".join(str(pins[p]["revision"]) if pinned else p for p in titles),
            }
            api_url = API + "?" + urllib.parse.urlencode(params)
            payload, _ = request(api_url)
            response = json.loads(payload)
            if "error" in response or "continue" in response:
                raise RuntimeError(response.get("error", response.get("continue")))
            for page in response["query"]["pages"]:
                if "missing" in page or not page.get("revisions"):
                    print("Missing page (not evidence):", page["title"])
                    continue
                rev = page["revisions"][0]
                data = rev["slots"]["main"]["content"].encode()
                prior = pins.get(page["title"])
                if prior:
                    assert hashlib.sha256(data).hexdigest() == prior["sha256_utf8_wikitext"], page["title"]
                url = "https://oldschool.runescape.wiki/w/" + urllib.parse.quote(page["title"].replace(" ", "_"), safe=":/") + "?oldid=" + str(rev["revid"])
                record("wiki." + str(rev["revid"]), url, data, "wiki_revision",
                       page=page["title"], revision=rev["revid"], revision_timestamp=rev["timestamp"],
                       identity="Exact API revision UTF-8 wikitext; no added newline",
                       prior_snapshot_ref="research/journey-rules/sources.json#" + prior["id"] if prior else None)
    print("Recorded wiki revisions:", sum(s["kind"] == "wiki_revision" for s in read_manifest()["sources"]))


def github():
    index = json.loads((OUT / ".local/repo-index.json").read_text())
    index["runelite/runelite"] = {"commit": "ac79ed8bd8926bec7bf172aa291574b4d944b0e7"}
    existing = {s["id"] for s in read_manifest()["sources"]}
    for repo, paths in GITHUB_FILES.items():
        pin = index[repo]["commit"]
        for path in paths:
            identifier = "code." + repo.split("/")[0] + "." + Path(path).stem
            if identifier in existing:
                continue
            url = f"https://raw.githubusercontent.com/{repo}/{pin}/{path}"
            data, _ = request(url)
            record(identifier, url, data, "public_code", repository=repo, path=path,
                   revision=pin, revision_timestamp=index[repo].get("commit_date"),
                   applicability="Community source/calculator, not Jagex server code or a live observation.")
    print("Recorded public code snapshots:", sum(s["kind"] == "public_code" for s in read_manifest()["sources"]))


def restore():
    manifest = read_manifest()
    RAW.mkdir(parents=True, exist_ok=True)
    revisions = [s for s in manifest["sources"] if s["kind"] == "wiki_revision"]
    for offset in range(0, len(revisions), 25):
        chunk = revisions[offset:offset + 25]
        params = {"action": "query", "format": "json", "formatversion": 2,
                  "prop": "revisions", "rvprop": "ids|content", "rvslots": "main",
                  "revids": "|".join(str(s["revision"]) for s in chunk)}
        payload, _ = request(API + "?" + urllib.parse.urlencode(params))
        response = json.loads(payload)
        if "error" in response or "continue" in response:
            raise RuntimeError(response.get("error", response.get("continue")))
        by_revision = {p["revisions"][0]["revid"]: p["revisions"][0]["slots"]["main"]["content"].encode()
                       for p in response["query"]["pages"]}
        for source in chunk:
            data = by_revision[source["revision"]]
            if hashlib.sha256(data).hexdigest() != source["sha256"]:
                raise ValueError("Pinned revision hash mismatch: " + source["id"])
            (RAW / (source["id"] + ".wikitext")).write_bytes(data)
    for source in manifest["sources"]:
        if source["kind"] == "wiki_revision":
            continue
        if source["kind"] == "public_price_feed":
            data = gzip.decompress((OUT / "inputs/guide-prices.json.gz").read_bytes())
        else:
            data, _ = request(source["url"])
        if hashlib.sha256(data).hexdigest() != source["sha256"]:
            raise ValueError("Pinned source hash mismatch: " + source["id"])
        (RAW / (source["id"] + ".data")).write_bytes(data)
    print("Restored and hash-verified exact snapshots:", len(manifest["sources"]))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wiki", action="store_true")
    parser.add_argument("--github", action="store_true")
    parser.add_argument("--restore", action="store_true", help="Restore committed immutable pins and retained price bytes; never refresh prices.")
    parser.add_argument("--url")
    parser.add_argument("--id")
    parser.add_argument("--kind", default="public_snapshot")
    parser.add_argument("--revision")
    args = parser.parse_args()
    if args.wiki:
        wiki()
    if args.github:
        github()
    if args.restore:
        restore()
    if args.url:
        if not args.id:
            parser.error("--url requires --id")
        data, headers = request(args.url)
        value = record(args.id, args.url, data, args.kind, revision=args.revision,
                       http_last_modified=headers.get("Last-Modified"), http_date=headers.get("Date"))
        print(json.dumps({k: value[k] for k in ["id", "bytes", "sha256"]}))


if __name__ == "__main__":
    main()

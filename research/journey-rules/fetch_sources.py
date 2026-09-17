#!/usr/bin/env python3
"""Reproduce public wiki snapshots without credentials or third-party packages."""

import argparse
import hashlib
import json
from pathlib import Path
import urllib.parse
import urllib.request


ROOT = Path(__file__).resolve().parent
API = "https://oldschool.runescape.wiki/api.php"
PINNED = {
    "skills": ("Skills", 15321845),
    "hitpoints": ("Hitpoints", 15337461),
    "learning_the_ropes": ("Learning the Ropes", 15315138),
    "tutorial_transcript": ("Transcript:Learning the Ropes", 15309360),
    "tutorial_island": ("Tutorial Island", 15275369),
    "cooks_assistant": ("Cook's Assistant", 15320469),
    "cooks_transcript": ("Transcript:Cook's Assistant", 15263168),
    "cooks_journal": ("Transcript:Cook's Assistant/Journal", 15105447),
    "bronze_pickaxe": ("Bronze pickaxe", 15182812),
    "copper_rocks": ("Copper rocks", 15209140),
    "grave": ("Grave", 15272680),
    "deaths_office": ("Death's Office", 15273706),
}
DISCOVERY = {
    "experience": "Experience",
    "game_tick": "Game tick",
    "run": "Run",
    "inventory": "Inventory",
    "equipment": "Worn Equipment",
    "mining": "Mining",
    "woodcutting": "Woodcutting",
    "fishing": "Fishing",
    "firemaking": "Firemaking",
    "cooking": "Cooking",
    "smelting": "Smelting",
    "smithing": "Smithing",
    "melee": "Melee",
    "ranged": "Ranged",
    "magic": "Magic",
    "wind_strike": "Wind Strike",
    "home_teleport": "Lumbridge Home Teleport",
    "bank": "Bank",
    "shop": "Shop",
    "lumbridge_general_store": "Lumbridge General Store",
    "death": "Death",
    "death_npc": "Death (NPC)",
    "items_kept_on_death": "Items Kept on Death",
    "respawn": "Spawning",
    "prayer": "Prayer",
    "bones": "Bones",
    "mill_lane_mill": "Mill Lane Mill",
    "bucket_of_milk": "Bucket of milk",
    "pot_of_flour": "Pot of flour",
    "egg": "Egg",
    "grain": "Grain",
    "small_fishing_net": "Small fishing net",
    "raw_shrimps": "Raw shrimps",
    "shrimps": "Shrimps",
    "bread": "Bread",
    "bronze_dagger": "Bronze dagger",
    "bronze_sword": "Bronze sword",
    "shortbow": "Shortbow",
    "bronze_arrow": "Bronze arrow",
    "goblin": "Goblin",
    "giant_rat": "Giant rat",
    "tutorial_rat": "Giant rat (Tutorial Island)",
    "chicken": "Chicken",
    "adventure_paths": "Adventure Paths",
    "energy": "Energy",
    "skilling_success_rate": "Skilling success rate",
    "melee_dps": "Damage per second/Melee",
    "ranged_dps": "Damage per second/Ranged",
    "magic_dps": "Damage per second/Magic",
    "tree": "Tree",
    "bronze_axe": "Bronze axe",
    "bronze_bar": "Bronze bar",
    "bread_dough": "Bread dough",
    "death_dialogue": "Transcript:Death (NPC)",
    "death_fees": "Death/Item Recovery Fees",
    "pot": "Pot",
    "bucket": "Bucket",
    "wooden_shield": "Wooden shield",
    "tutorial_chicken": "Chicken (Tutorial Island)",
    "hopper": "Hopper",
    "flour_bin": "Flour bin",
    "tinderbox": "Tinderbox",
    "hammer": "Hammer",
    "lumbridge_castle": "Lumbridge Castle",
    "pathfinding": "Pathfinding",
    "accuracy": "Accuracy",
    "maximum_ranged_hit": "Maximum ranged hit",
    "tin_rocks": "Tin rocks",
    "tutorial_bones": "Bones (Tutorial Island)",
    "action_lengths": "Game tick/Action lengths",
    "shop_calculator": "Calculator:Shop calculator",
    "stackable_items": "Stackable items",
    "cook_lumbridge": "Cook (Lumbridge)",
    "combat_options": "Combat Options",
    "combat_spells": "Combat spells",
    "fire": "Fire",
    "shop_module": "Module:Shop calculator",
    "hopper_transcript": "Transcript:Hopper",
    "store_line_docs": "Template:StoreLine/doc",
    "monster_infobox_docs": "Template:Infobox Monster/doc",
    "food": "Food",
    "arrow": "Arrow",
    "arrows": "Arrows",
    "item": "Item",
}


def fetch(params):
    url = API + "?" + urllib.parse.urlencode({
        "action": "query", "format": "json", "formatversion": 2,
        "prop": "revisions", "rvprop": "ids|timestamp|content",
        "rvslots": "main", **params,
    })
    req = urllib.request.Request(
        url, headers={"User-Agent": "ClubScapeSourceContract/1.0 (public research)"}
    )
    with urllib.request.urlopen(req, timeout=90) as response:
        result = json.load(response)
    if "error" in result or "continue" in result:
        raise RuntimeError(json.dumps(result.get("error", result.get("continue"))))
    return result["query"]["pages"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--discover", action="store_true",
                        help="First acquisition only; freezes explicitly listed slice pages.")
    args = parser.parse_args()
    manifest_path = ROOT / "sources.json"
    snapshots = ROOT / ".local" / "snapshots"
    snapshots.mkdir(parents=True, exist_ok=True)
    cache_path = ROOT / ".local" / "api-pages.json"
    cache = json.loads(cache_path.read_text()) if cache_path.exists() else []
    if args.discover:
        requests = {k: {"page": v[0], "oldid": v[1]} for k, v in PINNED.items()}
        requests.update({k: {"page": v} for k, v in DISCOVERY.items()})
    else:
        manifest = json.loads(manifest_path.read_text())
        requests = {s["id"].removeprefix("source.wiki."):
                    {"page": s["page"], "oldid": s["revision"]}
                    for s in manifest["sources"] if s["kind"] == "wiki_revision"}
    results = {}
    for pinned in (True, False):
        subset = [(key, value) for key, value in requests.items()
                  if ("oldid" in value) == pinned]
        for offset in range(0, len(subset), 40):
            chunk = subset[offset:offset + 40]
            field = "revids" if pinned else "titles"
            values = [str(v["oldid"] if pinned else v["page"]) for _, v in chunk]
            cached_keys = {
                str(p["revisions"][0]["revid"]) if pinned else p["title"]: p
                for p in cache if p.get("revisions")
            }
            missing = [v for v in values if v not in cached_keys]
            if missing:
                cache.extend(fetch({field: "|".join(missing)}))
                cache_path.write_text(json.dumps(cache))
            pages = cache
            by_revision = {p["revisions"][0]["revid"]: p for p in pages
                           if p.get("revisions")}
            by_title = {p["title"]: p for p in pages}
            for key, request in chunk:
                page = (by_revision.get(request["oldid"]) if pinned
                        else by_title.get(request["page"]))
                if not page or not page.get("revisions"):
                    raise RuntimeError(f"Missing source: {key} ({request})")
                revision = page["revisions"][0]
                content = revision["slots"]["main"]["content"].encode("utf-8")
                (snapshots / f"{key}.wikitext").write_bytes(content)
                results[key] = {
                    "id": f"source.wiki.{key}",
                    "kind": "wiki_revision",
                    "page": page["title"],
                    "revision": revision["revid"],
                    "revision_timestamp": revision["timestamp"],
                    "url": "https://oldschool.runescape.wiki/w/"
                           + urllib.parse.quote(page["title"].replace(" ", "_"), safe=":/")
                           + f"?oldid={revision['revid']}",
                    "sha256_utf8_wikitext": hashlib.sha256(content).hexdigest(),
                    "bytes_utf8_wikitext": len(content),
                }
    if args.discover:
        from datetime import datetime, timezone
        manifest = {
            "schema_version": 1,
            "id": "contract.journey.sources",
            "retrieved_at": datetime.now(timezone.utc).isoformat(),
            "hash_definition": "SHA-256 of API revision slots.main.content encoded UTF-8; no added newline",
            "snapshot_storage": ".local/snapshots (ignored; reproduce with python3 fetch_sources.py)",
            "license": {
                "attribution": "Old School RuneScape Wiki contributors; revision URLs preserve authorship history",
                "url": "https://oldschool.runescape.wiki/w/Old_School_RuneScape_Wiki:Copyrights",
                "note": "Full prose snapshots are local research inputs, not bundled as game dialogue or assets.",
            },
            "sources": list(results.values()),
        }
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    else:
        for source in manifest["sources"]:
            if source["kind"] != "wiki_revision":
                continue
            actual = results[source["id"].removeprefix("source.wiki.")]
            if actual["sha256_utf8_wikitext"] != source["sha256_utf8_wikitext"]:
                raise RuntimeError(f"Snapshot hash mismatch: {source['id']}")
    print(json.dumps({"snapshots": len(results), "hashes_verified": not args.discover}))


if __name__ == "__main__":
    main()

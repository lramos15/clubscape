#!/usr/bin/env python3
"""Acquire only missing M1 identity/position evidence, then replay pinned revisions."""

import argparse
import hashlib
import json
from pathlib import Path
import urllib.parse
import urllib.request


ROOT = Path(__file__).resolve().parents[2]
BINDINGS = ROOT / "research/m1-bindings"
CACHE = BINDINGS / ".local/wiki"
API = "https://oldschool.runescape.wiki/api.php"
EXISTING = (
    "tutorial_island", "tutorial_rat", "tutorial_chicken", "cook_lumbridge",
    "goblin", "deaths_office", "death_npc", "lumbridge_general_store",
    "mill_lane_mill", "bucket_of_milk", "egg", "grain", "pot", "bucket",
    "lumbridge_castle", "hopper", "flour_bin", "tutorial_bones",
    "shrimps", "bread", "bread_dough",
)
ADDITIONAL = (
    "Gielinor Guide", "Survival Expert", "Henja", "Master Chef",
    "Quest Guide", "Mining Instructor", "Combat Instructor", "Account Guide",
    "Brother Brace", "Magic Instructor", "Ironman tutor", "Adventurer Jon",
    "Lumbridge Guide", "Millie Miller", "Gillie Groats", "Dairy cow",
    "Shop keeper (Lumbridge)", "Shop assistant (Lumbridge)", "Fishing spot (Tutorial Island)",
    "Banker", "Death (NPC)",
    "Burnt shrimp", "Burnt bread", "Bottomless bucket of milk",
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def fetch(params):
    url = API + "?" + urllib.parse.urlencode({
        "action": "query", "format": "json", "formatversion": 2,
        "prop": "revisions", "rvprop": "ids|timestamp|content", "rvslots": "main",
        **params,
    })
    request = urllib.request.Request(
        url, headers={"User-Agent": "ClubScapeM1Bindings/1.0 (public source identity research)"}
    )
    with urllib.request.urlopen(request, timeout=90) as response:
        payload = json.load(response)
    if "error" in payload or "continue" in payload:
        raise ValueError(f"Incomplete wiki response: {payload.get('error', payload.get('continue'))}")
    return payload["query"]["pages"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--discover", action="store_true",
                        help="Explicit first acquisition of the bounded additional page list.")
    args = parser.parse_args()
    manifest_path = BINDINGS / "wiki-sources.json"
    CACHE.mkdir(parents=True, exist_ok=True)
    sources = json.loads((ROOT / "research/journey-rules/sources.json").read_text())["sources"]
    expected = {s["revision"]: s for s in sources
                if s["id"].removeprefix("source.wiki.") in EXISTING}
    if manifest_path.exists():
        expected.update({s["revision"]: s for s in json.loads(manifest_path.read_text())["sources"]})
    missing = [revision for revision, source in expected.items()
               if not (CACHE / f"{revision}.wikitext").exists()]
    requests = [{"revids": "|".join(map(str, missing[i:i + 35]))}
                for i in range(0, len(missing), 35)]
    if args.discover:
        known = {s["page"] for s in expected.values()}
        aliases = {"Combat Instructor": "Vannaka", "Bottomless bucket of milk": "Bottomless milk bucket"}
        titles = [title for title in ADDITIONAL if aliases.get(title, title) not in known]
        if titles:
            requests.append({"titles": "|".join(titles), "redirects": 1})
    unresolved = []
    for request in requests:
        for page in fetch(request):
            if not page.get("revisions"):
                unresolved.append(page["title"])
                continue
            revision = page["revisions"][0]
            number = revision["revid"]
            data = revision["slots"]["main"]["content"].encode("utf-8")
            previous = expected.get(number)
            if previous and digest(data) != previous["sha256_utf8_wikitext"]:
                raise ValueError(f"Source revision hash mismatch: {number}")
            (CACHE / f"{number}.wikitext").write_bytes(data)
            expected[number] = previous or {
                "id": "source.wiki.binding." + str(number),
                "kind": "wiki_revision",
                "page": page["title"],
                "revision": number,
                "revision_timestamp": revision["timestamp"],
                "url": "https://oldschool.runescape.wiki/w/"
                       + urllib.parse.quote(page["title"].replace(" ", "_"), safe=":/")
                       + f"?oldid={number}",
                "sha256_utf8_wikitext": digest(data),
                "bytes_utf8_wikitext": len(data),
            }
    for number, source in expected.items():
        data = (CACHE / f"{number}.wikitext").read_bytes()
        if digest(data) != source["sha256_utf8_wikitext"]:
            raise ValueError(f"Cached source revision hash mismatch: {number}")
    manifest = {
        "schema_version": 1,
        "purpose": "Bounded source identity, variant and spawn-position evidence; not source gameplay observations.",
        "license": "Old School RuneScape Wiki contributors; linked revision histories and "
                   "https://oldschool.runescape.wiki/w/Old_School_RuneScape_Wiki:Copyrights",
        "sources": sorted(expected.values(), key=lambda source: source["page"]),
        "unresolved_titles": sorted(unresolved),
    }
    if args.discover or not manifest_path.exists():
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps({"verified_revisions": len(expected), "unresolved_titles": unresolved}))


if __name__ == "__main__":
    main()

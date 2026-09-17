#!/usr/bin/env python3
"""Pin bounded M1 unlock/morph evidence without modifying audio assets or approved references."""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "research/interface-contracts/audio-authority"
DIRECTOR_REVISION = "15184423a8085ec32a6080700ad6ddadd8e808b3"
RUNE_REVISION = "ac79ed8bd8926bec7bf172aa291574b4d944b0e7"
GROUPS = {2, 62, 64, 76, 144, 145, 163, 327}
CUTOFF = "2026-09-08T10:30:08Z"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def fetch(url, maximum):
    request = urllib.request.Request(url, headers={"User-Agent": "ClubScape bounded source-authority research"})
    with urllib.request.urlopen(request, timeout=45) as response:
        data = response.read(maximum + 1)
    if len(data) > maximum:
        raise ValueError("Source response exceeds its explicit bound")
    return data


def write(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--director-root", type=Path, required=True)
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    inputs = []

    def director(path):
        raw = subprocess.check_output(["git", "-C", str(args.director_root), "show",
                                       f"{DIRECTOR_REVISION}:{path}"])
        inputs.append({"path": path, "commit": DIRECTOR_REVISION, "sha256": sha(raw), "bytes": len(raw)})
        return json.loads(raw)

    catalog = director("research/browser-audio-policy/native-catalog.json")
    objects = director("research/browser-audio-policy/native-objects.json")
    geography = director("research/browser-audio-policy/music-geography.json")
    publication = director("assets/manifests/osrs/audio-m1-supplement.json")
    base = json.loads((ROOT / "assets/manifests/osrs/audio-runtime.json").read_text())
    published = {row["source_group"] for row in base["assets"] + publication["assets"] if row["kind"] == "music"}
    if not GROUPS <= published:
        raise ValueError("A required menu group has no real audio publication")
    table = next(c for c in catalog["cases"] if c["case"] == "original-music-table44-and-area-group128")
    tracks = {}
    for row in table["selected_rows"] + table["same_area_rows"]:
        definition = row["definition"]
        values = definition["columnValues"]
        group = values[4][0]
        if group not in GROUPS:
            raise ValueError("Unrelated native track in bounded authority input")
        automatic = values[6] if values[6] is not None else table["table44"]["defaultColumnValues"][6]
        tracks[group] = {
            "group": group, "row": definition["id"], "name": values[1][0], "hint": values[2][0],
            "automatic_unlock": automatic[0], "variable": values[5],
            "area": None if values[7] is None else values[7][0], "source_sha256": row["source_sha256"],
        }
    if set(tracks) != GROUPS or tracks[62]["automatic_unlock"] != 1:
        raise ValueError("Native menu/automatic-unlock scope changed")
    for group in GROUPS - {62, 144}:
        if tracks[group]["hint"] != "in Lumbridge.":
            raise ValueError("Modern area membership is not substitute unlock evidence")
    definitions = next(c for c in objects["cases"] if c["case"] == "native-m1-object-audio-definitions")
    morphs = [{key: d[key] for key in ("id", "varbit", "varp", "transforms", "source_sha256")}
              for d in definitions["definitions"] if d["transforms"] is not None]
    bits = definitions["varbits"]
    if {(b["id"], b["varp"], b["lsb"], b["msb"]) for b in bits} != {(609, 491, 2, 2), (611, 491, 4, 4)}:
        raise ValueError("The calibrated M1 morph-variable scope changed")

    pages = json.loads((ROOT / "research/reference-pack/v1/pages.json").read_text())
    frozen = {}
    for title in ("Autumn Voyage", "Newbie Melody", "Scape Cave", "Music Player"):
        record = next(p for p in pages if p["title"] == title)
        raw = (ROOT / record["snapshot"]["path"]).read_bytes()
        if sha(raw) != record["snapshot"]["sha256"]:
            raise ValueError("An approved source snapshot changed")
        slot = json.loads(gzip.decompress(raw))["page"]["revisions"][0]["slots"]["main"]
        frozen[title] = {"url": record["url"], **record["snapshot"],
                         "text": slot.get("content", slot.get("*"))}

    query = {
        "action": "query", "format": "json", "formatversion": "2", "prop": "revisions",
        "rvslots": "main", "rvprop": "ids|timestamp|content", "rvlimit": "1", "rvstart": CUTOFF,
        "redirects": "1",
    }
    titles = ("Book of Spells", "Dream (music track)", "Flute Salad", "Harmony (music track)",
              "Yesteryear", "Mysterious ruins", "Water tiara", "Fire tiara")
    urls = ["https://oldschool.runescape.wiki/api.php?" + urllib.parse.urlencode(query | {"titles": title})
            for title in titles]
    snapshot = OUT / "pre-freeze-pages.json"
    if not snapshot.exists():
        captured = []
        for url in urls:
            result = json.loads(fetch(url, 250_000))
            if "error" in result:
                raise ValueError(result["error"])
            captured.extend(result["query"]["pages"])
        write(snapshot, {"query": {"pages": captured}})
    raw = snapshot.read_bytes()
    api = json.loads(raw)
    if "error" in api:
        raise ValueError(api["error"])
    fetched = {}
    for page in api["query"]["pages"]:
        revision = page["revisions"][0]
        if revision["timestamp"] > CUTOFF:
            raise ValueError("A source page is later than the selected game release")
        fetched[page["title"]] = {
            "url": "https://oldschool.runescape.wiki/w/" + urllib.parse.quote(page["title"].replace(" ", "_"))
                   + "?oldid=" + str(revision["revid"]),
            "revision": revision["revid"], "timestamp": revision["timestamp"],
            "text": revision["slots"]["main"]["content"],
        }
    inputs.append({"path": str(snapshot.relative_to(ROOT)), "urls": urls,
                   "sha256": sha(raw), "bytes": len(raw), "cutoff": CUTOFF})

    code = {}
    for name, required, expected in [
        ("VarPlayerID.java", {491}, "35cef2039d1ad526ba990f029e1c28f6d80c2079365c16f6214f4312140d5ab4"),
        ("VarbitID.java", {609, 611}, "2c4d64a79856aa16d29bd5f869f91b62be09de1dfa3819bec8f7fe094b037d94"),
    ]:
        url = f"https://raw.githubusercontent.com/runelite/runelite/{RUNE_REVISION}/runelite-api/src/main/java/net/runelite/api/gameval/{name}"
        raw = fetch(url, 2_000_000)
        if sha(raw) != expected:
            raise ValueError("Pinned original variable symbols changed")
        symbols = {int(value): symbol for symbol, value in re.findall(
            r"public static final int\s+(\w+)\s*=\s*(\d+);", raw.decode()) if int(value) in required}
        if set(symbols) != required:
            raise ValueError("Required original variable identity is missing")
        code[name] = {"url": url, "sha256": expected, "bytes": len(raw), "symbols": symbols}
    result = {
        "schema_version": 1, "cache_id": 2695, "menu_groups": sorted(GROUPS), "title_group_excluded": 0,
        "inputs": inputs, "tracks": tracks, "music_table_defaults": table["table44"]["defaultColumnValues"],
        "geography": geography, "morph_objects": morphs, "varbits": bits, "variable_symbols": code,
        "frozen_pages": frozen, "supplemental_pages": fetched,
        "qualification": "Exact native table/definition evidence and dated public references. Unlock trigger predicates are a separately audited source inference, never inferred from Modern playlist membership.",
        "source_assets_modified": False, "milestone_accepted": False,
    }
    path = OUT / "inputs.json"
    write(path, result)
    print(json.dumps({"menu_groups": sorted(tracks), "source_pages": len(frozen) + len(fetched),
                      "morphs": len(morphs), "varp": 491, "known_bits": 20, "inputs_sha256": sha(path.read_bytes())}))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Join the existing pinned music maps with one identified pre-freeze public map revision."""

import gzip
import hashlib
import json
from pathlib import Path
import re
import urllib.request

ROOT = Path(__file__).resolve().parents[3]
OUT = Path("research/browser-audio-policy")
URL = ("https://oldschool.runescape.wiki/api.php?action=query&format=json&revids=15258397"
       "&prop=revisions&rvprop=ids%7Ctimestamp%7Ccontent&rvslots=main")


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    if Path.cwd().resolve() != ROOT:
        raise ValueError("Run from the assigned worktree root")
    OUT.mkdir(parents=True, exist_ok=True)
    public = OUT / "lumbridge-map-15258397.json"
    if not public.exists():
        request = urllib.request.Request(URL, headers={"User-Agent": "ClubScape bounded source-policy research"})
        with urllib.request.urlopen(request, timeout=30) as response:
            data = response.read(500_000)
        page = next(iter(json.loads(data)["query"]["pages"].values()))
        if page["revisions"][0]["revid"] != 15258397:
            raise ValueError("Unexpected public source revision")
        public.write_bytes(data)
    raw = public.read_bytes()
    page = next(iter(json.loads(raw)["query"]["pages"].values()))
    body = page["revisions"][0]["slots"]["main"]["*"]
    match = re.search(r"\{\{Music track map\|(\[\[\[.*?\]\]\])\|plane=(\d)", body)
    if not match:
        raise ValueError("The exact public music polygon grammar changed")
    result = {
        "schema_version": 1,
        "classification": "dated_public_source_geography_with_native_music_table_corroboration",
        "not_a_native_server_polygon_capture": True,
        "frozen_source_pack_modified": False,
        "inputs": [{
            "path": str(public), "sha256": sha(raw), "url": URL, "revision": 15258397,
            "timestamp": page["revisions"][0]["timestamp"],
        }],
        "areas": [{
            "name": "Lumbridge", "native_area_id": 1, "plane": int(match[2]),
            "polygons": json.loads(match[1]), "groups": [2,64,327,163,76,145],
            "native_default_row": 2777, "native_default_group": 76,
        }],
    }
    pages = json.loads(Path("research/reference-pack/v1/pages.json").read_text())
    for title, group in [("Newbie Melody",62),("Scape Cave",144)]:
        source = next(p for p in pages if p["title"] == title)
        record = source["snapshot"]
        encoded = Path(record["path"]).read_bytes()
        if sha(encoded) != record["sha256"]:
            raise ValueError("An approved snapshot changed")
        data = json.loads(gzip.decompress(encoded))["page"]["revisions"][0]["slots"]["main"]
        text = data.get("content", data.get("*", ""))
        line = next(l for l in text.splitlines() if l.startswith("|map ="))
        shapes = []
        for coordinates in re.findall(r"\{\{Map\|([^{}]+)\}\}", line):
            points = []
            fields = {}
            for component in coordinates.split("|"):
                if re.fullmatch(r"\d+(?:\.\d+)?,\d+(?:\.\d+)?", component):
                    points.append([float(v) for v in component.split(",")])
                elif "=" in component:
                    k, v = component.split("=", 1)
                    fields[k] = v
            if group == 144 and not all(3000 <= p[0] <= 3200 and 9400 <= p[1] <= 9600 for p in points):
                continue
            if points:
                shapes.append(points)
        result["inputs"].append({**record, "url": source["url"]})
        result["areas"].append({"name": title, "plane": 0, "polygons": shapes, "groups": [group]})
    (OUT / "music-geography.json").write_text(json.dumps(result, separators=(",", ":")) + "\n")
    print(json.dumps({"source_revision":15258397,"areas":[
        {"name":a["name"],"polygons":len(a["polygons"]),"points":sum(map(len,a["polygons"]))}
        for a in result["areas"]]}))


if __name__ == "__main__":
    main()

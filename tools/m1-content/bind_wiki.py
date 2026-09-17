#!/usr/bin/env python3
"""Extract factual coordinate rows from pinned wiki text, without treating map pins as observations."""

import re

from common import BINDINGS, Inputs, load, sha, write


def templates(text, name):
    for match in re.finditer(r"\{\{" + re.escape(name) + r"(?=[|}\s])", text, re.I):
        depth, end = 1, match.end()
        while depth and end < len(text):
            if text[end:end + 2] == "{{":
                depth += 1
                end += 2
            elif text[end:end + 2] == "}}":
                depth -= 1
                end += 2
            else:
                end += 1
        if depth:
            raise ValueError(f"Unbalanced {name} template")
        yield text[match.start():end]


def coordinates(block):
    pairs = re.findall(r"(?:x:)?(\d{4,5})\s*,\s*(?:y:)?(\d{4,5})(?!\d)", block)
    if not pairs:
        x = re.search(r"\|\s*x\s*=\s*(\d+)", block)
        y = re.search(r"\|\s*y\s*=\s*(\d+)", block)
        if x and y:
            pairs = [(x[1], y[1])]
    plane = re.search(r"\|\s*plane\s*=\s*([0-3])", block)
    return [[int(x), int(y), int(plane[1]) if plane else 0] for x, y in pairs]


def field(block, key):
    value = re.search(r"\|\s*" + re.escape(key) + r"\s*=\s*([^|\n}]+)", block)
    return value[1].strip() if value else None


def main():
    inputs = Inputs()
    manifest = load(BINDINGS / "wiki-sources.json")
    rows, maps, stock = [], [], []
    for source in manifest["sources"]:
        path = BINDINGS / ".local/wiki" / f"{source['revision']}.wikitext"
        data = path.read_bytes()
        if sha(data) != source["sha256_utf8_wikitext"]:
            raise ValueError(f"Wiki snapshot integrity mismatch: {source['page']}")
        text = data.decode()
        for kind in ("LocLine", "ItemSpawnLine", "ObjectLocLine"):
            for block in templates(text, kind):
                selected = [point for point in coordinates(block) if any(
                    rect["min_x"] <= point[0] <= rect["max_x"] and
                    rect["min_y"] <= point[1] <= rect["max_y"]
                    for rect in inputs.selection["navigation_envelopes"])]
                if selected:
                    rows.append({
                        "page": source["page"], "source_ref": source["id"], "source_revision": source["revision"],
                        "row_kind": kind, "row_sha256": sha(block.encode()),
                        "location": field(block, "location"), "level": field(block, "levels"),
                        "drop_table": field(block, "dropversion"), "tiles": selected,
                        "classification": "verified_reference",
                        "evidence_scope": "Published source location rows, not observed current server spawns; "
                                          "visual NPC variant and live wandering are separate bindings.",
                    })
        for block in templates(text, "Map"):
            points = coordinates(block)
            if points and len(points) == 1:
                radius = field(block, "r")
                maps.append({
                    "page": source["page"], "source_ref": source["id"], "source_revision": source["revision"],
                    "tile": points[0], "radius": int(radius) if radius and radius.isdigit() else None,
                    "square_x": int(field(block, "squareX")) if field(block, "squareX") else None,
                    "square_y": int(field(block, "squareY")) if field(block, "squareY") else None,
                    "map_type": field(block, "mtype"), "classification": "inference",
                    "evidence_scope": "Published navigation/wander-area map anchor; not a certified NPC origin "
                                      "or player arrival tile. Never used as an observed camera or spawn.",
                })
        if source["page"] == "Lumbridge General Store":
            for block in templates(text, "StoreLine"):
                stock.append({
                    "name": field(block, "name"), "stock": int(field(block, "stock")),
                    "restock_ticks": int(field(block, "restock")),
                    "source_ref": source["id"], "source_revision": source["revision"],
                })
    result = {"schema_version": 1, "coordinate_rows": rows, "map_anchors": maps, "shop_stock": stock}
    write(BINDINGS / "wiki-facts.json", result, pretty=True)
    print(f"Bound {len(rows)} coordinate rows, {len(maps)} distinct map anchors, {len(stock)} stock lines.")


if __name__ == "__main__":
    main()

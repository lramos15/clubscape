"""Factor original settings widgets by their source struct identity, without panel crops."""
from __future__ import annotations

import re

CATEGORIES = ("activities", "audio", "chat", "controls", "display", "gameplay", "interfaces", "warnings")


def plain(value):
    return re.sub(r"<[^>]*>", "", value).replace("\n", " ")


def build_settings_catalog(definitions, templates):
    output = {}
    for category, name in enumerate(CATEGORIES):
        source = templates[f"bounded-settings-{name}"]
        body = [w for w in source if w["id"] >> 16 == 134 and (w["id"] & 65535) in (19, 20, 22) and w["index"] >= 0]
        root = next(w for w in source if w["id"] == (134 << 16 | 19) and w["index"] == -1)
        entries = definitions["categories"][str(category)]["settings"]
        labels = [w for w in body if w["id"] == root["id"] and w["type"] == 4 and
                  (w["color"] in (0xFF981F, 0xFFFFFF) or w["font"] == 496 and w["color"] == 0x9F9F9F)]
        backgrounds = [w for w in body if w["id"] == root["id"] and w["type"] == 3 and w["filled"] and
                       w["color"] == 0 and w["opacity"] in (200, 220) and w["width"] >= root["width"] - 10]
        rows = []
        used = set()
        for identifier in entries:
            params = definitions["settings"][str(identifier)]["definition"].get("params") or {}
            label = str(params.get("1086", ""))
            candidates = [w for w in labels if w["index"] not in used and label and
                          (plain(w["text"]) == label or params.get("1078") == 1 and plain(w["text"]).startswith(label + " - "))]
            if not candidates:
                continue
            text = min(candidates, key=lambda w: w["index"])
            used.add(text["index"])
            backdrop = [w for w in backgrounds if w["y"] <= text["y"] < w["y"] + w["height"]]
            action = [w for w in body if w["onOp"] and len(w["onOp"]) > 1 and w["onOp"][1] == identifier]
            top = max(backdrop, key=lambda w: w["y"])["y"] if backdrop else (
                min(action, key=lambda w: w["y"])["y"] if action else text["y"])
            rows.append({"id": identifier, "category": category, "label": label, "top": top,
                         "labelIndex": text["index"], "params": params, "sourceLocked": text["color"] == 0x9F9F9F})
        rows.sort(key=lambda row: (row["top"], row["labelIndex"]))
        for index, row in enumerate(rows):
            end = rows[index + 1]["top"] if index + 1 < len(rows) else root["y"] + root["height"]
            row["height"] = end - row["top"]
            row["widgets"] = [w for w in body if row["top"] <= w["y"] < end]
            row["descriptionIndices"] = [w["index"] for w in row["widgets"] if w["id"] == root["id"] and
                                         w["type"] == 4 and w["font"] == 495 and w["color"] == 0x9F9F9F]
            row["sourceSha256"] = definitions["settings"][str(row["id"])]["sourceSha256"]
            output[str(row["id"])] = row
        represented = {row["labelIndex"] for row in rows}
        unmatched = [w["text"] for w in labels if w["index"] not in represented and w["text"] and w["font"] == 496]
        if unmatched:
            raise ValueError(f"Source settings labels lack struct bindings in {name}: {unmatched}")
    return output

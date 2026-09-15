#!/usr/bin/env python3
"""Publish exact native control metadata, not replacement layouts or gameplay observations."""
import hashlib
import json
from pathlib import Path
import re
import urllib.request

ROOT = Path(__file__).resolve().parents[2]


def main():
    source = ROOT / ".local/evidence/ui-controls/native-probe.json"
    probe = json.loads(source.read_text())
    if probe["cache_id"] != 2695 or probe["configured_revision"] != 240:
        raise ValueError("Wrong native control source")
    manifest = json.loads((ROOT / "research/m1-bindings/code-sources.json").read_text())
    record = next(row for row in manifest["sources"] if row["id"] == "interfaces")
    with urllib.request.urlopen(record["url"], timeout=45) as response:
        raw = response.read(record["size_bytes"] + 1)
    if len(raw) != record["size_bytes"] or hashlib.sha256(raw).hexdigest() != record["sha256"]:
        raise ValueError("Pinned original interface symbols changed")
    text = raw.decode()
    groups = {}
    for name, group in [("LevelupDisplay", 233), ("NotificationDisplay", 660),
                        ("GravestoneRetrieval", 602), ("DeathOffice", 669),
                        ("GravestoneGeneric", 672)]:
        match = re.search(r"public static final class " + name + r"\s*\{([\s\S]*?)\n\t\}", text)
        if match is None:
            raise ValueError(f"Missing source group: {name}")
        names = {name: int(number.replace("_", ""), 16) for name, number in re.findall(
            r"public static final int (\w+) = (0x[\da-f_]+);", match[1])}
        widgets = {name: probe["widgets"][str(widget)] for name, widget in names.items()}
        if any(row["definition"]["id"] >> 16 != group for row in widgets.values()):
            raise ValueError("Native widget/group identity mismatch")
        groups[name] = {"source_group": group, "symbols": names, "widgets": widgets}
    result = {
        "schema_version": 1,
        "cache_id": 2695,
        "configured_original_revision": 240,
        "pinned_symbols": record,
        "native_indexes": probe["native_indexes"],
        "groups": {name: groups[name] for name in ("LevelupDisplay", "NotificationDisplay")},
        "canonical_associations": {
            "interface.level_up": {
                "source_group": 233, "style": "chatbox_level_up",
                "title_widget": 15269889, "body_widget": 15269890,
                "continue_widget": 15269891,
            },
            "interface.level_up_notification": {
                "source_group": 660, "style": "notification_popup",
                "title_widget": 43253764, "body_widget": 43253768,
                "source_script": 3343,
            },
        },
        "notification_script": {
            "source_id": 3343, "sha256": probe["scripts"]["3343"]["sha256"],
            "arguments": ["text_colour", "title_text", "body_text"],
            "default_colour_argument": -1, "resolved_default_colour": 16750623,
        },
        "qualification": [
            "Exact original widget definitions and pinned named child identities; no new images, layouts or source-pack changes.",
            "The source notification script is generic. Its existence does not prove every level gain opens both interfaces.",
            "Canonical M1 keeps its existing level-up chat presentation by default. A popup association is not a second reward, grant, client outcome or forced additional modal.",
            "The existing reward kind/interface/skill/level/continuation selects the actual source presentation; UI owns native layouts and local presentation preferences.",
        ],
        "milestone_accepted": False,
    }
    controls = {
        "schema_version": 1, "cache_id": 2695, "pinned_symbols": record,
        "native_indexes": probe["native_indexes"],
        "groups": {name: groups[name] for name in ("GravestoneRetrieval", "DeathOffice", "GravestoneGeneric")},
        "source_scripts": {str(id): probe["scripts"][str(id)] for id in (1987, 3490, 3492)},
        "source_enums": probe["enums"],
        "facts": [
            "Original DeathOffice669 exposes selected-item 1/5/X/All and Take-All. Script3492 labels the supplied varp263 fee as coins each for a stack; full remaining entry cost is not a unit price.",
            "Original GravestoneRetrieval602 script1987 exposes Bank-All only when server varp263 equals1 and the source retrieval service is unlocked. Varp261 selects the service through enums1753/1756/1757.",
            "DeathOffice669 uses varp263 as a unit fee, not the GravestoneRetrieval602 bank-permission flag. Reusing its integer as an Office Bank-All permission would invent authorization.",
            "Original GravestoneGeneric672 is a distinct free/paid normal-grave layout. Its existence does not authorize changing the existing M1 interface binding or source gameplay pin silently.",
            "Neither a client checkbox nor a template's button presence grants source banking access. Canonical normal M1 Bank-All availability must be explicitly source-bound; unknown remains unavailable.",
        ],
        "milestone_accepted": False,
    }
    output = ROOT / "research/interface-contracts"
    for name, value in [("level-up-native.json", result), ("control-bindings.json", controls)]:
        path = output / name
        path.write_text(json.dumps(value, sort_keys=True, indent=2, ensure_ascii=True) + "\n")
        print(name, hashlib.sha256(path.read_bytes()).hexdigest())


if __name__ == "__main__":
    main()

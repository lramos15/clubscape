#!/usr/bin/env python3
"""Capture bounded animation metadata and explicit evidence qualifications, not avatar assets."""
import hashlib
import json
from pathlib import Path
import re
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "research/interface-contracts/animation-authority"
REVISION = "ac79ed8bd8926bec7bf172aa291574b4d944b0e7"
SECONDARY = ("https://raw.githubusercontent.com/AlterRSPS/Alter/9ddbbbe3bdf47d79ff919a12d01ce1a7e6be6169/"
             "game-plugins/src/main/kotlin/org/alter/plugins/content/combat/CombatConfigs.kt")


def sha(data):
    return hashlib.sha256(data).hexdigest()


def fetched(url, expected, maximum):
    with urllib.request.urlopen(url, timeout=45) as response:
        raw = response.read(maximum + 1)
    if len(raw) > maximum or sha(raw) != expected:
        raise ValueError("Pinned animation reference changed")
    return raw


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    native = json.loads((ROOT / ".local/evidence/animation-authority/native-probe.json").read_text())
    if native["cache_id"] != 2695 or native["game_revision"] != 240:
        raise ValueError("Wrong native animation source")
    url = f"https://raw.githubusercontent.com/runelite/runelite/{REVISION}/runelite-api/src/main/java/net/runelite/api/gameval/AnimationID.java"
    raw = fetched(url, "9d721378614dc02cbcee83859678d8527a7729edbb83122d5a3f9dd79ad1eead", 1_000_000)
    wanted = {int(id) for id in native["sequences"]}
    names = {int(value): name for name, value in re.findall(
        r"public static final int\s+(\w+)\s*=\s*(\d+);", raw.decode()) if int(value) in wanted}
    if set(names) != wanted:
        raise ValueError("Source sequence inventory has an unqualified numeric identity")
    secondary = fetched(SECONDARY, "db58ebde778fc31c5ecd218efd742667fc56f642c285f6423975219f4af5456a", 100_000)
    media = json.loads((ROOT / "research/reference-pack/v1/public-media.json").read_text())
    home = next(row for row in media if row["id"] == "wiki.home-teleport-gif")
    if sha((ROOT / home["path"]).read_bytes()) != home["sha256"]:
        raise ValueError("Frozen Home Teleport reference changed")
    for id in wanted:
        native["sequences"][str(id)]["symbol"] = names[id]
    report = {
        "schema_version": 1, "cache_id": 2695, "game_revision": 240,
        "native": native,
        "symbols": {"url": url, "revision": REVISION, "sha256": sha(raw)},
        "secondary_dispatch_reference": {
            "url": SECONDARY, "sha256": sha(secondary), "classification": "qualified_secondary_inference",
            "scope": "Dispatch corroboration only, not Jagex server or frozen-pack evidence. Numeric style indexes are not copied.",
            "rejected_inconsistencies": [
                "The reference's axe and pickaxe animation-index branches disagree with its own attack-type branches. Bind the actual canonical attack type to the independently named native motion; do not copy those indexes.",
                "Defensive style labels are attack choices, not authority to play a defend/block reaction.",
            ],
        },
        "home_teleport": {
            "channel_ticks": 24,
            "actor_phases": [
                {"at_tick": 0, "sequence": 4847, "name": "draw_circle"},
                {"at_tick": 6, "sequence": 4850, "name": "sit"},
                {"at_tick": 12, "sequence": 4853, "name": "get_book"},
                {"at_tick": 16, "sequence": 4855, "name": "recite"},
                {"at_tick": 21, "sequence": 4857, "name": "teleport"},
            ],
            "reference": {key: home[key] for key in ("path", "sha256", "file_page_url", "upload_timestamp")},
            "reference_phase_starts_ms": [0, 3600, 7200, 9600, 12600],
            "active_cycles_before_terminal_hold": {"4847": 149, "4850": 43, "4853": 58, "4855": 150, "4857": 60},
            "qualification": "Qualified source-phase alignment: retained original crop and unchanged named native phases/hold frames, constrained to the independently frozen24-tick channel. GIF delays are not claimed as native wall-clock/server capture; no arbitrary 20-tick secondary-server timing is copied.",
        },
        "unknowns": {
            "recipe.cooking.dough": "No numeric actor sequence or deliberate silence has been verified; original potter-wheel883 is not a dough substitute.",
            "empty_container": "No numeric actor sequence or deliberate silence has been verified; keep the actual item action identity.",
        },
        "assets_or_frozen_reference_modified": False,
        "milestone_accepted": False,
    }
    path = OUT / "inputs.json"
    path.write_text(json.dumps(report, sort_keys=True, indent=2) + "\n")
    print(json.dumps({"sequences": len(wanted), "path": str(path.relative_to(ROOT)), "sha256": sha(path.read_bytes())}))


if __name__ == "__main__":
    main()

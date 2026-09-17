"""Record exact fetched research identities without publishing the prose/code snapshots."""

import hashlib
import json
from pathlib import Path
import re


def author():
    root = Path(".local/audio-import/reference-inputs")
    wiki = root / "wiki"
    records = []
    jingles = []
    for record in json.loads((wiki / "provenance.json").read_text()):
        raw = (wiki / f"{record['revision_id']}.wikitext").read_bytes()
        if len(raw) != record["size_bytes"] or hashlib.sha256(raw).hexdigest() != record["sha256"]:
            raise ValueError("Changed wiki source snapshot")
        match = re.search(r"\|cacheid\s*=\s*(\d+)", raw.decode())
        if not match:
            raise ValueError("Jingle source has no explicit cache ID")
        records.append({
            "id": f"wiki:{record['revision_id']}", **record,
            "source_kind": "wiki-revision-wikitext",
            "license_notice": "https://oldschool.runescape.wiki/w/RuneScape:Copyrights",
        })
        jingles.append({"name": record["title"], "index": 11, "group": int(match[1]), "reference": f"wiki:{record['revision_id']}"})
    overview = json.loads((root / "Jingles.source.json").read_text())
    records.append({
        "id": f"wiki:{overview['revision_id']}", **overview,
        "source_kind": "wiki-revision-wikitext",
        "license_notice": "https://oldschool.runescape.wiki/w/RuneScape:Copyrights",
    })
    revision = "fa13b3f67172ef36e15d6b1514358aee61411796"
    rsmod_paths = {
        "synth.sym": ".data/symbols/synth.sym",
        "jingle.sym": ".data/symbols/jingle.sym",
        "param.sym": ".data/symbols/param.sym",
        "BaseSynths.kt": "api/config/src/main/kotlin/org/rsmod/api/config/refs/BaseSynths.kt",
        "ElementalSpells.kt": "content/skills/magic/spell-attacks/src/main/kotlin/org/rsmod/content/skills/magic/spell/attacks/standard/ElementalSpells.kt",
        "MeleeAnimationAndSound.kt": "api/combat/combat-commons/src/main/kotlin/org/rsmod/api/combat/commons/fx/MeleeAnimationAndSound.kt",
    }
    for record in json.loads((root / "rsmod-provenance.json").read_text()):
        path = rsmod_paths[record["name"]]
        raw = (root / record["name"]).read_bytes()
        if hashlib.sha256(raw).hexdigest() != record["sha256"]:
            raise ValueError("Changed independent source snapshot")
        records.append({
            "id": "rsmod:" + record["name"], **record,
            "path": path,
            "source_kind": "independent-symbol-or-server-implementation",
            "url": f"https://github.com/rsmod/rsmod/blob/{revision}/{path}",
            "raw_url": f"https://raw.githubusercontent.com/rsmod/rsmod/{revision}/{path}",
            "license": "ISC",
            "limitation": "Names/independent behavior evidence only. Not a Jagex source session or proof of current trigger timing. RSMod custom parameters 654xx/655xx were not present in the selected source NPC/item definitions and are NOT treated as original bindings.",
        })
    runelite_revision = "ac79ed8bd8926bec7bf172aa291574b4d944b0e7"
    runelite = [
        ("SoundEffectID.java", "5e7bd6ed59daa103c556624e5cf1b438b75c0488353bcc01f7a66d44c641cbbf"),
        ("gameval/AnimationID.java", "9d721378614dc02cbcee83859678d8527a7729edbb83122d5a3f9dd79ad1eead"),
        ("gameval/SpotanimID.java", "92b1d9d3c08b1282dec9a5a42fed8f281cdef138321807406cb1003652a9fc91"),
    ]
    for path, digest in runelite:
        full = "runelite-api/src/main/java/net/runelite/api/" + path
        records.append({
            "id": "runelite:" + path, "revision": runelite_revision, "path": full, "sha256": digest,
            "url": f"https://github.com/runelite/runelite/blob/{runelite_revision}/{full}",
            "raw_url": f"https://raw.githubusercontent.com/runelite/runelite/{runelite_revision}/{full}",
            "source_kind": "source-ID-constants",
            "license_notice": "tools/audio-import/THIRD_PARTY_NOTICES.txt",
            "limitation": "Source symbol identity is not observed in-world trigger timing.",
        })
    records.append({
        "id": "smelting-candidate",
        "url": "https://github.com/Rims-Naps/Zyrox-Server/blob/075e805d0d249cb165bc6a0f3b65ed138d1db83f/src/main/java/com/zenyte/game/content/skills/smithing/Smelting.java",
        "git_blob": "2e66e13f44f3de40593291ae6b964e6b98b6e069",
        "size_bytes": 7407,
        "sha256": "5ee2ddbb5ba933f360fcf23d001f00d662de18f50a8831632e407c25bfc3cb78",
        "source_kind": "independent-unverified-candidate",
        "fact": "Declares SoundEffect(2725); uses a different animation (3243), not current requested899.",
        "limitation": "No code is copied. This is a candidate original cache sound only. Do not bind it as the verified current bronze-smelting sound without a source observation.",
    })
    source_symbols = {}
    for line in (root / "synth.sym").read_text().splitlines():
        source_id, name = line.split("\t")
        source_symbols[source_id] = name
    result = {
        "schema_version": 1,
        "source_selection_unchanged": True,
        "source_game_session_observed": False,
        "references": records,
        "jingle_identities": sorted(jingles, key=lambda value: value["group"]),
        "independent_sfx_symbols": source_symbols,
        "evidence_order": [
            "Actual selected source cache bytes and sequence/object references",
            "Unmodified selected original runtime decoding, sample synthesis and device output",
            "Pinned RuneLite ID names; pinned wiki jingle cacheid facts",
            "Explicitly labeled independent symbol/trigger candidates, never live-session proof",
        ],
    }
    destination = Path("research/audio-source/references.json")
    destination.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    author()

#!/usr/bin/env python3
"""Capture qualified public UI facts and approved native metadata; never alter source assets."""
import hashlib
import json
import re
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "research/interface-contracts"


def fetch(page, raw=False):
    url = "https://oldschool.runescape.wiki/w/" + page
    request = urllib.request.Request(url, headers={"User-Agent": "ClubScape-source-verification/1.0"})
    with urllib.request.urlopen(request, timeout=30) as response:
        data = response.read(2_000_001)
    if len(data) > 2_000_000:
        raise ValueError("Public source exceeded capture bound")
    text = data.decode()
    match = re.search(r'"wgRevisionId":(\d+)', text)
    if not match:
        raise ValueError(f"Missing source revision: {page}")
    revision = int(match[1])
    return {"page": page, "revision": revision,
            "url": f"{url}?oldid={revision}",
            "sha256": hashlib.sha256(data).hexdigest()}


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    pages = ["Energy_potion", "Potions", "Bones", "Game_tick/Action_lengths", "Bank", "Beer",
             "Death%27s_Coffer", "Tutorial_Island", "Chat_Interface", "Newcomer_map",
             "Security_book", "Transcript:Security_book"]
    sources = {page: fetch(page) for page in pages}
    transcript = sources["Transcript:Security_book"]
    raw_url = transcript["url"] + "&action=raw"
    text = urllib.request.urlopen(urllib.request.Request(raw_url, headers={"User-Agent": "ClubScape-source-verification/1.0"}), timeout=30).read(100_000).decode()
    # Source book markup is preserved as source text, not as user-provided rich text.
    text = re.sub(r"\{\{Transcript\|item\}\}\s*", "", text)
    text = re.sub(r"\{\{Colour\|#([0-9a-fA-F]+)\|([^{}]+)\}\}", r"<col=\1>\2</col>", text)
    pages_text = []
    for section in re.split(r"\n(?====)", text.strip()):
        section = re.sub(r"^===([\s\S]*?)===\s*", r"\1<br><br>", section)
        pages_text.append(section.strip().replace("\n\n", "<br><br>").replace("\n", " "))
    catalogue_path = ROOT / "assets/compiled/ui/manifest.json"
    catalogue_bytes = catalogue_path.read_bytes()
    catalogue = json.loads(catalogue_bytes)
    native = {
        "schema_version": 1,
        "source": str(catalogue_path.relative_to(ROOT)),
        "sha256": hashlib.sha256(catalogue_bytes).hexdigest(),
        "source_pack_sha256": catalogue["sourcePackSha256"],
        "tutorial_states": catalogue["tutorialStates"],
        "hud_signatures": catalogue["hudSignatures"],
        "combat_categories": catalogue["combatCategories"],
        "weapon_categories": catalogue["presentation"]["weaponCategories"],
        "qualification": "Original native control/label data. Signature omissions are unknown, never evidence to hide a control.",
    }
    facts = {
        "schema_version": 1, "sources": sources,
        "energy_restore_units": 1500, "drink_cooldown_ticks": 3, "bury_ticks": 2, "bones_xp_tenths": 45,
        "bank_maximum_extra_tabs": 9,
        "coffer": {"minimum_exchange_value": 10000, "credit_numerator": 105,
                   "credit_denominator": 100, "maximum_balance": 2147483647},
        "security_book_pages": pages_text,
        "notes": [
            "Energy is 15% (not obsolete 10%) at source revision15320560; each dose changes only the selected ordinary item.",
            "Potion timer is independent of food; it does not set a food attack delay.",
            "Beer source effects are HP+1, Strength+floor(base*2/100)+1, Attack drain floor(current*6/100)+1; the drink-timer grouping is a qualified shared-drink inference, not an observed beer packet trace.",
            "Bank insert/swap/9 extra tabs/placeholders/deposit-worn are source controls. Initial swap/placeholders-off are explicit native-default inferences, not invented account permissions.",
            "Coffer credit uses separately captured exchange values, not the max(exchange,alchemy) death-retention table. Coins are not a deposit mechanism; eligible ordinary M1 items are selected explicitly.",
            "Public chat is unavailable on Tutorial Island; only normal public channel is enabled. Text/audience/admission bounds remain explicitly qualified transport policy.",
            "BOOK392 and LEVELUP_DISPLAY233 are exact pinned native symbols. AIDE_MAP615 has original tutor/toggle children matching the newcomer map; this linkage is source-supported inference, not the incorrect old539 claim (539 is RAIDS_REWARDS).",
            "No source visibility is guessed from a numeric varp281 or from unmentioned controls in the11 approved signatures.",
        ],
        "acceptance": {"observed_gameplay": False, "milestone_accepted": False},
        "attribution": "OSRS Wiki facts/transcript: CC BY-NC-SA 3.0, source URLs/revisions above. Native identifiers/metadata: pinned RuneLite source and approved reference pack.",
    }
    for name, value in [("sources.json", facts), ("native-ui.json", native)]:
        (OUT / name).write_text(json.dumps(value, ensure_ascii=True, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"sources": len(sources), "states": len(native["tutorial_states"]),
                      "signatures": len(native["hud_signatures"]), "book_pages": len(pages_text),
                      "milestone_accepted": False}))


if __name__ == "__main__":
    main()

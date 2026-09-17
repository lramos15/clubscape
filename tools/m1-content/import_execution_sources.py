#!/usr/bin/env python3
"""Pin the narrow public-code evidence for v3 source selectors, never live observations."""

from concurrent.futures import ThreadPoolExecutor
import json
import urllib.parse
import urllib.request

from common import BINDINGS, ROOT, load, sha, write


REVISION = "fa13b3f67172ef36e15d6b1514358aee61411796"
CODE = {
    "combat_commons": "api/combat/combat-scripts/src/main/kotlin/org/rsmod/api/combat/CombatCommons.kt",
    "player_npc_combat": "api/combat/combat-scripts/src/main/kotlin/org/rsmod/api/combat/PvNCombat.kt",
    "npc_player_combat": "api/combat/combat-scripts/src/main/kotlin/org/rsmod/api/combat/NvPCombat.kt",
    "npc_combat_script": "api/combat/combat-scripts/src/main/kotlin/org/rsmod/api/combat/scripts/NvPCombatScript.kt",
    "logout_process": "api/game-process/src/main/kotlin/org/rsmod/api/game/process/player/PlayerLogoutProcess.kt",
    "logout_script": "content/interfaces/logout-tab/src/main/kotlin/org/rsmod/content/interfaces/logout/tab/LogoutTabScript.kt",
    "player_death": "api/death-plugin/src/main/kotlin/org/rsmod/api/death/plugin/PlayerDeathScript.kt",
    "death_operations": "api/death/src/main/kotlin/org/rsmod/api/death/PlayerDeath.kt",
    "combat_constants": "api/config/src/main/kotlin/org/rsmod/api/config/Constants.kt",
    "npc_follow": "api/game-process/src/main/kotlin/org/rsmod/api/game/process/npc/mode/NpcPlayerFollowModeProcessor.kt",
    "npc_modes": "api/game-process/src/main/kotlin/org/rsmod/api/game/process/npc/mode/NpcModeProcessor.kt",
    "npc_interactions": "api/game-process/src/main/kotlin/org/rsmod/api/game/process/npc/mode/AiPlayerModeProcessor.kt",
    "npc_combat_properties": "api/combat/combat-scripts/src/main/kotlin/org/rsmod/api/combat/npc/NpcProperties.kt",
    "player_combat_properties": "api/combat/combat-scripts/src/main/kotlin/org/rsmod/api/combat/player/PlayerProperties.kt",
    "player_combat_common": "api/combat/combat-scripts/src/main/kotlin/org/rsmod/api/combat/player/PlayerCommons.kt",
    "player_attack_manager": "api/combat/combat-manager/src/main/kotlin/org/rsmod/api/combat/manager/PlayerAttackManager.kt",
    "npc_player_interactions": "api/npc/src/main/kotlin/org/rsmod/api/npc/interact/AiPlayerInteractions.kt",
    "npc_extensions": "api/npc/src/main/kotlin/org/rsmod/api/npc/NpcExtensions.kt",
    "player_extensions": "api/player/src/main/kotlin/org/rsmod/api/player/PlayerExtensions.kt",
    "player_hit_processor": "api/player/src/main/kotlin/org/rsmod/api/player/hit/processor/StandardPlayerHitProcessor.kt",
}


def main():
    directory = BINDINGS / ".local/runtime3-sources"
    directory.mkdir(parents=True, exist_ok=True)
    output = BINDINGS / "runtime3-sources.json"
    previous = {record["id"]: record for record in load(output)["sources"]} if output.exists() else {}
    original = load(ROOT / "research/runtime-bindings/sources.json")["sources"]
    requests = [{"id": key, "url": f"https://raw.githubusercontent.com/rsmod/rsmod/{REVISION}/{path}",
                 "revision": REVISION, "kind": "public_code"} for key, path in CODE.items()]
    for page in ("Items", "Combat", "Drop", "Drops"):
        record = next(source for source in original if source.get("page") == page)
        requests.append({"id": "wiki_" + page.lower(), "url": record["url"], "revision": str(record["revision"]),
                         "kind": "wiki_revision", "sha256": record["sha256"], "page": page})
    for key, page in (("wiki_logout", "Log out"), ("wiki_trading", "Trading"), ("wiki_account", "Account")):
        if key in previous:
            requests.append(previous[key])
            continue
        url = "https://oldschool.runescape.wiki/api.php?" + urllib.parse.urlencode({
            "action": "query", "format": "json", "formatversion": 2, "prop": "revisions",
            "rvprop": "ids|content", "rvslots": "main", "titles": page, "redirects": 1,
        })
        with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "ClubScapeSourceBindings/3"}), timeout=60) as response:
            payload = json.load(response)
        if "error" in payload or "continue" in payload:
            raise ValueError("Incomplete source selector reference")
        page_data = payload["query"]["pages"][0]
        revision = page_data["revisions"][0]
        data = revision["slots"]["main"]["content"].encode()
        record = {"id": key, "url": "https://oldschool.runescape.wiki/w/" +
                  urllib.parse.quote(page_data["title"].replace(" ", "_"), safe=":/") + f"?oldid={revision['revid']}",
                  "page": page_data["title"], "revision": str(revision["revid"]),
                  "kind": "wiki_revision", "sha256": sha(data), "bytes": len(data)}
        (directory / f"{key}.txt").write_bytes(data)
        requests.append(record)
    def fetch(record):
        path = directory / (record["id"] + ".txt")
        if not path.exists():
            if record["kind"] == "wiki_revision":
                url = "https://oldschool.runescape.wiki/api.php?" + urllib.parse.urlencode({
                    "action": "query", "format": "json", "formatversion": 2, "prop": "revisions",
                    "rvprop": "ids|content", "rvslots": "main", "revids": record["revision"],
                })
            else:
                url = record["url"]
            request = urllib.request.Request(url, headers={"User-Agent": "ClubScapeSourceBindings/3"})
            with urllib.request.urlopen(request, timeout=60) as response:
                data = response.read()
            if record["kind"] == "wiki_revision":
                payload = json.loads(data)
                if "error" in payload or "continue" in payload:
                    raise ValueError("Incomplete pinned wiki source")
                revision = payload["query"]["pages"][0]["revisions"][0]
                if str(revision["revid"]) != record["revision"]:
                    raise ValueError("Wrong pinned source revision")
                data = revision["slots"]["main"]["content"].encode()
            path.write_bytes(data)
        data = path.read_bytes()
        expected = previous.get(record["id"], record).get("sha256")
        if expected and sha(data) != expected:
            raise ValueError("Pinned execution source changed: " + record["id"])
        return {**record, "sha256": sha(data), "bytes": len(data)}, data.decode()
    results = list(ThreadPoolExecutor(max_workers=4).map(fetch, requests))
    write(output, {
        "schema_version": 1, "classification": "source_supported_inference",
        "source_observation_claimed": False,
        "sources": [record for record, _ in results],
        "scope": "Existing immutable public reconstruction and wiki evidence. Not server-code access, "
                 "owner approval, a new account or a live source gameplay capture.",
    }, pretty=True)
    print(json.dumps({"pinned_sources": len(results), "source_index_sha256": sha(output.read_bytes()),
                      "source_observation_claimed": False}))


if __name__ == "__main__":
    main()

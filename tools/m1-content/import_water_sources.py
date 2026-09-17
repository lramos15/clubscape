#!/usr/bin/env python3
"""Pin narrow water-fill references before the existing source-selection cutoff."""

import json
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime
import re
import urllib.parse
import urllib.request

from common import ROOT, Inputs, canonical, load, sha, write


OUT = ROOT / "research/water-fill"
PAGES = ("Bucket of water", "Water", "Sink", "Money making guide/Filling buckets with water", "Module:Recipe")
API = "https://oldschool.runescape.wiki/api.php"


def request(parameters):
    url = API + "?" + urllib.parse.urlencode({
        "action": "query", "format": "json", "formatversion": 2,
        "prop": "revisions", "rvprop": "ids|timestamp|content", "rvslots": "main", **parameters,
    })
    with urllib.request.urlopen(urllib.request.Request(
        url, headers={"User-Agent": "ClubScapeSourceBindings/WaterFill"}), timeout=60) as response:
        data = response.read(2_000_001)
    if len(data) > 2_000_000:
        raise ValueError("Water source query exceeded its bounded response size")
    result = json.loads(data)
    if "error" in result:
        raise ValueError("Water source API rejected the query: " + json.dumps(result["error"]))
    if "continue" in result and not (parameters.get("rvlimit") == 1 and "|" not in parameters.get("titles", "|")):
        raise ValueError("Pinned water source query was incomplete")
    return result["query"]["pages"]


def main():
    output = OUT / "sources.json"
    selection = ROOT / "research/current-source/selection.json"
    cutoff = load(selection)["selected_at"]
    previous = load(output) if output.exists() else None
    if previous:
        if previous["source_selection_sha256"] != sha(selection.read_bytes()):
            raise ValueError("The water source-selection boundary changed")
        records = {record["page"]: record for record in previous["sources"]}
        if not set(records).issubset(PAGES):
            raise ValueError("The pinned water-reference scope changed")
        pages = request({"revids": "|".join(str(record["revision"]) for record in records.values())})
    else:
        records = {}
        pages = []
    missing = [page for page in PAGES if page not in records]
    if missing:
        with ThreadPoolExecutor(max_workers=4) as pool:
            batches = list(pool.map(
                lambda page: request({"titles": page, "rvstart": cutoff, "rvlimit": 1, "redirects": 1}), missing))
        pages.extend(page for batch in batches for page in batch)
    resolved = {}
    directory = OUT / ".local/source-pages"
    directory.mkdir(parents=True, exist_ok=True)
    for page in pages:
        name = page["title"]
        if name not in PAGES or page.get("missing") or len(page.get("revisions", [])) != 1:
            raise ValueError("Missing, redirected or ambiguous source page: " + name)
        revision = page["revisions"][0]
        data = revision["slots"]["main"]["content"].encode()
        if datetime.fromisoformat(revision["timestamp"]) > datetime.fromisoformat(cutoff):
            raise ValueError("Water reference is newer than the existing source cutoff")
        record = {
            "page": name, "revision": revision["revid"], "revision_timestamp": revision["timestamp"],
            "url": "https://oldschool.runescape.wiki/w/" + urllib.parse.quote(name.replace(" ", "_"), safe="/:")
                   + "?oldid=" + str(revision["revid"]),
            "kind": "wiki_revision", "sha256_utf8_wikitext": sha(data), "bytes_utf8_wikitext": len(data),
        }
        if name in records and record != records[name]:
            raise ValueError("Pinned water source bytes or identity changed: " + name)
        (directory / (str(revision["revid"]) + ".txt")).write_bytes(data)
        resolved[name] = record
        print(json.dumps({"page": name, "revision": revision["revid"], "sha256": sha(data)}))
    if set(resolved) != set(PAGES):
        raise ValueError("A requested water source page was not returned")
    write(output, {
        "schema_version": 1, "source_selection": str(selection.relative_to(ROOT)),
        "source_selection_sha256": sha(selection.read_bytes()), "source_cutoff": cutoff,
        "sources": [resolved[page] for page in PAGES],
        "scope": "Pinned public documentary evidence, not gameplay observations, source-server access or motion dispatch verification.",
    }, True)
    bucket = (directory / (str(resolved["Bucket of water"]["revision"]) + ".txt")).read_text()
    module = (directory / (str(resolved["Module:Recipe"]["revision"]) + ".txt")).read_text()
    creation = re.search(r"(?ms)^==Creation==\s*\{\{Recipe\s*\n(.*?)^\}\}", bucket)
    if creation is None:
        raise ValueError("The pinned bucket recipe cannot be located")
    parameters = {}
    for line in creation[1].splitlines():
        match = re.fullmatch(r"\|([a-zA-Z0-9]+)\s*=\s*(.*?)\s*", line)
        if match is None or match[1] in parameters:
            raise ValueError("Ambiguous source recipe parameter")
        parameters[match[1]] = match[2]
    if parameters != {"members": "No", "ticks": "1", "facilities": "Water", "mat1": "Bucket",
                      "output1": "Bucket of water"}:
        raise ValueError("The source water recipe changed")
    if ("local qty = params.default_to(args[objType..i..'quantity'],'1')" not in module
            or "local skill = args['skill'..i]" not in module
            or "if skill and params.has_content(skill) then" not in module
            or "ticks (' .. secs .. 's) per action" not in module):
        raise ValueError("The source quantity/skill/tick template interpretation changed")
    inputs = Inputs()
    sink = inputs.collections["object"][14868]
    if (sink["name"] != "Sink" or (sink["sizeX"], sink["sizeY"]) != (1, 2)
            or sink["ops"] != {"ops": [], "subOps": [], "conditionalOps": [], "conditionalSubOps": []}):
        raise ValueError("Source sink identity, footprint or menu operations changed")
    locator = inputs.collection_sources[inputs.asset("object", 14868)]
    write(OUT / "facts.json", {
        "schema_version": 1, "source_recipe_parameters": parameters,
        "source_recipe": resolved["Bucket of water"], "recipe_parameter_semantics": resolved["Module:Recipe"],
        "input": {"item": "item.bucket", "source_id": 1925, "quantity": 1},
        "output": {"item": "item.water.bucket", "source_id": 1929, "quantity": 1},
        "byproducts": [], "skill_requirements": [], "xp_rewards": [],
        "quantity_and_xp_basis": "The original Recipe module defaults each declared material/output to quantity1. "
                                  "Only an explicitly declared skill creates a requirement/XP row; omitted skill parameters "
                                  "are an empty skill list, not the '?' placeholder for unknown XP on a declared skill.",
        "ticks_per_conversion": 1, "seconds_per_conversion": "0.6",
        "cadence_qualification": "The source explicitly declares1 tick per action/output. Single uses that value directly; "
                                 "first/repeat reuse the same per-conversion duration as a source-supported interpretation. "
                                 "This does not establish automatic batching, Make-X availability or a separately captured startup/animation phase.",
        "dispatch": "item_on_world", "source_object_menu_operations": [],
        "sink": {"source_id": 14868, "source_asset": inputs.asset("object", 14868),
                 "collection": locator, "collection_sha256": sha((ROOT / locator).read_bytes()),
                 "definition_sha256": sha(canonical(sink)), "footprint": [1, 2],
                 "source_model_ids": sink["objectModels"], "source_scenery_animation": sink["animationID"]},
        "actor_motion": {"classification": "unresolved_source", "sequence": None, "no_animation_verified": False,
                         "reason": "Scenery animationID=-1 is not actor dispatch evidence. No filling sequence or deliberate "
                                   "absence was verified; do not use a similarly named vial, fountain or construction sequence."},
        "excluded_nonmatching_sources": [
            {"page": "Sink", "reason": "This article describes building POH sinks13563/13564 for300 Construction XP/5ticks, not using source sink14868."},
            {"page": "Money making guide/Filling buckets with water",
             "reason": "This guide describes Lunar Humidify, not the normal source item-on-sink action. Its Magic XP, runes, cast timing and throughput do not apply."},
        ],
        "source_observation_claimed": False, "migration_or_gameplay_admitted": False,
    }, True)


if __name__ == "__main__":
    main()

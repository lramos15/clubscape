"""Deterministic source loading and canonical M1 identities."""

import gzip
import hashlib
import json
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[2]
CONTENT = ROOT / "content/m1"
BINDINGS = ROOT / "research/m1-bindings"
SOURCE = ROOT / "assets/source/osrs/cache2695"
JOURNEY = ROOT / "research/journey-rules"
ASSET_PREFIX = "asset.source.osrs.cache2695."
PUBLICATION = ROOT / "assets/manifests/osrs/cache2695-consumables-published.json"
PUBLICATION_SHA256 = "2340ea5e3e6eeeeecdbda5de7f371f3ff2eb57576f4c783009344a1c348dbede"


def published_inputs():
    if sha(PUBLICATION.read_bytes()) != PUBLICATION_SHA256:
        raise ValueError("The selected consumables publication differs from its authorized hash")
    previous_path = list(sys.path)
    previous_bytecode = sys.dont_write_bytecode
    try:
        sys.dont_write_bytecode = True
        sys.path.insert(0, str(ROOT / "tools/cache-import"))
        from content_closure import load_published_inputs, publication_chain
        bundle, collections = load_published_inputs(PUBLICATION)
        return bundle, collections, publication_chain(PUBLICATION)
    finally:
        sys.path[:] = previous_path
        sys.dont_write_bytecode = previous_bytecode


def sha(data):
    return hashlib.sha256(data).hexdigest()


def load(path):
    data = Path(path).read_bytes()
    return json.loads(gzip.decompress(data) if str(path).endswith(".gz") else data)


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False) + "\n").encode()


def write(path, value, pretty=False):
    path = Path(path).resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    data = (json.dumps(value, sort_keys=True, indent=2, ensure_ascii=False) + "\n").encode() if pretty else canonical(value)
    if path.suffix == ".gz":
        compressed = bytearray(gzip.compress(data, compresslevel=9, mtime=0))
        compressed[9] = 255
        path.write_bytes(compressed)
    else:
        path.write_bytes(data)
    return {
        "path": str(path.relative_to(ROOT)),
        "bytes": path.stat().st_size,
        "sha256": sha(path.read_bytes()),
        "json_bytes": len(data),
        "json_sha256": sha(data),
    }


def tile(x, y, plane=0):
    return {"x": x, "y": y, "plane": plane}


def region_id(x, y):
    return f"region.osrs.{((x // 64) << 8) | (y // 64)}"


def position(value):
    return value["x"], value["y"], value["plane"]


def source_record(reference, notes, status="verified_reference", revision="240/cache2695"):
    return {"reference": reference, "revision": revision, "status": status, "notes": notes}


def item_stack(item, quantity=1):
    return {"item": item, "quantity": quantity, "instance": None}


def stack(source):
    return item_stack(source["item_ref"], source["quantity"])


def always():
    return {"kind": "always"}


def all_of(*guards):
    values = []
    for guard in guards:
        if guard["kind"] == "all":
            values.extend(guard["guards"])
        elif guard["kind"] != "always":
            values.append(guard)
    return {"kind": "all", "guards": values} if values else always()


def never():
    return {"kind": "not", "guard": always()}


def quest_at(stage):
    return {"kind": "quest_stage", "quest": "quest.cooks_assistant", "stage": stage}


def tutorial_at(stage):
    return {"kind": "tutorial_stage", "stage": stage}


def has_items(items):
    return {"kind": "has_items", "items": items}


def bound(value, source):
    return {"status": "bound", "value": value, "source": unique_sources(source)}


def unresolved(reason, source):
    return {"status": "unresolved", "reason": reason, "source": unique_sources(source)}


def level_domain(maximum=99, minimum=1, basis="current"):
    return {"minimum": minimum, "maximum": maximum, "basis": basis}


def requirement(skill, level=1, basis="current"):
    return {"skill": skill, "level": level, "basis": basis}


def constant_chance(numerator=1, denominator=1):
    return {"numerator_at_level_1": numerator, "numerator_at_level_99": numerator,
            "denominator": denominator, "domain": {"kind": "constant"}}


def skill_chance(low, high, maximum=99, minimum=1):
    return {"numerator_at_level_1": low + 1, "numerator_at_level_99": high + 1,
            "denominator": 256, "domain": {"kind": "skill", "levels": level_domain(maximum, minimum)}}


def counter_value(value):
    return {"type": "boolean" if type(value) is bool else "integer", "value": value}


def counter_guard(identifier, value=True, minimum=None, maximum=None):
    predicate = ({"kind": "equals", "value": counter_value(value)} if minimum is None else
                 {"kind": "integer_range", "minimum": minimum, "maximum": maximum})
    return {"kind": "counter", "counter": identifier, "predicate": predicate}


def set_counter(identifier, value=True):
    return {"kind": "set_counter", "counter": identifier, "value": counter_value(value)}


def restore_run():
    return {"kind": "restore_vital", "vital": "run_energy", "restoration": {"kind": "to_base_maximum"}}


def location(point, instance=None):
    return {"region": region_id(point["x"], point["y"]), "tile": point, "instance": instance}


class Inputs:
    def __init__(self):
        self.selection = load(BINDINGS / "selection.json")
        self.profile = load(BINDINGS / "profile-v2.json")
        for category in ("items", "npcs", "objects"):
            numbers = list(self.selection[category].values())
            if len(set(numbers)) != len(numbers):
                raise ValueError(f"Ambiguous semantic IDs for a source {category} definition")
        self.publication = load(PUBLICATION)
        self.bundle, source_collections, self.publication_chain = published_inputs()
        self.catalog_path = self.publication["merged_inventory"]["path"]
        self.assets = {record["asset_id"]: record for record in self.bundle["records"]}
        self.collections = {
            kind: {value["id"]: value for value in source_collections[kind].values()}
            for kind in ("item", "npc", "object", "sequence", "texture", "underlay", "overlay")
        }
        self.collection_sources = {
            identifier: str((SOURCE / f"collections/{kind}.json.gz").relative_to(ROOT))
            for kind, values in source_collections.items() for identifier in values
        }
        for _, publication in self.publication_chain:
            for kind, shard in publication.get("collection_extensions", {}).items():
                for identifier in load(ROOT / shard["path"]):
                    self.collection_sources[identifier] = shard["path"]
        self.supplement = load(BINDINGS / "definitions.json.gz")
        self.definition_supplements = {}
        for number, record in self.supplement["items"].items():
            number = int(number)
            existing = self.collections["item"].get(number)
            if existing is not None and existing != record["definition"]:
                raise ValueError(f"Supplement disagrees with original item {number}")
            self.collections["item"][number] = record["definition"]
        for number, record in self.supplement["npcs"].items():
            number = int(number)
            existing = self.collections["npc"].get(number)
            if existing is not None and existing != record:
                raise ValueError(f"Supplement disagrees with original NPC {number}")
            self.collections["npc"][number] = record
        for extra in [BINDINGS / "application-item-definitions.json.gz", BINDINGS / "ui-item-definitions.json.gz"]:
          if extra.exists():
            supplement = load(extra)
            if supplement["item_group"] != self.bundle["groups"]["2/10"]:
                raise ValueError("Additional item definitions have a different source archive identity")
            for number, record in supplement["items"].items():
                number = int(number)
                existing = self.collections["item"].get(number)
                if existing is not None and existing != record["definition"]:
                    raise ValueError(f"Additional definition disagrees with original item {number}")
                self.collections["item"][number] = record["definition"]
                self.definition_supplements[("item", number)] = str(extra.relative_to(ROOT))
        self.rules = {name: load(JOURNEY / f"{name}.json") for name in (
            "initial-state", "activities", "tutorial", "cooks-assistant",
            "vocabulary", "decisions", "expected-scenarios")}
        self.wiki_sources = {record["id"]: record for record in load(JOURNEY / "sources.json")["sources"]}
        self.wiki_sources.update({record["id"]: record for record in load(BINDINGS / "wiki-sources.json")["sources"]})
        runtime_sources = BINDINGS / "runtime-source-facts.json"
        if runtime_sources.exists():
            self.wiki_sources.update({entry["source"]["id"]: entry["source"] for entry in load(runtime_sources)["sources"]})
        self.by_page = {record.get("page"): record for record in self.wiki_sources.values()}
        self.code_sources = load(BINDINGS / "code-sources.json")
        self.object_ids = {number: identifier for identifier, number in self.selection["objects"].items()}
        self.item_ids = {number: identifier for identifier, number in self.selection["items"].items()}
        self.npc_ids = {number: identifier for identifier, number in self.selection["npcs"].items()}
        self.activity_rules = {rule["id"]: rule for name in ("activities", "cooks-assistant")
                               for rule in self.rules[name]["rules"]}

    def object_id(self, number):
        return self.object_ids.get(number, f"object.scenery.{number}")

    def object_spawn_id(self, row):
        number, x, y, plane, kind, orientation = row
        return f"spawn.{self.object_id(number)[7:]}.{x}.{y}.p{plane}.t{kind}.r{orientation}"

    def asset(self, kind, number):
        identifier = f"{ASSET_PREFIX}{kind}.{number}"
        return identifier if identifier in self.assets else None

    def definition_source(self, kind, number):
        asset = self.asset(kind, number)
        reference = (f"{self.collection_sources[asset]}#{asset}" if asset else
                     f"{self.definition_supplements.get((kind, number), 'research/m1-bindings/definitions.json.gz')}#{kind}s/{number}")
        return [source_record(reference, f"Original selected-cache {kind} definition {number}; "
                              "source assets, not approved presentation or observed gameplay.")]

    def basis(self, basis, notes="Pinned behavioral reference; not an executed game trace."):
        if basis is None:
            return []
        status = "verified_reference" if basis["classification"] == "verified_reference" else "inference"
        result = []
        for identifier in basis.get("source_refs", []):
            source = self.wiki_sources[identifier]
            result.append(source_record(
                source.get("url", source.get("path", identifier)), notes, status,
                str(source.get("revision", source.get("sha256", "source-contract-v1")))))
        for identifier in basis.get("assumption_refs", []):
            result.append(source_record("research/journey-rules/decisions.json#" + identifier,
                                        "Reversible assumption; not owner approval. " + notes,
                                        "inference", "source-contract-v1"))
        return unique_sources(result)

    def wiki(self, page, notes, status="verified_reference"):
        source = self.by_page[page]
        return source_record(source["url"], notes, status, str(source["revision"]))

    def rule_source(self, identifier, notes=None, inference=False):
        rule = self.activity_rules[identifier]
        filename = "cooks-assistant" if identifier.startswith("rule.cooks.") else "activities"
        records = self.basis(rule.get("basis"))
        records.append(source_record(
            f"research/journey-rules/{filename}.json#{identifier}",
            notes or "Typed source rule binding; arithmetic/content validation is not observed runtime gameplay.",
            "inference" if inference else "verified_reference", "source-contract-v1"))
        return unique_sources(records)

    def assumption(self, identifier, notes=None):
        return [source_record("research/journey-rules/decisions.json#" + identifier,
                              notes or "Explicit source-contract inference, not owner approval or a live observation.",
                              "inference", "source-contract-v1")]


def unique_sources(records):
    result = {}
    for record in records:
        key = record["reference"], record["revision"]
        if key in result:
            current = result[key]
            if record["status"] == "inference":
                current["status"] = "inference"
            if record["notes"] not in current["notes"]:
                current["notes"] += " " + record["notes"]
        else:
            result[key] = dict(record)
    return list(result.values())

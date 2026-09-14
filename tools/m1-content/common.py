"""Deterministic source loading and canonical M1 identities."""

import gzip
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTENT = ROOT / "content/m1"
BINDINGS = ROOT / "research/m1-bindings"
SOURCE = ROOT / "assets/source/osrs/cache2695"
JOURNEY = ROOT / "research/journey-rules"
ASSET_PREFIX = "asset.source.osrs.cache2695."


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
    return {"item": item, "quantity": quantity}


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


class Inputs:
    def __init__(self):
        self.selection = load(BINDINGS / "selection.json")
        for category in ("items", "npcs", "objects"):
            numbers = list(self.selection[category].values())
            if len(set(numbers)) != len(numbers):
                raise ValueError(f"Ambiguous semantic IDs for a source {category} definition")
        self.bundle = load(ROOT / "assets/manifests/osrs/cache2695-full-bundle.json.gz")
        self.assets = {record["asset_id"]: record for record in self.bundle["records"]}
        self.collections = {
            kind: {value["id"]: value for value in load(SOURCE / f"collections/{kind}.json.gz").values()}
            for kind in ("item", "npc", "object", "sequence", "texture", "underlay", "overlay")
        }
        self.supplement = load(BINDINGS / "definitions.json.gz")
        for number, record in self.supplement["items"].items():
            number = int(number)
            existing = self.collections["item"].get(number)
            if existing is not None and existing != record["definition"]:
                raise ValueError(f"Supplement disagrees with original item {number}")
            self.collections["item"][number] = record["definition"]
        self.collections["npc"].update(
            {int(number): record for number, record in self.supplement["npcs"].items()})
        self.rules = {name: load(JOURNEY / f"{name}.json") for name in (
            "initial-state", "activities", "tutorial", "cooks-assistant",
            "vocabulary", "decisions", "expected-scenarios")}
        self.wiki_sources = {record["id"]: record for record in load(JOURNEY / "sources.json")["sources"]}
        self.wiki_sources.update({record["id"]: record for record in load(BINDINGS / "wiki-sources.json")["sources"]})
        self.by_page = {record.get("page"): record for record in self.wiki_sources.values()}
        self.code_sources = load(BINDINGS / "code-sources.json")
        self.object_ids = {number: identifier for identifier, number in self.selection["objects"].items()}
        self.item_ids = {number: identifier for identifier, number in self.selection["items"].items()}
        self.npc_ids = {number: identifier for identifier, number in self.selection["npcs"].items()}

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
        reference = (f"assets/source/osrs/cache2695/collections/{kind}.json.gz#{asset}" if asset else
                     f"research/m1-bindings/definitions.json.gz#{kind}s/{number}")
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

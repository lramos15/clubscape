"""Additive, source-hashed closure of the compiled M1 asset references."""
from __future__ import annotations

import collections
import gzip
import hashlib
import json
from pathlib import Path

import import_cache as cache

ROOT = cache.ROOT
SOURCE = ROOT / "assets/source/osrs/cache2695"
MANIFESTS = ROOT / "assets/manifests/osrs"
BASE_BUNDLE = MANIFESTS / "cache2695-full-bundle.json.gz"
BASE_PUBLICATION = MANIFESTS / "cache2695-published.json"
PUBLICATION = MANIFESTS / "cache2695-content-v2-published.json"
REQUEST = ROOT / "research/current-source/m1-content-closure-request.json"
PREFIX = "asset.source.osrs.cache2695."
FIELDS = {
    "item_definition_ids": ("item", "item_ids"),
    "model_ids": ("model", "model_ids"),
    "npc_definition_ids": ("npc", "npc_ids"),
    "interface_groups": ("interface", "interface_groups"),
}
ITEM_MODELS = (
    "inventoryModel", "maleModel0", "maleModel1", "maleModel2", "femaleModel0",
    "femaleModel1", "femaleModel2", "maleHeadModel", "maleHeadModel2",
    "femaleHeadModel", "femaleHeadModel2",
)
ITEM_LINKS = ("notedID", "notedTemplate", "boughtId", "boughtTemplateId", "placeholderId", "placeholderTemplateId")


def identity(kind: str, number: int | str) -> str:
    return f"{PREFIX}{kind}.{number}"


def identifiers(values: list, field: str) -> set[int]:
    if not isinstance(values, list) or any(type(value) is not int or value < 0 for value in values):
        raise cache.InputError(f"Invalid nonnegative source IDs: {field}")
    if len(set(values)) != len(values):
        raise cache.InputError(f"Duplicate requested source ID: {field}")
    return set(values)


def record_index(records: list[dict]) -> dict[str, dict]:
    result = {}
    for record in records:
        key = record["asset_id"]
        if not key.startswith(PREFIX) or key in result:
            raise cache.InputError(f"Duplicate or foreign asset ID: {key}")
        if not key.startswith(PREFIX + record["kind"] + "."):
            raise cache.InputError(f"Asset kind/ID mismatch: {key}")
        result[key] = record
    return result


def input_record(path: Path) -> dict:
    if not path.is_file():
        raise cache.InputError(f"Missing closure input: {path}")
    return cache.file_record(path)


def required_inputs() -> tuple[dict, dict[str, set[int]], set[str], list[dict]]:
    paths = [
        ROOT / "content/m1/asset-references.json",
        ROOT / "content/m1/manifest.json",
        ROOT / "research/m1-bindings/item-bindings.json",
        ROOT / "research/m1-bindings/npc-bindings.json",
        ROOT / "research/m1-bindings/interface-bindings.json",
        ROOT / "research/m1-bindings/definitions.json.gz",
    ]
    values = [cache.read_json(path) for path in paths]
    assets, _, items, npcs, interfaces, _ = values
    by_kind = {kind: set() for kind in ("item", "npc", "model", "interface")}
    for kind, rows in (("item", items["items"]), ("npc", npcs)):
        for row in rows.values():
            number = row["source_id"]
            if row["source_asset"] != identity(kind, number):
                raise cache.InputError(f"Product binding/source ID mismatch: {kind}/{number}")
            by_kind[kind].add(number)
            by_kind["model"].update(identifiers(row["models"], f"{kind}/{number}/models"))
    for row in interfaces.values():
        by_kind["interface"].update(identifiers(row["source_groups"], "interface/source_groups"))
    existing = [row["id"] for row in assets["assets"]]
    if len(set(existing)) != len(existing) or any(not key.startswith(PREFIX) for key in existing):
        raise cache.InputError("Duplicate or foreign product asset reference")
    required = set(existing)
    for kind, numbers in by_kind.items():
        required.update(identity(kind, number) for number in numbers)
    return assets, by_kind, required, [input_record(path) for path in paths]


def plan(path: Path = REQUEST) -> dict:
    base = cache.read_json(BASE_BUNDLE)
    base_records = record_index(base["records"])
    assets, by_kind, required, inputs = required_inputs()
    reported = assets["source_closure_missing"]
    if set(reported) != set(FIELDS):
        raise cache.InputError("Unexpected compiled-content closure categories")
    definitions = ROOT / "research/m1-bindings/definitions.json.gz"
    snapshot = immutable_bytes(ROOT / "research/current-source/m1-content-closure-definitions.json.gz",
                               definitions.read_bytes())
    request = {
        "schema_version": 1, "cache_id": base["cache_id"], "game_revision": base["revision"],
        "scope": "Source views and source-data dependencies, not running the full game or presentation approval.",
        "base_bundle": input_record(BASE_BUNDLE), "base_publication": input_record(BASE_PUBLICATION),
        "selection": input_record(cache.DEFAULT_SELECTION),
        "decoder_lock": input_record(cache.TOOL / "dependencies.json"),
        "product_inputs": inputs, "product_definitions_snapshot": snapshot,
        "product_revision": cache.read_json(ROOT / "content/m1/manifest.json")["revision"],
        "product_inputs_policy": "Historical compiled-binding hashes; the immutable definition snapshot and exact IDs allow reproduction after parent binding refresh.",
        "reported_missing_ids": reported,
        "required_asset_ids": sorted(required), "audit_existing": {},
    }
    for field, (kind, key) in FIELDS.items():
        stated = identifiers(reported[field], field)
        measured = {number for number in by_kind[kind] if identity(kind, number) not in base_records}
        if stated != measured:
            raise cache.InputError(f"Stale product closure list {field}: reported={sorted(stated)}, actual={sorted(measured)}")
        request[key] = sorted(stated)
        if kind != "model":
            request["audit_existing"][key] = sorted(by_kind[kind] - stated)
    request["dependency_policy"] = (
        "Extract exact missing roots and re-decode already-published product item/NPC/interface roots "
        "to audit their transitive dependencies. Publish only new, nonconflicting asset IDs. "
        "Preserve every existing record/output hash; do not reaggregate the original collections."
    )
    validate_request(request, verify_product_inputs=True)
    immutable_json(path, request)
    return {"request": cache.file_record(path), "requested": {k: len(v) for k, v in reported.items()},
            "required_product_asset_ids": len(required)}


def validate_request(request: dict, verify_product_inputs: bool = False) -> dict:
    required_fields = {
        "schema_version", "cache_id", "game_revision", "base_bundle", "base_publication", "selection",
        "decoder_lock", "product_definitions_snapshot", "product_inputs", "reported_missing_ids",
        "required_asset_ids", "audit_existing", "item_ids", "npc_ids", "model_ids", "interface_groups",
    }
    if not isinstance(request, dict) or required_fields - request.keys():
        missing = sorted(required_fields - request.keys()) if isinstance(request, dict) else sorted(required_fields)
        raise cache.InputError("Incomplete content-closure request: missing " + ", ".join(missing))
    if (request["schema_version"], request["cache_id"], request["game_revision"]) != (1, 2695, 240):
        raise cache.InputError("Unsupported content-closure source identity/schema")
    inputs = [request["base_bundle"], request["base_publication"], request["selection"],
              request["decoder_lock"], request["product_definitions_snapshot"]]
    if verify_product_inputs:
        inputs += request["product_inputs"]
    for record in inputs:
        cache.checked_file(cache.within(ROOT, ROOT / record["path"]), record)
    base = cache.read_json(ROOT / request["base_bundle"]["path"])
    known = record_index(base["records"])
    for field, (kind, key) in FIELDS.items():
        numbers = identifiers(request[key], key)
        if numbers != identifiers(request["reported_missing_ids"][field], field):
            raise cache.InputError(f"Request differs from exact reported IDs: {key}")
        if any(identity(kind, number) in known for number in numbers):
            raise cache.InputError(f"Requested missing root is already in the base inventory: {key}")
        if kind != "model":
            existing = identifiers(request["audit_existing"][key], "audit_existing/" + key)
            if existing & numbers or any(identity(kind, number) not in known for number in existing):
                raise cache.InputError(f"Invalid existing-root audit: {key}")
    required = request["required_asset_ids"]
    if len(set(required)) != len(required) or any(not key.startswith(PREFIX) for key in required):
        raise cache.InputError("Duplicate or foreign required product asset ID")
    for field, (kind, key) in FIELDS.items():
        roots = request[key] + request["audit_existing"].get(key, [])
        if any(identity(kind, number) not in required for number in roots):
            raise cache.InputError(f"Requested/audited root is absent from frozen product references: {field}")
    return base


def merge_records(base: dict, extracted: dict) -> tuple[dict[str, dict], list[dict], list[str]]:
    if (base["cache_id"], base["revision"]) != (extracted["cache_id"], extracted["revision"]):
        raise cache.InputError("Source inventories belong to different caches/revisions")
    records = record_index(base["records"])
    additions, repeated = [], []
    for key, record in record_index(extracted["records"]).items():
        if key in records:
            if record != records[key]:
                raise cache.InputError(f"Existing original asset identity/output changed: {key}")
            repeated.append(key)
        else:
            additions.append(record)
            records[key] = record
    for key, group in extracted["groups"].items():
        if key in base["groups"] and group != base["groups"][key]:
            raise cache.InputError(f"Original archive CRC/full revision/hash changed: {key}")
    return records, sorted(additions, key=lambda row: row["asset_id"]), sorted(repeated)


def decoded_output(record: dict) -> dict | None:
    values = [output for output in record["outputs"] if output["path"].endswith((".json", ".json.gz"))]
    if len(values) > 1:
        raise cache.InputError(f"Ambiguous decoded source output: {record['asset_id']}")
    return values[0] if values else None


def dependencies(record: dict, value) -> list[tuple[str, str]]:
    kind = record["kind"]
    result = set()

    def add(target: str, number, field: str, unsigned: bool = False):
        if number is None or number == -1:
            return
        if type(number) is not int:
            raise cache.InputError(f"Noninteger source reference: {record['asset_id']}/{field}")
        if unsigned:
            number &= 65535
        if number < 0:
            raise cache.InputError(f"Invalid negative source reference: {record['asset_id']}/{field}")
        result.add((identity(target, number), field))

    def many(target: str, numbers, field: str, unsigned: bool = False):
        for number in numbers or []:
            add(target, number, field, unsigned)

    if kind == "item":
        for field in ITEM_MODELS:
            add("model", value[field], field)
        for field in ITEM_LINKS:
            add("item", value[field], field)
        if value["countObj"] is not None:
            if len(value["countObj"]) != len(value["countCo"]):
                raise cache.InputError("Item count variant/threshold cardinality mismatch")
            for index, (number, count) in enumerate(zip(value["countObj"], value["countCo"])):
                if count > 0:
                    add("item", number, f"countObj[{index}]")
        many("texture", value["textureReplace"], "textureReplace", True)
        add("font", 494, "native_item_icon_quantity_font")
    elif kind == "npc":
        for field in ("models", "chatheadModels"):
            many("model", value[field], field)
        many("npc", value["configs"], "configs")
        for field, number in value.items():
            if field.endswith("Animation"):
                add("sequence", number, field)
        many("sprite", value["headIconArchiveIds"], "headIconArchiveIds")
        many("texture", value["retextureToReplace"], "retextureToReplace", True)
    elif kind == "interface":
        for widget in value:
            prefix = f"widget[{widget['id'] & 65535}]."
            for field in ("spriteId", "alternateSpriteId"):
                add("sprite", widget[field], prefix + field)
            many("sprite", widget["sprites"], prefix + "sprites")
            add("font", widget["fontId"], prefix + "fontId")
            target = {1: "model", 2: "npc", 4: "item"}.get(widget["modelType"])
            if target:
                add(target, widget["modelId"], prefix + "modelId")
            add("model", widget["alternateModelId"], prefix + "alternateModelId")
            for field in ("animation", "alternateAnimation"):
                add("sequence", widget[field], prefix + field)
            for number in widget["itemIds"] or []:
                if number > 0:
                    add("item", number - 1, prefix + "itemIds_encoded_plus_one")
            for script in widget["clientScripts"] or []:
                for instruction in script or []:
                    if instruction["opcode"] in ("WIDGET_CONTAINS_ITEM_GET_QUANTITY", "WIDGET_CONTAINS_ITEM_STAR"):
                        add("item", instruction["operands"][2], prefix + "clientScripts.item")
    elif kind == "model":
        many("texture", value["model"]["faceTextures"], "faceTextures", True)
    elif kind == "texture":
        many("sprite", value["fileIds"], "fileIds")
    elif kind == "font":
        add("sprite", value["glyph_sprite_group"], "glyph_sprite_group")
    elif kind == "sequence":
        for field in ("frameIDs", "chatFrameIds"):
            for packed in value[field] or []:
                if packed >= 0:
                    result.add((identity("frame", f"{packed >> 16}.{packed & 65535}"), field))
        add("cached-animation", value["animMayaID"], "animMayaID")
        for field in ("leftHandItem", "rightHandItem"):
            number = value[field]
            if number >= 512:
                add("item", number - 512, field + "_equipment_encoding")
        for sounds in value["frameSounds"].values():
            for sound in sounds:
                add("sound", sound["id"], "frameSounds")
    elif kind == "frame":
        add("skeleton", value["framemap"]["id"], "framemap.id")
    elif kind == "cached-animation":
        add("skeleton", record["statistics"]["skeleton_id"], "skeleton_id")
    return sorted(result)


def validate_widgets(record: dict, widgets: list[dict], all_records: dict[str, dict], decoded: dict) -> dict:
    group = int(record["asset_id"].split(".")[-1])
    source_files = {source["file"] for source in record["source"]}
    ids = [widget["id"] for widget in widgets]
    if len(set(ids)) != len(ids) or {number & 65535 for number in ids} != source_files:
        raise cache.InputError(f"Duplicate/missing native widget file: interface/{group}")
    if any(number >> 16 != group for number in ids) or record["statistics"]["widgets"] != len(ids):
        raise cache.InputError(f"Widget group/count differs from source archive: {group}")
    glyphs = {}
    for widget in widgets:
        font_id = widget["fontId"]
        if font_id < 0:
            continue
        font_key = identity("font", font_id)
        if font_key not in decoded:
            raise cache.InputError(f"Missing interface font metrics: {group}/{font_id}")
        font = decoded[font_key]
        if (font["glyph_sprite_group"] != font_id or len(font["advances"]) != 256
                or sum(font["advances"]) <= 0 or font["ascent"] <= 0):
            raise cache.InputError(f"Invalid interface/font glyph reference: {group}/{font_id}")
        sprite_key = identity("sprite", font_id)
        sprite = all_records.get(sprite_key)
        if not sprite or sprite["statistics"]["frames"] != 256 or sprite_key not in decoded:
            raise cache.InputError(f"Missing interface font glyph atlas: {group}/{font_id}")
        frames = decoded[sprite_key]["frames"]
        if sorted(frame["frame"] for frame in frames) != list(range(256)):
            raise cache.InputError(f"Duplicate/missing native glyph index: {font_id}")
        width, height = sprite["statistics"]["atlas_width"], sprite["statistics"]["atlas_height"]
        for frame in frames:
            x, y, w, h = (frame[key] for key in ("atlas_x", "atlas_y", "width", "height"))
            if min(x, y, w, h) < 0 or x + w > width or y + h > height:
                raise cache.InputError(f"Glyph lies outside original atlas: {font_id}/{frame['frame']}")
            if len(frame["pixel_indices_unsigned"]) != w * h:
                raise cache.InputError(f"Glyph palette-index dimensions differ: {font_id}/{frame['frame']}")
        glyphs[str(font_id)] = {"glyphs": 256, "atlas_width": width, "atlas_height": height,
                               "ascent": font["ascent"], "source_asset_id": font_key}
    return {"interface_group": group, "widgets": len(ids), "glyph_references": glyphs}


def check_graph(records: dict[str, dict], decoded: dict, extracted_ids: set[str],
                request: dict) -> tuple[list[dict], list[dict]]:
    edges, missing, interfaces = [], set(), []
    for key in sorted(extracted_ids):
        record = records[key]
        for target, field in dependencies(record, decoded.get(key)):
            edges.append({"from": key, "field": field, "to": target})
            if target not in records:
                missing.add(target)
        if record["kind"] == "interface":
            interfaces.append(validate_widgets(record, decoded[key], records, decoded))
    missing.update(set(request["required_asset_ids"]) - records.keys())
    if missing:
        raise cache.InputError("Missing original content dependencies: " + json.dumps(sorted(missing)))
    return edges, interfaces


def validate_definitions(extracted: dict, decoded: dict, request: dict) -> dict:
    entry = request["product_definitions_snapshot"]
    supplement = cache.read_json(cache.checked_file(ROOT / entry["path"], entry))
    for group, other in (("2/10", supplement["item_group"]), ("2/9", supplement["npc_source"]["group"])):
        if extracted["groups"][group] != other:
            raise cache.InputError(f"Product definition archive identity/revision differs: {group}")
    records = record_index(extracted["records"])
    compared = collections.Counter()
    for kind in ("item", "npc"):
        for number, reference in supplement[kind + "s"].items():
            key = identity(kind, number)
            if key not in decoded:
                continue
            expected = reference["definition"] if kind == "item" else reference
            if decoded[key] != expected:
                raise cache.InputError(f"Decoded original definition differs from product source bytes: {key}")
            if kind == "item":
                source = records[key]["source"][0]
                if (source["sha256"], source["size_bytes"]) != (reference["payload_sha256"], reference["payload_size_bytes"]):
                    raise cache.InputError(f"Product item payload hash differs: {key}")
            compared[kind] += 1
    for field, kind in (("item_ids", "item"), ("npc_ids", "npc")):
        if any(identity(kind, number) not in decoded for number in request[field]):
            raise cache.InputError(f"Required original definitions are not decoded: {field}")
    return dict(compared)


def inspect_extraction(directory: Path) -> tuple[dict, dict, dict, list, list, dict]:
    extracted = cache.read_json(directory / "bundle.json")
    if extracted.get("scope") != "content-asset-closure":
        raise cache.InputError("Not a bounded content-closure extraction")
    request = extracted["request"]
    base = validate_request(request)
    records, additions, repeated = merge_records(base, extracted)
    decoded = {}
    for record in extracted["records"]:
        output = decoded_output(record)
        if output:
            decoded[record["asset_id"]] = cache.read_json(cache.checked_file(directory / output["path"], output))
    return base, extracted, records, additions, repeated, decoded


def validate_extraction(directory: Path) -> dict:
    validation = cache.validate_bundle(directory)
    base, extracted, records, additions, repeated, decoded = inspect_extraction(directory)
    request = extracted["request"]
    edges, interfaces = check_graph(records, decoded, set(record_index(extracted["records"])), request)
    definitions = validate_definitions(extracted, decoded, request)
    requested = {field: len(request["reported_missing_ids"][field]) for field in FIELDS}
    result = {
        "schema_version": 1, "result": "passed", "scope": "Original source data, not running the full game.",
        "requested": requested, "resolved": requested,
        "remaining_missing_inputs": [], "required_product_asset_ids": len(request["required_asset_ids"]),
        "resolved_product_asset_ids": len(set(request["required_asset_ids"]) & records.keys()),
        "new_asset_counts": dict(sorted(collections.Counter(row["kind"] for row in additions).items())),
        "new_asset_ids": [row["asset_id"] for row in additions],
        "redecoded_existing_assets_identical": len(repeated),
        "unchanged_base_inventory_records": len(base["records"]),
        "matching_product_original_definitions": definitions,
        "dependency_edges": len(edges), "interfaces_checked": interfaces,
        "new_model_geometry": [{"asset_id": row["asset_id"], **row["statistics"]}
                               for row in additions if row["kind"] == "model"],
        "validation": validation,
        "dynamic_state_boundary": extracted["closure"]["dynamic_interface_state"],
        "owner_reference_pack_approved": False, "source_gameplay_or_presentation_accepted": False,
    }
    cache.write_json(directory / "dependency-graph.json.gz", {"schema_version": 1, "edges": edges})
    cache.write_json(directory / "closure-report.json", result)
    return result


def encoded_json(path: Path, value) -> bytes:
    raw = (json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
    return gzip.compress(raw, mtime=0) if path.suffix == ".gz" else raw


def immutable_bytes(path: Path, data: bytes) -> dict:
    path = cache.within(ROOT, path)
    if path.exists():
        expected = {"size_bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
        cache.checked_file(path, expected)
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("xb") as stream:
            stream.write(data)
    return cache.file_record(path)


def immutable_json(path: Path, value) -> dict:
    return immutable_bytes(path, encoded_json(path, value))


def publish(directory: Path, request_path: Path = REQUEST) -> dict:
    report = validate_extraction(directory)
    base, extracted, records, additions, repeated, decoded = inspect_extraction(directory)
    if cache.read_json(request_path) != extracted["request"]:
        raise cache.InputError("Publication request differs from the actual extraction")
    destination = SOURCE / "content-v2"
    published = []
    for record in additions:
        for output in record["outputs"]:
            original = cache.checked_file(directory / output["path"], output)
            target = cache.within(destination, destination / output["path"])
            published.append({**immutable_bytes(target, original.read_bytes()),
                              "source_asset_id": record["asset_id"], "extraction_path": output["path"]})
    collections_by_kind = {}
    for record in additions:
        if record["kind"] in {"item", "npc", "sequence", "frame", "skeleton", "sound", "texture"}:
            collections_by_kind.setdefault(record["kind"], {})[record["asset_id"]] = decoded[record["asset_id"]]
    shards = {}
    for kind, values in sorted(collections_by_kind.items()):
        shards[kind] = {**immutable_json(destination / "collections" / f"{kind}.json.gz", values),
                        "source_records": len(values), "transformation": "JSON aggregation only; original decoded fields unchanged."}
    merged = dict(base)
    merged["records"] = base["records"] + additions
    merged["groups"] = {**base["groups"], **extracted["groups"]}
    merged["counts"] = dict(sorted(collections.Counter(row["kind"] for row in merged["records"]).items()))
    merged["counts"]["placed_objects"] = base["counts"]["placed_objects"]
    merged["extensions"] = [{"scope": extracted["scope"], "request": extracted["request"],
                             "new_asset_ids": [row["asset_id"] for row in additions],
                             "source_bundle": cache.file_record(directory / "bundle.json")}]
    merged["closure"] = {**base["closure"], "compiled_content_v2_missing_inputs": [],
                         "compiled_content_v2_scope": extracted["closure"]}
    merged_record = immutable_json(MANIFESTS / "cache2695-content-v2-bundle.json.gz", merged)
    extracted_record = immutable_bytes(MANIFESTS / "cache2695-content-v2-extraction.json.gz",
                                       gzip.compress((directory / "bundle.json").read_bytes(), mtime=0))
    graph = immutable_bytes(destination / "dependency-graph.json.gz", (directory / "dependency-graph.json.gz").read_bytes())
    report["merged_inventory"] = merged_record
    report_record = immutable_json(ROOT / "research/current-source/m1-content-closure.json", report)
    publication = {
        "schema_version": 1, "cache_id": 2695, "game_revision": 240, "scope": extracted["scope"],
        "base_bundle": extracted["request"]["base_bundle"],
        "base_publication": extracted["request"]["base_publication"],
        "merged_inventory": merged_record, "extraction_inventory": extracted_record,
        "request": cache.file_record(request_path), "closure_report": report_record,
        "published_files": published, "collection_extensions": shards, "dependency_graph": graph,
        "reproduction": [
            "python3 tools/cache-import/import_cache.py reuse --reuse-source /path/to/verified/current-source",
            "python3 tools/cache-import/import_cache.py plan-closure",
            "python3 tools/cache-import/import_cache.py extract-closure",
            "python3 tools/cache-import/import_cache.py publish-closure",
            "python3 tools/cache-import/import_cache.py validate-published",
        ],
        "original_full_output_default_directory": ".local/current-source/extracted",
        "extension_full_output_default_directory": ".local/current-source/content-closure",
        "new_assets_all_outputs_published": True, "existing_source_files_replaced": 0,
        "existing_inventory_records_preserved": len(base["records"]),
        "redecoded_existing_assets_identical": len(repeated),
        "source_gameplay_or_presentation_accepted": False, "owner_reference_pack_approved": False,
    }
    immutable_json(PUBLICATION, publication)
    result = validate_publication(PUBLICATION)
    result["publication_manifest"] = cache.file_record(PUBLICATION)
    result["closure_report"] = report_record
    return result


def validate_publication(path: Path = PUBLICATION) -> dict:
    manifest = cache.read_json(path)
    if (manifest["schema_version"], manifest["cache_id"], manifest["game_revision"]) != (1, 2695, 240):
        raise cache.InputError("Published closure source identity/schema changed")
    for record in [manifest["base_bundle"], manifest["base_publication"], manifest["merged_inventory"],
                   manifest["extraction_inventory"], manifest["request"], manifest["closure_report"],
                   manifest["dependency_graph"], *manifest["collection_extensions"].values()]:
        cache.checked_file(cache.within(ROOT, ROOT / record["path"]), record)
    request = cache.read_json(ROOT / manifest["request"]["path"])
    base = validate_request(request)
    extracted = cache.read_json(ROOT / manifest["extraction_inventory"]["path"])
    if extracted["request"] != request:
        raise cache.InputError("Published extraction request changed")
    records, additions, repeated = merge_records(base, extracted)
    merged = cache.read_json(ROOT / manifest["merged_inventory"]["path"])
    if record_index(merged["records"]) != records or merged["records"][:len(base["records"])] != base["records"]:
        raise cache.InputError("Merged catalog dropped, duplicated or altered original assets")
    if merged["groups"] != {**base["groups"], **extracted["groups"]}:
        raise cache.InputError("Merged source archive identities changed")
    merged_counts = dict(collections.Counter(row["kind"] for row in records.values()))
    merged_counts["placed_objects"] = base["counts"]["placed_objects"]
    if merged["counts"] != merged_counts or extracted["counts"] != dict(collections.Counter(row["kind"] for row in extracted["records"])):
        raise cache.InputError("Published source inventory counts disagree with actual records")
    expected, extraction_paths = {}, set()
    for row in additions:
        for output in row["outputs"]:
            key = (row["asset_id"], output["path"])
            if key in expected or output["path"] in extraction_paths:
                raise cache.InputError("Duplicate closure inventory output path")
            expected[key] = output
            extraction_paths.add(output["path"])
    seen, paths, decoded = set(), set(), {}
    for published in manifest["published_files"]:
        key = (published["source_asset_id"], published["extraction_path"])
        if key not in expected or key in seen or published["path"] in paths:
            raise cache.InputError("Unrequested or duplicate published closure output")
        seen.add(key)
        paths.add(published["path"])
        output = expected[key]
        if (published["size_bytes"], published["sha256"]) != (output["size_bytes"], output["sha256"]):
            raise cache.InputError("Published closure output differs from original extraction")
        target = cache.checked_file(cache.within(SOURCE / "content-v2", ROOT / published["path"]), published)
        if target.suffix == ".png":
            stats = records[key[0]]["statistics"]
            if cache.validate_png(target.read_bytes()) != (stats["atlas_width"], stats["atlas_height"]):
                raise cache.InputError("Published sprite atlas dimensions changed")
        if "json_sha256" in output:
            raw = gzip.decompress(target.read_bytes())
            if (len(raw), hashlib.sha256(raw).hexdigest()) != (output["json_size_bytes"], output["json_sha256"]):
                raise cache.InputError("Published decoded JSON integrity mismatch")
        if target.name.endswith((".json", ".json.gz")):
            decoded[key[0]] = cache.read_json(target)
        if records[key[0]]["kind"] == "model" and target.name.endswith(".json.gz"):
            cache.validate_model(decoded[key[0]], records[key[0]]["statistics"])
        if records[key[0]]["kind"] == "frame" and target.suffix == ".json":
            cache.validate_frame(decoded[key[0]])
    if seen != expected.keys():
        raise cache.InputError("Missing published closure files: " + repr(sorted(expected.keys() - seen)))
    original = cache.read_json(ROOT / manifest["base_publication"]["path"])
    for published in original["published_files"]:
        cache.checked_file(cache.within(SOURCE, ROOT / published["path"]), published)
    for record in extracted["records"]:
        key = record["asset_id"]
        if key in decoded:
            continue
        output = decoded_output(record)
        if not output:
            continue
        kind = record["kind"]
        collection = SOURCE / "collections" / f"{kind}.json.gz"
        if collection.is_file():
            if kind not in decoded:
                decoded[kind] = cache.read_json(collection)
            decoded[key] = decoded[kind][key]
        elif (SOURCE / output["path"]).is_file():
            decoded[key] = cache.read_json(cache.checked_file(SOURCE / output["path"], output))
        elif kind == "model":
            # Repeated model payloads stay in the unchanged full extraction; their texture edges are hash-locked below.
            continue
        else:
            raise cache.InputError(f"Missing previously published dependency data: {key}")
    definition_matches = validate_definitions(extracted, decoded, request)
    edges = cache.read_json(ROOT / manifest["dependency_graph"]["path"])["edges"]
    expected_edges = []
    interface_checks = {}
    extracted_records = record_index(extracted["records"])
    for key, record in extracted_records.items():
        if key in decoded or record["kind"] == "cached-animation":
            expected_edges += [{"from": key, "field": field, "to": target}
                               for target, field in dependencies(record, decoded.get(key))]
        if record["kind"] == "interface":
            check = validate_widgets(record, decoded[key], records, decoded)
            interface_checks[check["interface_group"]] = check
    edge_set = {(row["from"], row["field"], row["to"]) for row in edges}
    if len(edge_set) != len(edges):
        raise cache.InputError("Duplicate dependency graph edge")
    if any(row["from"] not in extracted_records or row["to"] not in records for row in edges):
        raise cache.InputError("Missing source dependency graph endpoint")
    if not {(row["from"], row["field"], row["to"]) for row in expected_edges} <= edge_set:
        raise cache.InputError("Published graph omits a decoded source reference")
    for kind, shard in manifest["collection_extensions"].items():
        values = cache.read_json(ROOT / shard["path"])
        expected_values = {row["asset_id"]: decoded[row["asset_id"]] for row in additions if row["kind"] == kind}
        if values != expected_values or len(values) != shard["source_records"]:
            raise cache.InputError(f"Published collection shard differs from original definitions: {kind}")
    missing = sorted(set(request["required_asset_ids"]) - records.keys())
    report = cache.read_json(ROOT / manifest["closure_report"]["path"])
    counts = dict(sorted(collections.Counter(row["kind"] for row in additions).items()))
    requested = {field: len(request["reported_missing_ids"][field]) for field in FIELDS}
    if missing or report["remaining_missing_inputs"] or report["requested"] != requested or report["resolved"] != requested:
        raise cache.InputError("Published content closure is incomplete or misreported")
    if counts != report["new_asset_counts"] or report["new_asset_ids"] != [row["asset_id"] for row in additions]:
        raise cache.InputError("Published content closure counts/IDs disagree")
    model_geometry = [{"asset_id": row["asset_id"], **row["statistics"]} for row in additions if row["kind"] == "model"]
    reported_interfaces = {row["interface_group"]: row for row in report["interfaces_checked"]}
    if (report["new_model_geometry"] != model_geometry or reported_interfaces != interface_checks
            or len(reported_interfaces) != len(report["interfaces_checked"])
            or report["matching_product_original_definitions"] != definition_matches
            or report["required_product_asset_ids"] != len(request["required_asset_ids"])
            or report["resolved_product_asset_ids"] != len(request["required_asset_ids"])
            or report["dependency_edges"] != len(edges)):
        raise cache.InputError("Published content report differs from verified geometry/glyph/dependency evidence")
    return {"result": "passed", "requested": requested, "resolved": requested,
            "remaining_missing_inputs": missing, "new_asset_counts": counts,
            "new_published_files": len(paths), "new_published_bytes": sum(row["size_bytes"] for row in manifest["published_files"]),
            "merged_inventory_records": len(records), "redecoded_existing_assets_identical": len(repeated),
            "unchanged_original_published_files": len(original["published_files"]),
            "source_gameplay_or_presentation_accepted": False}


def load_published_inputs(path: Path = PUBLICATION) -> tuple[dict, dict[str, dict]]:
    """Parent content builders can consume the merged catalog plus original collection shards."""
    validate_publication(path)
    publication = cache.read_json(path)
    bundle = cache.read_json(ROOT / publication["merged_inventory"]["path"])
    values = {file.name.split(".")[0]: cache.read_json(file) for file in sorted((SOURCE / "collections").glob("*.json.gz"))}
    for kind, shard in publication["collection_extensions"].items():
        additions = cache.read_json(ROOT / shard["path"])
        collection = values.setdefault(kind, {})
        if collection.keys() & additions.keys():
            raise cache.InputError(f"Duplicate asset ID in collection extension: {kind}")
        collection.update(additions)
    return bundle, values

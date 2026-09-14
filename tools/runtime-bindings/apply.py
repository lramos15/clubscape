#!/usr/bin/env python3
"""Prepare an owned candidate; never overwrite product content or source evidence."""

import argparse
from copy import deepcopy
import gzip
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "research/runtime-bindings"


def canonical_hash(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def at(root, pointer):
    value = root
    for key in pointer:
        value = value[key]
    return value


def put(root, pointer, value):
    parent = at(root, pointer[:-1])
    parent[pointer[-1]] = deepcopy(value)


def prepare(original, report):
    candidate = deepcopy(original)
    for path, record in report["resolutions"].items():
        current = at(candidate, record["pointer"])
        if canonical_hash(current) != record["original_binding_sha256"]:
            raise ValueError("Input binding changed; reconcile explicitly: " + path)
        if record["apply_to_ordinary_profile"]:
            if record["classification"] not in {"known_fact", "source_supported_inference", "codebug_parent_fixed"}:
                raise ValueError("Refusing to promote a decision/inactive/unsupported value: " + path)
            put(candidate, record["pointer"], record["replacement"])
    for change in report["coupled_updates"]:
        parent = at(candidate, change["pointer"][:-1])
        key = change["pointer"][-1]
        if change["operation"] == "add":
            if key in parent:
                raise ValueError("Coupled addition already exists: " + change["id"])
        elif canonical_hash(parent[key]) != change["original_value_sha256"]:
            raise ValueError("Coupled field changed; reconcile explicitly: " + change["id"])
        put(candidate, change["pointer"], change["value"])
    return candidate


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", default="content/m1/game-content.json.gz")
    parser.add_argument("--output", default="research/runtime-bindings/.local/game-content.candidate.json")
    parser.add_argument("--allow-unrelated-input-changes", action="store_true",
                        help="Still require every exact target/coupled field to match its original hash.")
    args = parser.parse_args()
    source = (ROOT / args.input).resolve()
    destination = (ROOT / args.output).resolve()
    if not destination.is_relative_to(OUT) or destination.suffix != ".json":
        parser.error("--output must be a JSON file inside research/runtime-bindings/")
    if destination.name in {"resolutions.json", "sources.json", "oracles.json", "death-values.json", "profile-resolutions.json"}:
        parser.error("Refusing to overwrite a contract/evidence input")
    payload = source.read_bytes()
    report = json.loads((OUT / "resolutions.json").read_text())
    if not args.allow_unrelated_input_changes and hashlib.sha256(payload).hexdigest() != report["inputs"]["content_sha256"]:
        raise ValueError("Input artifact differs; inspect, then use --allow-unrelated-input-changes only for unrelated edits")
    original = json.loads(gzip.decompress(payload) if source.suffix == ".gz" else payload)
    candidate = prepare(original, report)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(candidate, separators=(",", ":"), sort_keys=True) + "\n")
    print(json.dumps({
        "candidate": str(destination.relative_to(ROOT)),
        "bound_paths": sum(r["apply_to_ordinary_profile"] for r in report["resolutions"].values()),
        "coupled_updates": len(report["coupled_updates"]),
        "remaining_dispositions_not_promoted": [p for p, r in report["resolutions"].items() if not r["apply_to_ordinary_profile"]],
        "product_modified": False, "runtime_journey_passed": False,
    }, indent=2))


if __name__ == "__main__":
    main()

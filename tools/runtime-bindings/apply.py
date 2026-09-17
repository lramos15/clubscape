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


def prepare_with_audit(original, report, context=None):
    if context is not None and context["resolutions_sha256"] != canonical_hash(report):
        raise ValueError("Application context belongs to a different source-resolution report")
    candidate = deepcopy(original)
    audit = {"bindings": [], "coupled_updates": []}
    for path, record in report["resolutions"].items():
        current = at(candidate, record["pointer"])
        current_hash = canonical_hash(current)
        replacement = record["replacement"]
        exact_original = current_hash == record["original_binding_sha256"]
        exact_replacement = replacement is not None and current_hash == canonical_hash(replacement)
        accepted = (context or {}).get("accepted_current_targets", {}).get(path)
        accepted_current = accepted is not None and current_hash == accepted["sha256"]
        if accepted_current:
            if accepted["original_binding_sha256"] != record["original_binding_sha256"]:
                raise ValueError("Application target source fingerprint changed: " + path)
            if accepted["reason"] == "parent_bound_semantic_replacement":
                if (record["classification"] != "codebug_parent_fixed"
                        or set(current) != {"status", "value", "source"}
                        or current["status"] != "bound" or current["value"] != record["proposed_value"]):
                    raise ValueError("Parent-bound source value is not the exact semantic replacement: " + path)
            elif accepted["reason"] == "asset_provenance_only":
                if (set(current) != {"status", "reason", "source"} or current["status"] != "unresolved"
                        or current["reason"] != record["original_reason"]):
                    raise ValueError("Asset refresh changed source-target semantics: " + path)
            else:
                raise ValueError("Unknown application fingerprint exception: " + path)
        if not (exact_original or exact_replacement or accepted_current):
            raise ValueError("Input binding changed; reconcile explicitly: " + path)
        if record["apply_to_ordinary_profile"]:
            if record["classification"] not in {"known_fact", "source_supported_inference", "codebug_parent_fixed"}:
                raise ValueError("Refusing to promote a decision/inactive/unsupported value: " + path)
            already = exact_replacement or (accepted_current and accepted["reason"] == "parent_bound_semantic_replacement")
            if not already:
                put(candidate, record["pointer"], replacement)
            action = "already_applied" if already else "applied"
        else:
            action = "retained_" + record["classification"]
        audit["bindings"].append({
            "path": path, "classification": record["classification"], "action": action,
            "input_sha256": current_hash, "original_binding_sha256": record["original_binding_sha256"],
            "output_sha256": canonical_hash(at(candidate, record["pointer"])),
            "accepted_current_fingerprint": accepted_current,
        })
    for change in report["coupled_updates"]:
        parent = at(candidate, change["pointer"][:-1])
        key = change["pointer"][-1]
        present = key in parent
        already = present and canonical_hash(parent[key]) == canonical_hash(change["value"])
        if not already and change["operation"] == "add":
            if present:
                raise ValueError("Coupled addition already exists: " + change["id"])
        elif not already and (not present or canonical_hash(parent[key]) != change["original_value_sha256"]):
            raise ValueError("Coupled field changed; reconcile explicitly: " + change["id"])
        before = canonical_hash(parent[key]) if present else None
        if not already:
            put(candidate, change["pointer"], change["value"])
        audit["coupled_updates"].append({
            "id": change["id"], "pointer": change["pointer"], "operation": change["operation"],
            "action": "already_applied" if already else "applied",
            "input_sha256": before, "output_sha256": canonical_hash(change["value"]),
            "classification": change["classification"],
        })
    return candidate, audit


def prepare(original, report, context=None):
    return prepare_with_audit(original, report, context)[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", default="content/m1/game-content.json.gz")
    parser.add_argument("--output", default="research/runtime-bindings/.local/game-content.candidate.json")
    parser.add_argument("--allow-unrelated-input-changes", action="store_true",
                        help="Still require every exact target/coupled field to match its original hash.")
    parser.add_argument("--application-context", help="An exact, audited application*.json target fingerprint context.")
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
    context = None
    if args.application_context:
        context_path = (ROOT / args.application_context).resolve()
        if context_path.parent != OUT or not context_path.name.startswith("application") or context_path.suffix != ".json":
            parser.error("Application context must be an owned research/runtime-bindings/application*.json file")
        context = json.loads(context_path.read_text())
    candidate, audit = prepare_with_audit(original, report, context)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(candidate, separators=(",", ":"), sort_keys=True) + "\n")
    print(json.dumps({
        "candidate": str(destination.relative_to(ROOT)),
        "bound_paths": sum(r["apply_to_ordinary_profile"] for r in report["resolutions"].values()),
        "coupled_updates": len(report["coupled_updates"]),
        "remaining_dispositions_not_promoted": [p for p, r in report["resolutions"].items() if not r["apply_to_ordinary_profile"]],
        "product_modified": False, "runtime_journey_passed": False,
        "already_applied": sum(row["action"] == "already_applied" for row in audit["bindings"]),
    }, indent=2))


if __name__ == "__main__":
    main()

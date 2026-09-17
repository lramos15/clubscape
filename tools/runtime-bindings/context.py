#!/usr/bin/env python3
"""Freeze the narrowly audited parent/asset changes before canonical source application."""

import gzip
import hashlib
import json
from pathlib import Path
import subprocess

from apply import at, canonical_hash


ROOT = Path(__file__).resolve().parents[2]
DIRECTORY = ROOT / "research/runtime-bindings"
OUTPUT = DIRECTORY / "application-context.json"


def main():
    if OUTPUT.exists():
        raise ValueError("Application context is immutable; reconcile a new baseline explicitly.")
    report = json.loads((DIRECTORY / "resolutions.json").read_text())
    original_bytes = subprocess.check_output(["git", "show", report["base"] + ":" + report["inputs"]["content_path"]], cwd=ROOT)
    if hashlib.sha256(original_bytes).hexdigest() != report["inputs"]["content_sha256"]:
        raise ValueError("Original source content snapshot does not match its resolution fingerprint")
    original = json.loads(gzip.decompress(original_bytes))
    current_bytes = (ROOT / report["inputs"]["content_path"]).read_bytes()
    current = json.loads(gzip.decompress(current_bytes))
    accepted = {}
    for path, row in report["resolutions"].items():
        previous, actual = at(original, row["pointer"]), at(current, row["pointer"])
        if canonical_hash(previous) != row["original_binding_sha256"]:
            raise ValueError("Original target fingerprint changed: " + path)
        if canonical_hash(actual) == row["original_binding_sha256"]:
            continue
        if row["classification"] == "codebug_parent_fixed":
            if actual.get("status") != "bound" or actual.get("value") != row["proposed_value"]:
                raise ValueError("Parent fix did not preserve the exact source value: " + path)
            reason = "parent_bound_semantic_replacement"
        else:
            if {key: value for key, value in previous.items() if key != "source"} != {
                    key: value for key, value in actual.items() if key != "source"}:
                raise ValueError("Non-provenance source target changed: " + path)
            old_sources = [{key: value for key, value in record.items() if key != "reference"} for record in previous["source"]]
            new_sources = [{key: value for key, value in record.items() if key != "reference"} for record in actual["source"]]
            if old_sources != new_sources:
                raise ValueError("Source-target evidence changed beyond the asset locator: " + path)
            for old, new in zip(previous["source"], actual["source"], strict=True):
                if old["reference"] != new["reference"] and not (
                        old["reference"].startswith("research/m1-bindings/definitions.json.gz#")
                        and new["reference"].startswith("assets/source/osrs/cache2695/content-v2/collections/")):
                    raise ValueError("Unexpected source provenance replacement: " + path)
            reason = "asset_provenance_only"
        accepted[path] = {
            "sha256": canonical_hash(actual), "original_binding_sha256": row["original_binding_sha256"],
            "reason": reason, "current_source": actual["source"],
        }
    for change in report["coupled_updates"]:
        parent = at(current, change["pointer"][:-1])
        key = change["pointer"][-1]
        if change["operation"] == "add":
            if key in parent:
                raise ValueError("Coupled addition unexpectedly exists in parent baseline")
        elif canonical_hash(parent[key]) != change["original_value_sha256"]:
            raise ValueError("Coupled source target changed: " + change["id"])
    document = {
        "schema_version": 1, "parent_base": "0aea6c0", "original_source_base": report["base"],
        "resolutions_sha256": canonical_hash(report),
        "original_content_sha256": report["inputs"]["content_sha256"],
        "current_content_sha256": hashlib.sha256(current_bytes).hexdigest(),
        "accepted_current_targets": accepted,
        "policy": "Every source/coupled target retains an exact fingerprint. Exceptions are only the audited "
                  "asset-locator refreshes and the parent's identical bound Wind Strike semantic value. "
                  "No broad changed-field tolerance or source reclassification is authorized.",
    }
    OUTPUT.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"exact_current_target_exceptions": len(accepted),
                      "asset_locator_only": sum(row["reason"] == "asset_provenance_only" for row in accepted.values()),
                      "already_bound_source_values": sum(row["reason"] == "parent_bound_semantic_replacement" for row in accepted.values())}))


if __name__ == "__main__":
    main()

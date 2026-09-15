#!/usr/bin/env python3
"""Archive completed, explicitly component-scoped UI/source evidence without accepting M1."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[2]
RESULTS = ROOT / "web/ui/test-results"
EVIDENCE = ROOT / "web/ui/evidence"
NATIVE = ROOT / "tools/ui-assets/.cache/native"


def read(path):
    return json.loads(path.read_text())


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def passed(name, count):
    report = read(RESULTS / name)
    rows = report.get("results", report.get("cases", []))
    if len(rows) != count or not all(row["passed"] for row in rows) or report.get("errors"):
        raise ValueError(f"Incomplete or failed evidence: {name}")
    return report


def main():
    comparisons = {
        "source": passed("source-comparison.json", 87),
        "modes": passed("mode-comparison.json", 106),
        "presentations": passed("presentation-comparison.json", 45),
    }
    components = passed("component-tests.json", 20)
    versioned = passed("gameplay-ui-v1-tests.json", 15)
    glyphs = read(RESULTS / "glyph-proof.json")
    if glyphs["failures"] or glyphs["checkedSpriteFrames"] < 1099 or glyphs["checkedFontGlyphs"] != 1024:
        raise ValueError("Original glyph/frame proof is incomplete")
    tap = (RESULTS / "unit.tap").read_text()
    tests = re.search(r"^# tests (\d+)$", tap, re.MULTILINE)
    passes = re.search(r"^# pass (\d+)$", tap, re.MULTILINE)
    failures = re.search(r"^# fail (\d+)$", tap, re.MULTILINE)
    if not tests or not passes or not failures or int(tests[1]) != 28 or tests[1] != passes[1] or failures[1] != "0":
        raise ValueError("Expected the complete passing unit TAP report")
    if (RESULTS / "typecheck.log").read_text().strip():
        raise ValueError("TypeScript diagnostics remain")
    strict = read(ROOT / "tools/ui-assets/.cache/source-validation.json")
    if not strict["complete_reference_pack"] or strict["hash_bound_files_checked"] != 1307:
        raise ValueError("Frozen source validation is incomplete")

    for name in ("component-tests.json", "source-comparison.json", "source-captures.json", "mode-comparison.json", "glyph-proof.json"):
        copy(RESULTS / name, EVIDENCE / name)
    copy(ROOT / "tools/ui-assets/.cache/source-validation.json", EVIDENCE / "strict-pack-validation.json")
    for path in (RESULTS / "components").glob("*.png"):
        copy(path, EVIDENCE / "components" / path.name)
    for path in (RESULTS / "source").glob("*.png"):
        copy(path, EVIDENCE / "source" / path.name)
    for name in ("gameplay-ui-v1-tests.json", "typecheck.log", "unit.tap"):
        copy(RESULTS / name, EVIDENCE / "versioned" / name)
    for path in (RESULTS / "versioned").glob("*.png"):
        copy(path, EVIDENCE / "versioned" / path.name)

    archive = EVIDENCE / "native-presentations"
    references = []
    browser = read(RESULTS / "presentation-browser.json")
    if browser["errors"] or len(browser["names"]) != 23:
        raise ValueError("Native presentation browser evidence is incomplete")
    for name in browser["names"]:
        if Path(name).name != name:
            raise ValueError("Unexpected source case path")
        source = NATIVE / "ui-only" / (name + ".png")
        if not source.exists():
            source = archive / "original/ui" / (name + ".png")
        if source != archive / "original/ui" / (name + ".png"):
            copy(source, archive / "original/ui" / source.name)
        references.append({"kind": "ui", "name": source.name, "sha256": digest(source), "bytes": source.stat().st_size})
    for kind in ("presentations", "presentation-projections"):
        for path in (RESULTS / kind).glob("*.png"):
            copy(path, archive / kind / path.name)
    copy(RESULTS / "presentation-comparison.json", archive / "comparison.json")
    copy(RESULTS / "presentation-browser.json", archive / "browser.json")
    native_inputs = NATIVE / "presentation-inputs.json"
    write(archive / "source-inputs.json", {
        "scope": "Original controlled UI widget/model fields; synthetic fixture quantities, never production balances or rewards",
        "references": references,
        "inputs": read(native_inputs) if native_inputs.exists() else read(archive / "source-inputs.json")["inputs"],
        "sourcePackSha256": glyphs["sourcePackSha256"],
        "finalAcceptance": False,
    })
    manifest = read(ROOT / "assets/compiled/ui/manifest.json")
    owned_sources = sorted([
        *ROOT.glob("web/ui/*.ts"), *ROOT.glob("web/ui/tests/*.ts"), *ROOT.glob("web/ui/tests/*.mjs"),
        *ROOT.glob("tools/ui-assets/*.java"), *ROOT.glob("tools/ui-assets/*.py"),
    ])
    summary = {
        "scope": "UI component and original-source validation only. No real-server journey, final visual/audio or hardware acceptance.",
        "status": "blocked",
        "implementation_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_pack_sha256": glyphs["sourcePackSha256"],
        "published_contract_upstream": "d1532d6fc063d2a02383a7b4bb1ca3c449e214ce",
        "contract_implementation_inferred_from_types": False,
        "typecheck": {"passed": True, "command": "pnpm exec tsc --noEmit"},
        "unit": {"passed": int(passes[1]), "total": int(tests[1])},
        "component_browser": {"legacy": len(components["cases"]), "versioned": len(versioned["cases"]), "browser": versioned["browser"]},
        "source_comparisons": {kind: {"passed": len(report["results"]), "total": len(report["results"]), "tolerance": 0}
                               for kind, report in comparisons.items()},
        "original_frames": glyphs,
        "strict_source_hashes_checked": strict["hash_bound_files_checked"],
        "compiled_assets": {
            "sprites": len(manifest["sprites"]), "fonts": len(manifest["fonts"]), "items": len(manifest["items"]),
            "native_templates": len(manifest["templates"]), "static_model_contexts": len(manifest["staticModels"]),
            "static_model_images": len({model["asset"] for model in manifest["staticModels"].values()}),
            "manifest_sha256": digest(ROOT / "assets/compiled/ui/manifest.json"),
        },
        "implementation_files": [{"path": str(path.relative_to(ROOT)), "sha256": digest(path)} for path in owned_sources],
        "remaining": read(ROOT / "web/ui/contract-gaps.json"),
        "full_panels_excluded": 0,
        "gameplay_acceptance": False, "final_m1_acceptance": False,
    }
    write(EVIDENCE / "summary.json", summary)
    print(json.dumps({key: summary[key] for key in ("status", "unit", "component_browser", "source_comparisons", "compiled_assets")}))


if __name__ == "__main__":
    main()

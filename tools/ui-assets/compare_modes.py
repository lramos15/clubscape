#!/usr/bin/env python3
"""Exact additional ORIGINAL native UI/effect comparisons; no masks or golden updates."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--flames-only", action="store_true")
    parser.add_argument("--presentations", action="store_true", help="Compare the separate published-contract modal fixture lane")
    args = parser.parse_args()
    results = []
    directory = ROOT / "web/ui/test-results"
    lanes = [
        ("flames", ROOT / "tools/ui-assets/.cache/native/flames", "*.png"),
        ("modes", ROOT / "tools/ui-assets/.cache/native/ui-only", "native-*.png"),
        ("projections", ROOT / "tools/ui-assets/.cache/native/ui-only", "native-*.png"),
    ]
    if args.presentations:
        lanes = [(kind, ROOT / "tools/ui-assets/.cache/native/ui-only", "native-*.png")
                 for kind in ("presentations", "presentation-projections")]
    for kind, source_directory, pattern in lanes:
        if args.flames_only and kind != "flames":
            continue
        for path in sorted((directory / kind).glob(pattern)):
            if path.name.endswith("-diff.png"):
                continue
            source = source_directory / path.name
            if not source.exists():
                source_kind = "flames" if kind == "flames" else "ui"
                archive = ROOT / "web/ui/evidence" / ("native-presentations" if args.presentations else "native-modes")
                source = archive / "original" / source_kind / path.name
                records = json.loads((archive / "source-inputs.json").read_text())["references"]
                record = next(record for record in records if record["kind"] == source_kind and record["name"] == path.name)
                if hashlib.sha256(source.read_bytes()).hexdigest() != record["sha256"]:
                    raise ValueError("Archived original mode reference is corrupt: " + path.name)
            expected, candidate = Image.open(source).convert("RGBA"), Image.open(path).convert("RGBA")
            assert expected.size == candidate.size, f"Native size mismatch: {path.name}"
            a, b = Image.new("RGB", expected.size), Image.new("RGB", candidate.size)
            a.paste(expected, mask=expected.getchannel("A")); b.paste(candidate, mask=candidate.getchannel("A"))
            diff = ImageChops.difference(a, b)
            different = sum(pixel != (0, 0, 0) for pixel in diff.getdata())
            diff.save(path.with_name(path.stem + "-diff.png"))
            results.append({"case": path.stem, "kind": kind, "size": list(a.size), "checked_pixels": a.width * a.height,
                            "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                            "candidate_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                            "different_pixels": different, "tolerance": 0, "coverage": 1, "passed": different == 0})
    report = {"scope": "Additional ORIGINAL UI component/effect fixture comparisons, not live gameplay",
              "sourcePackSha256": "b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d",
              "results": results, "fullPanelsExcluded": 0, "finalAcceptance": False}
    (directory / ("presentation-comparison.json" if args.presentations else "mode-comparison.json")).write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"checked": len(results), "passed": sum(r["passed"] for r in results),
                      "failures": [{"case": r["case"], "different": r["different_pixels"]} for r in results if not r["passed"]]}))
    return int(not results or any(not result["passed"] for result in results))


if __name__ == "__main__":
    raise SystemExit(main())

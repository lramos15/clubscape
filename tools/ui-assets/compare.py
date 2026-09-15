#!/usr/bin/env python3
"""Full-region component comparisons to approved ORIGINAL frames, never golden updates."""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image, ImageChops

ROOT = Path(__file__).resolve().parents[2]
RESULTS = ROOT / "web/ui/test-results"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("names", nargs="*")
    args = parser.parse_args()
    source = ROOT / "assets/reference/osrs240/native-hud"
    captures = json.loads((source / "captures.json").read_text())["captures"]
    results = []
    for capture in captures:
        name = Path(capture["path"]).stem
        if args.names and name not in args.names:
            continue
        path = RESULTS / f"source/{name}.png"
        if not path.exists():
            continue
        # The same ORIGINAL painter also renders the same widgets with only contentType1337
        # (the world surface) detached. UI alpha and every panel pixel remain in the comparison.
        expected_path = ROOT / f"tools/ui-assets/.cache/native/ui-only/{name}.png"
        if not expected_path.exists():
            expected_path = ROOT / f"web/ui/evidence/original-ui-only/{name}.png"
            records = json.loads((ROOT / "web/ui/evidence/source-inputs.json").read_text())["ui_only"]
            expected_hash = next(record["sha256"] for record in records if record["name"] == name)
            if hashlib.sha256(expected_path.read_bytes()).hexdigest() != expected_hash:
                raise ValueError("Archived original UI-only reference is corrupt: " + name)
        expected, actual = Image.open(expected_path).convert("RGBA"), Image.open(path).convert("RGBA")
        expected_rgb = Image.new("RGB", expected.size)
        expected_rgb.paste(expected, mask=expected.getchannel("A"))
        actual_rgb = Image.new("RGB", actual.size)
        actual_rgb.paste(actual, mask=actual.getchannel("A"))
        expected, actual = expected_rgb, actual_rgb
        assert expected.size == actual.size == (1920, 1080), "No rescaling or realignment allowed"
        regions = capture["source"]["native_ui_regions"] + [
            {"name": "full-ui-canvas", "bounds": [0, 0, 1920, 1080]}]
        for region in regions:
            x, y, width, height = region["bounds"]
            bounds = (x, y, x + width, y + height)
            left, right = expected.crop(bounds), actual.crop(bounds)
            diff = ImageChops.difference(left, right)
            pixels = list(diff.getdata())
            different = sum(pixel != (0, 0, 0) for pixel in pixels)
            result = {"case": name, "region": region["name"], "bounds": region["bounds"],
                      "source": str(expected_path.relative_to(ROOT)),
                      "accounting_role": "whole_overlay" if region["name"] == "full-ui-canvas" else "nested_diagnostic_do_not_sum",
                      "profile": "native_hud", "checked_pixels": width * height,
                      "coverage": 1.0, "different_pixels": different,
                      "maximum_channel_error": max(max(p) for p in pixels),
                      "tolerance_different_pixels": 0, "passed": different == 0}
            results.append(result)
            diff.save(RESULTS / f"source/{name}-{region['name']}-diff.png")
    if not args.names:
        for path in sorted((RESULTS / "source").glob("owner-*.png")):
            if path.name.endswith("-diff.png"):
                continue
            name = path.stem.removeprefix("owner-")
            original = ROOT / f"research/reference-pack/v1/proposals/{name}.png"
            if not original.exists():
                continue
            expected, actual = Image.open(original).convert("RGB"), Image.open(path).convert("RGB")
            assert expected.size == actual.size == (1920, 1080)
            diff = ImageChops.difference(expected, actual)
            pixels = list(diff.getdata())
            different = sum(pixel != (0, 0, 0) for pixel in pixels)
            results.append({"case": "owner-" + name, "region": "full-composition", "bounds": [0, 0, 1920, 1080],
                            "profile": "owner_composition", "source": str(original.relative_to(ROOT)),
                            "accounting_role": "whole_composition", "checked_pixels": 1920 * 1080, "coverage": 1.0,
                            "different_pixels": different, "maximum_channel_error": max(max(p) for p in pixels),
                            "tolerance_different_pixels": 0, "passed": different == 0})
            diff.save(RESULTS / f"source/owner-{name}-diff.png")
    output = {"scope": "Full native UI region accounting on component-only transparent world surface",
              "sourcePackSha256": "b62e19704e17d3d3e4e819f803ef49ba7cc54034ae407184b423427c65d9674d",
              "comparison": "original source, not prior ClubScape output", "results": results,
              "panelsExcluded": 0, "gameplayAcceptance": False, "finalAcceptance": False}
    (RESULTS / "source-comparison.json").write_text(json.dumps(output, indent=2) + "\n")
    print(json.dumps({"checks": len(results), "passed": sum(r["passed"] for r in results),
                      "failures": [{"case": r["case"], "region": r["region"], "different": r["different_pixels"]} for r in results if not r["passed"]]}))
    return int(not results or any(not r["passed"] for r in results))


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Compare exported individual bitmaps with original publication atlases, including alpha."""
import gzip
import json
import argparse
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-exports", action="store_true", help="Also verify every compiled frame against the hash-bound original cache export")
    args = parser.parse_args()
    compiled = ROOT / "assets/compiled/ui"
    manifest = json.loads((compiled / "manifest.json").read_text())
    checked, failures, glyphs = 0, [], 0
    for source in [ROOT / "assets/source/osrs/cache2695", ROOT / "assets/source/osrs/cache2695/content-v2"]:
        for path in sorted((source / "sprites").glob("*.json.gz")):
            identifier = path.name.split(".")[0]
            if identifier not in manifest["sprites"]:
                continue
            original_frames = json.loads(gzip.decompress(path.read_bytes()))["frames"]
            exported_frames = manifest["sprites"][identifier]["frames"]
            original = Image.open(path.with_name(identifier + ".png")).convert("RGBA")
            exported = Image.open(compiled / "sprites" / (identifier + ".png")).convert("RGBA")
            assert len(original_frames) == len(exported_frames)
            for left, right in zip(original_frames, exported_frames, strict=True):
                bounds = (left["atlas_x"], left["atlas_y"], left["atlas_x"] + left["width"], left["atlas_y"] + left["height"])
                other = (right["x"], right["y"], right["x"] + right["width"], right["y"] + right["height"])
                assert (left["width"], left["height"], left["offset_x"], left["offset_y"], left["canvas_width"], left["canvas_height"]) == (
                    right["width"], right["height"], right["offsetX"], right["offsetY"], right["canvasWidth"], right["canvasHeight"])
                a, b = original.crop(bounds), exported.crop(other)
                if a.tobytes() != b.tobytes():
                    failures.append({"sprite": identifier, "frame": left["frame"]})
                checked += 1
                if int(identifier) in [494, 495, 496, 497]:
                    glyphs += 1
    result = {"scope": "Original individual source frames/masks/offsets/alpha, no panel crops",
              "checkedSpriteFrames": checked, "checkedFontGlyphs": glyphs, "failures": failures,
              "toleranceDifferentPixels": 0, "sourcePackSha256": manifest["sourcePackSha256"],
              "gameplayAcceptance": False, "finalAcceptance": False}
    if args.native_exports:
        native = ROOT / "tools/ui-assets/.cache/native/sprites"
        atlases, frames = 0, 0
        for identifier, record in manifest["sprites"].items():
            source = json.loads((native / (identifier + ".json")).read_text())
            if source["sourceRawSha256"] != record["sourceRawSha256"] or source["frames"] != record["frames"]:
                failures.append({"sprite": identifier, "kind": "native_metadata"})
            original = Image.open(native / (identifier + ".png")).convert("RGBA")
            exported = Image.open(compiled / "sprites" / (identifier + ".png")).convert("RGBA")
            if original.size != exported.size or original.tobytes() != exported.tobytes():
                failures.append({"sprite": identifier, "kind": "native_pixels"})
            atlases += 1
            frames += len(record["frames"])
        result["nativeExportCopyProof"] = {
            "atlases": atlases, "frames": frames, "toleranceDifferentPixels": 0,
            "scope": "Original-cache export raw hashes, complete frame metrics and every RGBA pixel retained by compilation; separate from the publication-atlas comparison count.",
        }
    output = ROOT / "web/ui/test-results"
    output.mkdir(parents=True, exist_ok=True)
    (output / "glyph-proof.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result))
    return bool(failures) or glyphs != 1024


if __name__ == "__main__":
    raise SystemExit(main())

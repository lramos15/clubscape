#!/usr/bin/env python3
"""Compare candidate renders against approved source fixture PNGs with the
`native_scene_model` profile (research/reference-pack/v1/comparison-policy.json).

Nothing is masked: every pixel is compared. The 1-pixel edge band is derived from SOURCE edges
only (pixels whose 4-neighbourhood in the source differs by more than EDGE_STEP in any channel,
dilated by edge_band_radius_px). Interior = everything outside that band.

Usage: compare.py <candidate.png> <source.png> [--diff out.png] [--json]
       compare.py --batch <candidate_dir> <source_dir> [--diff-dir dir] [--json]
"""
from __future__ import annotations

import argparse
import json
import struct
import sys
import zlib
from pathlib import Path

PROFILE = {
    "nonedge_max_channel_error_8bit": 2,
    "nonedge_mean_abs_channel_error_8bit": 0.35,
    "edge_band_radius_px": 1,
    "edge_band_changed_fraction_of_full_image_max": 0.005,
    "missing_geometry_pixels": 0,
}
# Adjacent source pixels differing by more than this in any channel form a geometry/shading edge.
# Gouraud 4-pixel banding steps are far smaller than this; silhouettes and texture edges exceed it.
EDGE_STEP = 24


def read_png(path: Path) -> tuple[int, int, bytearray]:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"{path} is not a PNG")
    pos = 8
    width = height = 0
    color_type = bit_depth = 0
    idat = bytearray()
    while pos < len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            width, height, bit_depth, color_type = struct.unpack(">IIBB", body[:10])
            if bit_depth != 8 or body[12] != 0:
                raise ValueError(f"{path}: unsupported PNG (bit depth {bit_depth}, interlace {body[12]})")
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
    channels = {2: 3, 6: 4, 0: 1, 4: 2}[color_type]
    raw = zlib.decompress(bytes(idat))
    stride = width * channels
    out = bytearray(width * height * 3)
    prev = bytearray(stride)
    src = 0
    for y in range(height):
        filt = raw[src]
        src += 1
        line = bytearray(raw[src:src + stride])
        src += stride
        bpp = channels
        if filt == 1:
            for i in range(bpp, stride):
                line[i] = (line[i] + line[i - bpp]) & 255
        elif filt == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 255
        elif filt == 3:
            for i in range(stride):
                left = line[i - bpp] if i >= bpp else 0
                line[i] = (line[i] + ((left + prev[i]) >> 1)) & 255
        elif filt == 4:
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                b = prev[i]
                c = prev[i - bpp] if i >= bpp else 0
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pred) & 255
        prev = line
        row = y * width * 3
        if channels >= 3:
            for x in range(width):
                out[row + x * 3:row + x * 3 + 3] = line[x * channels:x * channels + 3]
        else:
            for x in range(width):
                v = line[x * channels]
                out[row + x * 3:row + x * 3 + 3] = bytes((v, v, v))
    return width, height, out


def write_png(path: Path, width: int, height: int, rgb: bytes) -> None:
    raw = bytearray()
    stride = width * 3
    for y in range(height):
        raw.append(0)
        raw += rgb[y * stride:(y + 1) * stride]

    def chunk(kind: bytes, body: bytes) -> bytes:
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)

    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(bytes(raw), 6)) + chunk(b"IEND", b""))


def compare(candidate: Path, source: Path, diff_out: Path | None) -> dict:
    cw, ch, cand = read_png(candidate)
    sw, sh, src = read_png(source)
    result: dict = {"candidate": str(candidate), "source": str(source), "size": [cw, ch], "source_size": [sw, sh]}
    if (cw, ch) != (sw, sh):
        result.update({"passed": False, "reason": "size mismatch"})
        return result
    w, h = cw, ch
    try:
        import numpy as np  # optional acceleration
    except ImportError:  # pragma: no cover
        np = None
    if np is not None:
        a = np.frombuffer(bytes(cand), dtype=np.uint8).reshape(h, w, 3).astype(np.int16)
        b = np.frombuffer(bytes(src), dtype=np.uint8).reshape(h, w, 3).astype(np.int16)
        err = np.abs(a - b)
        edge = np.zeros((h, w), dtype=bool)
        dx = np.abs(b[:, 1:, :] - b[:, :-1, :]).max(axis=2) > EDGE_STEP
        dy = np.abs(b[1:, :, :] - b[:-1, :, :]).max(axis=2) > EDGE_STEP
        edge[:, 1:] |= dx
        edge[:, :-1] |= dx
        edge[1:, :] |= dy
        edge[:-1, :] |= dy
        band = edge.copy()
        for _ in range(PROFILE["edge_band_radius_px"]):
            grown = band.copy()
            grown[1:, :] |= band[:-1, :]
            grown[:-1, :] |= band[1:, :]
            grown[:, 1:] |= band[:, :-1]
            grown[:, :-1] |= band[:, 1:]
            band = grown
        interior = ~band
        per_pixel_max = err.max(axis=2)
        changed = per_pixel_max > 0
        interior_err = err[interior]
        result.update({
            "pixels": int(w * h),
            "identical_pixels": int((~changed).sum()),
            "changed_pixels": int(changed.sum()),
            "source_edge_band_pixels": int(band.sum()),
            "interior_pixels": int(interior.sum()),
            "interior_max_channel_error": int(interior_err.max()) if interior_err.size else 0,
            "interior_mean_abs_channel_error": float(interior_err.mean()) if interior_err.size else 0.0,
            "interior_changed_pixels": int((per_pixel_max[interior] > 0).sum()),
            "edge_band_changed_pixels": int((per_pixel_max[band] > 0).sum()),
            "edge_band_changed_fraction_of_full_image": float((per_pixel_max[band] > 0).sum() / (w * h)),
            "max_channel_error_anywhere": int(err.max()),
        })
        if diff_out is not None:
            vis = np.zeros((h, w, 3), dtype=np.uint8)
            gray = (b.mean(axis=2) // 3).astype(np.uint8)
            vis[..., 0] = gray
            vis[..., 1] = gray
            vis[..., 2] = gray
            vis[changed & band] = (255, 200, 0)
            vis[changed & interior] = (255, 0, 0)
            write_png(diff_out, w, h, vis.tobytes())
    else:
        raise SystemExit("numpy is required for compare.py")
    result["profile"] = "native_scene_model"
    result["edge_step_used_for_source_edges"] = EDGE_STEP
    result["passed"] = (
        result["interior_max_channel_error"] <= PROFILE["nonedge_max_channel_error_8bit"]
        and result["interior_mean_abs_channel_error"] <= PROFILE["nonedge_mean_abs_channel_error_8bit"]
        and result["edge_band_changed_fraction_of_full_image"] <= PROFILE["edge_band_changed_fraction_of_full_image_max"]
    )
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("candidate")
    parser.add_argument("source")
    parser.add_argument("--batch", action="store_true", help="treat candidate/source as directories and match by file name")
    parser.add_argument("--diff", help="write a diff visualisation PNG (single mode)")
    parser.add_argument("--diff-dir", help="write diff PNGs here (batch mode)")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    results = []
    if args.batch:
        cdir, sdir = Path(args.candidate), Path(args.source)
        diff_dir = Path(args.diff_dir) if args.diff_dir else None
        if diff_dir:
            diff_dir.mkdir(parents=True, exist_ok=True)
        for source in sorted(sdir.glob("*.png")):
            candidate = cdir / source.name
            if not candidate.exists():
                results.append({"candidate": str(candidate), "source": str(source), "passed": False, "reason": "candidate missing"})
                continue
            results.append(compare(candidate, source, diff_dir / f"{source.stem}.diff.png" if diff_dir else None))
    else:
        results.append(compare(Path(args.candidate), Path(args.source), Path(args.diff) if args.diff else None))
    if args.json:
        print(json.dumps(results, indent=1))
    else:
        for r in results:
            name = Path(r["source"]).name
            if "reason" in r:
                print(f"FAIL {name}: {r['reason']}")
                continue
            print(f"{'PASS' if r['passed'] else 'FAIL'} {name}: identical={r['identical_pixels']}/{r['pixels']} "
                  f"interior max={r['interior_max_channel_error']} mean={r['interior_mean_abs_channel_error']:.4f} "
                  f"band changed={r['edge_band_changed_pixels']} ({r['edge_band_changed_fraction_of_full_image']:.5f}) "
                  f"max anywhere={r['max_channel_error_anywhere']}")
    return 0 if all(r["passed"] for r in results) else 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Generate independent float/channel oracles from hash-verified original supplement PCM."""

from array import array
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

sys.dont_write_bytecode = True


def main():
    spec = importlib.util.spec_from_file_location("native_supplement_codec", "tools/audio-import/codec.py")
    codec = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(codec)
    decoder = codec.Flac()
    manifest_path = Path("assets/manifests/osrs/audio-m1-supplement.json")
    manifest = json.loads(manifest_path.read_text())
    assets = []
    for record in manifest["assets"]:
        path = Path(record["path"])
        assert hashlib.sha256(path.read_bytes()).hexdigest() == record["sha256"]
        samples, info = decoder.read(path)
        assert hashlib.sha256(samples.tobytes()).hexdigest() == record["encoding"]["decoded_pcm_sha256"]
        native = array("h", (value >> 16 for value in samples))
        assert hashlib.sha256(native.tobytes()).hexdigest() == record["encoding"]["source_pcm_s16le_sha256"]
        channel_hashes = []
        for channel in range(info.channels):
            floats = array("f", (value / 2147483648 for value in samples[channel::info.channels]))
            channel_hashes.append(hashlib.sha256(floats.tobytes()).hexdigest())
        assets.append({
            "asset_id": record["asset_id"], "frames": info.frames, "sample_rate": info.samplerate,
            "channels": info.channels, "float32_channel_sha256": channel_hashes,
            "full_scale_samples": record["signal"]["full_scale_samples"],
            "source_pcm_s16le_sha256": record["encoding"]["source_pcm_s16le_sha256"],
        })
    output = Path("research/browser-audio-policy/supplement-browser-oracles.json")
    output.write_text(json.dumps({
        "schema_version": 1,
        "manifest_sha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        "conversion": "Exact signed32 power-of-two mapping of lossless unity FLAC24 containing original16 bits",
        "generated_before_browser_comparison": True, "assets": assets,
    }, indent=2) + "\n")
    print(f"Generated exact source-PCM float oracles for {len(assets)} additive originals.")


if __name__ == "__main__":
    main()

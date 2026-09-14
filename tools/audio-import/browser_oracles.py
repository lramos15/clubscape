"""Integer-to-float codec oracles, computed from verified PCM before browser decoding."""

from array import array
import hashlib
import json
from pathlib import Path

from codec import Flac
from audio_import import file_record, verify, write_json


def models(samples, channels, container_bits):
    storage_bits = samples.itemsize * 8
    scale = 1 << (storage_bits - 1)
    result = []
    for mode, bits in [("signed-power-of-two", storage_bits)] + [
        ("positive-peak-f32-reciprocal", bits) for bits in sorted({storage_bits, container_bits})
    ]:
        shift = storage_bits - bits
        denominator = 1 << (bits - 1)
        reciprocal = array("f", [1 / (denominator - 1)])[0]
        channel_hashes = []
        max_error = 0.0
        for channel in range(channels):
            values = samples[channel::channels]
            if mode == "signed-power-of-two":
                converted = array("f", (value / scale for value in values))
            else:
                converted = array("f", (
                    (value >> shift) * reciprocal if value > 0 else (value >> shift) / denominator
                    for value in values
                ))
            channel_hashes.append(hashlib.sha256(converted.tobytes()).hexdigest())
            max_error = max(max_error, max(
                (abs(actual - value / scale) for actual, value in zip(converted, values)),
                default=0,
            ))
        result.append({
            "mode": mode,
            "integer_width": bits,
            "float32_channel_sha256": channel_hashes,
            "maximum_deviation_from_power_of_two_float": max_error,
        })
    return result


def main():
    manifest_path = Path("assets/manifests/osrs/audio-runtime.json")
    manifest = json.loads(manifest_path.read_text())
    codec = Flac()
    records = {}
    for record in manifest["assets"]:
        verify(record["path"], record)
        samples, info = codec.read(record["path"])
        if hashlib.sha256(samples.tobytes()).hexdigest() != record["encoding"]["decoded_pcm_sha256"]:
            raise ValueError("PCM changed before browser oracle generation")
        original = array("h", (value >> 15 for value in samples)) if record["kind"] == "sfx" else samples
        if hashlib.sha256(original.tobytes()).hexdigest() != record["encoding"]["source_pcm_s16le_sha256"]:
            raise ValueError("Browser oracle does not preserve the original source integers")
        records[record["asset_id"]] = models(samples, info.channels, record["encoding"]["bits_per_sample"])
    write_json(Path(".local/audio-import/browser-oracles.json"), {
        "manifest": file_record(manifest_path),
        "scope": "Exact forward integer-to-float conversion models, not fitted error tolerances. Full original PCM hashes are checked before these expectations are computed.",
        "models": records,
    })


if __name__ == "__main__":
    main()

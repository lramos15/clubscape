"""Compare identifiable public recordings with the already verified original PCM."""

import argparse
import hashlib
import json
from pathlib import Path

import av
import numpy as np


def decode(path, rate=22050):
    chunks = []
    first_time = None
    with av.open(str(path)) as container:
        resampler = av.AudioResampler(format="fltp", layout="mono", rate=rate)
        for source in container.decode(audio=0):
            for frame in resampler.resample(source):
                if first_time is None:
                    first_time = float(frame.pts * frame.time_base) if frame.pts is not None else 0
                chunks.append(frame.to_ndarray().reshape(-1))
        for frame in resampler.resample(None):
            chunks.append(frame.to_ndarray().reshape(-1))
    if not chunks:
        raise ValueError("Recording has no decoded audio")
    samples = np.concatenate(chunks).astype(np.float32)
    if not np.isfinite(samples).all() or np.max(np.abs(samples)) == 0:
        raise ValueError("Recording audio is invalid or silent")
    return samples, first_time


def correlate(recording, template, rate, count=5):
    if len(recording) < len(template) or len(template) < rate / 2:
        raise ValueError("Invalid bounded correlation lengths")
    reference = template.astype(np.float64)
    reference -= reference.mean()
    signal = recording.astype(np.float64)
    size = 1 << (len(signal) + len(reference) - 2).bit_length()
    correlation = np.fft.irfft(np.fft.rfft(signal, size) * np.conj(np.fft.rfft(reference, size)), size)
    correlation = correlation[: len(signal) - len(reference) + 1]
    energy = np.concatenate(([0.0], np.cumsum(signal * signal)))
    totals = np.concatenate(([0.0], np.cumsum(signal)))
    window_energy = energy[len(reference) :] - energy[:-len(reference)]
    means = totals[len(reference) :] - totals[:-len(reference)]
    variance = np.maximum(window_energy - means * means / len(reference), 0)
    denominator = np.sqrt(variance * np.dot(reference, reference))
    scores = np.divide(correlation, denominator, out=np.zeros_like(correlation), where=denominator > 1e-15)
    result = []
    remaining = np.abs(scores).copy()
    exclusion = round(rate)
    for _ in range(count):
        position = int(np.argmax(remaining))
        result.append({"offset_seconds": position / rate, "coefficient": float(scores[position])})
        remaining[max(0, position - exclusion) : min(len(remaining), position + exclusion)] = -1
    return result


def spectral_features(samples, fft_size=2048, hop=256):
    windows = np.lib.stride_tricks.sliding_window_view(samples, fft_size)[::hop]
    frequencies = np.fft.rfftfreq(fft_size, 1 / 22050)
    note = np.full(len(frequencies), -1, dtype=int)
    positive = frequencies > 0
    note[positive] = np.rint(69 + 12 * np.log2(frequencies[positive] / 440)).astype(int)
    bands = [(note == value) for value in range(24, 121)]
    result = np.zeros((len(bands), len(windows)), dtype=np.float32)
    hann = np.hanning(fft_size)
    for start in range(0, len(windows), 512):
        power = np.abs(np.fft.rfft(windows[start : start + 512] * hann, axis=1)) ** 2
        for band, mask in enumerate(bands):
            result[band, start : start + len(power)] = np.log1p(np.sum(power[:, mask], axis=1) * 100)
    result -= result.mean(axis=0, keepdims=True)
    norm = np.sqrt(np.sum(result * result, axis=0, keepdims=True))
    return np.divide(result, norm, out=np.zeros_like(result), where=norm > 0)


def spectral_correlate(recording, template, rate, count=5):
    reference = template.astype(np.float64)
    reference -= reference.mean(axis=1, keepdims=True)
    signal = recording.astype(np.float64)
    length = reference.shape[1]
    size = 1 << (signal.shape[1] + length - 2).bit_length()
    cross = np.sum(np.fft.rfft(signal, size, axis=1) * np.conj(np.fft.rfft(reference, size, axis=1)), axis=0)
    correlation = np.fft.irfft(cross, size)[: signal.shape[1] - length + 1]
    energy = np.concatenate(([0.0], np.cumsum(np.sum(signal * signal, axis=0))))
    sums = np.concatenate((np.zeros((len(signal), 1)), np.cumsum(signal, axis=1)), axis=1)
    totals = sums[:, length:] - sums[:, :-length]
    variance = np.maximum(energy[length:] - energy[:-length] - np.sum(totals * totals, axis=0) / length, 0)
    denominator = np.sqrt(variance * np.sum(reference * reference))
    scores = np.divide(correlation, denominator, out=np.zeros_like(correlation), where=denominator > 1e-15)
    result = []
    remaining = scores.copy()
    exclusion = round(rate)
    for _ in range(count):
        position = int(np.argmax(remaining))
        result.append({"offset_seconds": position / rate, "coefficient": float(scores[position])})
        remaining[max(0, position - exclusion) : min(len(remaining), position + exclusion)] = -1
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("recording", type=Path)
    parser.add_argument("--jingles", default="152,153,154,33,34")
    parser.add_argument("--start", type=float, default=0)
    parser.add_argument("--end", type=float)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--spectral", action="store_true")
    parser.add_argument("--native-template-directory", type=Path)
    args = parser.parse_args()
    manifest = json.loads(Path("assets/manifests/osrs/audio-runtime.json").read_text())
    by_group = {row["source_group"]: row for row in manifest["assets"] if row["kind"] == "jingle"}
    signal, origin = decode(args.recording)
    stop = len(signal) if args.end is None else min(len(signal), round(args.end * 22050))
    begin = round(args.start * 22050)
    if begin < 0 or begin >= stop:
        raise ValueError("Invalid recording interval")
    cropped = signal[begin:stop:4]
    features = spectral_features(signal[begin:stop]) if args.spectral else None
    matches = []
    for group in map(int, args.jingles.split(",")):
        record = by_group[group]
        template_path = Path(record["path"])
        if args.native_template_directory:
            template_path = args.native_template_directory / f"jingle-{group}.wav"
        raw = template_path.read_bytes()
        if not args.native_template_directory and hashlib.sha256(raw).hexdigest() != record["sha256"]:
            raise ValueError("Changed original template")
        original, _ = decode(template_path)
        first = int(np.flatnonzero(original)[0])
        source_end = record["loop"]["source_engine_end_frame"]
        for seconds in (2.0, 5.0):
            end = min(source_end, first + round(seconds * 22050))
            template = original[first:end:4]
            if args.spectral:
                peaks = spectral_correlate(features, spectral_features(original[first:end]), 22050 / 256)
            else:
                peaks = correlate(cropped, template, 22050 / 4)
            for peak in peaks:
                peak["estimated_cue_start_seconds"] = peak.pop("offset_seconds") + args.start + origin - first / 22050
            matches.append({
                "source_jingle_group": group, "source_template_path": str(template_path),
                "source_template_sha256": hashlib.sha256(raw).hexdigest(),
                "template_start_frame": first, "template_end_frame": end,
                "template_duration_seconds": len(template) / (22050 / 4), "peaks": peaks,
            })
    result = {
        "recording_path": str(args.recording), "recording_sha256": hashlib.sha256(args.recording.read_bytes()).hexdigest(),
        "decoded_frames": len(signal), "decoded_sample_rate": 22050, "first_decoded_timestamp": origin,
        "interval_seconds": [args.start, stop / 22050],
        "method": (
            "Normalized temporal correlation of log-power semitone-band features: FFT2048/Hann/hop256, MIDI bands24..120, frame spectral mean removed/L2 normalized, template temporal means removed and each window variance normalized."
            if args.spectral else
            "Normalized time-domain cross-correlation after shared fourfold sample selection; template means removed, each candidate window variance normalized."
        ) + " This measures matches, not by itself a game-state/selector assertion.",
        "decoder": {"pyav": av.__version__, "numpy": np.__version__, "resampling": "libswresample mono FLTP22050"},
        "matches": matches,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    for match in matches:
        print(match["source_jingle_group"],round(match["template_duration_seconds"], 2),match["peaks"][0])


if __name__ == "__main__":
    main()

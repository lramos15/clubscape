"""Measure decoded signals rather than just container headers."""

import cmath
import math


def fft(values):
    size = len(values)
    if size < 2 or size & (size - 1):
        raise ValueError("FFT length must be a power of two")
    result = list(map(complex, values))
    j = 0
    for i in range(1, size):
        bit = size >> 1
        while j & bit:
            j ^= bit
            bit >>= 1
        j ^= bit
        if i < j:
            result[i], result[j] = result[j], result[i]
    width = 2
    while width <= size:
        step = cmath.exp(-2j * math.pi / width)
        for start in range(0, size, width):
            weight = 1
            for offset in range(width // 2):
                left = start + offset
                right = left + width // 2
                odd = weight * result[right]
                even = result[left]
                result[left], result[right] = even + odd, even - odd
                weight *= step
        width *= 2
    return result


def decibels(amplitude):
    return round(20 * math.log10(amplitude), 6) if amplitude > 0 else None


def measure(samples, rate, channels):
    if rate != 22050 or channels not in (1, 2) or len(samples) % channels or not samples:
        raise ValueError("Invalid bounded source PCM")
    scale = 32768 if samples.typecode == "h" else 2147483648
    frames = len(samples) // channels
    peak = max(abs(value) for value in samples)
    if peak == 0:
        raise ValueError("Original audio decoded to silence")
    full_scale = sum(value >= scale - 1 or value <= -scale for value in samples)
    if full_scale:
        raise ValueError(f"Published audio reaches full scale: {full_scale} samples")
    square_sum = sum(value * value for value in samples)
    rms = math.sqrt(square_sum / len(samples)) / scale
    nonzero = sum(value != 0 for value in samples)
    first = next(i // channels for i, value in enumerate(samples) if value)
    last = (len(samples) - 1 - next(i for i, value in enumerate(reversed(samples)) if value)) // channels
    channel_data = []
    for channel in range(channels):
        values = samples[channel::channels]
        crossings = sum((a < 0) != (b < 0) for a, b in zip(values, values[1:]))
        channel_data.append({
            "rms_dbfs": decibels(math.sqrt(sum(value * value for value in values) / frames) / scale),
            "dc_offset": round(sum(values) / frames / scale, 10),
            "zero_crossings_per_second": round(crossings * rate / max(frames - 1, 1), 4),
        })
    quarter_rms = []
    for quarter in range(4):
        part = samples[(frames * quarter // 4) * channels : (frames * (quarter + 1) // 4) * channels]
        quarter_rms.append(decibels(math.sqrt(sum(value * value for value in part) / max(1, len(part))) / scale))
    spectra = []
    window = 2048
    for fraction in (0.2, 0.5, 0.8):
        if frames < window:
            continue
        start = min(int(frames * fraction), frames - window)
        values = [
            sum(samples[(start + i) * channels : (start + i + 1) * channels]) / channels / scale
            * (0.5 - 0.5 * math.cos(2 * math.pi * i / (window - 1)))
            for i in range(window)
        ]
        spectrum = [abs(value) ** 2 for value in fft(values)[1 : window // 2 + 1]]
        energy = sum(spectrum)
        if energy == 0:
            continue
        mean = energy / len(spectrum)
        spectra.append({
            "start_frame": start,
            "window_frames": window,
            "window": "Hann",
            "dominant_bin_hz": round((max(range(len(spectrum)), key=spectrum.__getitem__) + 1) * rate / window, 4),
            "spectral_centroid_hz": round(sum((i + 1) * rate / window * value for i, value in enumerate(spectrum)) / energy, 4),
            "spectral_flatness": round(math.exp(sum(math.log(max(value, 1e-30)) for value in spectrum) / len(spectrum)) / mean, 8),
        })
    return {
        "frames": frames,
        "sample_rate": rate,
        "channels": channels,
        "duration_seconds": round(frames / rate, 9),
        "peak": round(peak / scale, 10),
        "peak_dbfs": decibels(peak / scale),
        "rms_dbfs": decibels(rms),
        "crest_factor_db": decibels((peak / scale) / rms),
        "full_scale_samples": full_scale,
        "nonzero_fraction": round(nonzero / len(samples), 8),
        "distinct_amplitudes": len(set(samples)),
        "first_nonzero_frame": first,
        "last_nonzero_frame": last,
        "channel_statistics": channel_data,
        "quarter_rms_dbfs": quarter_rms,
        "spectral_windows": spectra,
        "stereo_difference_rms": (
            round(math.sqrt(sum((a - b) ** 2 for a, b in zip(samples[::2], samples[1::2])) / frames) / scale, 10)
            if channels == 2 else None
        ),
    }

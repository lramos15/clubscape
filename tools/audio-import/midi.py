"""Bounded SMF event/timing inspection; never a synthesizer."""

from collections import Counter
import hashlib
import json
import struct


def parse(data: bytes, *, allow_meta_running_status: bool = False) -> dict:
    if len(data) < 14 or data[:8] != b"MThd\0\0\0\x06":
        raise ValueError("Missing or unsupported MIDI header")
    kind, track_count, division = struct.unpack_from(">HHH", data, 8)
    if kind not in (0, 1) or not 1 <= track_count <= 64 or not 1 <= division < 32768:
        raise ValueError("Unsupported MIDI format, track count or clock division")
    offset = 14
    tracks = []
    for _ in range(track_count):
        if offset + 8 > len(data) or data[offset : offset + 4] != b"MTrk":
            raise ValueError("Missing MIDI track")
        end = offset + 8 + int.from_bytes(data[offset + 4 : offset + 8], "big")
        if end > len(data):
            raise ValueError("Truncated MIDI track")
        pos = offset + 8
        tick = 0
        running = None
        events = []
        ended = False

        def byte():
            nonlocal pos
            if pos >= end:
                raise ValueError("Truncated MIDI event")
            value = data[pos]
            pos += 1
            return value

        def vlq():
            value = 0
            for _ in range(4):
                part = byte()
                value = (value << 7) | (part & 127)
                if part < 128:
                    return value
            raise ValueError("Oversized MIDI variable-length value")

        while pos < end:
            if ended:
                raise ValueError("MIDI events after end of track")
            tick += vlq()
            if tick > 100_000_000:
                raise ValueError("MIDI tick bound exceeded")
            if pos >= end:
                raise ValueError("Missing MIDI event status")
            status = data[pos]
            if status >= 128:
                pos += 1
                running = status if status < 240 or allow_meta_running_status else None
            else:
                if running is None:
                    raise ValueError("MIDI running status without a predecessor")
                status = running
            if status == 255:
                meta = byte()
                count = vlq()
                if pos + count > end:
                    raise ValueError("Truncated MIDI meta event")
                payload = data[pos : pos + count]
                pos += count
                if meta == 47:
                    if payload:
                        raise ValueError("Invalid MIDI end-of-track payload")
                    ended = True
                if meta == 81 and (len(payload) != 3 or int.from_bytes(payload, "big") == 0):
                    raise ValueError("Invalid MIDI tempo")
                events.append((tick, status, meta, payload.hex()))
            elif status in (240, 247):
                count = vlq()
                if pos + count > end:
                    raise ValueError("Truncated MIDI SysEx")
                events.append((tick, status, data[pos : pos + count].hex()))
                pos += count
            elif 128 <= status < 240:
                count = 1 if status >> 4 in (12, 13) else 2
                payload = bytes(byte() for _ in range(count))
                if any(value >= 128 for value in payload):
                    raise ValueError("Invalid MIDI channel data")
                events.append((tick, status, payload.hex()))
            else:
                raise ValueError("Unsupported MIDI system event")
            if len(events) > 1_000_000:
                raise ValueError("MIDI event count bound exceeded")
        if not ended:
            raise ValueError("MIDI track has no end event")
        tracks.append(events)
        offset = end
    if offset != len(data):
        raise ValueError("Trailing MIDI bytes")
    return {"format": kind, "division": division, "tracks": tracks}


def describe(data: bytes, *, allow_meta_running_status: bool = False) -> dict:
    midi = parse(data, allow_meta_running_status=allow_meta_running_status)
    ordered = sorted(
        (event[0], track, position, event)
        for track, events in enumerate(midi["tracks"])
        for position, event in enumerate(events)
    )
    tempo = 500000
    tick = 0
    time_units = 0
    note_on = 0
    channels = Counter()
    programs = Counter()
    tempo_events = []
    for current, _, _, event in ordered:
        time_units += (current - tick) * tempo
        tick = current
        status = event[1]
        if status == 255 and event[2] == 81:
            tempo = int(event[3], 16)
            tempo_events.append({"tick": tick, "microseconds_per_quarter": tempo})
        elif status >> 4 == 9 and bytes.fromhex(event[2])[1]:
            note_on += 1
            channels[status & 15] += 1
        elif status >> 4 == 12:
            programs[bytes.fromhex(event[2])[0]] += 1
    division = midi["division"]
    clock_step = division * 1_000_000 // 22050
    canonical = json.dumps(midi, separators=(",", ":"), sort_keys=True).encode()
    return {
        "format": midi["format"],
        "tracks": len(midi["tracks"]),
        "division": division,
        "end_tick": tick,
        "event_counts": [len(events) for events in midi["tracks"]],
        "note_on_count": note_on,
        "note_on_channels": dict(sorted(channels.items())),
        "program_change_counts": dict(sorted(programs.items())),
        "tempo_events": tempo_events,
        "duration_microseconds_numerator": time_units,
        "duration_microseconds_denominator": division,
        "duration_seconds": time_units / division / 1_000_000,
        "original_clock_units_per_frame": clock_step,
        "expected_engine_end_frame": time_units // clock_step + 1,
        "semantic_sha256": hashlib.sha256(canonical).hexdigest(),
    }


def compare_runtime(published: bytes, runtime: bytes) -> dict:
    expected = describe(published)
    actual = describe(runtime, allow_meta_running_status=True)
    if expected["semantic_sha256"] != actual["semantic_sha256"]:
        raise ValueError("Original runtime and source-library MIDI event streams differ")
    return {
        "semantic_events_equal": True,
        "byte_equal": published == runtime,
        "semantic_sha256": expected["semantic_sha256"],
        "note": (
            "Identical bytes."
            if published == runtime
            else "Original runtime permits repeated 0xFF meta running status; the cache-library SMF writes explicit statuses. Every normalized event, track and tick is equal."
        ),
    }

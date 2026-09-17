"""Read MP4 container facts without transcoding or inventing capture settings."""

import struct


def atoms(data):
    offset = 0
    while offset < len(data):
        if len(data) - offset < 8:
            raise ValueError("Truncated MP4 atom header")
        size, kind = struct.unpack_from(">I4s", data, offset)
        header = 8
        if size == 1:
            if len(data) - offset < 16:
                raise ValueError("Truncated MP4 extended size")
            size = struct.unpack_from(">Q", data, offset + 8)[0]
            header = 16
        elif size == 0:
            size = len(data) - offset
        if size < header or offset + size > len(data):
            raise ValueError("MP4 atom extends beyond actual bytes")
        yield kind.decode("ascii"), data[offset + header:offset + size]
        offset += size


def one(items, kind):
    found = [data for name, data in items if name == kind]
    if len(found) != 1:
        raise ValueError(f"Expected one MP4 {kind} atom; found {len(found)}")
    return found[0]


def duration(header):
    if header[0] == 0:
        scale, ticks = struct.unpack_from(">II", header, 12)
    elif header[0] == 1:
        scale = struct.unpack_from(">I", header, 20)[0]
        ticks = struct.unpack_from(">Q", header, 24)[0]
    else:
        raise ValueError("Unsupported MP4 time header version")
    if not scale:
        raise ValueError("Zero MP4 timescale")
    return {"timescale": scale, "duration_ticks": ticks,
            "duration_seconds": ticks / scale}


def mp4_facts(data):
    top = list(atoms(data))
    if not any(k == "ftyp" for k, _ in top) or not any(k == "mdat" for k, _ in top):
        raise ValueError("Not a complete MP4 recording")
    movie = list(atoms(one(top, "moov")))
    tracks = []
    for kind, track_data in movie:
        if kind != "trak":
            continue
        track = list(atoms(track_data))
        track_header = one(track, "tkhd")
        media = list(atoms(one(track, "mdia")))
        handler = one(media, "hdlr")[8:12].decode("ascii")
        samples = list(atoms(one(list(atoms(one(media, "minf"))), "stbl")))
        descriptions = list(atoms(one(samples, "stsd")[8:]))
        timing = one(samples, "stts")
        count = struct.unpack_from(">I", timing, 4)[0]
        if len(timing) != 8 + count * 8:
            raise ValueError("Invalid MP4 sample timing table")
        rows = [struct.unpack_from(">II", timing, 8 + i * 8) for i in range(count)]
        entry = {
            "handler": handler, **duration(one(media, "mdhd")),
            "sample_count": sum(n for n, _ in rows),
            "sample_time_runs": [list(row) for row in rows],
            "codecs": [name for name, _ in descriptions],
        }
        if handler == "vide":
            width, height = struct.unpack_from(">II", track_header, len(track_header) - 8)
            entry["dimensions"] = [width / 65536, height / 65536]
        tracks.append(entry)
    video = [track for track in tracks if track["handler"] == "vide"]
    if len(video) != 1 or not video[0]["sample_count"]:
        raise ValueError("Expected one nonempty source video track")
    return {
        "format": "MP4", **duration(one(movie, "mvhd")),
        "dimensions": video[0]["dimensions"],
        "frame_count": video[0]["sample_count"],
        "tracks": tracks,
        "container_structure_checked": True,
        "actual_video_decode": "See browser-media.json; container parsing alone is not decode proof.",
    }

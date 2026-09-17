"""Lossless packaging of original PCM using the checksum-locked installed codec."""

from array import array
import ctypes as C
import hashlib
import json
from pathlib import Path
import sys
import wave


class Info(C.Structure):
    _fields_ = [
        ("frames", C.c_int64),
        ("samplerate", C.c_int),
        ("channels", C.c_int),
        ("format", C.c_int),
        ("sections", C.c_int),
        ("seekable", C.c_int),
    ]


class Flac:
    def __init__(self, lock_path=Path("tools/audio-import/dependencies.json")):
        if sys.byteorder != "little":
            raise ValueError("This exact encoding lock requires a little-endian host")
        self.lock = json.loads(Path(lock_path).read_text())["lossless_encoder"]
        self.loaded = [C.CDLL(record["soname"]) for record in self.lock["libraries"]]
        self.lib = self.loaded[0]
        self.lib.sf_version_string.restype = C.c_char_p
        version = self.lib.sf_version_string().decode()
        if version != self.lock["version"]:
            raise ValueError(f"Unpinned libsndfile version: {version}")
        mapped = {line.split()[-1] for line in Path("/proc/self/maps").read_text().splitlines() if "/" in line}
        for record in self.lock["libraries"]:
            paths = [Path(path) for path in mapped if Path(path).name == record["filename"]]
            if len(paths) != 1:
                raise ValueError(f"Cannot identify loaded encoder {record['filename']}")
            path = paths[0]
            with path.open("rb") as source:
                digest = hashlib.file_digest(source, "sha256").hexdigest()
            if path.stat().st_size != record["size_bytes"] or digest != record["sha256"]:
                raise ValueError(f"Unpinned encoder library: {path.name}")
        self.lib.sf_open.argtypes = [C.c_char_p, C.c_int, C.POINTER(Info)]
        self.lib.sf_open.restype = C.c_void_p
        self.lib.sf_close.argtypes = [C.c_void_p]
        self.lib.sf_close.restype = C.c_int
        self.lib.sf_strerror.argtypes = [C.c_void_p]
        self.lib.sf_strerror.restype = C.c_char_p
        self.lib.sf_command.argtypes = [C.c_void_p, C.c_int, C.c_void_p, C.c_int]
        self.lib.sf_command.restype = C.c_int
        for name, typ in (("short", C.c_int16), ("int", C.c_int32)):
            for operation in ("read", "write"):
                method = getattr(self.lib, f"sf_{operation}f_{name}")
                method.argtypes = [C.c_void_p, C.POINTER(typ), C.c_int64]
                method.restype = C.c_int64

    def error(self, handle):
        return self.lib.sf_strerror(handle).decode()

    def read(self, path):
        info = Info()
        handle = self.lib.sf_open(str(path).encode(), 0x10, C.byref(info))
        if not handle:
            raise ValueError(f"Audio decode failed: {path}: {self.error(None)}")
        try:
            subtype = info.format & 0xFFFF
            if info.samplerate != 22050 or info.channels not in (1, 2) or subtype not in (2, 3):
                raise ValueError("Unselected sample format/rate/channels")
            if not 0 < info.frames <= 22050 * 601:
                raise ValueError("Audio frame count exceeds the bounded source request")
            result = array("h" if subtype == 2 else "i", [0]) * (info.frames * info.channels)
            typ = C.c_int16 if subtype == 2 else C.c_int32
            pointer = (typ * len(result)).from_buffer(result)
            method = self.lib.sf_readf_short if subtype == 2 else self.lib.sf_readf_int
            count = method(handle, pointer, info.frames)
            if count != info.frames:
                raise ValueError(f"Short audio decode: {count}/{info.frames}: {self.error(handle)}")
            return result, info
        finally:
            self.lib.sf_close(handle)

    def encode(self, native_wav, output, *, effect=False):
        with wave.open(str(native_wav), "rb") as source:
            rate, channels, frames = source.getframerate(), source.getnchannels(), source.getnframes()
            if source.getsampwidth() != 2 or rate != 22050 or channels not in (1, 2):
                raise ValueError("Expected original 22050-Hz signed-16 PCM")
            native = array("h", source.readframes(frames))
            if len(native) != frames * channels or not frames:
                raise ValueError("Empty or truncated original PCM")
        # FLAC24 stores all original 16 bits at exactly half gain; no limiter or bit is discarded.
        samples = array("i", (value << 15 for value in native)) if effect else native
        info = Info(frames=frames, samplerate=rate, channels=channels, format=0x170000 | (3 if effect else 2))
        output = Path(output)
        output.parent.mkdir(parents=True, exist_ok=True)
        handle = self.lib.sf_open(str(output).encode(), 0x20, C.byref(info))
        if not handle:
            raise ValueError(f"FLAC encode failed: {self.error(None)}")
        try:
            level = C.c_double(self.lock["compression_level"])
            if self.lib.sf_command(handle, 0x1301, C.byref(level), C.sizeof(level)) != 1:
                raise ValueError("Pinned FLAC compression setting was not accepted")
            typ = C.c_int32 if effect else C.c_int16
            pointer = (typ * len(samples)).from_buffer(samples)
            method = self.lib.sf_writef_int if effect else self.lib.sf_writef_short
            if method(handle, pointer, frames) != frames:
                raise ValueError(f"Incomplete FLAC output: {self.error(handle)}")
        finally:
            if self.lib.sf_close(handle):
                raise ValueError("FLAC finalization failed")
        restored, decoded = self.read(output)
        if decoded.frames != frames or decoded.channels != channels or restored != samples:
            raise ValueError("FLAC round trip differs from original PCM")
        recovered = array("h", (value >> 15 for value in restored)) if effect else restored
        if recovered != native:
            raise ValueError("Export gain was not exactly reversible")
        return restored, decoded, {
            "codec": "FLAC",
            "bits_per_sample": 24 if effect else 16,
            "effective_source_bits": 16,
            "gain_numerator": 1,
            "gain_denominator": 2 if effect else 1,
            "source_pcm_s16le_sha256": hashlib.sha256(native.tobytes()).hexdigest(),
            "decoded_pcm_sha256": hashlib.sha256(restored.tobytes()).hexdigest(),
            "decoded_pcm_hash_format": "s32le-left-aligned" if effect else "s16le",
            "lossless_round_trip_verified": True,
            "gain_exactly_reversible": True,
            "native_full_scale_samples": sum(value in (-32768, 32767) for value in native),
        }

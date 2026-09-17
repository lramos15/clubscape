import hashlib
import importlib.util
from pathlib import Path
import shutil
import struct
import unittest
import uuid
import zlib

SPEC = importlib.util.spec_from_file_location("source_capture", Path(__file__).with_name("capture.py"))
CAPTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPTURE)


def chunk(kind, payload):
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload))


def png():
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0)) + chunk(
        b"IDAT", zlib.compress(b"\0\xff\0\0\xff")
    ) + chunk(b"IEND", b"")


class CaptureTests(unittest.TestCase):
    def setUp(self):
        self.directory = CAPTURE.ROOT / ".local/source-capture-tests" / uuid.uuid4().hex
        self.directory.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.directory)

    def test_missing_source_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "Missing/corrupt"):
            CAPTURE.verify(self.directory / "missing.jar", {"size_bytes": 3, "sha256": "unused"})

    def test_corrupt_source_fails_closed(self):
        path = self.directory / "source"
        path.write_bytes(b"abd")
        with self.assertRaisesRegex(ValueError, "Missing/corrupt"):
            CAPTURE.verify(path, {"size_bytes": 3, "sha256": hashlib.sha256(b"abc").hexdigest()})

    def test_source_reuse_checks_target_integrity(self):
        source, target = self.directory / "source", self.directory / "target"
        source.write_bytes(b"abc")
        target.write_bytes(b"abd")
        record = {"size_bytes": 3, "sha256": hashlib.sha256(b"abc").hexdigest()}
        with self.assertRaisesRegex(ValueError, "Missing/corrupt"):
            CAPTURE.link(source, target, record)
        self.assertEqual(source.read_bytes(), b"abc")

    def test_original_capture_png_dimensions(self):
        self.assertEqual(CAPTURE.png_dimensions(png()), (1, 1))

    def test_corrupt_png_chunk_rejected(self):
        data = bytearray(png())
        data[-1] ^= 1
        with self.assertRaisesRegex(ValueError, "Corrupt PNG"):
            CAPTURE.png_dimensions(bytes(data))

    def test_truncated_png_rejected(self):
        with self.assertRaisesRegex(ValueError, "Truncated"):
            CAPTURE.png_dimensions(png()[:-4])

    def test_empty_png_rejected(self):
        data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
        data += chunk(b"IEND", b"")
        with self.assertRaisesRegex(ValueError, "Incomplete"):
            CAPTURE.png_dimensions(data)

    def test_invalid_scanline_rejected(self):
        data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
        data += chunk(b"IDAT", zlib.compress(b"\5\xff\0\0\xff")) + chunk(b"IEND", b"")
        with self.assertRaisesRegex(ValueError, "scanlines"):
            CAPTURE.png_dimensions(data)

    def test_swallowed_original_render_exception_rejected(self):
        log = ("[info][exceptions] Exception <a 'java/lang/NullPointerException'>\n"
               " thrown in interpreter method <{method} 'draw' '()V' in 'fx'>\n"
               " at bci 10\n")
        self.assertEqual(len(CAPTURE.native_render_errors(log)), 1)

    def test_jvm_linker_probes_not_misreported_as_render_failures(self):
        log = ("[info][exceptions] Exception <a 'java/lang/NoSuchMethodError'>\n"
               " thrown in interpreter method <{method} 'resolve' '()V' in 'java/lang/invoke/MethodHandle'>\n")
        self.assertEqual(CAPTURE.native_render_errors(log), [])

    def test_committed_original_capture_bundle(self):
        result = CAPTURE.validate(CAPTURE.ROOT / "assets/reference/osrs240")
        self.assertEqual(result["captures"], 93)
        self.assertEqual(result["complete_native_animation_cycles"], [5666, 5668, 6180, 6181])
        self.assertFalse(result["owner_reference_pack_approved"])
        self.assertFalse(result["source_journey_verified"])


if __name__ == "__main__":
    unittest.main()

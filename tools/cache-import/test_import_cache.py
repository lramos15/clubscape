import hashlib
import importlib.util
import io
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import unittest
from unittest.mock import patch
import uuid
import zipfile
import zlib

SPEC = importlib.util.spec_from_file_location("cache_import", Path(__file__).with_name("import_cache.py"))
IMPORTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IMPORTER)


def chunk(kind, payload):
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload))


def png():
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0)) + chunk(
        b"IDAT", zlib.compress(b"\0\xff\0\0\xff")
    ) + chunk(b"IEND", b"")


class ImportTests(unittest.TestCase):
    def setUp(self):
        self.directory = IMPORTER.ROOT / ".local/cache-import-tests" / uuid.uuid4().hex
        self.directory.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.directory)

    def test_missing_and_corrupt_input(self):
        path = self.directory / "source.bin"
        record = {"size_bytes": 3, "sha256": hashlib.sha256(b"abc").hexdigest()}
        with self.assertRaisesRegex(IMPORTER.InputError, "Missing"):
            IMPORTER.checked_file(path, record)
        path.write_bytes(b"abc")
        self.assertEqual(IMPORTER.checked_file(path, record), path)
        path.write_bytes(b"abd")
        with self.assertRaisesRegex(IMPORTER.InputError, "SHA-256"):
            IMPORTER.checked_file(path, record)
        path.write_bytes(b"a")
        with self.assertRaisesRegex(IMPORTER.InputError, "Size"):
            IMPORTER.checked_file(path, record)

    def test_missing_cache_cli_exits_nonzero(self):
        command = [sys.executable, str(IMPORTER.TOOL / "import_cache.py"), "verify",
                   "--cache", str(self.directory / "missing")]
        result = subprocess.run(command, cwd=IMPORTER.ROOT, text=True, capture_output=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn("Missing input", result.stderr)

    def test_download_verifies_actual_stream(self):
        path = self.directory / "download.bin"
        record = {"url": "https://example.invalid/source.bin", "size_bytes": 3,
                  "sha256": hashlib.sha256(b"abc").hexdigest()}
        with patch.object(IMPORTER.urllib.request, "urlopen", return_value=io.BytesIO(b"abc")):
            IMPORTER.download(record, path)
        self.assertEqual(path.read_bytes(), b"abc")
        self.assertFalse(path.with_name(path.name + ".part").exists())

    def test_download_rejects_oversized_stream_and_cleans_partial(self):
        path = self.directory / "oversized.bin"
        record = {"url": "https://example.invalid/source.bin", "size_bytes": 3,
                  "sha256": hashlib.sha256(b"abc").hexdigest()}
        with patch.object(IMPORTER.urllib.request, "urlopen", return_value=io.BytesIO(b"abcd")):
            with self.assertRaisesRegex(IMPORTER.InputError, "exceeds"):
                IMPORTER.download(record, path)
        self.assertFalse(path.exists())
        self.assertFalse(path.with_name(path.name + ".part").exists())

    def test_download_rejects_corrupt_stream_and_cleans_partial(self):
        path = self.directory / "corrupt.bin"
        record = {"url": "https://example.invalid/source.bin", "size_bytes": 3,
                  "sha256": hashlib.sha256(b"abc").hexdigest()}
        with patch.object(IMPORTER.urllib.request, "urlopen", return_value=io.BytesIO(b"abd")):
            with self.assertRaisesRegex(IMPORTER.InputError, "SHA-256"):
                IMPORTER.download(record, path)
        self.assertFalse(path.exists())
        self.assertFalse(path.with_name(path.name + ".part").exists())

    def test_output_escape_is_rejected(self):
        with self.assertRaisesRegex(IMPORTER.InputError, "escapes"):
            IMPORTER.within(self.directory, self.directory / "../escape")

    def test_cache_zip_rejects_path_traversal(self):
        path = self.directory / "evil.zip"
        with zipfile.ZipFile(path, "w") as archive:
            archive.writestr("cache/../../escape", b"abc")
        with self.assertRaisesRegex(IMPORTER.InputError, "Unexpected"):
            IMPORTER.unpack_cache(path, self.directory / "output", [])

    def test_cache_zip_rejects_unadvertised_size(self):
        path = self.directory / "wrong-size.zip"
        with zipfile.ZipFile(path, "w") as archive:
            archive.writestr("cache/main_file_cache.dat2", b"abc")
        records = [{"name": "main_file_cache.dat2", "size_bytes": 2, "sha256": "unused"}]
        with self.assertRaisesRegex(IMPORTER.InputError, "size"):
            IMPORTER.unpack_cache(path, self.directory / "output", records)

    def test_cache_zip_rejects_missing_member(self):
        path = self.directory / "missing.zip"
        with zipfile.ZipFile(path, "w") as archive:
            archive.writestr("cache/", b"")
        records = [{"name": "main_file_cache.dat2", "size_bytes": 3, "sha256": "unused"}]
        with self.assertRaisesRegex(IMPORTER.InputError, "incomplete"):
            IMPORTER.unpack_cache(path, self.directory / "output", records)

    def test_cache_zip_roundtrip_and_corruption(self):
        path = self.directory / "cache.zip"
        data = b"original-source-test-bytes"
        with zipfile.ZipFile(path, "w") as archive:
            archive.writestr("cache/", b"")
            archive.writestr("cache/main_file_cache.dat2", data)
        records = [{"name": "main_file_cache.dat2", "size_bytes": len(data),
                    "sha256": hashlib.sha256(data).hexdigest()}]
        output = self.directory / "output"
        IMPORTER.unpack_cache(path, output, records)
        self.assertEqual((output / records[0]["name"]).read_bytes(), data)
        (output / records[0]["name"]).write_bytes(b"x" * len(data))
        with self.assertRaisesRegex(IMPORTER.InputError, "SHA-256"):
            IMPORTER.unpack_cache(path, output, records)

    def test_geometry_rejects_out_of_range_triangle(self):
        value = {"model": {"vertexCount": 3, "faceCount": 1, "vertexX": [0, 1, 0],
                          "vertexY": [0, 0, -1], "vertexZ": [0, 0, 0],
                          "faceIndices1": [0], "faceIndices2": [1], "faceIndices3": [2]}}
        IMPORTER.validate_model(value)
        value["model"]["faceIndices3"][0] = 3
        with self.assertRaisesRegex(IMPORTER.InputError, "out of bounds"):
            IMPORTER.validate_model(value)

    def test_geometry_rejects_bad_attribute_count(self):
        value = {"model": {"vertexCount": 3, "faceCount": 1, "vertexX": [0, 1, 0],
                          "vertexY": [0, 0, -1], "vertexZ": [0, 0, 0],
                          "faceIndices1": [0], "faceIndices2": [1], "faceIndices3": [2],
                          "faceColors": [1, 2]}}
        with self.assertRaisesRegex(IMPORTER.InputError, "cardinality"):
            IMPORTER.validate_model(value)

    def test_native_rgba_png_and_corrupt_crc(self):
        data = png()
        self.assertEqual(IMPORTER.validate_png(data), (1, 1))
        bad = bytearray(data)
        bad[-1] ^= 1
        with self.assertRaisesRegex(IMPORTER.InputError, "CRC"):
            IMPORTER.validate_png(bytes(bad))

    def test_png_rejects_missing_image_data(self):
        data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
        data += chunk(b"IEND", b"")
        with self.assertRaisesRegex(IMPORTER.InputError, "Incomplete"):
            IMPORTER.validate_png(data)

    def test_terrain_rejects_relocated_placements(self):
        value = {"dimensions": [4, 64, 64], "base_x": 3200, "base_y": 3200,
                 "placements": [[1277, 3201, 3201, 0, 10, 0]]}
        for name in ["heights", "underlay_ids", "overlay_ids", "overlay_shapes", "overlay_rotations",
                     "tile_settings", "encoded_heights"]:
            value[name] = [0] * 16384
        IMPORTER.validate_region(value, {1277})
        value["placements"][0][1] = 1
        with self.assertRaisesRegex(IMPORTER.InputError, "Relocated"):
            IMPORTER.validate_region(value, {1277})

    def test_committed_original_source_samples(self):
        result = IMPORTER.validate_published(IMPORTER.read_json(IMPORTER.DEFAULT_SELECTION))
        self.assertEqual(result["result"], "passed")
        self.assertGreater(result["published_files"], 0)
        self.assertFalse(result["source_capture_or_gameplay_acceptance"])


if __name__ == "__main__":
    unittest.main()

import copy
import hashlib
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("reference_inputs", ROOT / "tools/reference_inputs.py")
reference_inputs = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(reference_inputs)


class ReferenceInputTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)
        self.manifest = json.loads((ROOT / "research/first-slice-sources.json").read_text())
        self.item = copy.deepcopy(self.manifest["inputs"][0])
        self.data = b"synthetic unit fixture, not an OSRS source"
        self.item.update(
            size_bytes=len(self.data),
            sha256=hashlib.sha256(self.data).hexdigest(),
        )

    def test_known_manifest_has_nonempty_unique_hash_pinned_inputs(self):
        self.assertEqual(
            len(reference_inputs.validate_manifest(self.manifest, self.root)),
            len(self.manifest["inputs"]),
        )
        self.manifest["inputs"] = []
        with self.assertRaises(ValueError):
            reference_inputs.validate_manifest(self.manifest, self.root)

    def test_fetch_verifies_before_write_and_reuses_identical_inputs(self):
        self.assertTrue(reference_inputs.fetch_one(self.root, self.item, lambda _: self.data))
        path = reference_inputs.input_path(self.root, self.item)
        self.assertEqual(path.read_bytes(), self.data)
        self.assertFalse(reference_inputs.fetch_one(self.root, self.item, lambda _: self.fail("Refetched pinned bytes")))

    def test_hash_failure_leaves_no_input_or_temporary_file(self):
        with self.assertRaisesRegex(ValueError, "size/hash"):
            reference_inputs.fetch_one(self.root, self.item, lambda _: b"wrong input")
        self.assertEqual(list(self.root.rglob("*")), [])

    def test_changed_existing_data_is_not_overwritten(self):
        reference_inputs.fetch_one(self.root, self.item, lambda _: self.data)
        path = reference_inputs.input_path(self.root, self.item)
        path.write_bytes(b"owner change")
        with self.assertRaises(ValueError):
            reference_inputs.fetch_one(self.root, self.item, lambda _: self.data)
        self.assertEqual(path.read_bytes(), b"owner change")

    def test_missing_hash_foreign_url_duplicate_and_oversized_input_fail(self):
        for change in ("hash", "url", "duplicate", "size"):
            manifest = copy.deepcopy(self.manifest)
            if change == "hash":
                manifest["inputs"][0]["sha256"] = None
            elif change == "url":
                manifest["inputs"][0]["url"] = "http://127.0.0.1/internal"
            elif change == "duplicate":
                manifest["inputs"].append(manifest["inputs"][0])
            else:
                manifest["inputs"][0]["size_bytes"] = reference_inputs.MAX_INPUT_BYTES + 1
            with self.assertRaises(ValueError):
                reference_inputs.validate_manifest(manifest, self.root)

    def test_path_and_symlink_escape_are_rejected(self):
        for path in ("/tmp/reference.dat", "research/inputs/../../../outside", "crates/server/src/main.rs"):
            item = dict(self.item, path=path)
            with self.assertRaises(ValueError):
                reference_inputs.input_path(self.root, item)
        (self.root / "research").symlink_to(self.root)
        with self.assertRaises(ValueError):
            reference_inputs.input_path(self.root, self.item)


if __name__ == "__main__":
    unittest.main()

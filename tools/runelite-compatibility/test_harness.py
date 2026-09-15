import os
import json
from pathlib import Path
import stat
import unittest
from unittest.mock import patch

from prepare import LOCAL, ROOT, linux_arm64
from pack import package, private_descriptor_limit
from run import clean_environment, write_secret, validate_pack_selection


class HarnessChecks(unittest.TestCase):
    def test_secret_creation_is_private_exclusive_and_cleaned(self):
        path = LOCAL / "unit-secret"
        self.assertFalse(path.exists())
        try:
            value = write_secret(path)
            self.assertGreaterEqual(len(value), 32)
            self.assertTrue(path.read_text() == value)
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            with self.assertRaises(FileExistsError):
                write_secret(path)
        finally:
            path.unlink(missing_ok=True)

    def test_java_account_and_override_environment_is_not_inherited(self):
        with patch.dict(os.environ, {"JX_ACCESS_TOKEN": "synthetic-test-value",
                                     "_JAVA_OPTIONS": "-Duser.home=unisolated",
                                     "DATABASE_URL": "synthetic-test-value"}):
            result = clean_environment(LOCAL / "unit-home")
        for key in ["JX_ACCESS_TOKEN", "_JAVA_OPTIONS", "DATABASE_URL"]:
            self.assertNotIn(key, result)
        self.assertEqual(result["HOME"], str(LOCAL / "unit-home"))

    def test_pinned_native_platform_selection(self):
        self.assertTrue(linux_arm64({}))
        self.assertTrue(linux_arm64({"platform": [{"name": "linux", "arch": "aarch64"}]}))
        self.assertFalse(linux_arm64({"platform": [{"name": "linux"}]}))
        self.assertFalse(linux_arm64({"platform": [{"name": "win", "arch": "aarch64"}]}))

    def test_descriptor_capacity_comes_from_current_shared_source(self):
        self.assertEqual(private_descriptor_limit(), 512 * 1024)

    def test_changed_canonical_pack_cannot_overwrite_historical_outputs(self):
        directory = LOCAL / "unit-pack-history"
        directory.mkdir()
        descriptor = directory / "clubscape-game.json"
        artifact = directory / "world.csc"
        report = directory / "report.json"
        catalog = directory / "catalog.json"
        current = json.loads((ROOT / "content/m1/manifest.json").read_text())[
            "compiled_artifact"]["uncompressed_sha256"]
        try:
            artifact.write_bytes(b"historical-artifact-sentinel")
            descriptor.write_text(json.dumps({"sha256": "0" * 64}))
            with self.assertRaisesRegex(ValueError, "new --output"):
                package(directory, directory, report_path=report, catalog_path=catalog)
            descriptor.write_text(json.dumps({"sha256": current}))
            report.write_text(json.dumps({"artifact_sha256": "0" * 64}))
            with self.assertRaisesRegex(ValueError, "new --report"):
                package(directory, directory, report_path=report, catalog_path=catalog)
            report.unlink()
            catalog.write_text(json.dumps({"content_revision": "historical"}))
            with self.assertRaisesRegex(ValueError, "new --catalog"):
                package(directory, directory, report_path=report, catalog_path=catalog)
            self.assertEqual(artifact.read_bytes(), b"historical-artifact-sentinel")
        finally:
            for path in [descriptor, artifact, report, catalog]:
                path.unlink(missing_ok=True)
            directory.rmdir()

    def test_runner_rejects_obsolete_artifact_or_mismatched_catalog(self):
        directory = LOCAL / "unit-current-pack-selection"
        directory.mkdir()
        descriptor = directory / "clubscape-game.json"
        public = directory / "manifest.json"
        catalog = directory / "catalog.json"
        current = json.loads((ROOT / "content/m1/manifest.json").read_text())[
            "compiled_artifact"]["uncompressed_sha256"]
        try:
            descriptor.write_text(json.dumps({"sha256": "0" * 64}))
            with self.assertRaisesRegex(ValueError, "current canonical"):
                validate_pack_selection(directory, catalog)
            descriptor.write_text(json.dumps({"sha256": current}))
            public.write_text(json.dumps({"content_revision": "current"}))
            catalog.write_text(json.dumps({"content_revision": "obsolete"}))
            with self.assertRaisesRegex(ValueError, "different source content"):
                validate_pack_selection(directory, catalog)
            catalog.write_text(json.dumps({"content_revision": "current"}))
            validate_pack_selection(directory, catalog)
        finally:
            for path in [descriptor, public, catalog]:
                path.unlink(missing_ok=True)
            directory.rmdir()


if __name__ == "__main__":
    unittest.main()

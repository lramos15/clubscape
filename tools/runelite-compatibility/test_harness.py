import os
from pathlib import Path
import stat
import unittest
from unittest.mock import patch

from prepare import LOCAL, linux_arm64
from run import clean_environment, write_secret


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


if __name__ == "__main__":
    unittest.main()

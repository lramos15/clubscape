import copy
import importlib.util
import json
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
SPEC = importlib.util.spec_from_file_location("reference_runtime", ROOT / "tools/reference_runtime.py")
reference_runtime = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(reference_runtime)


class ReferenceRuntimeTests(unittest.TestCase):
    def setUp(self):
        self.manifest = json.loads((ROOT / "research/reference-runtime.json").read_text())

    def test_runtime_uses_two_exact_published_artifacts_not_legacy_archive(self):
        self.assertEqual(len(reference_runtime.validate_manifest(self.manifest)), 2)
        for key, value in [("url", "https://example.com/client.jar"), ("path", "../client.jar"), ("sha256", "")]:
            manifest = copy.deepcopy(self.manifest)
            manifest["artifacts"][0][key] = value
            with self.assertRaises(ValueError):
                reference_runtime.validate_manifest(manifest)

    def test_duplicate_or_unversioned_artifact_is_rejected(self):
        self.manifest["artifacts"][1] = self.manifest["artifacts"][0]
        with self.assertRaises(ValueError):
            reference_runtime.validate_manifest(self.manifest)
        self.manifest["version"] = "latest"
        with self.assertRaises(ValueError):
            reference_runtime.validate_manifest(self.manifest)

    def test_missing_build_method_is_not_inferred_from_package_name(self):
        with self.assertRaises(ValueError):
            reference_runtime.build_id_section("public class client {}")
        output = "  public int getBuildID();\n    Code:\n       0: sipush 240\n       3: ireturn\n\n  public void other();"
        self.assertEqual(reference_runtime.build_id_section(output), output.splitlines()[:4])


if __name__ == "__main__":
    unittest.main()

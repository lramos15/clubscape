"""Explicit-restore preflight fixtures only; never starts a database or a game service."""

import json
import unittest
from unittest.mock import patch

import private_checkpoint as PRIVATE
import resume
import test_private_checkpoint as fixtures


class ResumePreflightTests(unittest.TestCase):
    def fixture(self, *, unknown=False):
        fixture = fixtures.PrivateCheckpointTests("test_capture_pairs_private_identities_without_authorizing_resume_or_publishing_secrets")
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        fixture.database["world"]["last_sequence"] = 244
        fixture.database["selected_actual_receipts"] = []
        if not unknown:
            fixture.capsule["latest_attempt"]["observed_response"] = {
                "kind": "error_received", "http_status": 409,
            }
        fixture.capsule_path.unlink()
        fixture.store_json(fixture.capsule_path, fixture.capsule)
        scenario = json.loads(fixture.report_path.read_text())
        scenario["private_client_checkpoint"]["sha256"] = PRIVATE.digest(fixture.capsule_path)
        scenario["tutorial_edges_passed"] = [{} for _ in range(59)]
        scenario["segments"] = {"onboarding_recovery": {"status": "passed"}}
        fixture.report_path.unlink()
        fixture.store_json(fixture.report_path, scenario)
        fixture.store_json(fixture.root / "content/m1/manifest.json", {
            "compiled_artifact": {"uncompressed_sha256": fixture.identity["artifact_sha256"]},
        })
        available = fixture.preserve()
        self.assertEqual(available["status"], "available")
        return fixture, available

    def test_matching_private_data_requires_explicit_hash_and_does_not_restore(self):
        fixture, available = self.fixture()
        with patch.object(resume, "ROOT", fixture.root), \
                patch.object(PRIVATE, "private_command") as command:
            result = resume.verify_checkpoint(available["directory"], available["database_archive"]["sha256"])
        self.assertEqual(result[1]["run_id"], fixture.run_id)
        command.assert_not_called()

    def test_wrong_hash_and_unknown_write_are_not_resume_permissions(self):
        fixture, available = self.fixture(unknown=True)
        with patch.object(resume, "ROOT", fixture.root):
            with self.assertRaises(PRIVATE.CheckpointError):
                resume.verify_checkpoint(available["directory"], "0" * 64)
            with self.assertRaises(PRIVATE.CheckpointError) as error:
                resume.verify_checkpoint(available["directory"], available["database_archive"]["sha256"])
        self.assertEqual(error.exception.code, "unknown_attempt_requires_separate_reconciliation")

    def test_modified_private_file_cannot_be_restored(self):
        fixture, available = self.fixture()
        directory = fixture.root / available["directory"]
        (directory / "world.pgcustom").write_bytes(b"changed")
        with patch.object(resume, "ROOT", fixture.root):
            with self.assertRaises(PRIVATE.CheckpointError):
                resume.verify_checkpoint(available["directory"], available["database_archive"]["sha256"])


if __name__ == "__main__":
    unittest.main()

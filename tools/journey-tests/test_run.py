"""Client/orchestrator machinery tests only; never M1 gameplay evidence."""

import importlib.util
import json
from pathlib import Path
import shutil
import signal
import sys
import unittest
from unittest.mock import Mock, patch
import uuid


SPEC = importlib.util.spec_from_file_location("journey_orchestrator", Path(__file__).with_name("run.py"))
RUN = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = RUN
SPEC.loader.exec_module(RUN)


class OrchestratorTests(unittest.TestCase):
    def test_no_absolute_or_traversal_output(self):
        for path in ["/tmp/journey", "../journey", ".local/../journey", "/var/tmp/journey"]:
            with self.subTest(path=path), self.assertRaises(RUN.JourneyError):
                RUN.project_path(path)
        self.assertEqual(RUN.project_path(".local/evidence/run.json"), RUN.ROOT / ".local/evidence/run.json")

    def test_origin_is_literal_loopback_without_credentials_or_path(self):
        self.assertEqual(RUN.origin("http://127.0.0.1:43123/"), "http://127.0.0.1:43123")
        for url in [
            "http://127.0.0.1.evil.test:4000", "http://example.test:4000",
            "http://name:password@127.0.0.1:4000", "http://127.0.0.1:4000/path",
            "http://127.0.0.1:4000/?secret=x", "http://127.0.0.1",
        ]:
            with self.subTest(url=url), self.assertRaises(RUN.JourneyError):
                RUN.origin(url)

    def test_partial_or_fake_success_shaped_report_never_passes(self):
        report = {
            "scenario": "m1_fresh_account", "status": "passed",
            "full_journey_passed": True, "milestone_accepted": False,
            "tutorial_edges_passed": [{}] * 70,
            "segments": {segment: {"status": "passed"} for segment in RUN.SEGMENTS},
        }
        self.assertTrue(RUN.full_journey_passed(report))
        report["segments"]["source_death_office_grave_recovery"]["status"] = "unchecked"
        self.assertFalse(RUN.full_journey_passed(report))
        report["segments"]["source_death_office_grave_recovery"]["status"] = "passed"
        report["tutorial_edges_passed"].pop()
        self.assertFalse(RUN.full_journey_passed(report))
        report["tutorial_edges_passed"].append({})
        report["milestone_accepted"] = True
        self.assertFalse(RUN.full_journey_passed(report))

    def test_container_cleanup_rejects_other_owners(self):
        result = Mock(returncode=0, stdout=json.dumps({
            "clubscape.scope": "m1-headless-journey", "clubscape.owner": "someone-else",
        }))
        with patch.object(RUN, "bounded", return_value=result) as command:
            with self.assertRaises(RUN.JourneyError):
                RUN.cleanup_container("clubscape-journey-test", {})
            self.assertEqual(command.call_count, 1)

    def test_container_cleanup_only_removes_the_exact_owned_name(self):
        name = "clubscape-journey-exact"
        results = [
            Mock(returncode=0, stdout=json.dumps({
                "clubscape.scope": "m1-headless-journey", "clubscape.owner": name,
            })),
            Mock(returncode=0), Mock(returncode=1),
        ]
        report = {}
        with patch.object(RUN, "bounded", side_effect=results) as command:
            RUN.cleanup_container(name, report)
            self.assertEqual(command.call_args_list[1].args[0], ["docker", "rm", "--force", name])
        self.assertTrue(report["owned_database_removed"])

    def test_only_the_owned_process_object_is_terminated(self):
        server = RUN.OwnedServer.__new__(RUN.OwnedServer)
        server.process = Mock()
        server.process.poll.return_value = None
        server.process.returncode = 0
        server.log = Mock()
        self.assertEqual(server.stop(), 0)
        server.process.terminate.assert_called_once()
        server.process.kill.assert_not_called()
        server.process.wait.assert_called_once_with(timeout=20)

    def test_intentional_crash_is_not_a_clean_shutdown_claim(self):
        server = RUN.OwnedServer.__new__(RUN.OwnedServer)
        server.process = Mock()
        server.process.poll.return_value = None
        server.process.returncode = -signal.SIGKILL
        server.log = Mock()
        self.assertEqual(server.stop(crash=True), -signal.SIGKILL)
        server.process.kill.assert_called_once()
        server.process.terminate.assert_not_called()

    def test_atomic_evidence_and_symlink_rejection_stay_in_project(self):
        relative = Path(".local/journey-machinery-tests") / uuid.uuid4().hex
        directory = RUN.private_directory(relative)
        try:
            path = directory / "evidence.json"
            RUN.write_json(path, {"full_journey_passed": False})
            self.assertEqual(RUN.read_json(path), {"full_journey_passed": False})
            self.assertFalse(path.with_suffix(".pending.json").exists())
            link = directory / "link"
            link.symlink_to(directory, target_is_directory=True)
            with self.assertRaises(RUN.JourneyError):
                RUN.project_path(relative / "link" / "evidence.json")
        finally:
            shutil.rmtree(directory)

    def test_diagnostics_redact_database_credentials(self):
        value = "postgresql://user:sensitive@127.0.0.1:4321/isolated"
        self.assertNotIn("sensitive", RUN.redact(f"failure {value}", {"DATABASE_URL": value}))

    def test_compact_descriptor_changes_encoding_not_required_assets(self):
        relative = Path(".local/journey-machinery-tests") / uuid.uuid4().hex
        directory = RUN.private_directory(relative)
        try:
            data = {"assets": {"asset.original.one": "/assets/0", "asset.original.two": "/assets/1"}}
            compact = directory / "descriptor.json"
            pretty = directory / "evidence.json"
            RUN.write_json(compact, data, compact=True)
            RUN.write_json(pretty, data)
            self.assertEqual(RUN.read_json(compact), data)
            self.assertEqual(RUN.read_json(pretty), data)
            self.assertLess(compact.stat().st_size, pretty.stat().st_size)
        finally:
            shutil.rmtree(directory)


if __name__ == "__main__":
    unittest.main()

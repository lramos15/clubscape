"""Client/orchestrator machinery tests only; never M1 gameplay evidence."""

import importlib.util
import json
from pathlib import Path
import shutil
import signal
import sys
import tempfile
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

    def test_product_source_defaults_remain_canonical_and_choose_a_new_world(self):
        manifest, world = RUN.product_source_selection()
        self.assertEqual(manifest, RUN.ROOT / "content/m1/manifest.json")
        self.assertEqual(str(uuid.UUID(world)), world)
        self.assertNotEqual(uuid.UUID(world).int, 0)
        self.assertNotEqual(RUN.product_source_selection()[1], world)

    def test_explicit_product_source_retains_the_named_profile_and_world(self):
        world = "01234567-89ab-4cde-8f01-23456789abcd"
        path = "content/m1/legacy5e-water/manifest.json"
        self.assertEqual(
            RUN.product_source_selection(path, world),
            (RUN.ROOT / path, world),
        )

    def test_product_source_rejects_escaping_inputs_and_noncanonical_worlds(self):
        for path in ["/tmp/source.json", "../source.json", ".local/../source.json"]:
            with self.subTest(path=path), self.assertRaises(RUN.JourneyError):
                RUN.product_source_selection(path)
        for world in [
            "", "not-a-world", 1, str(uuid.UUID(int=0)),
            "01234567-89AB-4CDE-8F01-23456789ABCD",
            "{01234567-89ab-4cde-8f01-23456789abcd}",
        ]:
            with self.subTest(world=world), self.assertRaises(RUN.JourneyError):
                RUN.product_source_selection(world_id=world)

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

    def test_owned_shutdown_caps_both_waits_at_the_callers_remaining_deadline(self):
        server = RUN.OwnedServer.__new__(RUN.OwnedServer)
        server.process = Mock(**{"poll.return_value": None, "returncode": -signal.SIGKILL})
        server.process.wait.side_effect = [RUN.subprocess.TimeoutExpired("synthetic", 2), -signal.SIGKILL]
        server.log = Mock()
        with patch.object(RUN.time, "monotonic", side_effect=[100, 101.5]), \
                self.assertRaises(RUN.JourneyError):
            server.stop(deadline=102)
        self.assertEqual([call.kwargs["timeout"] for call in server.process.wait.call_args_list], [2, 0.5])
        server.process.kill.assert_called_once()

    def test_database_setup_propagates_absolute_budget_without_extending_old_defaults(self):
        for remaining, expected in ((None, [120, 10, 10]), (1, [1, 1, 1])):
            with tempfile.TemporaryDirectory(prefix="synthetic-database-deadline-", dir=RUN.ROOT / ".local") as name:
                report = {}
                outputs = [Mock(stdout="a" * 64, returncode=0), Mock(returncode=0),
                           Mock(stdout="127.0.0.1:43210", returncode=0)]
                with patch.object(RUN, "bounded", side_effect=outputs) as command, \
                        patch.object(RUN.time, "monotonic", return_value=100):
                    RUN.start_database(
                        Path(name), "synthetic-owner", report,
                        **({} if remaining is None else {"deadline": 100 + remaining, "command_timeout": 90}),
                    )
                self.assertEqual([call.kwargs["timeout"] for call in command.call_args_list], expected)
                self.assertEqual(report["owned_container_id"], "a" * 64)

    def test_expired_database_budget_is_refused_before_credentials_or_container_creation(self):
        with tempfile.TemporaryDirectory(prefix="synthetic-database-deadline-", dir=RUN.ROOT / ".local") as name:
            with patch.object(RUN, "bounded") as command, \
                    patch.object(RUN.time, "monotonic", return_value=100), \
                    self.assertRaises(RUN.JourneyError):
                RUN.start_database(Path(name), "synthetic-owner", {}, deadline=99, command_timeout=90)
            command.assert_not_called()
            self.assertEqual(list(Path(name).iterdir()), [])

    def test_expired_readiness_budget_does_not_open_logs_or_issue_health_requests(self):
        server = RUN.OwnedServer.__new__(RUN.OwnedServer)
        server.log_path = Mock()
        with patch.object(RUN.time, "monotonic", return_value=100), \
                patch.object(RUN.urllib.request, "build_opener") as opener, \
                self.assertRaises(RUN.JourneyError):
            server.ready(deadline=99)
        server.log_path.open.assert_not_called()
        opener.assert_not_called()

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

    def test_payload_storage_uses_allowed_binary_extension_without_changing_public_identity(self):
        self.assertEqual(RUN.payload_locations(0), ("/assets/0", "assets/0.bin"))
        self.assertEqual(RUN.payload_locations(35), ("/assets/23", "assets/23.bin"))
        for index in (-1, 20000, "1"):
            with self.assertRaises(RUN.JourneyError):
                RUN.payload_locations(index)

    def test_reaped_server_error_cannot_turn_into_clean_world_success(self):
        report = {"status": "passed", "full_journey_passed": True}
        RUN.record_server_exit(report, 1)
        self.assertTrue(report["owned_server_reaped"])
        self.assertFalse(report["server_exit_clean"])
        self.assertFalse(report["full_journey_passed"])
        self.assertEqual(report["status"], "blocked")
        original = {"phase": "source_tick", "reason": "unknown outcome"}
        report["first_failure"] = original
        RUN.record_server_exit(report, 1)
        self.assertEqual(report["first_failure"], original)

    def test_backup_failure_is_explicit_and_does_not_disclose_command_diagnostics(self):
        report = {"current_phase": "real_m1_fresh_account_scenario", "status": "blocked"}
        with patch.object(RUN, "preserve_checkpoint", side_effect=OSError(5, "private-auth-must-not-escape")):
            result = RUN.preserve_blocked_checkpoint(Path("unused"), report, None)
        self.assertEqual(result["status"], "failed")
        self.assertFalse(result["snapshot_available"])
        self.assertFalse(result["recoverable_checkpoint"])
        self.assertNotIn("private-auth-must-not-escape", json.dumps(result))

    def test_backup_precedes_database_cleanup_even_when_capture_raises(self):
        order = []
        report = {"status": "blocked", "full_journey_passed": False,
                  "first_failure": {"reason": "original source refusal"}}
        directory = RUN.ROOT / ".local/journey-runs/0123456789abcdef"
        def fail_capture(*_):
            order.append("checkpoint")
            raise RuntimeError("controlled machinery failure")
        with patch.object(RUN, "preserve_blocked_checkpoint", side_effect=fail_capture), \
                patch.object(RUN, "cleanup_container", side_effect=lambda *_: order.append("database")), \
                patch.object(RUN.shutil, "rmtree", side_effect=lambda *_: order.append("credentials")), \
                patch.object(RUN, "write_json", side_effect=lambda *_: order.append("report")):
            with self.assertRaises(RuntimeError):
                RUN.preserve_and_cleanup(directory, "owned-name", report, None, [], directory / "report.json")
        self.assertEqual(order, ["checkpoint", "database", "credentials", "report"])
        self.assertTrue(report["cleanup_passed"])
        self.assertFalse(report["private_checkpoint"]["snapshot_available"])
        self.assertEqual(report["first_failure"]["reason"], "original source refusal")


if __name__ == "__main__":
    unittest.main()

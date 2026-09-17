"""Narrow mainland continuation and full-success preservation machinery only."""

import json
import unittest
from unittest.mock import patch

import journey_contract
import mainland_continue as MAINLAND
import private_checkpoint as PRIVATE
import run
import test_private_checkpoint as fixtures


class MainlandContinuationTests(unittest.TestCase):
    def fixture(self):
        value = fixtures.PrivateCheckpointTests(
            "test_capture_pairs_private_identities_without_authorizing_resume_or_publishing_secrets")
        value.setUp()
        self.addCleanup(value.doCleanups)
        return value

    def successful_scenario(self, fixture):
        scenario = json.loads(fixture.report_path.read_text())
        scenario.update(
            scenario="m1_fresh_account", status="passed", full_journey_passed=True,
            milestone_accepted=False, tutorial_edges_passed=[{} for _ in range(70)],
            segments={name: {"status": "passed"} for name in journey_contract.SEGMENTS},
        )
        fixture.report_path.unlink()
        fixture.store_json(fixture.report_path, scenario)
        return scenario

    def test_full_success_is_captured_before_owned_data_is_removed(self):
        fixture = self.fixture()
        scenario = self.successful_scenario(fixture)
        self.assertTrue(journey_contract.full_journey_passed(scenario))
        result = fixture.preserve()
        self.assertEqual(result["status"], "available")
        self.assertTrue(result["captured_full_journey_passed"])
        self.assertFalse(result["private_payloads_published"])
        self.assertNotIn(fixture.secret, json.dumps(result))

    def test_success_label_without_all_original_segments_is_not_a_success_checkpoint(self):
        fixture = self.fixture()
        scenario = self.successful_scenario(fixture)
        scenario["segments"]["after_quest_recovery"]["status"] = "unchecked"
        fixture.report_path.unlink()
        fixture.store_json(fixture.report_path, scenario)
        self.assertFalse(journey_contract.full_journey_passed(scenario))
        result = fixture.preserve()
        self.assertEqual(result["status"], "failed")
        self.assertFalse(result["snapshot_available"])

    def test_full_success_capture_failure_cannot_leave_overall_success(self):
        fixture = self.fixture()
        scenario = self.successful_scenario(fixture)
        report = {"status": "passed", "full_journey_passed": True, "scenario": scenario,
                  "current_phase": "real_m1_fresh_account_scenario"}
        directory = run.ROOT / ".local/journey-runs/0123456789abcdef"
        order = []
        with patch.object(run, "preserve_blocked_checkpoint", return_value={"status": "failed"}), \
                patch.object(run, "cleanup_container", side_effect=lambda *_: order.append("database")), \
                patch.object(run.shutil, "rmtree", side_effect=lambda *_: order.append("files")), \
                patch.object(run, "write_json"):
            run.preserve_and_cleanup(directory, "owned", report, None, [], directory / "report.json")
        self.assertEqual(order, ["database", "files"])
        self.assertTrue(report["cleanup_passed"])
        self.assertEqual(report["status"], "blocked")
        self.assertFalse(report["full_journey_passed"])
        self.assertEqual(report["first_failure"]["phase"], "full_success_checkpoint")

    def test_completed_source_report_reaches_the_existing_private_capture(self):
        fixture = self.fixture()
        scenario = self.successful_scenario(fixture)
        report = {"status": "passed", "full_journey_passed": True, "scenario": scenario,
                  "current_phase": "real_m1_fresh_account_scenario"}
        with patch.object(run, "preserve_checkpoint", return_value={"status": "available"}) as capture:
            result = run.preserve_blocked_checkpoint(None, report, None)
        capture.assert_called_once()
        self.assertEqual(result["status"], "available")

    def test_mainland_authority_does_not_reuse_the_observation_attempt_record(self):
        fixture = self.fixture()
        path = MAINLAND.reserve_attempt(fixture.root, "actual-run", "committed-code")
        self.assertIn("mainland-continuation-authorizations", path)
        self.assertIn(MAINLAND.AUTHORITY, path)
        with self.assertRaises(FileExistsError):
            MAINLAND.reserve_attempt(fixture.root, "retry-not-authorized", "different-code")

    def test_native_preflight_must_decode_the_recorded_successful_poll(self):
        value = {"status": "validated", "network_operations": 0, "world_inputs": 0,
                 "mainland_boundary": {"authority": MAINLAND.AUTHORITY,
                   "decoded_original_control": {"decoded_command": "poll_world",
                     "observed_http_status": 200, "failed_control_exception_used": False}}}
        MAINLAND.validate_native_preflight(value)
        value["mainland_boundary"]["decoded_original_control"]["decoded_command"] = "logout"
        with self.assertRaises(PRIVATE.CheckpointError):
            MAINLAND.validate_native_preflight(value)


if __name__ == "__main__":
    unittest.main()

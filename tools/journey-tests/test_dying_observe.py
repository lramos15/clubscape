"""Observation-only adapter machinery; no database, server or restore is started."""

import copy
import json
import unittest
from unittest.mock import patch

import dying_observe as OBSERVE
import private_checkpoint as PRIVATE
import resume
import run
import test_private_checkpoint as fixtures


class DyingObservationTests(unittest.TestCase):
    def test_exact_pre_start_comparison_does_not_allow_even_derived_style_changes(self):
        expected = {"world": {"tick": 1616, "style": "old-sword"}, "rng": "original"}
        resume.verify_restored_identity(copy.deepcopy(expected), expected)
        changed = copy.deepcopy(expected)
        changed["world"]["style"] = "unarmed"
        with self.assertRaises(PRIVATE.CheckpointError):
            resume.verify_restored_identity(changed, expected)

    def test_private_phase_progression_is_separate_from_preserved_gameplay_data(self):
        before = {
            "owned_progress_sha256": "owned", "death_items_sha256": "death-items",
            "active_death": "death.original", "last_sequence": 301,
            "world_tick": 1616, "world_revision": 1928, "hitpoints": 0,
            "life": {"kind": "dying"}, "combat_style": "old-sword",
        }
        after = copy.deepcopy(before)
        after.update(world_tick=1625, world_revision=1940, hitpoints=10,
                     life={"kind": "first_death_office"}, combat_style="unarmed")
        proof = OBSERVE.compare_saved(before, after)
        self.assertTrue(proof["post_startup_lifecycle_and_ticks_not_claimed_unchanged"])
        self.assertEqual(proof["private_saved_before_startup"]["hitpoints"], 0)
        after["owned_progress_sha256"] = "changed-items-or-xp"
        with self.assertRaises(PRIVATE.CheckpointError):
            OBSERVE.compare_saved(before, after)

    def test_observation_success_requires_a_real_poll_and_zero_new_gameplay_inputs(self):
        scenario = {
            "status": "observed", "observation_only": True, "full_journey_passed": False,
            "input_count": 398, "last_sequence": 301,
            "dying_observation": {"successful_stop": True, "public_poll_succeeded": True,
                                  "world_inputs_emitted": 0},
        }
        OBSERVE.validate_observation_report(scenario, 398)
        scenario["input_count"] = 399
        with self.assertRaises(PRIVATE.CheckpointError):
            OBSERVE.validate_observation_report(scenario, 398)
        scenario["input_count"] = 398
        scenario["dying_observation"]["public_poll_succeeded"] = False
        with self.assertRaises(PRIVATE.CheckpointError):
            OBSERVE.validate_observation_report(scenario, 398)

    def test_observation_success_is_preserved_but_generic_success_is_not(self):
        fixture = fixtures.PrivateCheckpointTests(
            "test_capture_pairs_private_identities_without_authorizing_resume_or_publishing_secrets")
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        scenario = json.loads(fixture.report_path.read_text())
        scenario.update(status="observed", observation_only=True)
        scenario["dying_observation"] = {
            "authority": OBSERVE.AUTHORITY, "successful_stop": True,
            "public_poll_succeeded": True, "world_inputs_emitted": 0,
        }
        fixture.report_path.unlink()
        fixture.store_json(fixture.report_path, scenario)
        result = fixture.preserve()
        self.assertEqual(result["status"], "available")
        self.assertFalse(result["resume_authorized"])
        scenario["dying_observation"]["world_inputs_emitted"] = 1
        self.assertFalse(OBSERVE.success_checkpoint_eligible(scenario))

    def test_observation_success_and_failure_both_reach_capture_before_cleanup(self):
        for status in ("observed", "blocked"):
            with self.subTest(status=status):
                report = {"current_phase": "real_m1_dying_observation",
                          "observation_only": True, "status": status}
                with patch.object(run, "preserve_checkpoint", return_value={"status": "available"}) as capture:
                    value = run.preserve_blocked_checkpoint(None, report, None)
                capture.assert_called_once()
                self.assertEqual(value["status"], "available")

    def test_only_the_specific_failed_query_can_reach_native_decode_validation(self):
        valid = {"observed_http_status": 409, "operation_id": OBSERVE.REQUEST_ID,
                 "observed_error": [3, OBSERVE.ERROR_ID]}
        self.assertTrue(OBSERVE.failed_control_candidate(valid))
        self.assertFalse(OBSERVE.failed_control_candidate({**valid, "observed_http_status": 503}))
        self.assertFalse(OBSERVE.failed_control_candidate({**valid, "operation_id": "other"}))
        decoded = {"status": "validated", "network_operations": 0, "world_inputs": 0,
                   "observation_boundary": {"authority": OBSERVE.AUTHORITY,
                     "original_recorded_control": {"failed_control_exception_used": True,
                       "decoded_command": "logout", "operation_id": OBSERVE.REQUEST_ID,
                       "observed_http_status": 409, "observed_error": [3, OBSERVE.ERROR_ID]}}}
        with self.assertRaises(PRIVATE.CheckpointError):
            OBSERVE.validate_native_preflight(decoded)

    def test_single_authorized_restore_cannot_be_reserved_twice(self):
        fixture = fixtures.PrivateCheckpointTests(
            "test_capture_pairs_private_identities_without_authorizing_resume_or_publishing_secrets")
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        path = OBSERVE.reserve_attempt(fixture.root, "first-owned-run", "committed-code")
        original = (fixture.root / path).read_bytes()
        with self.assertRaises(FileExistsError):
            OBSERVE.reserve_attempt(fixture.root, "second-run", "other-code")
        self.assertEqual((fixture.root / path).read_bytes(), original)


if __name__ == "__main__":
    unittest.main()

"""Known907 admission and one-shot preservation, never a simulated game journey."""

from copy import deepcopy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import cook_continue as COOK
import journey_contract
import private_checkpoint as PRIVATE


ROOT = Path(__file__).resolve().parents[2]


class CookContinuationTests(unittest.TestCase):
    def fixture(self):
        authority = COOK.pinned_record(ROOT, COOK.AUTHORITY, COOK.AUTHORITY_PATH)
        takeover = COOK.pinned_record(ROOT, COOK.TAKEOVER, COOK.TAKEOVER_PATH)
        start = authority["required_start"]
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        (root / ".local").mkdir(mode=0o700)
        directory = root / start["checkpoint"]
        player = {
            "actor_id": start["actor_id"], "region": start["last_public_region"],
            "tile": start["last_public_tile"], "instance": None,
            "active_death": start["active_death_record"],
            "quests": {"quest.cooks_assistant": start["cook_stage"]},
            "skills": {"skill.cooking": {"xp_tenths": 0}},
            "quest_points": 1, "inventory": [],
        }
        state = {"tick": 1886, "revision": 2255, "next_sequence": 356, "player": player}
        scenario = {
            "status": "blocked", "full_journey_passed": False, "last_snapshot": state,
            "current_action": "cooks.actual_pot_cellar_bucket_dairy_milk",
            "first_failure": {"action": "cooks.actual_pot_cellar_bucket_dairy_milk"},
            "checks_passed": 336, "tutorial_edges_passed": [{} for _ in range(70)],
            "observation_checks_passed": 3,
            "segments": {name: {"status": "unchecked" if name in COOK.REMAINING else "passed"}
                         for name in journey_contract.SEGMENTS},
        }
        capsule = {"last_observed_state": deepcopy(state),
                   "latest_attempt": {"sequence": 355, "observed_response": {"kind": "acknowledgment_received"}}}
        available = {
            "database_archive": {"sha256": start["archive_sha256"]},
            "private_inventory_sha256": start["private_inventory_sha256"],
            "source_artifact_sha256": start["source_artifact_sha256"],
        }
        identity = {"database": {"world": {key: start[key] for key in (
            "world_id", "account_id", "actor_id", "last_sequence",
        )}}}
        return root, directory, available, capsule, scenario, identity, authority, takeover

    def admit(self, values):
        root, directory, available, capsule, scenario, identity, authority, takeover = values
        with patch.object(COOK, "pinned_record", side_effect=[authority, takeover]), \
                patch.object(COOK, "verify_execution_root"), patch.object(COOK, "verify_repair"), \
                patch.object(PRIVATE, "digest", return_value=authority["required_start"]["private_identity_sha256"]):
            return COOK.boundary(root, directory, available, capsule, scenario, identity)

    def test_known_boundary_uses_the_original_allowance_and_completed_segment_labels(self):
        values = self.fixture()
        before = deepcopy(values[2:])
        self.assertEqual(self.admit(values), values[6])
        self.assertEqual(values[2:], before)

    def test_different_scope_frontier_or_progress_is_rejected(self):
        for case in ("budget", "key", "account", "inventory", "source_count", "death", "already_delivered", "attempt"):
            with self.subTest(case=case):
                values = self.fixture()
                _, _, _, capsule, scenario, identity, authority, takeover = values
                if case == "budget":
                    authority["bounds"]["actual_archive_restore_invocations"] = 2
                elif case == "key":
                    takeover["execution"]["same_one_attempt_key"] = "new-key"
                elif case == "account":
                    identity["database"]["world"]["account_id"] = "another-account"
                elif case == "inventory":
                    scenario["last_snapshot"]["player"]["inventory"] = [{"not": "a legitimate replacement"}]
                elif case == "source_count":
                    scenario["checks_passed"] = 337
                elif case == "death":
                    scenario["segments"]["source_death_office_grave_recovery"]["status"] = "unchecked"
                elif case == "already_delivered":
                    scenario["segments"][COOK.REMAINING[0]]["status"] = "passed"
                else:
                    capsule["latest_attempt"]["observed_response"]["kind"] = "unresolved"
                with self.assertRaises(PRIVATE.CheckpointError):
                    self.admit(values)

    def test_director_assignment_does_not_mint_a_second_attempt_key(self):
        root = self.fixture()[0]
        path = COOK.reserve_attempt(root, "actual-run", "committed-code")
        self.assertEqual(path, f".local/cook-continuation-authorizations/{COOK.AUTHORITY}.json")
        with self.assertRaises(FileExistsError):
            COOK.reserve_attempt(root, "unapproved-second-run", "new-code")

    def test_native_preflight_cannot_admit_network_input_or_a_failed_control(self):
        value = {
            "status": "validated", "network_operations": 0, "world_inputs": 0,
            "cook_boundary": {
                "authority": COOK.AUTHORITY, "latest_acknowledged_sequence": 355,
                "completed_segments_replayed": False,
                "decoded_original_control": {"decoded_command": "poll_world",
                    "observed_http_status": 200, "failed_control_exception_used": False},
            },
        }
        COOK.validate_native_preflight(value)
        for field in ("network_operations", "world_inputs"):
            changed = deepcopy(value)
            changed[field] = 1
            with self.assertRaises(PRIVATE.CheckpointError):
                COOK.validate_native_preflight(changed)
        value["cook_boundary"]["decoded_original_control"]["failed_control_exception_used"] = True
        with self.assertRaises(PRIVATE.CheckpointError):
            COOK.validate_native_preflight(value)

    def test_ground_repair_identity_is_checked_without_modifying_it(self):
        root = self.fixture()[0]
        paths = ["crates/world-engine/src/actions.rs", "crates/world-engine/src/permissions.rs",
                 "tools/simulator/src/journey/source.rs"]
        for path in paths:
            target = root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(b"reviewed source fixture")
        with patch.object(COOK, "git_output", return_value=b"reviewed source fixture"):
            COOK.verify_repair(root)
            (root / paths[0]).write_bytes(b"changed")
            with self.assertRaises(PRIVATE.CheckpointError):
                COOK.verify_repair(root)


if __name__ == "__main__":
    unittest.main()

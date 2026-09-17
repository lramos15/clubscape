"""Pure operator-oracle/admission tests; no server, database, account or saved state."""

import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import water_operator as operator


def snapshot():
    return {
        "world": {
            "world_id": "01234567-89ab-4cde-8f01-23456789abc0",
            "content_revision": "old", "schema_version": 1, "revision": 9, "tick": 8,
            "runtime_artifact_sha256": "1" * 64, "runtime_random_key": "do-not-disclose",
            "lease_owner": None, "lease_expires_at": None, "lease_fence": 3,
            "last_tick_result": {"acknowledged": 8},
            "state": {"content_revision": "old", "schema_version": 1, "revision": 9, "tick": 8,
                      "characters": {}, "runtime": {"ui_version": 1, "audio_authority_version": 1}},
        },
        "counts": dict.fromkeys(operator.COUNTS, 0),
        "world_ids": ["01234567-89ab-4cde-8f01-23456789abc0"],
        "audits": [],
    }


def profile():
    return {
        "world_id": snapshot()["world"]["world_id"],
        "from": {"identity": {"artifact_sha256": "1" * 64},
                 "preparation": {"source_revision": "old"}},
        "to": {"identity": {"artifact_sha256": "2" * 64},
               "preparation": {"source_revision": "new"}},
    }


def migrated():
    result = snapshot()
    result["world"].update(content_revision="new", revision=10,
                           runtime_artifact_sha256="2" * 64, lease_fence=4)
    result["world"]["state"].update(content_revision="new", revision=10)
    result["audits"] = [{
        "world_id": result["world"]["world_id"], "from_artifact": "1" * 64,
        "to_artifact": "2" * 64, "world_revision": 10, "migrated_at": "2030-01-01T00:00:00Z",
    }]
    return result


def event(name, *, level="INFO", **fields):
    return json.dumps({"level": level, "fields": {"event": name, **fields}})


def admission():
    profiles = []
    for index, name in enumerate(("legacy5e", "current5b")):
        world = f"01234567-89ab-4cde-8f01-23456789abc{index}"
        row = {"profile": name, "world_id": world}
        for direction in ("from", "to"):
            row[direction] = {
                "game_root": f".local/water-source-operator-01/{name}/{direction}/game-root",
                "identity": {"world_id": world,
                             "artifact_sha256": str(index * 2 + 1 + (direction == "to")) * 64},
                "preparation": {"actual_source_asset_mappings": 5015},
            }
        profiles.append(row)
    preparation = {
        "status": "passed", "canonical_manifest_unchanged": True,
        "account_created": False, "saved_checkpoint_accessed": False,
        "saved_world_migrated": False, "profiles": profiles,
    }
    authority = {
        "task": "M1-WATER-UPGRADE-PREFLIGHT", "executor": "director",
        "worktree": operator.WORKTREE, "status": "admitted_isolated_synthetic_validation_only",
        "workload": {**operator.LIMITS, "source_operator_journal": f"{operator.WORKTREE}/{operator.JOURNAL}",
                     "database": "clubscape_m1_test", "automatic_retry": False},
        "saved_world_migration_or_continuation_admitted": False, "milestone_accepted": False,
        "component_progress": {"independent_native_review_fix_still_required": False},
        "prepared_source_operator": {
            "execution": {
                "status": "admitted", "candidate_head": "a" * 40,
                "binary": {"path": "target/debug/clubscape-server", "sha256": "b" * 64},
                "runner_sha256": "c" * 64, "review_record_sha256": "d" * 64,
            },
            "operator_invocations_used": 0,
            "worlds": [{"profile": row["profile"], "world_id": row["world_id"]} for row in profiles],
        },
        "exact_source_pairs": [
            {"profile": row["profile"],
             "from_raw_sha256": row["from"]["identity"]["artifact_sha256"],
             "to_raw_sha256": row["to"]["identity"]["artifact_sha256"]} for row in profiles
        ],
    }
    review = {"status": "passed", "fix_review": {"status": "passed"}}
    return authority, review, preparation


class WaterOperatorTests(unittest.TestCase):
    def test_exact_single_revision_transition_preserves_inputs(self):
        before, after = snapshot(), migrated()
        saved = copy.deepcopy((before, after))
        operator.require_migrated(before, after, profile())
        self.assertEqual((before, after), saved)

    def test_migration_cannot_hide_key_tick_history_or_ui_mutation(self):
        for change in (
            lambda row: row["world"].update(runtime_random_key="changed-secret"),
            lambda row: row["world"].update(tick=9),
            lambda row: row["world"]["state"]["runtime"].update(ui_version=2),
            lambda row: row["world"]["state"]["runtime"].update(audio_authority_version=2),
            lambda row: row["world"].update(last_tick_result=None),
            lambda row: row["world"]["state"].update(characters={"unexpected": {}}),
            lambda row: row["counts"].update(processed_game_commands=1),
        ):
            after = migrated()
            change(after)
            with self.subTest(change=change), self.assertRaises(operator.journey.JourneyError) as error:
                operator.require_migrated(snapshot(), after, profile())
            self.assertNotIn("secret", str(error.exception))
            self.assertNotIn("do-not-disclose", str(error.exception))

    def test_audit_is_exact_not_a_normalized_or_duplicate_receipt(self):
        for change in (
            lambda row: row["audits"].append(copy.deepcopy(row["audits"][0])),
            lambda row: row["audits"][0].update(from_artifact="3" * 64),
            lambda row: row["audits"][0].update(to_artifact="3" * 64),
            lambda row: row["audits"][0].update(world_revision=11),
            lambda row: row["audits"][0].update(migrated_at=""),
        ):
            after = migrated()
            change(after)
            with self.subTest(change=change), self.assertRaises(operator.journey.JourneyError):
                operator.require_migrated(snapshot(), after, profile())

    def test_idempotent_and_rejected_calls_only_advance_ownership_fence(self):
        before = migrated()
        after = copy.deepcopy(before)
        after["world"]["lease_fence"] += 1
        operator.require_unchanged(before, after)
        for change in (
            lambda row: row["world"].update(revision=11),
            lambda row: row["world"].update(lease_fence=4),
            lambda row: row["world"].update(lease_owner="still-owned"),
            lambda row: row["audits"][0].update(migrated_at="rewritten"),
        ):
            invalid = copy.deepcopy(after)
            change(invalid)
            with self.subTest(change=change), self.assertRaises(operator.journey.JourneyError):
                operator.require_unchanged(before, invalid)

    def test_busy_refusal_allows_live_ticks_but_not_pin_or_owner_changes(self):
        before = snapshot()
        before["world"].update(lease_owner="active-owner", lease_expires_at="future")
        after = copy.deepcopy(before)
        after["world"].update(tick=9, revision=10, lease_expires_at="renewed")
        after["world"]["state"].update(tick=9, revision=10)
        operator.require_busy_unchanged(before, after)
        for field in ("runtime_artifact_sha256", "runtime_random_key", "lease_owner", "lease_fence"):
            invalid = copy.deepcopy(after)
            invalid["world"][field] = "changed"
            with self.subTest(field=field), self.assertRaises(operator.journey.JourneyError):
                operator.require_busy_unchanged(before, invalid)

    def test_operator_failure_must_be_the_exact_expected_boundary(self):
        expected = event("startup_failure", level="ERROR", stage="game_ui_migration",
                         error_kind="exclusive_world_ownership")
        operator.require_operator_result(1, expected, "exclusive_world_ownership")
        for code, text in (
            (-9, expected), (0, expected), (1, ""),
            (1, expected.replace("exclusive_world_ownership", "state_migration")),
            (1, expected + "\n" + event("startup_failure", level="ERROR",
                                      stage="game_ui_migration", error_kind="cleanup")),
        ):
            with self.subTest(code=code, text=text), self.assertRaises(operator.journey.JourneyError):
                operator.require_operator_result(code, text, "exclusive_world_ownership")

    def test_operator_success_requires_one_completion_and_no_cleanup_error(self):
        complete = event("game_ui_migration_complete")
        operator.require_operator_result(0, complete, None)
        for code, text in (
            (1, complete), (0, ""), (0, complete + "\n" + complete),
            (0, complete + "\n" + event("startup_failure", level="ERROR", error_kind="cleanup")),
        ):
            with self.subTest(code=code), self.assertRaises(operator.journey.JourneyError):
                operator.require_operator_result(code, text, None)

    def test_exclusive_reservation_never_overwrites_a_previous_attempt(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "authority.json"
            first = {"status": "reserved", "operator_invocations": []}
            operator.reserve_journal(path, first)
            with self.assertRaises(FileExistsError):
                operator.reserve_journal(path, {"status": "passed"})
            self.assertEqual(json.loads(path.read_text()), first)
            link = Path(directory) / "linked.json"
            link.symlink_to(path)
            with self.assertRaises(FileExistsError):
                operator.reserve_journal(link, {"status": "passed"})
            self.assertEqual(json.loads(path.read_text()), first)

    def test_pending_native_review_refuses_before_any_execution(self):
        authority, _, _ = admission()
        authority["component_progress"]["independent_native_review_fix_still_required"] = True
        with patch.object(operator.OperatorRun, "execute") as execute:
            with self.assertRaisesRegex(operator.journey.JourneyError, "persisted-state repair"):
                operator.validate_admission(authority, {"status": "changes_required"}, {})
            execute.assert_not_called()

    def test_admission_needs_both_exact_worlds_with_unchanged_caps_and_no_saved_authority(self):
        authority, review, preparation = admission()
        self.assertEqual(operator.validate_admission(authority, review, preparation),
                         authority["prepared_source_operator"]["execution"])
        for change in (
            lambda auth, prep: auth["workload"].update(source_operator_invocations=11),
            lambda auth, prep: auth["workload"].update(source_server_starts=5),
            lambda auth, prep: auth["workload"].update(source_operator_accounts_or_gameplay_commands=False),
            lambda auth, prep: auth["workload"].update(database="production"),
            lambda auth, prep: auth["workload"].update(automatic_retry=True),
            lambda auth, prep: auth.update(saved_world_migration_or_continuation_admitted=True),
            lambda auth, prep: auth["prepared_source_operator"].update(execution=None),
            lambda auth, prep: auth["prepared_source_operator"].update(operator_invocations_used=1),
            lambda auth, prep: auth["prepared_source_operator"]["execution"].update(runner_sha256=""),
            lambda auth, prep: auth["prepared_source_operator"]["execution"]["binary"].update(path="../other"),
            lambda auth, prep: prep["profiles"].append(copy.deepcopy(prep["profiles"][0])),
            lambda auth, prep: prep["profiles"][0]["from"].update(game_root="../escaped"),
            lambda auth, prep: prep["profiles"][0]["from"]["identity"].update(artifact_sha256="wrong"),
            lambda auth, prep: prep.update(saved_world_migrated=True),
        ):
            changed_auth, changed_prep = copy.deepcopy((authority, preparation))
            change(changed_auth, changed_prep)
            with self.subTest(change=change), self.assertRaises(operator.journey.JourneyError):
                operator.validate_admission(changed_auth, review, changed_prep)

    def test_actual_delivery_byte_bound_and_symlinks_are_not_only_manifest_claims(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(operator, "BUNDLE_MAX_BYTES", 20):
            root = Path(directory)
            (root / "first.bin").write_bytes(b"1" * 20)
            operator.validate_bundle_files(root)
            (root / "extra.bin").write_bytes(b"1")
            with self.assertRaises(operator.journey.JourneyError):
                operator.validate_bundle_files(root)
            (root / "extra.bin").unlink()
            (root / "linked.bin").symlink_to(root / "first.bin")
            with self.assertRaises(operator.journey.JourneyError):
                operator.validate_bundle_files(root)

    def test_uncertain_docker_start_still_cleans_only_the_named_owned_container(self):
        runner = operator.OperatorRun.__new__(operator.OperatorRun)
        runner.container = "clubscape-water-operator-owned"
        runner.container_id = None
        runner.report = {}
        identity = "a" * 64
        responses = [
            Mock(stdout=identity),
            Mock(stdout=json.dumps({"clubscape.scope": "m1-water-source-operator",
                                    "clubscape.owner": runner.container})),
            Mock(stdout=identity), Mock(stdout=""),
        ]
        with patch.object(operator.journey, "bounded", side_effect=responses) as command:
            runner.cleanup_database()
        self.assertEqual(command.call_args_list[2].args[0], ["docker", "rm", "--force", identity])
        self.assertTrue(runner.report["owned_database_removed"])

    def test_database_cleanup_refuses_another_owner_even_after_an_uncertain_start(self):
        runner = operator.OperatorRun.__new__(operator.OperatorRun)
        runner.container = "clubscape-water-operator-owned"
        runner.container_id = None
        runner.report = {}
        responses = [
            Mock(stdout="a" * 64),
            Mock(stdout=json.dumps({"clubscape.scope": "m1-water-source-operator",
                                    "clubscape.owner": "someone-else"})),
        ]
        with patch.object(operator.journey, "bounded", side_effect=responses) as command:
            with self.assertRaises(operator.journey.JourneyError):
                runner.cleanup_database()
        self.assertEqual(command.call_count, 2)

    def test_snapshot_failure_does_not_expose_captured_private_payload(self):
        runner = operator.OperatorRun.__new__(operator.OperatorRun)
        runner.worlds = {profile()["world_id"]}
        runner.container_id = "a" * 64
        private = "private-runtime-key-must-not-be-logged"
        with patch.object(operator.subprocess, "run",
                          return_value=Mock(returncode=1, stdout=private, stderr=private)):
            with self.assertRaises(operator.journey.JourneyError) as error:
                runner.snapshot(profile())
        self.assertNotIn(private, str(error.exception))
        self.assertIn("private output withheld", str(error.exception))

    def test_exhausted_process_budgets_reject_before_preparing_or_starting_a_child(self):
        runner = operator.OperatorRun.__new__(operator.OperatorRun)
        runner.server = None
        runner.report = {"server_starts": [{}] * 4, "operator_invocations": [{}] * 10}
        with patch.object(runner, "environment") as environment:
            with self.assertRaises(operator.journey.JourneyError):
                runner.start_server(profile(), "from")
            with self.assertRaises(operator.journey.JourneyError):
                runner.operator(profile(), "extra", "1" * 64, None)
        environment.assert_not_called()


if __name__ == "__main__":
    unittest.main()

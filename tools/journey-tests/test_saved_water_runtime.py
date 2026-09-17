"""Synthetic migration/orchestration/capture fixtures. Every subprocess is replaced."""

import copy
import errno
import json
from pathlib import Path
import sys
import threading
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import private_checkpoint as PRIVATE
import saved_water_gate as WATER
import saved_water_runtime as RUNTIME
from test_saved_water import SyntheticFixture, store


def migration_pair():
    actor = {
        "inventory": {"slots": [{"item": "item.flour.pot", "quantity": 1},
                                {"item": "item.bucket", "quantity": 1}]},
        "bank": {"synthetic_owned_item": 9}, "equipment": {"synthetic_slot": "synthetic_item"},
        "skills": {"skill.cooking": {"xp_tenths": 3000, "base_level": 4}},
        "quests": {"quest.cooks_assistant": "stage.cooks.completed"}, "quest_points": 2,
        "runtime": {"ui": {"history": ["synthetic-ui"]},
                    "audio_authority": {"history": ["synthetic-audio"]}},
    }
    before = {
        "database": "clubscape_journey", "other_clients": 0,
        "worlds": [{
            "world_id": WATER.WORLD, "runtime_artifact_sha256": WATER.FROM,
            "content_revision": WATER.FROM_REVISION, "revision": 3804, "tick": 3290,
            "lease_fence": 8, "lease_owner": None, "lease_expires_at": None,
            "runtime_random_key_sha256": "9" * 64, "last_tick_result": {"synthetic_receipt": True},
            "state": {"content_revision": WATER.FROM_REVISION, "revision": 3804, "tick": 3290,
                      "characters": {WATER.ACTOR: actor},
                      "runtime": {"rng_cursor": "synthetic", "ui_version": 1, "audio_authority_version": 1}},
        }],
        "characters": [{"world_id": WATER.WORLD, "actor_id": WATER.ACTOR, "account_id": WATER.ACCOUNT,
                        "revision": 900, "last_sequence": 498}],
        "accounts": "a" * 64, "account_sessions": "b" * 64, "game_sessions": "c" * 64,
        "commands": {"count": 498, "sha256": "d" * 64},
        "lifecycle": {"count": 10, "sha256": "e" * 64}, "migrations": [],
        "schema_migrations": "f" * 64,
    }
    after = copy.deepcopy(before)
    world = after["worlds"][0]
    world.update(runtime_artifact_sha256=WATER.TO, content_revision=WATER.TO_REVISION,
                 revision=3805, lease_fence=9)
    world["state"].update(content_revision=WATER.TO_REVISION, revision=3805)
    after["characters"][0]["revision"] += 1
    after["migrations"] = [{
        "world_id": WATER.WORLD, "from_artifact": WATER.FROM, "to_artifact": WATER.TO,
        "world_revision": 3805, "migrated_at": "2030-01-01T00:00:00Z",
    }]
    return before, after


class MigrationOracleTests(unittest.TestCase):
    def test_complete_synthetic_minimal_migration_changes_only_pin_revision_audit_and_fence(self):
        before, after = migration_pair()
        proof = RUNTIME.verify_migration(before, after)
        self.assertTrue(proof["complete_private_conservation"])
        self.assertNotIn("synthetic_owned_item", json.dumps(proof))
        self.assertNotIn("9" * 64, json.dumps(proof))
        self.assertEqual(before["worlds"][0]["revision"], 3804)

    def test_each_private_gameplay_history_and_identity_drift_is_rejected(self):
        before, expected = migration_pair()
        for key in ("inventory", "bank", "equipment", "skills", "quests", "quest_points", "runtime"):
            after = copy.deepcopy(expected)
            after["worlds"][0]["state"]["characters"][WATER.ACTOR][key] = "synthetic-drift"
            with self.subTest(key=key), self.assertRaises(PRIVATE.CheckpointError):
                RUNTIME.verify_migration(before, after)
        for key in ("tick", "runtime_random_key_sha256", "last_tick_result", "lease_fence", "lease_owner",
                    "lease_expires_at", "runtime_artifact_sha256", "content_revision"):
            after = copy.deepcopy(expected)
            after["worlds"][0][key] = "synthetic-drift"
            with self.subTest(key=key), self.assertRaises(PRIVATE.CheckpointError):
                RUNTIME.verify_migration(before, after)
        for key in ("accounts", "account_sessions", "game_sessions", "commands", "lifecycle",
                    "characters", "schema_migrations"):
            after = copy.deepcopy(expected)
            after[key] = [] if key == "characters" else "synthetic-drift"
            with self.subTest(key=key), self.assertRaises(PRIVATE.CheckpointError):
                RUNTIME.verify_migration(before, after)

    def test_audit_duplication_wrong_from_or_no_fence_is_not_migration_success(self):
        before, expected = migration_pair()
        cases = []
        after = copy.deepcopy(expected)
        after["migrations"] *= 2
        cases.append(after)
        after = copy.deepcopy(expected)
        after["migrations"][0]["from_artifact"] = "0" * 64
        cases.append(after)
        after = copy.deepcopy(expected)
        after["worlds"][0]["lease_fence"] = before["worlds"][0]["lease_fence"]
        cases.append(after)
        for after in cases:
            with self.assertRaises(PRIVATE.CheckpointError):
                RUNTIME.verify_migration(before, after)
        with self.assertRaises(PRIVATE.CheckpointError):
            RUNTIME.verify_migration(expected, expected)


class RuntimeTests(SyntheticFixture):
    def execution(self):
        reservation = self.reservation()
        root_patch = patch.object(RUNTIME.JOURNEY, "ROOT", self.root)
        root_patch.start()
        self.addCleanup(root_patch.stop)
        execution = RUNTIME.Execution(reservation)
        execution.saved = WATER.read_saved52(reservation)
        return execution

    def synthetic_commands(self, execution, events):
        before, after = migration_pair()
        def command(phase, arguments, **kwargs):
            events.append((phase, [str(value) for value in arguments]))
            if phase == "migrate_ui":
                self.assertEqual(arguments[1:], ["migrate-ui", "--from", WATER.FROM])
                self.assertTrue(execution.report["restored_private_identity_equal"])
                self.assertEqual(execution.report["migration_outcome"], "unknown")
            directory = kwargs.get("directory") or execution.directory
            output = kwargs.get("output") or f"{phase}.stdout"
            store(directory / output, b"")
        def sql(directory, phase, text):
            events.append((phase, text))
            if phase == "empty_database":
                return 0
            return before if phase in {"migration_before", "restored_world"} else after
        return command, sql

    def test_exact_private_restore_identity_precedes_only_production_migrate_ui(self):
        execution = self.execution()
        events = []
        command, sql = self.synthetic_commands(execution, events)
        def start_database(directory, owner, report, **kwargs):
            self.assertEqual(kwargs, {"deadline": execution.deadline, "command_timeout": 90})
            events.append(("owned_database", owner))
            report["owned_container_id"] = "a" * 64
            return "postgresql://synthetic-only"
        def identity(*args, **kwargs):
            self.assertGreater(kwargs["timeout"], 0)
            self.assertLessEqual(kwargs["timeout"], 90)
            events.append(("complete_identity_equality", None))
            return copy.deepcopy(self.database)
        with patch.object(RUNTIME.JOURNEY, "start_database", side_effect=start_database), \
                patch.object(execution, "owner_identity"), \
                patch.object(execution, "sql", side_effect=sql), \
                patch.object(execution, "command", side_effect=command), \
                patch.object(PRIVATE, "database_identity", side_effect=identity):
            execution.restore()
            execution.migrate()
        names = [row[0] for row in events]
        self.assertLess(names.index("empty_database"), names.index("restore"))
        self.assertLess(names.index("restore"), names.index("complete_identity_equality"))
        self.assertLess(names.index("complete_identity_equality"), names.index("migrate_ui"))
        self.assertEqual(names.count("restore"), 1)
        self.assertEqual(names.count("migrate_ui"), 1)
        self.assertEqual(execution.report["migration_outcome"], "proved")
        self.assertTrue((execution.reservation.directory / "migration_proved.json").exists())

    def test_nonempty_or_nonidentical_restore_never_reaches_migration(self):
        execution = self.execution()
        with patch.object(execution, "sql", return_value=1), \
                patch.object(execution, "owner_identity"), \
                patch.object(RUNTIME.JOURNEY, "start_database", return_value="synthetic"), \
                patch.object(execution, "command") as command:
            with self.assertRaises(PRIVATE.CheckpointError):
                execution.restore()
            command.assert_not_called()
        execution.report["owned_container_id"] = "a" * 64
        with patch.object(execution, "sql", return_value=0), \
                patch.object(execution, "owner_identity"), \
                patch.object(RUNTIME.JOURNEY, "start_database", return_value="synthetic"), \
                patch.object(execution, "command"), \
                patch.object(PRIVATE, "database_identity", return_value={"not_the_original": True}):
            with self.assertRaises(PRIVATE.CheckpointError):
                execution.restore()
        self.assertFalse(execution.report["restored_private_identity_equal"])
        with patch.object(execution, "command") as command, self.assertRaises(PRIVATE.CheckpointError):
            execution.migrate()
        command.assert_not_called()

    def test_migration_timeout_or_bad_conservation_stays_unknown_without_retry_or_gameplay(self):
        execution = self.execution()
        execution.report["restored_private_identity_equal"] = True
        before, _ = migration_pair()
        execution.restored_view = before
        with patch.object(execution, "sql", return_value=before), \
                patch.object(execution, "command", side_effect=PRIVATE.CheckpointError("migration", "command_timeout")) as command:
            with self.assertRaises(PRIVATE.CheckpointError):
                execution.migrate()
        self.assertEqual(command.call_count, 1)
        self.assertEqual(execution.report["migration_outcome"], "unknown")
        self.assertFalse((execution.reservation.directory / "migration_proved.json").exists())
        with patch.object(execution, "sql", return_value=before), \
                patch.object(execution, "command") as command:
            with self.assertRaises(FileExistsError):
                execution.migrate()
            command.assert_not_called()

    def test_wrong_pin_or_drift_since_verified_restore_never_invokes_the_migration_cli(self):
        execution = self.execution()
        before, _ = migration_pair()
        execution.report["restored_private_identity_equal"] = True
        execution.restored_view = before
        for change in ("pin", "state"):
            altered = copy.deepcopy(before)
            if change == "pin":
                altered["worlds"][0]["runtime_artifact_sha256"] = "a" * 64
            else:
                altered["worlds"][0]["state"]["characters"][WATER.ACTOR]["quest_points"] = 99
            with patch.object(execution, "sql", return_value=altered), \
                    patch.object(execution, "command") as command, self.assertRaises(PRIVATE.CheckpointError):
                execution.migrate()
            command.assert_not_called()
        self.assertEqual(execution.report["migration_outcome"], "not_started")

    def test_native_arguments_never_choose_old_modes_default_content_or_unbounded_history_budget(self):
        execution = self.execution()
        arguments = list(map(str, execution.native_arguments(preflight=True)))
        self.assertIn("--continue-saved-water", arguments)
        self.assertIn(WATER.MANIFEST, arguments)
        self.assertIn("--validate-resume-only", arguments)
        self.assertEqual(arguments[arguments.index("--max-inputs") + 1], "128")
        self.assertEqual(arguments[arguments.index("--max-seconds") + 1], "600")
        for forbidden in ("--continue-cook", "--observe-dying", "--continue-mainland", "water_operator.py"):
            self.assertNotIn(forbidden, arguments)

    def test_orchestrator_failure_captures_before_cleanup_and_capture_failure_retains_everything(self):
        execution = self.execution()
        self.drive_failure(execution, capture_available=True)
        self.assertTrue(execution.report["cleanup_passed"])

    def test_capture_failure_never_deletes_owned_database_or_credentials(self):
        execution = self.execution()
        self.drive_failure(execution, capture_available=False)
        self.assertFalse(execution.report["cleanup_passed"])
        self.assertTrue(execution.report["owned_resources_retained"])
        self.assertTrue(execution.directory.exists())

    def drive_failure(self, execution, *, capture_available):
        events = []
        saved = execution.saved
        def native_command(phase, arguments, **kwargs):
            store(execution.directory / "native-preflight.json", {
                "status": "validated", "network_operations": 0, "world_inputs": 0,
                "saved_water_boundary": {"next_sequence": 499, "to_artifact": WATER.TO},
            })
        def restore():
            events.append("restore")
            execution.report["owned_container_id"] = "a" * 64
            execution.report["restored_private_identity_equal"] = True
        def migrate():
            events.append("migration_unknown")
            execution.report["migration_outcome"] = "unknown"
            raise PRIVATE.CheckpointError("migration", "synthetic_timeout")
        def capture():
            events.append("capture")
            return {"status": "available" if capture_available else "failed",
                    "snapshot_available": capture_available}
        def cleanup():
            events.append("cleanup")
            self.assertEqual(events[-2], "capture")
            execution.report["cleanup_passed"] = True
            execution.report["owned_resources_retained"] = False
        with patch.object(RUNTIME, "verify_delivery", return_value=self.root / "synthetic-delivery"), \
                patch.object(WATER, "read_saved52", return_value=saved), \
                patch.object(execution, "prepare_controls"), \
                patch.object(PRIVATE, "copy_game_root"), \
                patch.object(execution, "command", side_effect=native_command), \
                patch.object(execution, "restore", side_effect=restore), \
                patch.object(execution, "migrate", side_effect=migrate), \
                patch.object(execution, "gameplay", side_effect=AssertionError("No gameplay after unknown migration")), \
                patch.object(execution, "quiesce", side_effect=lambda: events.append("quiesce") or []), \
                patch.object(execution, "capture", side_effect=capture), \
                patch.object(execution, "cleanup", side_effect=cleanup):
            result = execution.run()
        self.assertEqual(result["status"], "blocked")
        self.assertEqual(result["migration_outcome"], "unknown")
        self.assertEqual(events, ["restore", "migration_unknown", "quiesce", "capture"]
                         + (["cleanup"] if capture_available else []))
        self.assertFalse(result["automatic_resume"])

    def capture_fixture(self, execution):
        execution.report["owned_container_id"] = "a" * 64
        execution.report["migration_outcome"] = "unknown"
        store(execution.directory / "postgres-password", b"synthetic-password\n")
        def sql(directory, phase, text):
            value = {"database": "clubscape_journey", "other_clients": 0, "public_tables": []}
            store(directory / f"{phase}.json", value)
            return value
        def command(phase, arguments, **kwargs):
            directory = kwargs["directory"]
            if phase == "database_dump":
                store(directory / kwargs["output"], b"PGDMP-synthetic-dump-not-postgresql")
            elif phase == "archive_integrity":
                self.assertIn("--file=/dev/null", arguments)
                self.assertNotIn("--dbname", arguments)
                store(directory / "archive-check.stdout", b"synthetic validation double")
            else:
                self.fail(phase)
        def source_copy(root, source, destination, identity):
            destination.mkdir(mode=0o700)
            store(destination / "synthetic-source.json", {"source_only_fixture": True})
        return sql, command, source_copy

    def test_fresh_failure_archive_retains_every_original_lineage_byte_and_no_resume_authority(self):
        execution = self.execution()
        sql, command, source_copy = self.capture_fixture(execution)
        with patch.object(execution, "owner_identity"), \
                patch.object(execution, "sql", side_effect=sql), \
                patch.object(execution, "command", side_effect=command), \
                patch.object(PRIVATE, "copy_game_root", side_effect=source_copy):
            result = execution.capture()
        self.assertEqual(result["status"], "available")
        self.assertFalse(result["resume_authorized"])
        self.assertFalse(result["automatic_restore"])
        self.assertEqual(result["migration_outcome"], "unknown")
        self.assertEqual(result["client_observation"], "original5e_capsule_only_no_new_client_capture")
        directory = self.root / result["directory"]
        for row in execution.saved.inventory["files"]:
            self.assertEqual(PRIVATE.digest(directory / "lineage" / row["path"]), row["sha256"])
        self.assertEqual(PRIVATE.digest(self.checkpoint / "private-inventory.json"), WATER.INVENTORY)
        self.assertNotIn("synthetic-password", json.dumps(result))

    def test_missing_new_capsule_is_explicit_capture_failure_not_success_shaped_fallback(self):
        execution = self.execution()
        sql, command, source_copy = self.capture_fixture(execution)
        execution.native = SimpleNamespace(poll=lambda: 1)
        with patch.object(execution, "owner_identity"), \
                patch.object(execution, "sql", side_effect=sql), \
                patch.object(execution, "command", side_effect=command), \
                patch.object(PRIVATE, "copy_game_root", side_effect=source_copy):
            result = execution.capture()
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["reason"], "new_native_capsule_missing_database_must_be_retained")
        self.assertTrue((self.root / result["directory"] / "world.pgcustom").exists())
        self.assertFalse((self.root / result["directory"] / "availability.json").exists())
        execution.report["private_checkpoint"] = result
        with patch.object(execution, "command") as command, self.assertRaises(PRIVATE.CheckpointError):
            execution.cleanup()
        command.assert_not_called()

    def test_cleanup_error_keeps_credentials_and_cannot_claim_resources_removed(self):
        execution = self.execution()
        execution.report["owned_container_id"] = "a" * 64
        execution.report["private_checkpoint"] = {"status": "available", "snapshot_available": True}
        store(execution.directory / "postgres-password", b"synthetic-only")
        with patch.object(execution, "owner_identity"), \
                patch.object(execution, "command", side_effect=PRIVATE.CheckpointError("cleanup", "synthetic_failure")), \
                self.assertRaises(PRIVATE.CheckpointError):
            execution.cleanup()
        self.assertTrue((execution.directory / "postgres-password").exists())
        self.assertFalse(execution.report["cleanup_passed"])

    def test_restart_reuses_only_owned_exact_database_and_target_not_arbitrary_request_origin(self):
        execution = self.execution()
        execution.server = SimpleNamespace(origin="http://127.0.0.1:1")
        store(execution.control / "restart-after_quest.request.json", {
            "checkpoint": "after_quest", "origin": "http://127.0.0.1:2",
            "require_same_isolated_database": True, "require_same_build_and_content": True,
            "next_sequence": 520,
        })
        with patch.object(execution, "start_server") as start, self.assertRaises(PRIVATE.CheckpointError):
            execution.restart_if_requested()
        start.assert_not_called()

    def test_successful_orchestration_still_captures_before_cleanup_and_never_accepts_m1(self):
        execution = self.execution()
        saved = execution.saved
        events = []
        def preflight(*args, **kwargs):
            store(execution.directory / "native-preflight.json", {
                "status": "validated", "network_operations": 0, "world_inputs": 0,
                "saved_water_boundary": {"next_sequence": 499, "to_artifact": WATER.TO},
            })
        def restore():
            events.append("restore")
            execution.report["owned_container_id"] = "a" * 64
        def migrate():
            events.append("migration")
            execution.report["migration_outcome"] = "proved"
        def gameplay():
            events.append("native_remaining")
            execution.report["status"] = "continued"
        def cleanup():
            events.append("cleanup")
            execution.report["cleanup_passed"] = True
        with patch.object(RUNTIME, "verify_delivery", return_value=self.root / "synthetic-delivery"), \
                patch.object(WATER, "read_saved52", return_value=saved), \
                patch.object(execution, "prepare_controls"), patch.object(PRIVATE, "copy_game_root"), \
                patch.object(execution, "command", side_effect=preflight), \
                patch.object(execution, "restore", side_effect=restore), \
                patch.object(execution, "migrate", side_effect=migrate), \
                patch.object(execution, "gameplay", side_effect=gameplay), \
                patch.object(execution, "quiesce", side_effect=lambda: events.append("quiesce") or []), \
                patch.object(execution, "capture", side_effect=lambda: events.append("capture") or {
                    "status": "available", "snapshot_available": True,
                }), patch.object(execution, "cleanup", side_effect=cleanup):
            report = execution.run()
        self.assertEqual(events, ["restore", "migration", "native_remaining", "quiesce", "capture", "cleanup"])
        self.assertEqual(report["status"], "continued")
        self.assertFalse(report["milestone_accepted"])
        self.assertFalse(report["full_journey_passed"])
        self.assertFalse(report["automatic_resume"])

    def native_report_fixture(self, execution, *, burn=False):
        scenario = copy.deepcopy(execution.saved.scenario)
        scenario.update(
            status="continued", saved_water_remainder_passed=True, saved_water_lifecycle_only_entry=True,
            identity={"content_artifact": {"uncompressed_sha256": WATER.TO}},
            saved_water_history={"source_identity": execution.saved.scenario["identity"]},
            checks_passed=378, source_transition={"multi_artifact_journey": True},
            saved_water_range={
                "status": "passed", "successes": int(not burn), "burns": int(burn),
                "cooking_xp_tenths": 3000 if burn else 3400, "rng_controlled": False,
                "old_reward_replayed": False, "ingredient_reacquisition": False,
            },
            saved_water_conversion={
                "status": "passed", "source_recipe": "recipe.water.bucket", "converted_buckets": 1,
                "extra_items_xp_or_quests": False, "acknowledged_operation_id": "synthetic-water", "sequence": 499,
            },
            saved_water_postquest_recovery={
                "status": "passed", "new_water_receipt_only": True,
                "same_target_logout_login_reconnect_restart": True, "replays": 4,
                "replayed_operation_id": "synthetic-water",
            },
        )
        for name in ("cooks_reward_and_range", "after_quest_recovery"):
            scenario["segments"][name] = {"status": "passed"}
        data = [{"operation_id": operation, "sequence": 499 + index, "retry_of_same_operation": False}
                for index, operation in enumerate(("synthetic-water", "synthetic-dough", "synthetic-range"))]
        data.extend({"operation_id": "synthetic-water", "sequence": 499, "retry_of_same_operation": True}
                    for _ in range(4))
        original = (execution.saved.directory / "evidence/scenario.trace.jsonl").read_bytes()
        trace = execution.directory / "scenario.trace.jsonl"
        store(trace, original + b"".join(json.dumps({
            "index": execution.saved.scenario["trace_records"] + index + 1,
            "kind": "input", "data": value, "synthetic_fixture_only": True,
        }).encode() + b"\n" for index, value in enumerate(data)))
        scenario["trace_records"] = execution.saved.scenario["trace_records"] + len(data)
        scenario["input_count"] = 603 + len(data)
        scenario["saved_water_new_input_count"] = len(data)
        return scenario, trace

    def test_complete_synthetic_trace_accepts_success_and_burn_and_keeps_historical_prefix(self):
        execution = self.execution()
        for burn in (False, True):
            scenario, trace = self.native_report_fixture(execution, burn=burn)
            RUNTIME.verify_native_report(execution.saved, scenario, trace)
            scenario["saved_water_range"]["cooking_xp_tenths"] += 400
            with self.assertRaises(PRIVATE.CheckpointError):
                RUNTIME.verify_native_report(execution.saved, scenario, trace)

    def test_completed_segment_old_reward_replay_and_trace_byte_rewrites_are_rejected(self):
        execution = self.execution()
        scenario, trace = self.native_report_fixture(execution)
        scenario["segments"]["goblin_combat"] = {"status": "passed"}
        with self.assertRaises(PRIVATE.CheckpointError):
            RUNTIME.verify_native_report(execution.saved, scenario, trace)
        scenario, trace = self.native_report_fixture(execution)
        rows = [json.loads(line) for line in trace.read_bytes().splitlines()]
        rows[-1]["data"]["sequence"] = 461
        rows[-1]["data"]["operation_id"] = "synthetic-old-cook-reward"
        original = (execution.saved.directory / "evidence/scenario.trace.jsonl").read_bytes()
        store(trace, original + b"".join(json.dumps(row).encode() + b"\n" for row in rows[1:]))
        with self.assertRaises(PRIVATE.CheckpointError):
            RUNTIME.verify_native_report(execution.saved, scenario, trace)
        scenario, trace = self.native_report_fixture(execution)
        store(trace, b" " + trace.read_bytes())
        with self.assertRaises(PRIVATE.CheckpointError) as caught:
            RUNTIME.verify_native_report(execution.saved, scenario, trace)
        self.assertEqual(caught.exception.code, "historical_trace_prefix_changed")

    def test_fresh_success_and_failed_native_capsules_keep_the_existing_schema_and_old_lineage(self):
        execution = self.execution()
        sql, command, source_copy = self.capture_fixture(execution)
        execution.native = SimpleNamespace(poll=lambda: 0)
        scenario, trace = self.native_report_fixture(execution, burn=True)
        capsule = copy.deepcopy(execution.saved.capsule)
        capsule.update(source_identity=scenario["identity"],
                       last_observation_origin="actual_current_invocation",
                       last_observed_state=scenario["last_snapshot"])
        store(execution.control / "private-client-checkpoint.json", capsule)
        scenario["private_client_checkpoint"] = {
            "status": "captured", "sha256": PRIVATE.digest(execution.control / "private-client-checkpoint.json"),
        }
        store(execution.directory / "scenario.json", scenario)
        with patch.object(execution, "owner_identity"), \
                patch.object(execution, "sql", side_effect=sql), \
                patch.object(execution, "command", side_effect=command), \
                patch.object(PRIVATE, "copy_game_root", side_effect=source_copy):
            result = execution.capture()
        self.assertEqual(result["status"], "available")
        current = PRIVATE.read_json(self.root / result["directory"] / "private-client.json", private=True)
        original = PRIVATE.read_json(self.root / result["directory"] / "lineage/private-client.json", private=True)
        self.assertEqual(current["kind"], "private_m1_client_checkpoint")
        self.assertEqual(current["source_identity"]["content_artifact"]["uncompressed_sha256"], WATER.TO)
        self.assertEqual(original["source_identity"]["content_artifact"]["uncompressed_sha256"], WATER.FROM)
        self.assertFalse(result["resume_authorized"])
        self.assertFalse(result["full_journey_passed"])

    def test_post_restore_failed_native_capsule_with_unknown_input_is_preserved_without_replay(self):
        execution = self.execution()
        sql, command, source_copy = self.capture_fixture(execution)
        execution.native = SimpleNamespace(poll=lambda: 1)
        scenario, trace = self.native_report_fixture(execution)
        scenario["status"] = "blocked"
        capsule = copy.deepcopy(execution.saved.capsule)
        capsule.update(source_identity=scenario["identity"], last_observation_origin="actual_current_invocation")
        capsule["latest_attempt"]["observed_response"] = {"kind": "unresolved"}
        store(execution.control / "private-client-checkpoint.json", capsule)
        scenario["private_client_checkpoint"] = {
            "status": "captured", "sha256": PRIVATE.digest(execution.control / "private-client-checkpoint.json"),
        }
        store(execution.directory / "scenario.json", scenario)
        with patch.object(execution, "owner_identity"), \
                patch.object(execution, "sql", side_effect=sql), \
                patch.object(execution, "command", side_effect=command), \
                patch.object(PRIVATE, "copy_game_root", side_effect=source_copy):
            result = execution.capture()
        self.assertEqual(result["status"], "available")
        saved = PRIVATE.read_json(self.root / result["directory"] / "private-client.json", private=True)
        self.assertEqual(saved["latest_attempt"]["observed_response"], {"kind": "unresolved"})
        self.assertFalse(result["resume_authorized"])

    def test_blocking_restart_cannot_extend_the_native_gameplay_deadline(self):
        execution = self.execution()
        observations = []
        for restart_delay in (0.0, 2.0):
            execution.native = None
            if (execution.directory / "native-gameplay.log").exists():
                (execution.directory / "native-gameplay.log").unlink()
            with patch.dict(WATER.BOUNDS, {"scenario_seconds": 1}), \
                    patch.object(execution, "phase"), \
                    patch.object(execution, "start_server"), \
                    patch.object(execution, "start_native_phase"), \
                    patch.object(execution, "native_arguments", return_value=[
                        sys.executable, "-c", "import time; time.sleep(30)",
                    ]), \
                    patch.object(execution, "restart_if_requested",
                                 side_effect=lambda: time.sleep(restart_delay)):
                started = time.monotonic()
                observation = {
                    "scenario_budget_seconds": 1,
                    "blocking_restart_seconds": restart_delay,
                    "sample_after_seconds": 1.5,
                }
                def sample_native():
                    observation["sample_elapsed_seconds"] = time.monotonic() - started
                    observation["native_still_running_after_deadline"] = (
                        execution.native is not None and execution.native.poll() is None
                    )
                sampler = threading.Timer(1.5, sample_native)
                sampler.start()
                try:
                    with self.assertRaises(PRIVATE.CheckpointError) as caught:
                        execution.gameplay()
                    self.assertEqual(caught.exception.code, "native_gameplay_deadline")
                    observation["gameplay_elapsed_seconds"] = time.monotonic() - started
                finally:
                    execution.quiesce()
                    sampler.join()
                observations.append(observation)
        print(json.dumps({"synthetic_deadline_observations": observations}), flush=True)
        for observation in observations:
            self.assertLess(observation["sample_elapsed_seconds"], 1.9, observation)
            self.assertFalse(observation["native_still_running_after_deadline"], observation)

    def test_expired_execution_refuses_phase_and_server_launch_without_taking_preservation_time(self):
        execution = self.execution()
        overall = execution.overall_deadline
        execution.deadline = time.monotonic() - 1
        with patch.object(execution.reservation.__class__, "verify") as verify, \
                self.assertRaises(PRIVATE.CheckpointError):
            execution.phase("synthetic_expired")
        verify.assert_not_called()
        with patch.object(execution, "check_target") as target, \
                patch.object(RUNTIME.JOURNEY, "OwnedServer") as server, \
                self.assertRaises(PRIVATE.CheckpointError):
            execution.start_server()
        target.assert_not_called()
        server.assert_not_called()
        self.assertEqual(execution.overall_deadline, overall)
        self.assertEqual(execution.report["server_starts"], 0)

    def test_public_gate_time_is_charged_and_cleanup_uses_only_the_original_overall_deadline(self):
        reservation = self.reservation()
        started = time.monotonic() - 30
        with patch.object(RUNTIME.JOURNEY, "ROOT", self.root):
            execution = RUNTIME.Execution(reservation, started_at=started)
            self.assertEqual(execution.overall_deadline, started + WATER.BOUNDS["total_seconds"])
            self.assertEqual(execution.deadline, execution.overall_deadline - 960)
            def quiesce():
                self.assertEqual(execution.deadline, execution.overall_deadline)
                return []
            with patch.object(execution, "phase", side_effect=PRIVATE.CheckpointError(
                    "saved_water", "synthetic_pre_restore_failure")), \
                    patch.object(execution, "quiesce", side_effect=quiesce):
                report = execution.run()
        self.assertEqual(report["status"], "blocked")
        self.assertGreaterEqual(report["elapsed_seconds"], 30)
        self.assertEqual(execution.overall_deadline, started + 2400)

    def test_expired_preservation_cannot_start_a_snapshot_or_cleanup(self):
        execution = self.execution()
        execution.deadline = time.monotonic() - 1
        with patch.object(PRIVATE, "private_directory") as directory, \
                self.assertRaises(PRIVATE.CheckpointError):
            execution.capture()
        directory.assert_not_called()
        with patch.object(execution, "owner_identity") as owner, \
                patch.object(execution, "command") as command, \
                self.assertRaises(PRIVATE.CheckpointError):
            execution.cleanup()
        owner.assert_not_called()
        command.assert_not_called()


class NativeDeadlineTests(unittest.TestCase):
    def test_unavailable_watchdog_stops_the_owned_native_before_reporting_failure(self):
        process = unittest.mock.Mock(pid=123456, **{"poll.return_value": None})
        watchdog = RUNTIME.NativeDeadline(process, time.monotonic() + 60)
        with patch.object(watchdog.timer, "start", side_effect=RuntimeError("synthetic")), \
                patch.object(RUNTIME.os, "killpg") as kill, \
                self.assertRaises(PRIVATE.CheckpointError) as caught:
            with watchdog:
                self.fail("Unavailable watchdog must not enter gameplay monitoring")
        self.assertEqual(caught.exception.code, "native_deadline_watchdog_unavailable")
        kill.assert_called_once_with(process.pid, RUNTIME.signal.SIGKILL)
        self.assertFalse(watchdog.timer.is_alive())

    def test_signal_failure_is_explicit_not_a_successful_timeout_claim(self):
        process = unittest.mock.Mock(pid=123456, **{"poll.return_value": None})
        with patch.object(RUNTIME.os, "killpg", side_effect=PermissionError(errno.EPERM, "synthetic")), \
                self.assertRaises(PRIVATE.CheckpointError) as caught:
            with RUNTIME.NativeDeadline(process, time.monotonic() + 60) as watchdog:
                watchdog.expire()
        self.assertEqual(caught.exception.code, f"native_deadline_signal_failed_{errno.EPERM}")
        self.assertFalse(watchdog.timer.is_alive())

    def test_monitor_failure_kills_only_the_owned_group_and_keeps_the_original_error(self):
        process = unittest.mock.Mock(pid=123456, **{"poll.return_value": None})
        with patch.object(RUNTIME.os, "killpg") as kill:
            with self.assertRaises(PRIVATE.CheckpointError) as caught:
                with RUNTIME.NativeDeadline(process, time.monotonic() + 60) as watchdog:
                    raise PRIVATE.CheckpointError("synthetic_monitor", "original_failure")
        self.assertEqual(caught.exception.code, "original_failure")
        kill.assert_called_once_with(process.pid, RUNTIME.signal.SIGKILL)
        self.assertFalse(watchdog.timer.is_alive())

    def test_context_exit_enforces_expiry_even_if_the_timer_has_not_run(self):
        process = unittest.mock.Mock(pid=123456, **{"poll.return_value": None})
        with patch.object(RUNTIME.os, "killpg") as kill, \
                self.assertRaises(PRIVATE.CheckpointError) as caught:
            with RUNTIME.NativeDeadline(process, time.monotonic() + 60) as watchdog:
                watchdog.deadline = time.monotonic() - 1
        self.assertEqual(caught.exception.code, "native_gameplay_deadline")
        kill.assert_called_once_with(process.pid, RUNTIME.signal.SIGKILL)
        self.assertFalse(watchdog.timer.is_alive())


if __name__ == "__main__":
    unittest.main()

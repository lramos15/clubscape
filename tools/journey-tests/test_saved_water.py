"""Synthetic private files and public authority doubles only; no saved account is accessed."""

import copy
import contextlib
import gzip
import io
import json
from pathlib import Path
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch

import private_checkpoint as PRIVATE
import saved_water_gate as WATER


REAL_ROOT = Path(__file__).resolve().parents[2]
HEAD = "1" * 40
ADMISSION_REVISION = "2" * 40
REVIEW_REVISION = "3" * 40


def store(path, value):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    if path.exists():
        path.unlink()
    with PRIVATE.private_file(path) as stream:
        stream.write(value if isinstance(value, bytes) else json.dumps(value, sort_keys=True).encode())


def synthetic_boundary():
    state = {
        "tick": 3280, "revision": 3794, "next_sequence": 499,
        "player": {
            "actor_id": WATER.ACTOR, "region": "region.osrs.12850",
            "tile": {"x": 3208, "y": 3216, "plane": 0}, "instance": None,
            "tutorial_stage": "stage.tutorial.mainland",
            "quests": {"quest.cooks_assistant": "stage.cooks.completed"}, "quest_points": 2,
            "skills": {"skill.cooking": {"xp_tenths": 3000, "base_level": 4}},
            "inventory": [{"index": index, "stack": {"item": item, "quantity": 1}}
                          for index, item in enumerate(("item.flour.pot", "item.bucket"))],
        },
    }
    source = {"content_revision": WATER.FROM_REVISION,
              "content_artifact": {"uncompressed_sha256": WATER.FROM}}
    capsule = {
        "schema_version": 1, "kind": "private_m1_client_checkpoint", "synthetic_fixture_only": True,
        "scenario": "m1_fresh_account", "source_identity": source, "last_observed_state": state,
        "actor_id": WATER.ACTOR, "automatic_restore": False, "resume_authorized": False,
        "private_authentication_do_not_publish": {
            "account_id": WATER.ACCOUNT, "login_name": "synthetic_m1",
            "password": "synthetic-not-an-account-password",
        },
        "latest_attempt": {
            "operation_id": "11111111-2222-4333-8444-555555555555", "sequence": 498,
            "world_input_protobuf": [],  # Native typed-decoder fixtures live in saved_water.rs.
            "observed_response": {"kind": "acknowledgment_received", "duplicate": False, "next_sequence": 499},
        },
        "latest_control_request": None, "original_receipts": {"onboarding": None, "reward": None},
    }
    scenario = {
        "synthetic_fixture_only": True, "scenario": "m1_fresh_account", "status": "blocked",
        "full_journey_passed": False, "milestone_accepted": False, "identity": source,
        "last_snapshot": state, "current_action": "cooks.reward_range_actual_recipe",
        "first_failure": {"action": "cooks.reward_range_actual_recipe"},
        "checks_passed": 377, "observation_checks_passed": 3, "input_count": 603,
        "tutorial_edges_passed": [{"synthetic_edge": index} for index in range(70)],
        "trace_records": 1,
        "segments": {name: {"status": "unchecked" if name in {
            "cooks_reward_and_range", "after_quest_recovery",
        } else "passed", "synthetic_historical_record": index}
            for index, name in enumerate(WATER.JOURNEY.SEGMENTS)},
    }
    database = {
        "database": "clubscape_journey", "world_count": 1, "character_count": 1,
        "other_clients": 0, "server_version_num": "synthetic",
        "world": {
            "world_id": WATER.WORLD, "account_id": WATER.ACCOUNT, "actor_id": WATER.ACTOR,
            "last_sequence": 498, "tick": 3290, "revision": 3804,
            "artifact_sha256": WATER.FROM, "content_revision": WATER.FROM_REVISION,
            "private_rng_key_sha256": "9" * 64, "world_state_sha256": "8" * 64,
        },
        "selected_actual_receipts": [{
            "operation_id": capsule["latest_attempt"]["operation_id"], "sequence": 498,
        }],
    }
    return capsule, scenario, database


class SyntheticFixture(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="synthetic-saved-water-", dir=REAL_ROOT / ".local")
        self.addCleanup(temporary.cleanup)
        self.canonical = Path(temporary.name) / "canonical"
        self.root = self.canonical / WATER.WORKTREE
        (self.root / ".local").mkdir(parents=True, mode=0o700)
        self.checkpoint = self.canonical / WATER.SOURCE_WORKTREE / WATER.CHECKPOINT
        self.checkpoint.mkdir(parents=True, mode=0o700)
        self.checkpoint.parent.chmod(0o700)
        self.capsule, self.scenario, self.database = synthetic_boundary()
        self.archive = b"PGDMP-SYNTHETIC-NOT-A-REAL-ARCHIVE" + b"\0" * (239240 - 32)
        self.archive = self.archive[:239240].ljust(239240, b"\0")
        self.original = {"scenario": self.scenario, "synthetic_fixture_only": True}
        self.identity = {
            "schema_version": 1, "database": self.database,
            "database_archive_sha256": WATER.sha_bytes(self.archive),
            "game_root_identity": {"world_id": WATER.WORLD, "artifact_sha256": WATER.FROM},
        }
        data = {
            "world.pgcustom": self.archive, "private-client.json": self.capsule,
            "private-service.json": {"synthetic_fixture_only": True},
            "evidence/scenario.trace.jsonl": b'{"index":1,"kind":"synthetic_history","data":{}}\n',
            "game-root/clubscape-game.json": {"synthetic_fixture_only": True},
            "game-root/clubscape-game-assets.json": {"synthetic_fixture_only": True},
        }
        for name, value in data.items():
            store(self.checkpoint / name, value)
        self.scenario["private_client_checkpoint"] = {
            "status": "captured", "sha256": PRIVATE.digest(self.checkpoint / "private-client.json"),
        }
        self.identity["client_capsule_sha256"] = PRIVATE.digest(self.checkpoint / "private-client.json")
        self.identity["service_config_sha256"] = PRIVATE.digest(self.checkpoint / "private-service.json")
        store(self.checkpoint / "evidence/scenario.json", self.scenario)
        store(self.checkpoint / "private-orchestrator-before-cleanup.json", self.original)
        store(self.checkpoint / "private-identity.json", self.identity)
        for index in range(49):
            store(self.checkpoint / f"synthetic-{index:02}.json", {"synthetic_only": index})
        inventory_hash = PRIVATE.seal_inventory(self.checkpoint)
        inventory = PRIVATE.read_json(self.checkpoint / "private-inventory.json", private=True)
        self.assertEqual(len(inventory["files"]), 58)
        available = {
            "schema_version": 1, "run_id": WATER.RUN, "directory": WATER.CHECKPOINT,
            "status": "available", "snapshot_available": True,
            "database_archive": {"path": "world.pgcustom", "bytes": 239240, "sha256": WATER.sha_bytes(self.archive)},
            "private_inventory_sha256": inventory_hash, "source_artifact_sha256": WATER.FROM,
            "resume_authorized": False, "metadata_before_after_equal": True,
            "archive_integrity_verified": True,
        }
        store(self.checkpoint / "availability.json", available)
        for name in WATER.PUBLIC_INPUTS:
            store(self.canonical / name, {"synthetic_public_input_only": name})
        public_report = gzip.compress(json.dumps({"scenario": self.scenario}, sort_keys=True).encode(), mtime=0)
        store(self.canonical / WATER.PUBLIC_REPORT, public_report)
        store(self.root / WATER.MANIFEST, {"synthetic_manifest_only": True})
        replacements = {
            "ARCHIVE": WATER.sha_bytes(self.archive), "INVENTORY": inventory_hash,
            "IDENTITY": PRIVATE.digest(self.checkpoint / "private-identity.json"),
            "PUBLIC_REPORT_HASH": WATER.sha_bytes(public_report),
            "DELIVERY_INDEX_HASH": PRIVATE.digest(self.canonical / WATER.DELIVERY_INDEX),
            "DELIVERY_FILES_HASH": PRIVATE.digest(self.canonical / WATER.DELIVERY_FILES),
            "MANIFEST_HASH": PRIVATE.digest(self.root / WATER.MANIFEST),
        }
        patcher = patch.multiple(WATER, **replacements)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.review = {
            "kind": "m1_saved_water_continuation_code_review", "status": "passed",
            "code_revision": HEAD, "reviewer": "synthetic-independent-reviewer",
            "saved_execution_admitted": False, "synthetic_fixture_only": True,
        }
        store(self.canonical / WATER.REVIEW_PATH, self.review)
        self.record = {
            "schema_version": 1, "kind": "m1_saved52_water_execution", "status": "admitted",
            "authority": "director", "milestone": "m1-starter-journey",
            "saved_execution_admitted": True, "checkpoint_access_admitted": True,
            "automatic_resume": False, "milestone_accepted": False, "later_milestone_authorized": False,
            "canonical_git_root": str(self.canonical),
            "executor": {"id": "synthetic-executor", "root": str(self.root), "worktree": WATER.WORKTREE,
                         "branch": WATER.BRANCH, "code_revision": HEAD},
            "start": WATER.expected_start(self.canonical), "target": WATER.expected_target(self.canonical),
            "bounds": WATER.BOUNDS, "journal": str(self.root / WATER.JOURNAL),
            "output": str(self.root / WATER.OUTPUT),
            "independent_review": {"revision": REVIEW_REVISION, "path": WATER.REVIEW_PATH,
                                   "sha256": PRIVATE.digest(self.canonical / WATER.REVIEW_PATH)},
            "public_inputs": {name: PRIVATE.digest(self.canonical / name) for name in WATER.PUBLIC_INPUTS},
            "binaries": {}, "synthetic_fixture_only": True,
        }
        for name, path in WATER.BINARY_PATHS.items():
            store(self.root / path, b"synthetic-not-an-executable")
            (self.root / path).chmod(0o700)
            self.record["binaries"][name] = {"path": path, "sha256": PRIVATE.digest(self.root / path)}
        self.publish_record()
        self.git_calls = []
        git_patch = patch.object(WATER, "git", side_effect=self.git_double)
        git_patch.start()
        self.addCleanup(git_patch.stop)

    def publish_record(self):
        store(self.canonical / WATER.ADMISSION_PATH, self.record)
        self.admission_hash = PRIVATE.digest(self.canonical / WATER.ADMISSION_PATH)

    def git_double(self, root, arguments):
        self.git_calls.append(arguments)
        if arguments == ["rev-parse", "--path-format=absolute", "--git-common-dir"]:
            return str(self.canonical / ".git").encode()
        if arguments == ["rev-parse", "--show-toplevel"]:
            return str(root).encode()
        if arguments == ["branch", "--show-current"]:
            return WATER.BRANCH.encode()
        if arguments == ["rev-parse", "HEAD"]:
            return HEAD.encode()
        if arguments == ["status", "--porcelain", "--untracked-files=normal"]:
            return b""
        if arguments == ["show", f"{ADMISSION_REVISION}:{WATER.ADMISSION_PATH}"]:
            return (self.canonical / WATER.ADMISSION_PATH).read_bytes()
        if arguments == ["show", f"{REVIEW_REVISION}:{WATER.REVIEW_PATH}"]:
            return (self.canonical / WATER.REVIEW_PATH).read_bytes()
        self.fail(f"Unexpected synthetic public Git command: {arguments}")

    def admission(self):
        return WATER.verify_admission(ADMISSION_REVISION, self.admission_hash,
                                      "synthetic-executor", root=self.root)

    def reservation(self):
        return WATER.reserve(self.admission())


class AdmissionTests(SyntheticFixture):
    def test_actual_script_bootstrap_shares_runtime_authority_types_and_cannot_retry(self):
        import saved_water_runtime as runtime
        original_verify = WATER.verify_admission
        def verify(revision, digest, executor, *, root=None):
            self.assertIn(root, (None, self.root))
            return original_verify(revision, digest, executor, root=self.root)
        def execute(reservation, *, started_at):
            self.assertIs(type(reservation), WATER.Reservation)
            self.assertIs(type(reservation.admission), WATER.Admission)
            self.assertGreater(started_at, 0)
            saved = runtime.GATE.read_saved52(reservation)
            self.assertEqual(saved.directory, self.checkpoint)
            self.assertEqual(saved.scenario, self.scenario)
            return 0
        script = REAL_ROOT / "tools/journey-tests/saved_water.py"
        arguments = [str(script), "--admission-revision", ADMISSION_REVISION,
                     "--admission-sha256", self.admission_hash, "--executor-id", "synthetic-executor"]
        with patch.object(WATER, "verify_admission", side_effect=verify), \
                patch.object(runtime, "execute", side_effect=execute) as execution, \
                patch.object(WATER.signal, "signal"), \
                patch.object(sys, "argv", arguments), contextlib.redirect_stdout(io.StringIO()):
            for expected in (0, 1):
                with self.assertRaises(SystemExit) as stopped:
                    runpy.run_path(str(script), run_name="__main__")
                self.assertEqual(stopped.exception.code, expected)
        execution.assert_called_once()
        self.assertTrue((self.root / WATER.JOURNAL).is_file())

    def test_valid_public_gate_never_touches_the_checkpoint_until_exclusive_reservation(self):
        original = PRIVATE.project_path
        protected_calls = []
        def guarded(root, value):
            candidate = root / value
            if candidate.is_relative_to(self.checkpoint):
                protected_calls.append(candidate)
                self.assertTrue((self.root / WATER.JOURNAL).is_file())
                self.assertTrue((self.root / WATER.OUTPUT / "protected_read_started.json").is_file())
            return original(root, value)
        with patch.object(PRIVATE, "project_path", side_effect=guarded), \
                patch.object(PRIVATE, "private_command", side_effect=AssertionError("No native/database commands")):
            admission = self.admission()
            self.assertFalse(protected_calls)
            reservation = WATER.reserve(admission)
            self.assertFalse(protected_calls)
            saved = WATER.read_saved52(reservation)
        self.assertEqual(saved.identity["database"], self.database)
        self.assertEqual(saved.capsule["source_identity"]["content_artifact"]["uncompressed_sha256"], WATER.FROM)
        self.assertTrue(protected_calls)
        self.assertEqual(saved.identity["database"]["world"]["tick"], 3290)
        self.assertEqual(saved.scenario["last_snapshot"]["tick"], 3280)

    def test_implementation_owner_preparation_and_wrong_exact_admissions_are_not_execution_authority(self):
        cases = [
            ("kind", "bounded_source_content_upgrade"), ("status", "admitted_code_and_fixture_work_only"),
            ("saved_execution_admitted", False), ("checkpoint_access_admitted", False),
            ("canonical_git_root", str(self.canonical / "other")),
            ("start", {**self.record["start"], "checkpoint_root": str(self.canonical / "other-checkpoint")}),
            ("target", {**self.record["target"], "artifact_sha256": "a" * 64}),
            ("bounds", {**WATER.BOUNDS, "restores": 2}),
            ("journal", str(self.root / ".local/alternate-journal.json")),
        ]
        original = copy.deepcopy(self.record)
        with patch.object(WATER, "reserve", side_effect=AssertionError("Denial must precede reservation")), \
                patch.object(WATER, "read_saved52", side_effect=AssertionError("Protected read forbidden")):
            for key, value in cases:
                self.record = copy.deepcopy(original)
                self.record[key] = value
                self.publish_record()
                with self.subTest(key=key), self.assertRaises(PRIVATE.CheckpointError):
                    self.admission()
        self.assertFalse((self.root / WATER.JOURNAL).exists())

    def test_code_only_record_is_rejected_even_when_committed_and_hash_selected(self):
        self.record = json.loads((REAL_ROOT / "milestones/evidence/m1-saved-water-continuation-implementation.json").read_bytes())
        self.publish_record()
        with self.assertRaises(PRIVATE.CheckpointError) as rejected:
            self.admission()
        self.assertEqual(rejected.exception.code, "separate_saved_execution_admission_required")

    def test_executor_id_branch_code_binary_and_independent_review_are_bound(self):
        original = copy.deepcopy(self.record)
        for key, value in [
            ("executor", {**original["executor"], "id": "someone-else"}),
            ("executor", {**original["executor"], "branch": "main"}),
            ("executor", {**original["executor"], "code_revision": "0" * 40}),
            ("binaries", {**original["binaries"], "server": {
                **original["binaries"]["server"], "sha256": "0" * 64,
            }}),
            ("independent_review", {**original["independent_review"], "path": WATER.ADMISSION_PATH}),
        ]:
            self.record = copy.deepcopy(original)
            self.record[key] = value
            self.publish_record()
            with self.subTest(key=key), self.assertRaises(PRIVATE.CheckpointError):
                self.admission()

    def test_reservation_and_protected_reader_are_each_one_shot_and_never_renew_by_run_id(self):
        reservation = self.reservation()
        self.assertEqual((self.root / WATER.JOURNAL).stat().st_mode & 0o777, 0o600)
        with self.assertRaises(FileExistsError):
            WATER.reserve(reservation.admission)
        WATER.read_saved52(reservation)
        with self.assertRaises(FileExistsError):
            WATER.read_saved52(reservation)
        with self.assertRaises(PRIVATE.CheckpointError):
            WATER.existing_reservation(reservation.admission, "a" * 16)

    def test_inventory_archive_and_source_symlinks_fail_closed(self):
        reservation = self.reservation()
        inventory = self.checkpoint / "private-inventory.json"
        previous = inventory.read_bytes()
        inventory.unlink()
        elsewhere = self.root / "synthetic-outside-inventory"
        store(elsewhere, previous)
        inventory.symlink_to(elsewhere)
        with self.assertRaises(PRIVATE.CheckpointError) as caught:
            WATER.read_saved52(reservation)
        self.assertEqual(caught.exception.code, "symlink_rejected")

    def test_changed_private_bytes_are_not_read_as_a_valid_boundary(self):
        reservation = self.reservation()
        store(self.checkpoint / "world.pgcustom", b"synthetic-tampering")
        with self.assertRaises(PRIVATE.CheckpointError) as caught:
            WATER.read_saved52(reservation)
        self.assertEqual(caught.exception.code, "original_private_file_changed")

    def test_boundary_rejects_unknown_ack_and_does_not_compare_against_current_root_content(self):
        reservation = self.reservation()
        saved = WATER.read_saved52(reservation)
        self.assertEqual(saved.scenario, self.scenario)
        for response in ({"kind": "unresolved"}, {
            "kind": "acknowledgment_received", "duplicate": True, "next_sequence": 499,
        }):
            capsule = copy.deepcopy(self.capsule)
            capsule["latest_attempt"]["observed_response"] = response
            with self.assertRaises(PRIVATE.CheckpointError):
                WATER.verify_boundary(saved.availability, capsule, self.original, self.identity,
                                      self.scenario, self.scenario)
        self.assertFalse((self.root / "content/m1/manifest.json").exists())

    def test_native_claim_requires_proved_migration_and_is_exclusive_before_control_hashes(self):
        reservation = self.reservation()
        WATER.read_saved52(reservation)
        run = self.root / ".local/journey-runs" / reservation.run_id / "control"
        files = {}
        for name in ("resume-client-checkpoint.json", "resume-report.json", "resume-trace.jsonl"):
            store(run / name, b"synthetic-native-control")
            files[name] = PRIVATE.digest(run / name)
        reservation.phase("controls_prepared", {
            "run_id": reservation.run_id, "original_inventory_sha256": WATER.INVENTORY, "files": files,
        })
        for mode in ("preflight", "gameplay"):
            reservation.phase(f"native_{mode}_started", {
                "run_id": reservation.run_id, "mode": mode, "admission_sha256": self.admission_hash,
            })
        value = WATER.claim_native(reservation.admission, reservation.run_id, "preflight")
        self.assertEqual(value["status"], "claimed")
        with self.assertRaises(FileExistsError):
            WATER.claim_native(reservation.admission, reservation.run_id, "preflight")
        with self.assertRaises(FileNotFoundError):
            WATER.claim_native(reservation.admission, reservation.run_id, "gameplay")
        reservation.phase("migration_proved", {
            "status": "proved", "from_artifact": WATER.FROM, "to_artifact": WATER.TO,
            "complete_private_conservation": True, "restored_private_identity_equal": True,
        })
        value = WATER.claim_native(reservation.admission, reservation.run_id, "gameplay")
        self.assertEqual(value["mode"], "gameplay")
        self.assertFalse(value["network_operations"])

    def test_tampered_reservation_fails_before_any_protected_stat(self):
        reservation = self.reservation()
        store(self.root / WATER.JOURNAL, {"synthetic_tampering": True})
        original = Path.lstat
        protected = []
        def lstat(path, *args, **kwargs):
            if path.is_relative_to(self.checkpoint.parent):
                protected.append(path)
            return original(path, *args, **kwargs)
        with patch.object(Path, "lstat", lstat), self.assertRaises(PRIVATE.CheckpointError):
            WATER.read_saved52(reservation)
        self.assertEqual(protected, [])

    def test_unverified_admission_object_cannot_reserve_or_read(self):
        admission = self.admission()
        forged = WATER.Admission(self.root, self.canonical, ADMISSION_REVISION, "0" * 64, self.record)
        with self.assertRaises(PRIVATE.CheckpointError):
            WATER.reserve(forged)
        self.assertFalse((self.root / WATER.JOURNAL).exists())
        reservation = WATER.reserve(admission)
        forged_reservation = WATER.Reservation(forged, reservation.run_id, reservation.journal_sha256)
        with patch.object(PRIVATE, "project_path", wraps=PRIVATE.project_path) as paths, \
                self.assertRaises(PRIVATE.CheckpointError):
            WATER.read_saved52(forged_reservation)
        self.assertFalse(any(str(call.args[1]).startswith(WATER.SOURCE_WORKTREE) for call in paths.call_args_list))

    def test_a_symlinked_original_source_root_is_not_a_cross_worktree_reader(self):
        reservation = self.reservation()
        source = self.canonical / WATER.SOURCE_WORKTREE
        moved = source.with_name("synthetic-source-storage")
        source.rename(moved)
        source.symlink_to(moved)
        with self.assertRaises(PRIVATE.CheckpointError) as caught:
            WATER.read_saved52(reservation)
        self.assertEqual(caught.exception.code, "symlink_rejected")


if __name__ == "__main__":
    unittest.main()

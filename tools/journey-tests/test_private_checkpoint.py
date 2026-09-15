"""Private checkpoint machinery fixtures only: no PostgreSQL/server or gameplay is executed."""

import copy
import hashlib
import json
from pathlib import Path
import secrets
import stat
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import uuid

import private_checkpoint as CHECKPOINT


ROOT = Path(__file__).resolve().parents[2]


def hashed(data):
    return hashlib.sha256(data).hexdigest()


class PrivateCheckpointTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="checkpoint-machinery-", dir=ROOT / ".local")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.run_id = uuid.uuid4().hex[:16]
        self.run = self.root / ".local/journey-runs" / self.run_id
        (self.run / "control").mkdir(parents=True, mode=0o700)
        self.secret = secrets.token_urlsafe(32)
        self.actor = "actor.machinery_fixture"
        self.account = str(uuid.uuid4())
        self.world_id = str(uuid.uuid4())
        self.operation = str(uuid.uuid4())
        self.game_root = self.run / "game-root"
        self.game_root.mkdir(mode=0o700)
        self.artifact = b"machinery-fixture-not-a-game-artifact"
        self.store(self.game_root / "world.csc", self.artifact)
        self.store(self.game_root / "assets/source.bin", b"original-fixture-bytes")
        self.descriptor = {
            "schema_version": 1, "world_id": self.world_id, "artifact": "world.csc",
            "sha256": hashed(self.artifact), "assets": {"asset.fixture": "/assets/0"},
        }
        self.store_json(self.game_root / "clubscape-game.json", self.descriptor)
        self.store_json(self.game_root / "clubscape-game-assets.json", {
            "schema_version": 1, "files": [{
                "url": "/assets/0", "path": "assets/source.bin",
                "sha256": hashed(b"original-fixture-bytes"),
            }],
        })
        self.identity = {
            "world_id": self.world_id, "artifact_sha256": hashed(self.artifact),
            "descriptor_sha256": CHECKPOINT.digest(self.game_root / "clubscape-game.json"),
            "assets_manifest_sha256": CHECKPOINT.digest(self.game_root / "clubscape-game-assets.json"),
        }
        self.binary = self.root / "not-a-server.bin"
        self.store(self.binary, b"not-an-executable")
        self.server = SimpleNamespace(
            binary=self.binary, origin="http://127.0.0.1:4000",
            process=SimpleNamespace(pid=123, returncode=0, poll=lambda: 0),
            env={
                "DATABASE_URL": f"postgresql://clubscape:{self.secret}@127.0.0.1:45678/clubscape_journey",
                "CLUBSCAPE_GAME_ROOT": str(self.game_root), "CLUBSCAPE_BIND": "127.0.0.1:0",
                "CLUBSCAPE_BUILD_REVISION": "fixture-only",
                "UNRELATED_HOST_SECRET": secrets.token_urlsafe(32),
            },
        )
        self.report_path = self.root / ".local/evidence/scenario.json"
        self.capsule = {
            "schema_version": 1, "kind": "private_m1_client_checkpoint",
            "scenario": "m1_fresh_account",
            "private_authentication_do_not_publish": {
                "account_id": self.account, "login_name": "m1_fixture",
                "password": self.secret, "token": self.secret,
            },
            "actor_id": self.actor,
            "source_identity": {
                "content_revision": "fixture-only",
                "content_artifact": {"uncompressed_sha256": hashed(self.artifact)},
            },
            "report_path": str(self.report_path.relative_to(self.root)),
            "trace_path": str(self.report_path.with_suffix(".trace.jsonl").relative_to(self.root)),
            "last_observed_state": {
                "next_sequence": 245, "revision": 1234, "tick": 1000,
                "player": {"actor_id": self.actor},
            },
            "server_build_revision": "fixture-only",
            "latest_attempt": {
                "operation_id": self.operation, "sequence": 245,
                "observed_response": {"kind": "unresolved"},
                "world_input_protobuf": [1, 2, 3], "retry_authorized": False,
            },
            "latest_control_request": None,
            "original_receipts": {"onboarding": None, "reward": None},
            "automatic_restore": False, "resume_authorized": False,
        }
        self.capsule_path = self.run / "control/private-client-checkpoint.json"
        self.store_json(self.capsule_path, self.capsule)
        self.store_json(self.report_path, {
            "status": "blocked", "synthetic_account_id": self.account,
            "last_snapshot": self.capsule["last_observed_state"],
            "identity": self.capsule["source_identity"],
            "server_build_revision": "fixture-only",
            "private_client_checkpoint": {
                "status": "captured", "sha256": CHECKPOINT.digest(self.capsule_path),
            },
        })
        self.store(self.report_path.with_suffix(".trace.jsonl"), b'{"machinery_fixture":true}\n')
        self.store(self.run / "postgres-password", (self.secret + "\n").encode("ascii"))
        self.report = {
            "run_id": self.run_id, "journey_report": str(self.report_path.relative_to(self.root)),
            "owned_container_name": f"clubscape-journey-{self.run_id}",
            "owned_container_id": "a" * 64, "binary_sha256": {"server": CHECKPOINT.digest(self.binary)},
            "game_root_identity": self.identity, "server_entrypoint": "binary",
            "database_image": "fixture-only-no-image-started",
        }
        self.database = {
            "database": "clubscape_journey", "other_clients": 0, "server_version_num": "160015",
            "world_count": 1, "character_count": 1,
            "world": {
                "world_id": self.world_id, "account_id": self.account, "actor_id": self.actor,
                "login_name": "m1_fixture", "artifact_sha256": hashed(self.artifact),
                "content_revision": "fixture-only", "last_sequence": 245,
                "revision": 1235, "tick": 1001,
                "private_rng_key_sha256": hashed(secrets.token_bytes(32)),
                "world_state_sha256": "1" * 64, "actor_state_sha256": "2" * 64,
                "account_sha256": "3" * 64,
            },
            "commands": {"count": 245},
            "selected_actual_receipts": [{"operation_id": self.operation, "sequence": 245}],
        }
        self.calls = []

    def store(self, path, data):
        path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        with CHECKPOINT.private_file(path) as stream:
            stream.write(data)

    def store_json(self, path, value):
        self.store(path, json.dumps(value).encode("utf-8"))

    def command_fixture(self, root, directory, phase, arguments, *, output, **kwargs):
        self.calls.append((phase, arguments, kwargs))
        self.store(directory / f"{phase}.stderr", b"")
        if phase == "ownership":
            self.store_json(directory / output, {
                "id": self.report["owned_container_id"], "running": True,
                "labels": {"clubscape.scope": "m1-headless-journey",
                           "clubscape.owner": self.report["owned_container_name"]},
                "ports": {"5432/tcp": [{"HostIp": "127.0.0.1", "HostPort": "45678"}]},
            })
        elif phase in ("database_before", "database_after"):
            self.store_json(directory / output, self.database)
        elif phase == "database_dump":
            self.store(directory / output, b"PGDMP-unit-machinery-only-not-a-valid-real-backup")
        elif phase == "archive_integrity":
            self.assertIn("--file=/dev/null", arguments)
            self.assertFalse(any("--dbname" in argument for argument in arguments))
            self.store(directory / output, b"")
        else:
            self.fail(f"Unexpected private command phase {phase}")

    def preserve(self, command=None):
        with patch.object(CHECKPOINT, "private_command", side_effect=command or self.command_fixture):
            return CHECKPOINT.preserve_checkpoint(self.root, self.run, self.report, self.server)

    def test_capture_pairs_private_identities_without_authorizing_resume_or_publishing_secrets(self):
        result = self.preserve()
        self.assertEqual(result["status"], "available")
        self.assertFalse(result["resume_authorized"])
        self.assertFalse(result["restore_executed"])
        self.assertTrue(result["receipt_state_reconciliation_required"])
        public = json.dumps(result)
        for private in (self.secret, self.account, self.actor, self.database["world"]["private_rng_key_sha256"]):
            self.assertNotIn(private, public)
        directory = self.root / result["directory"]
        copied = CHECKPOINT.read_json(directory / "private-client.json", private=True)
        self.assertEqual(copied, self.capsule)
        self.assertEqual(copied["latest_attempt"]["sequence"], 245)
        self.assertFalse(copied["latest_attempt"]["retry_authorized"])
        service = CHECKPOINT.read_json(directory / "private-service.json", private=True)
        self.assertNotIn("UNRELATED_HOST_SECRET", service["original_config_do_not_publish"])
        self.assertEqual((directory / "game-root/world.csc").read_bytes(), self.artifact)
        for path in directory.rglob("*"):
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o700 if path.is_dir() else 0o600)
        self.assertEqual([call[0] for call in self.calls], [
            "ownership", "database_before", "database_dump", "archive_integrity", "database_after",
        ])

    def test_changed_database_never_claims_a_recoverable_checkpoint(self):
        def command(*args, **kwargs):
            if args[2] == "database_after":
                self.database["world"]["world_state_sha256"] = "4" * 64
            self.command_fixture(*args, **kwargs)
        result = self.preserve(command)
        self.assertEqual(result["reason"], "database_changed_around_snapshot")
        self.assertFalse(result["snapshot_available"])
        self.assertFalse(result["recoverable_checkpoint"])

    def test_missing_acknowledged_progress_or_rng_is_an_explicit_failure(self):
        for field, value in (("last_sequence", 243), ("private_rng_key_sha256", None)):
            with self.subTest(field=field):
                original = copy.deepcopy(self.database)
                self.database["world"][field] = value
                directory = self.root / f"identity-{field}"
                directory.mkdir(mode=0o700)
                self.store(directory / "identity.sql", CHECKPOINT.IDENTITY_SQL.encode())
                with patch.object(CHECKPOINT, "private_command", side_effect=self.command_fixture):
                    with self.assertRaises(CHECKPOINT.CheckpointError):
                        CHECKPOINT.database_identity(
                            self.root, directory, self.report["owned_container_name"],
                            self.capsule, self.world_id, "database_before",
                        )
                self.database = original

    def test_unexpected_extra_source_actor_is_not_archived(self):
        self.database["character_count"] = 2
        result = self.preserve()
        self.assertEqual(result["reason"], "database_not_owned_and_quiescent")
        self.assertFalse(result["snapshot_available"])
        self.assertNotIn("database_dump", [call[0] for call in self.calls])

    def test_live_server_and_wrong_owner_are_rejected_before_dump(self):
        self.server.process.poll = lambda: None
        with self.assertRaises(CHECKPOINT.CheckpointError):
            self.preserve()
        self.server.process.poll = lambda: 0
        def command(*args, **kwargs):
            self.command_fixture(*args, **kwargs)
            if args[2] == "ownership":
                path = args[1] / kwargs["output"]
                value = json.loads(path.read_text())
                value["labels"]["clubscape.owner"] = "another-owner"
                path.write_text(json.dumps(value))
        result = self.preserve(command)
        self.assertEqual(result["reason"], "container_identity_or_owner_mismatch")
        self.assertEqual([call[0] for call in self.calls], ["ownership"])

    def test_failed_archive_validation_remains_private_and_unavailable(self):
        def command(*args, **kwargs):
            if args[2] == "archive_integrity":
                raise CHECKPOINT.CheckpointError("archive_integrity", "command_exit_1")
            self.command_fixture(*args, **kwargs)
        result = self.preserve(command)
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["phase"], "archive_integrity")
        self.assertFalse(result["snapshot_available"])
        self.assertNotIn(self.secret, json.dumps(result))
        self.assertFalse((self.root / result["directory"] / "availability.json").exists())

    def test_missing_or_nonprivate_client_capsule_is_not_recoverable(self):
        self.capsule_path.chmod(0o644)
        result = self.preserve()
        self.assertEqual(result["reason"], "file_must_be_owned_0600")
        self.assertFalse(result["snapshot_available"])
        self.assertEqual(self.calls, [])

    def test_server_configuration_mismatch_never_discloses_password(self):
        self.server.env["DATABASE_URL"] = "postgresql://clubscape:wrong@127.0.0.1:45678/clubscape_journey"
        result = self.preserve()
        self.assertEqual(result["reason"], "database_configuration_identity_mismatch")
        self.assertNotIn(self.secret, json.dumps(result))
        self.assertEqual([call[0] for call in self.calls], ["ownership"])

    def test_existing_checkpoint_is_never_overwritten(self):
        result = self.preserve()
        original = (self.root / result["directory"] / "world.pgcustom").read_bytes()
        with self.assertRaises(FileExistsError):
            self.preserve()
        self.assertEqual((self.root / result["directory"] / "world.pgcustom").read_bytes(), original)

    def test_failed_durability_does_not_leave_a_success_availability_marker(self):
        original_sync = CHECKPOINT.sync_directory
        def fail_after_marker(path):
            if (path / "availability.json").exists():
                raise OSError(5, "controlled sync failure")
            original_sync(path)
        with patch.object(CHECKPOINT, "sync_directory", side_effect=fail_after_marker):
            result = self.preserve()
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["phase"], "durability")
        self.assertFalse((self.root / result["directory"] / "availability.json").exists())

    def test_source_symlink_or_hash_drift_is_not_copied_as_valid_content(self):
        (self.game_root / "assets/source.bin").write_bytes(b"changed")
        result = self.preserve()
        self.assertEqual(result["reason"], "source_hash_mismatch")
        self.assertFalse(result["snapshot_available"])
        self.assertNotIn("database_dump", [call[0] for call in self.calls])

    def test_identity_sql_is_read_only_and_scoped_to_actual_world_and_account(self):
        sql = CHECKPOINT.IDENTITY_SQL.lower()
        for mutation in ("insert ", "update ", "delete ", "alter ", "create ", "truncate ", "drop "):
            self.assertNotIn(mutation, sql)
        self.assertIn(":'world_id'::uuid", sql)
        self.assertIn(":'account_id'::uuid", sql)
        self.assertIn("runtime_random_key", sql)
        self.assertIn("processed_game_commands", sql)

    def test_private_command_error_streams_are_bounded_and_not_reported(self):
        directory = self.root / "command"
        directory.mkdir(mode=0o700)
        code = "import sys; sys.stdout.write(sys.argv[1]); sys.stderr.write(sys.argv[1]); sys.exit(7)"
        with self.assertRaises(CHECKPOINT.CheckpointError) as error:
            CHECKPOINT.private_command(
                self.root, directory, "failure", [sys.executable, "-c", code, self.secret],
                output="private.stdout",
            )
        self.assertEqual(error.exception.code, "command_exit_7")
        self.assertNotIn(self.secret, str(error.exception))
        self.assertEqual((directory / "private.stdout").read_text(), self.secret)
        self.assertEqual(stat.S_IMODE((directory / "failure.stderr").stat().st_mode), 0o600)

    def test_private_command_enforces_size_and_time_limits(self):
        directory = self.root / "bounded"
        directory.mkdir(mode=0o700)
        with self.assertRaises(CHECKPOINT.CheckpointError) as error:
            CHECKPOINT.private_command(
                self.root, directory, "size", [sys.executable, "-c", "print('x' * 4096)"],
                output="oversized.stdout", maximum=32,
            )
        self.assertEqual(error.exception.code, "command_output_limit")
        self.assertLessEqual((directory / "oversized.stdout").stat().st_size, 32)
        with self.assertRaises(CHECKPOINT.CheckpointError) as error:
            CHECKPOINT.private_command(
                self.root, directory, "time", [sys.executable, "-c", "import time; time.sleep(5)"],
                output="timeout.stdout", timeout=0.05,
            )
        self.assertEqual(error.exception.code, "command_timeout")


if __name__ == "__main__":
    unittest.main()
